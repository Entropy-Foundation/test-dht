use async_std::{io, task};
use futures::prelude::*;
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::{
    record::Key, AddProviderOk, Kademlia, KademliaEvent, PeerRecord, PutRecordOk, QueryResult,
    Quorum, Record,
};
use libp2p::{development_transport, identity, swarm::{NetworkBehaviourEventProcess, SwarmEvent}, NetworkBehaviour, PeerId, Swarm, Multiaddr};
use std::{
    error::Error,
    task::{Context, Poll},
};
use std::str::FromStr;
use libp2p::identify::{Identify, IdentifyConfig, IdentifyEvent};
use async_std::channel::{unbounded, Sender};

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol.
    let transport = development_transport(local_key.clone()).await?;

    // We create a custom network behaviour that combines Kademlia and mDNS.
    #[derive(NetworkBehaviour)]
    #[behaviour(event_process = true)]
    struct MyBehaviour {
        kademlia: Kademlia<MemoryStore>,
        identify: Identify,
    }

    impl NetworkBehaviourEventProcess<IdentifyEvent> for MyBehaviour {
        fn inject_event(&mut self, event: IdentifyEvent) {
            match event {
                IdentifyEvent::Received {peer_id,info} => {
                    // println!("peer_id: {:?}, info: {:?}", &peer_id, &info.listen_addrs);
                    for addr in &info.listen_addrs {
                        self.kademlia.add_address(&peer_id, addr.clone());
                    }
                }
                _ => {}
            }
        }
    }

    impl NetworkBehaviourEventProcess<KademliaEvent> for MyBehaviour {

        // Called when `kademlia` produces an event.
        fn inject_event(&mut self, message: KademliaEvent) {
            match message {
                KademliaEvent::OutboundQueryCompleted { result, .. } => match result {
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
                        for PeerRecord {
                            record: Record { key, value, .. },
                            ..
                        } in ok.records
                        {
                            println!(
                                "Value for {:?} is:- {:?}",
                                std::str::from_utf8(key.as_ref()).unwrap(),
                                std::str::from_utf8(&value).unwrap(),
                            );
                        }
                    }
                    QueryResult::GetRecord(Err(err)) => {
                        eprintln!("Failed to get record: {:?}", err);
                    }
                    QueryResult::PutRecord(Ok(PutRecordOk { key })) => {
                        println!("Data successfully saved for {:?}", std::str::from_utf8(key.as_ref()).unwrap());
                    }
                    QueryResult::PutRecord(Err(err)) => {
                        eprintln!("Failed to put record: {:?}", err);
                    }
                    QueryResult::StartProviding(Ok(AddProviderOk { key })) => {
                        println!(
                            "Successfully put provider record {:?}",
                            std::str::from_utf8(key.as_ref()).unwrap()
                        );
                    }
                    QueryResult::StartProviding(Err(err)) => {
                        eprintln!("Failed to put provider record: {:?}", err);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    // Create a swarm to manage peers and events.
    let mut swarm = {
        // Create a Kademlia behaviour.
        let store = MemoryStore::new(local_peer_id);
        let kademlia = Kademlia::new(local_peer_id, store);

        let identify_config = IdentifyConfig::new("dht/1.0.0".to_string(),local_key.public().clone());
        let identify = Identify::new(identify_config);
        let behaviour = MyBehaviour { kademlia, identify };

        Swarm::new(transport, behaviour, local_peer_id)
    };

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns.
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    let str = "identity";
    let key = Key::new(&str);

    swarm.behaviour_mut().kademlia.start_providing(key);
    let (tx, mut rx) = unbounded::<String>();

    // Kick it off.
    task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {
        loop {
            match stdin.try_poll_next_unpin(cx)? {
                Poll::Ready(Some(line)) => {
                    handle_input_line(&mut swarm.behaviour_mut().kademlia, line, tx.clone())
                }
                Poll::Ready(None) => panic!("Stdin closed"),
                Poll::Pending => break,
            }
        }
        loop {
            match swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) => {
                    match event {
                        SwarmEvent::NewListenAddr {address, ..} => {
                            println!("Listening on with peer {} {} ", local_peer_id, address);
                        },
                        
                        _ => {}
                    }
                }
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => break,
            }
        }

        loop {
            match rx.poll_next_unpin(cx) {
                Poll::Ready(Some(result)) => {
                    let addr = result.parse().unwrap();
                    swarm.dial_addr(addr).unwrap();
                },
                Poll::Pending => break,
                _ => break,
            }
        }
        Poll::Pending
    }))
}

fn handle_input_line(kademlia: &mut Kademlia<MemoryStore>, line: String, tx: Sender<String>) {
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
        Some("GET_PROVIDERS") => {
            let key = {
                match args.next() {
                    Some(key) => Key::new(&key),
                    None => {
                        eprintln!("Expected key");
                        return;
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
            kademlia
                .put_record(record, Quorum::One)
                .expect("Failed to store record locally.");
        }
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

            kademlia
                .start_providing(key)
                .expect("Failed to start providing key");
        }
        Some("ADD_NODE") => {
            let key = {
                match args.next() {
                    Some(key) => {
                        eprintln!("Connected");
                        key
                    },
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
                        eprintln!("Expected multiaddress");
                        return;
                    }
                }
            };
            println!("{}",&key);
            println!("{}",&address);
            let peer_id = PeerId::from_str(key).unwrap();
            let multiaddr = Multiaddr::from_str(&address).unwrap();
            kademlia.add_address(&peer_id, multiaddr);
            tx.try_send(address.to_string()).unwrap();
        }
        _ => {
            eprintln!("expected GET, GET_PROVIDERS, PUT or PUT_PROVIDER");
        }
    }
}