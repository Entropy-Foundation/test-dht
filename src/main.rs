use async_std::{io, task};
use futures::prelude::*;
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::{
    record::Key, Kademlia, KademliaEvent, PeerRecord, PutRecordOk, QueryResult, Quorum, Record,
};
use libp2p::{
    development_transport, identity,
    swarm::{NetworkBehaviourEventProcess, SwarmEvent},
    Multiaddr, NetworkBehaviour, PeerId, Swarm,
};
use std::{
    error::Error,
    str::FromStr,
    task::{Context, Poll},
};

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
struct KademliaBehaviour {
    kademlia: Kademlia<MemoryStore>,
}

impl KademliaBehaviour {
    pub fn new(peer_id: PeerId, store: MemoryStore) -> Self {
        KademliaBehaviour {
            kademlia: Kademlia::new(peer_id, store),
        }
    }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for KademliaBehaviour {
    // Called when `kademlia` produces an event.
    fn inject_event(&mut self, message: KademliaEvent) {
        if let KademliaEvent::OutboundQueryCompleted { result, .. } = message {
            match result {
                QueryResult::GetRecord(Ok(ok)) => {
                    for PeerRecord {
                        record: Record { key, value, .. },
                        ..
                    } in ok.records
                    {
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
                _ => {}
            }
        }
    }
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    println!("Peer ID {:?}", local_peer_id.to_string());

    // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol.
    let transport = development_transport(local_key).await?;

    // Create a swarm to manage peers and events.
    let mut swarm = {
        Swarm::new(
            transport,
            KademliaBehaviour::new(local_peer_id, MemoryStore::new(local_peer_id)),
            local_peer_id,
        )
    };

    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns.
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    // Kick it off.
    task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {
        loop {
            match stdin.try_poll_next_unpin(cx)? {
                Poll::Ready(Some(line)) => {
                    handle_input_line(&mut swarm.behaviour_mut().kademlia, line)
                }
                Poll::Ready(None) => panic!("Stdin closed"),
                Poll::Pending => break,
            }
        }
        loop {
            match swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) => {
                    if let SwarmEvent::NewListenAddr { address, .. } = event {
                        println!("Listening on {:?}", address);
                    }
                }
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => break,
            }
        }
        Poll::Pending
    }))
}

fn handle_input_line(kademlia: &mut Kademlia<MemoryStore>, line: String) {
    let mut args = line.split(' ');

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
            kademlia
                .put_record(record, Quorum::One)
                .expect("Failed to store record locally.");
        }
        Some("ADD_NODE") => {
            let key = {
                match args.next() {
                    Some(key) => key,
                    None => {
                        eprintln!("Expected peer id");
                        return;
                    }
                }
            };

            let address = {
                match args.next() {
                    Some(key) => key,
                    None => {
                        eprintln!("Expected multi-address");
                        return;
                    }
                }
            };
            println!("{}", &key);
            println!("{}", &address);
            let peer_id = PeerId::from_str(key).unwrap();
            let multi_addr = Multiaddr::from_str(address).unwrap();
            kademlia.add_address(&peer_id, multi_addr);
        }
        _ => {
            eprintln!("Expected ADD_NODE, GET or PUT");
        }
    }
}
