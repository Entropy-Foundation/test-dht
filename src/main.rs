use async_std::{io, task};
use futures::prelude::*;
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::{
    AddProviderOk,
    Kademlia,
    KademliaEvent,
    PeerRecord,
    PutRecordOk,
    QueryResult,
    Quorum,
    Record,
    record::Key,
};
use libp2p::{
    NetworkBehaviour,
    PeerId,
    Swarm,
    development_transport,
    identity,
    // Multiaddr,
    swarm::NetworkBehaviourEventProcess,
    Multiaddr
};
use std::str::FromStr;
// use std::str::FromStr;
use std::{error::Error, task::{Context, Poll}};
use jsonrpc_http_server::*;
use jsonrpc_http_server::jsonrpc_core::{
    IoHandler,
    Value,
    types::params::Params
};

#[derive(NetworkBehaviour)]
struct MyBehaviour {
    kademlia: Kademlia<MemoryStore>,
    // mdns: Mdns
}

impl NetworkBehaviourEventProcess<KademliaEvent> for MyBehaviour {
    // Called when `kademlia` produces an event.
    fn inject_event(&mut self, message: KademliaEvent) {
        match message {
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

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("Welcome to rust coding with libp2p...Have fun!");
    env_logger::init();

    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    println!("Peer ID {:?}", local_peer_id.to_string());
    // Set up a an encrypted DNS-enabled TCP Transport over the Mplex protocol.
    let transport = development_transport(local_key).await?;
    // We create a custom network behaviour that combines Kademlia and mDNS.

    // Create a swarm to manage peers and events.
    let mut swarm = {
        // Create a Kademlia behaviour.
        let store = MemoryStore::new(local_peer_id.clone());
        let kademlia = Kademlia::new(local_peer_id.clone(), store);
        let behaviour = MyBehaviour { kademlia};
        Swarm::new(transport, behaviour, local_peer_id)
    };
    // Listen on all interfaces and whatever port the OS assigns.
    &swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
    // println!("swarm listening on /ip4/0.0.0.0/tcp/7865");

    let mut stdin = io::BufReader::new(io::stdin()).lines();
    let swarm_thread = start_swarm(&mut stdin, &mut swarm);


    futures::join!(swarm_thread);

    Ok(())
}

async fn start_swarm(stdin: &mut futures::io::Lines<async_std::io::BufReader<async_std::io::Stdin>>, swarm: &mut libp2p::Swarm<MyBehaviour>){
    // Kick it off.
    let mut listening = false;
    task::block_on(future::poll_fn(|cx: &mut Context<'_>| {
        loop {
            match stdin.try_poll_next_unpin(cx) {
                Poll::Ready(Some(line)) => handle_input_line(&mut swarm.behaviour_mut().kademlia, line.unwrap()),
                Poll::Ready(None) => panic!("Stdin closed"),
                Poll::Pending => break
            }
        }
        loop {
            match swarm.poll_next_unpin(cx) {
                Poll::Ready(Some(event)) => println!("{:?}", event),
                Poll::Ready(None) => return Poll::Ready(()),
                Poll::Pending => {
                    if !listening {
                        if let Some(a) = Swarm::listeners(&swarm).next() {
                            println!("Listening on {:?}", a);
                            listening = true;
                        }
                    }
                    break
                },
            }
        }
        Poll::Pending
    }));
}

async fn start_rpc_server(swarm_thread: impl futures::Future)
{
    println!("inside start_rpc_server");
    let mut io = IoHandler::default();
    io.add_method("say_hello", |params:Params| async {
        println!("someone is saying hello!!!");
        if let Params::Map(m) = params {
            let param_key = m.get("m").unwrap();
            let key_string = param_key.as_str().unwrap();

            let key = Key::new(&key_string);
            println!("p_key: {:?}, key_str: {:?}, key: {:?}", &param_key, &key_string, &key);
        }
        Ok(Value::String("hello".into()))
    });
    let server = ServerBuilder::new(io)
        .cors(DomainsValidation::AllowOnly(vec![AccessControlAllowOrigin::Null]))
        .start_http(&"127.0.0.1:3030".parse().unwrap())
        .expect("Unable to start RPC server");
    println!("rpc server listening on http://127.0.0.1:3030");
    // futures::join!(swarm_thread);
    // server.wait();

}

fn handle_input_line(kademlia: &mut Kademlia<MemoryStore>, line: String) {
    // println!("handle_input_line called with, line: {}", line);
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
        },
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
            println!("{}",&key);
            println!("{}",&address);
            let peer_id = PeerId::from_str(key).unwrap();
            let multiaddr = Multiaddr::from_str(address).unwrap();
            kademlia.add_address(&peer_id, multiaddr);
        }
        _ => {
            eprintln!("expected GET, GET_PROVIDERS, PUT or PUT_PROVIDER");
        }
    }
}
