/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Implementation of [ParallelChain protocol](https://github.com/parallelchain-io/parallelchain-protocol)
//! peer-to-peer (P2P) networking.
//! 
//! ## Starting a peer
//! 
//! ```no_run
//! use crate::configuration::Config; 
//! use crate::messages::MessageGateChain;
//! use crate::engine;
//! use crate::network::{SendCommand, NetworkTopic};
//! 
//! // 1. Build a configuration.
//! let config = Config::new();
//! 
//! // 2. Create message gate chain. 
//! let gates = MessageGateChain::new()
//! // .chain(message_gate1) 
//! // .chain(message_gate2)
//! // ...
//! 
//! // 3. Start P2P network.
//! let network = engine::start(config, vec![], gates).await.unwrap();
//! 
//! // 4. Send out messages.
//! let sender = network.sender();
//! let _ = sender.send(SendCommand::Broadcast(NetworkTopic::new("topic".to_string()), Vec::new())).await;
//! ```

pub mod configuration;

pub mod engine;

pub mod messages;

pub mod network;

pub mod peer_info;

pub(crate) mod behaviour;

pub(crate) mod conversions;
