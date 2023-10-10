
use libp2p::identity::ed25519::Keypair;
use libp2p::{PeerId, Multiaddr};
use pchain_types::cryptography::PublicAddress;

use crate::messages::Topic;

pub(crate) struct Config {
    pub keypair: Keypair,
    pub topics_to_subscribe: Vec<Topic>,
    pub listening_port: u16,
    pub boot_nodes: Vec<(PeerId, Multiaddr)>,
    pub outgoing_msgs_buffer_capacity: usize,
    pub incoming_msgs_buffer_capacity: usize,
    pub peer_discovery_interval: u64,
    pub kademlia_protocol_name: String,
}

fn fullnode_topics(public_address: PublicAddress) -> Vec<Topic> {
    vec![Topic::HotStuffRsBroadcast, Topic::HotStuffRsSend(public_address).into(), Topic::Mempool, Topic::DroppedTxns]
}