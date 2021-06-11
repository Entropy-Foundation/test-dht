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
use libp2p::{NetworkBehaviour, Multiaddr};
use libp2p::swarm::NetworkBehaviourEventProcess;
use libp2p::{PeerId, development_transport, identity, Transport};
use libp2p::ping::{Ping, PingConfig};
use libp2p::swarm::Swarm;
use libp2p_kad::{QueryId, QueryInfo, kbucket, Kademlia as SKademlia};
use sc_network::{NetworkWorker, DhtEvent};
use std::{error::Error, env, task::Poll, fmt, net::Ipv4Addr};
use async_std::{io, task};

// #[async_std::main]
fn main() {
    env_logger::init();

    find_nodes();
}

fn find_nodes() -> Result<(), Box<dyn Error>> {
    // Create a random key for ourselves.
    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());
    let localhost = Ipv4Addr::new(127, 0, 0, 1);

    println!("Local peer id: {}", local_peer_id);

    let transport = block_on(development_transport(local_key))?;

    let store = MemoryStore::new(local_peer_id.clone());
    let mut kademlia = Kademlia::new(local_peer_id.clone(), store);
    kademlia.get_closest_peers(local_peer_id);

    let mut behaviour = SupraBehaviour {
        kademlia: kademlia
    };
    let mut swarm = Swarm::new(transport, behaviour, local_peer_id);

    let test_tcp: &str = "4242";
    let targetted_tcp = multiaddr::Protocol::Ip4(localhost);

    // refernece: https://docs.libp2p.io/concepts/addressing/
    let address = format!("{}/tcp/{}/p2p/{}", targetted_tcp, test_tcp, local_peer_id.clone());

    println!("{}", address);

    swarm.listen_on(address.parse().unwrap())?;

    Ok(())
}

fn announce_peer() {
    // args: id, info_hash, port, opaque_tokens
}

fn ping() {

}

#[derive(NetworkBehaviour)]
pub struct SupraBehaviour {
    kademlia: Kademlia<MemoryStore>
}

impl NetworkBehaviourEventProcess<KademliaEvent> for SupraBehaviour {
    fn inject_event(&mut self, event: KademliaEvent) {
        todo!()
        //add a ping event. Ref: https://docs.rs/libp2p/0.38.0/libp2p/ping/struct.Ping.html
        //use match so other forms of event can be considered!
        // for response to event, we can use sc_network DhtEvent lib: https://docs.rs/sc-network/0.9.0/sc_network/enum.DhtEvent.html
    }
}

pub struct SupraTransport {

}