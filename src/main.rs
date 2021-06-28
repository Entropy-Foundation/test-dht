use async_std::future;
use futures::executor::block_on;

use futures::{task, executor, stream::once};
use futures::future::{ok, ready, Ready};
use libp2p::core::transport::MemoryTransport;
use libp2p::core::{Executor, transport, muxing, multiaddr::{Protocol, Multiaddr}};
use libp2p::kad::record::store::MemoryStore;
use libp2p::kad::{Kademlia, KademliaEvent};
use libp2p::swarm::ExpandedSwarm;
use libp2p::{PeerId, development_transport, identity, Transport};
use libp2p::ping::{Ping, PingConfig};
use libp2p::swarm::{Swarm, SwarmBuilder};
use sc_network::{NetworkWorker, DhtEvent};
use std::{env, task::Poll, fmt, net::Ipv4Addr};
use actix_web::{get, web, App, Error, HttpResponse, HttpServer};

fn main() {
    env_logger::init();

    block_on(discover_peer());
    // println!("{:?}", multi_addr);

    
}

async fn discover_peer() {
    // identity
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from_public_key(local_key.public().clone());
    println!("Local peer id: {:?}", &local_peer_id);
    
    
    //address
    let localhost = Ipv4Addr::new(0, 0, 0, 0);
    let test_tcp: &str = "0";
    let targetted_tcp = Protocol::Ip4(localhost);
    
    let address = format!("{}/tcp/{}/p2p/{}", targetted_tcp, test_tcp, local_peer_id.clone());
    let multi_addr: Multiaddr = address.parse().unwrap();
    println!("{}", multi_addr);


    // storage
    let store = MemoryStore::new(local_peer_id.clone());
    let mut kademlia = Kademlia::new(local_peer_id, store);

    let transport = block_on(development_transport(local_key)).unwrap();
    let mut swarm = Swarm::new(transport, kademlia, local_peer_id);


    // swarm.listen_on(multi_addr.clone()).await;
    let event = swarm.behaviour_mut();

    
}
