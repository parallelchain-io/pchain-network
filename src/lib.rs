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
//! use crate::message_gate::MessageGateChain;
//! 
//!
//! // 1. Build a configuration.
//! let config = Config::default();
//!
//! // 2. Create message gate chain.
//! let gates = MessageGateChain::new()
//! // .append(message_gate1)
//! // .append(message_gate2)
//! // ...
//!
//! // 3. Start P2P network.
//! let network = pchain_network::NetworkHandle::start(network_config, subscribe_topics, message_gate_chain).await;
//!
//! // 4. Send out messages.
//! network.broadcast_mempool_tx_msg(txn);

pub mod config;
pub use config::Config;

pub(crate) mod engine;

pub mod messages;

pub mod message_gate;

pub mod network_handle;
pub use network_handle::Peer;

pub(crate) mod behaviour;

pub(crate) mod constants;

pub(crate) mod conversions;
