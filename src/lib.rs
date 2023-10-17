/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Implementation of [ParallelChain Protocol](https://github.com/parallelchain-io/parallelchain-protocol)
//! peer-to-peer (P2P) networking.
//!
//! ## Starting a peer
//!
//! ```no_run
//! use crate::Config;
//! use crate::peer::PeerBuilder;
//! use crate::messages::Message;
//!
//!
//! // 1. Build a configuration.
//! let config = Config {
//!     keypair, // libp2p_identity::ed25519::Keypair::generate()
//!     topics_to_subscribe, // vec![Topic::HotStuffRsBroadcast]
//!     listening_port, // 25519
//!     boot_nodes, // vec![]
//!     outgoing_msgs_buffer_capacity, // 8
//!     incoming_msgs_buffer_capacity, // 10
//!     peer_discovery_interval, // 10
//!     kademlia_protocol_name // "/pchain_p2p/1.0.0"
//! };
//!
//! // 2. Create a message handler 
//! let (tx, rx) = mpsc::channel();
//! let message_sender = tx.clone();
//! let message_handler = move |msg_orgin: [u8;32], msg: Message| {
//!     // processing the message...
//!     let _ = message_sender.send((msg_origin, msg));
//! };
//!  
//! // 3. Start P2P network.
//! let peer = PeerBuilder::new(config)
//!     .on_receive_msg(message_handler)
//!     .build()
//!     .await
//!     .unwrap();
//! 
//! // 4. Send out messages.
//! peer.broadcast_mempool_msg(txn);
//!
//!


pub mod behaviour;

pub mod config;

pub mod conversions;

pub mod engine;

pub mod messages;

pub mod peer;
