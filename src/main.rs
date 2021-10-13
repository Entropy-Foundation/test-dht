use async_std::channel::{unbounded, Sender};
use async_std::{io, task};
use futures::prelude::*;
use libp2p::identify::{Identify, IdentifyConfig, IdentifyEvent};
use libp2p::kad::record::store::{MemoryStore, MemoryStoreConfig};
use libp2p::kad::{
    record::Key, Kademlia, KademliaConfig, KademliaEvent, PeerRecord, PutRecordOk, QueryResult,
    Quorum, Record,
};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{
    development_transport, identity,
    swarm::{NetworkBehaviourEventProcess, SwarmEvent},
    Multiaddr, NetworkBehaviour, PeerId, Swarm,
};
use std::str::FromStr;
use std::time::Duration;
use std::{
    error::Error,
    task::{Context, Poll},
};
use std::fs;
use serde_json;
use serde::{Serialize, Deserialize};

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
struct MyBehaviour {
    kademlia: Kademlia<MemoryStore>,
    identify: Identify,
    #[behaviour(ignore)]
    local_peer_id: PeerId,
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol.
    let transport = development_transport(local_key.clone()).await?;

    // We create a custom network behaviour that combines Kademlia and mDNS.

    impl NetworkBehaviourEventProcess<IdentifyEvent> for MyBehaviour {
        fn inject_event(&mut self, event: IdentifyEvent) {
            if let IdentifyEvent::Received { peer_id, info } = event {
                println!("Connected: {:?}", &peer_id);
                for addr in &info.listen_addrs {
                    self.kademlia.add_address(&peer_id, addr.clone());
                }
            }
        }
    }

    impl NetworkBehaviourEventProcess<KademliaEvent> for MyBehaviour {
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
                        println!(
                            "Data successfully saved for {:?}",
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

    // Create a swarm to manage peers and events.
    let mut swarm = {
        let m_config = MemoryStoreConfig {
            max_provided_keys: 1024,
            max_providers_per_key: 20,
            max_records: 102400,
            max_value_bytes: 650*1024*1024,
        };
        // Create a Kademlia behaviour.
        let store = MemoryStore::with_config(local_peer_id, m_config);


        let mut kad_config = KademliaConfig::default();
        kad_config.set_connection_idle_timeout(Duration::from_secs(100000));
        let kademlia = Kademlia::with_config(local_peer_id, store, kad_config);

        let identify_config = IdentifyConfig::new("dht/1.0.0".to_string(), local_key.public());
        let identify = Identify::new(identify_config);
        let behaviour = MyBehaviour {
            kademlia,
            identify,
            local_peer_id,
        };

        Swarm::new(transport, behaviour, local_peer_id)
    };

    // Read full lines from stdin
    let mut stdin = io::BufReader::new(io::stdin()).lines();

    // Listen on all interfaces and whatever port the OS assigns.
    swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

    swarm
        .behaviour_mut()
        .kademlia
        .start_providing(Key::new(&"identity"))
        .unwrap();
    let (tx, mut rx) = unbounded::<String>();

    // Kick it off.
    task::block_on(future::poll_fn(move |cx: &mut Context<'_>| {
        loop {
            match stdin.try_poll_next_unpin(cx)? {
                Poll::Ready(Some(line)) => {
                    let behaviour = swarm.behaviour_mut();
                    handle_input_line(line, tx.clone(), behaviour)
                }
                Poll::Ready(None) => panic!("Stdin closed"),
                Poll::Pending => break,
            }
        }
        loop {
            match swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) => match event {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        if !(address.to_string().contains("127.0.0.1"))
                            && !(address.to_string().contains("172.17.0.1"))
                        {
                            println!("Listening on with peer {} {} ", local_peer_id, address);
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        for address in swarm.behaviour_mut().kademlia.addresses_of_peer(&peer_id) {
                            swarm
                                .behaviour_mut()
                                .kademlia
                                .remove_address(&peer_id, &address);
                        }
                        println!("Removed:{:?}", &peer_id);
                    }
                    _ => {}
                },
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Pending => break,
            }
        }

        loop {
            match rx.poll_next_unpin(cx) {
                Poll::Ready(Some(result)) => {
                    let addr = result.parse().unwrap();
                    swarm.dial_addr(addr).unwrap();
                }
                Poll::Pending => break,
                _ => break,
            }
        }
        Poll::Pending
    }))
}

#[derive(Serialize, Deserialize, Debug)]
struct Transaction {
    transaction_id:String,
    from_account:String,
    to_account:String,
    amount:usize,
    signature:String,
}

fn handle_input_line(line: String, tx: Sender<String>, behaviour: &mut MyBehaviour) {
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
            behaviour.kademlia.get_record(&key, Quorum::One);
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
                publisher: Some(behaviour.local_peer_id),
                expires: None,
            };
            behaviour
                .kademlia
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
                        eprintln!("Expected multiaddress");
                        return;
                    }
                }
            };

            let peer_id = PeerId::from_str(key).unwrap();
            let multiaddr = Multiaddr::from_str(address).unwrap();
            behaviour.kademlia.add_address(&peer_id, multiaddr);
            tx.try_send(address.to_string()).unwrap();
        }

        Some("ADD_JSON") => {

            let file = fs::File::open("transaction_batch_1M.json")
                .expect("file should open read only");

            let json: serde_json::Value = serde_json::from_reader(file)
                .expect("file should be proper JSON");

            let value_json : Vec<Transaction> = serde_json::from_str(json["params"]["message"].to_string().as_str()).unwrap();

            for (i, j) in value_json.iter().enumerate() {

                let key_name: String = format!("data_{}",i);

                let key = Key::new(&key_name);

                let value_store = serde_json::to_vec(&j).unwrap();

                let record = Record {
                    key,
                    value:value_store,
                    publisher: Some(behaviour.local_peer_id),
                    expires: None,
                };
                behaviour
                    .kademlia
                    .put_record(record, Quorum::One)
                    .expect("Failed to store record locally.");

            }
        }

        Some("GET_JSON_DATA_FROM_DHT") => {
            let file = fs::File::open("transaction_batch_1M.json")
                .expect("file should open read only");

            let json: serde_json::Value = serde_json::from_reader(file)
                .expect("file should be proper JSON");

            let value_json : Vec<Transaction> = serde_json::from_str(json["params"]["message"].to_string().as_str()).unwrap();

            for (i, j) in value_json.iter().enumerate() {
                let key_name: String = format!("data_{}",i);

                let key = Key::new(&key_name);
                behaviour.kademlia.get_record(&key, Quorum::One);
            }
        }

        _ => {
            eprintln!("expected GET, PUT or ADD_NODE");
        }
    }
}
