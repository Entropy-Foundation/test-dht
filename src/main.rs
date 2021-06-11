use futures::executor::block_on;
use futures::prelude::*;
use libp2p::core::{transport, muxing, multiaddr};
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::{
    AddProviderOk,
    Kademlia,
    KademliaEvent,
    kbucket::KBucketsTable,
    PeerRecord,
    PutRecordOk,
    QueryResult,
    Quorum,
    Record,
    record::Key,
};
use libp2p::{NetworkBehaviour, Multiaddr, mdns::{Mdns, MdnsConfig, MdnsEvent}};
use libp2p::swarm::NetworkBehaviourEventProcess;
use libp2p::{PeerId, development_transport, identity, Transport};
use libp2p::ping::{Ping, PingConfig};
use libp2p::swarm::Swarm;
use libp2p_kad::{QueryId, QueryInfo, kbucket, Kademlia as SKademlia};
use sc_network::{NetworkWorker, DhtEvent};
use std::task::Context;
use std::{error::Error, env, task::Poll, fmt, net::Ipv4Addr};
use async_std::{io, task};

// #[async_std::main]
// fn main() {
    

//     let _a = block_on(find_nodes());
// }
#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();
    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    let localhost = Ipv4Addr::new(127, 0, 0, 1);

    println!("Local peer id: {}", local_peer_id);

    let transport = block_on(development_transport(local_key))?;

    // let store = MemoryStore::new(local_peer_id.clone());
    // let mut kademlia = Kademlia::new(local_peer_id.clone(), store);
    // kademlia.get_closest_peers(local_peer_id);

    // let mut behaviour = SupraBehaviour {
    //     kademlia: kademlia,
    //     mdns: (),
    // };

    // Create a swarm to manage peers and events.
    let mut swarm = {
        // Create a Kademlia behaviour.
        let store = MemoryStore::new(local_peer_id.clone());
        let kademlia = Kademlia::new(local_peer_id.clone(), store);
        let mdns = task::block_on(Mdns::new(MdnsConfig::default()))?;
        let behaviour = SupraBehaviour { kademlia, mdns };
        Swarm::new(transport, behaviour, local_peer_id)
    };

    // let mut swarm = Swarm::new(transport, behaviour, local_peer_id);

    let test_tcp: &str = "4242";
    let targetted_tcp = multiaddr::Protocol::Ip4(localhost);

    // refernece: https://docs.libp2p.io/concepts/addressing/
    let address = format!("{}/tcp/{}/p2p/{}", targetted_tcp, test_tcp, local_peer_id.clone());

    println!("{}", address);

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // swarm.listen_on(address.parse().unwrap())?;
    // Listen on all interfaces and whatever port the OS assigns.
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Kick it off.
    let mut listening = false;
    task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {
        loop {
            match stdin.try_poll_next_unpin(cx)? {
                Poll::Ready(Some(line)) => handle_input_line(&mut swarm.behaviour_mut().kademlia, line),
                Poll::Ready(None) => panic!("Stdin closed"),
                Poll::Pending => break
            }
        }
        loop {
            match swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) => println!("{:?}", event),
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => {
                    if !listening {
                        if let Some(a) = Swarm::listeners(&swarm).next() {
                            println!("Listening on {:?}", a);
                            listening = true;
                        }
                    }
                    break
                }
            }
        }
        Poll::Pending
    }))
}

fn announce_peer() {
    // args: id, info_hash, port, opaque_tokens
}

fn ping() {

}

#[derive(NetworkBehaviour)]
pub struct SupraBehaviour {
    kademlia: Kademlia<MemoryStore>,
    mdns: Mdns
}

impl NetworkBehaviourEventProcess<MdnsEvent> for SupraBehaviour {
    // Called when `mdns` produces an event.
    fn inject_event(&mut self, event: MdnsEvent) {
        if let MdnsEvent::Discovered(list) = event {
            for (peer_id, multiaddr) in list {
                self.kademlia.add_address(&peer_id, multiaddr);
            }
        }
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for SupraBehaviour {
    fn inject_event(&mut self, event: KademliaEvent) {
        match event {
            KademliaEvent::QueryResult { result, .. } => match result {
                QueryResult::GetProviders(Ok(ok)) => {
                    for peer in ok.providers {
                        println!(
                            "Peer {:?} provides key {:?}",
                            peer,
                            std::str::from_utf8(ok.key.as_ref()).unwrap()
                        );
                    }
                }
                QueryResult::GetProviders(Err(err)) => {
                    eprintln!("Failed to get providers: {:?}", err);
                }
                QueryResult::GetRecord(Ok(ok)) => {
                    for PeerRecord { record: Record { key, value, .. }, ..} in ok.records {
                        println!(
                            "Got record {:?} {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap(),
                            std::str::from_utf8(&value).unwrap(),
                        );
                    }
                }
                QueryResult::GetRecord(Err(err)) => {
                    eprintln!("Failed to get record: {:?}", err);
                }
                QueryResult::PutRecord(Ok(PutRecordOk { key })) => {
                    println!(
                        "Successfully put record {:?}",
                        std::str::from_utf8(key.as_ref()).unwrap()
                    );
                }
                QueryResult::PutRecord(Err(err)) => {
                    eprintln!("Failed to put record: {:?}", err);
                }
                QueryResult::StartProviding(Ok(AddProviderOk { key })) => {
                    println!("Successfully put provider record {:?}",
                        std::str::from_utf8(key.as_ref()).unwrap()
                    );
                }
                QueryResult::StartProviding(Err(err)) => {
                    eprintln!("Failed to put provider record: {:?}", err);
                }
                _ => {}
            }
            _ => {}
        }
    }
}

pub struct SupraTransport {

}

fn handle_input_line(kademlia: &mut Kademlia<MemoryStore>, line: String) {
    let mut args = line.split(" ");

    match args.next() {
        Some("GET") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
                    }
                }
            };
            kademlia.get_record(&key, Quorum::One);
        }
        Some("GET_PROVIDERS") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return
                    }
                }
            };
            kademlia.get_providers(key);
        }
        Some("PUT") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
                    }
                }
            };
            let value = {
                match args.next() {
                    Some(value) => value.as_bytes().to_vec(),
                    None => {
                        eprintln!("Expected value");
                        return;
                    }
                }
            };
            let record = Record {
                key,
                value,
                publisher: None,
                expires: None,
            };
            kademlia.put_record(record, Quorum::One).expect("Failed to store record locally.");
        },
        Some("PUT_PROVIDER") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
                    }
                }
            };

            kademlia.start_providing(key).expect("Failed to start providing key");
        }
        _ => {
            eprintln!("expected GET, GET_PROVIDERS, PUT or PUT_PROVIDER");
        }
    }
}