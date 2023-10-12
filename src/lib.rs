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
//!
//!

// TODO jonas: update example usage above

pub mod behaviour;

pub mod config;

pub mod conversions;

pub mod engine;

pub mod messages;

pub mod peer;
