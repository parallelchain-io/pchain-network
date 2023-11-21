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
//! use crate::Peer;
//! use crate::messages::Message;
//!
//!
//! // 1. Build a configuration.
//! let config = Config {
//!     keypair, // pchain_types::cryptography::Keypair
//!     topics_to_subscribe, // vec![Topic::HotStuffRsBroadcast]
//!     listening_port, // 25519
//!     boot_nodes, // vec![]
//!     outgoing_msgs_buffer_capacity, // 8
//!     peer_discovery_interval, // 10
//!     kademlia_protocol_name // "/pchain_p2p/1.0.0"
//! };
//!
//! // 2. Create message handlers 
//! let (tx, rx) = tokio::sync::mpsc::channel();
//! let message_sender = tx.clone();
//! let message_handler = move |msg_origin: [u8;32], msg: Message| {
//!     match msg {
//!         Message::HotStuffRs(hotstuff_message) => {
//!             // process hotstuff message
//!             let _ = hotstuff_sender.try_send((msg_origin, Message::HotStuffRs(hotstuff_message)));
//!         }
//!         _ => {}
//!     }
//! };
//!  
//! // 3. Start P2P network.
//! let peer = Peer::start(config, Box::new(message_handler))
//!     .await
//!     .unwrap();
//! 
//! // 4. Send out messages.
//! peer.broadcast_mempool_msg(txn);
//! ```
//!


pub mod behaviour;

pub mod config;
pub use config::Config;

pub mod conversions;

pub mod messages;
pub use messages::Message;

pub mod peer;
pub use peer::Peer;
