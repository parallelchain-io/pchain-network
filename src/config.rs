/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Configuration parameters provided by the library user.
//!
//! Example:
//!
//! ```no_run
//!     let config = Config {
//!         keypair: libp2p_identity::ed25519::Keypair::generate(),
//!         topics_to_subscribe: vec![Topic::HotStuffRsBroadcast],
//!         listening_port: 25519,
//!         boot_nodes: vec![],
//!         outgoing_msgs_buffer_capacity: 8,
//!         incoming_msgs_buffer_capacity: 10,
//!         peer_discovery_interval: 10,
//!         kademlia_protocol_name: String::from("/pchain_p2p/1.0.0"),
//!     };
//! ```
//! 
use libp2p::{Multiaddr, identity::{ed25519::Keypair, PeerId}};
use pchain_types::cryptography::PublicAddress;

use crate::messages::Topic;

pub struct Config {
    /// Keypair used for identifying the peer
    pub keypair: Keypair,

    /// List of topics to subscribe
    pub topics_to_subscribe: Vec<Topic>,

    /// Port number for listening to events
    pub listening_port: u16,

    /// Bootstrap nodes for the initial connection
    pub boot_nodes: Vec<(PeerId, Multiaddr)>,

    /// Buffer size of outgoing messages
    pub outgoing_msgs_buffer_capacity: usize,

    /// Buffer size of incoming messages
    pub incoming_msgs_buffer_capacity: usize,

    /// Interval in seconds for querying the network to discover peers
    pub peer_discovery_interval: u64,

    /// Protocol name to communicate with other Kademlia nodes
    pub kademlia_protocol_name: String,
}

// Returns a complete list of accepted topics in pchain-network
pub(crate) fn fullnode_topics(public_address: PublicAddress) -> Vec<Topic> {
    vec![Topic::HotStuffRsBroadcast, Topic::HotStuffRsSend(public_address).into(), Topic::Mempool, Topic::DroppedTxns]
}