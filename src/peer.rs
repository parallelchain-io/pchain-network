/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Entry point to the pchain_network library.
//!
//! It starts a ParallelChain Network peer and keeps the peer alive -- the peer stops working when the
//! thread is dropped.
//!
//! To start a pchain-network peer, the users start with an empty [PeerBuilder]. A [Config] instance, which
//! contains peer's keypair, or other deplaoyment-specific parameters, such as listening ports, bootstrap
//! nodes etc, will be passed into the [PeerBuilder]. Then, the users need to pass in the handlers for processing
//! the [Message]. Lastly, building the [PeerBuilder] will return an instance of [Peer].
//!
//!
//! Example:
//!
//! // 1. Define the configurations
//! let config = Config {...}
//!
//! // 2. Build the instance of Peer
//! let peer = PeerBuilder::new(config)
//!     .on_receive_msg(msg_handler1)
//!     .on_receive_msg(msg_handler2)
//!     .build();
//!
//! // 3. Broadcast or send the message
//! peer.broadcast_mempool_msg(txn);

use pchain_types::blockchain::TransactionV1;
use pchain_types::cryptography::PublicAddress;
use tokio::task::JoinHandle;

use crate::config::Config;
use crate::messages::{DroppedTxnMessage, Message, Topic};

/// The builder struct for constructing a [Peer].
pub struct PeerBuilder {
    /// Configurations of the peer
    pub config: Config,

    /// Message handler to process received messages from the network
    pub handlers: Vec<Box<dyn Fn(PublicAddress, Message) + Send>>,
}

impl PeerBuilder {
    pub fn new(config: Config) -> PeerBuilder {
        PeerBuilder {
            config,
            handlers: vec![],
        }
    }

    pub fn on_receive_msg(
        mut self,
        handler: impl Fn(PublicAddress, Message) + Send + 'static,
    ) -> PeerBuilder {
        self.handlers.push(Box::new(handler));
        self
    }

    /// Constructs a [Peer] from the given configuration and handlers, and start the thread for the p2p network.
    pub async fn build(self) -> Result<Peer, EngineError> {
        crate::engine::start(self).await
    }
}

pub struct Peer {
    /// Network handle for the [tokio::task] which is the main thread for the
    /// p2p network (see [crate::engine]).
    pub(crate) engine: JoinHandle<()>,

    /// mpsc sender for delivering messages to the p2p network.
    pub(crate) to_engine: tokio::sync::mpsc::Sender<EngineCommand>,
}

impl Peer {
    pub fn broadcast_mempool_msg(&self, txn: TransactionV1) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::Mempool,
            Message::Mempool(txn),
        ));
    }

    pub fn broadcast_dropped_tx_msg(&self, msg: DroppedTxnMessage) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::DroppedTxns,
            Message::DroppedTxns(msg),
        ));
    }

    pub fn broadcast_hotstuff_rs_msg(&self, msg: hotstuff_rs::messages::Message) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::HotStuffRsBroadcast,
            Message::HotStuffRs(msg),
        ));
    }

    pub fn send_hotstuff_rs_msg(
        &self,
        address: PublicAddress,
        msg: hotstuff_rs::messages::Message,
    ) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::HotStuffRsSend(address),
            Message::HotStuffRs(msg),
        ));
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.to_engine.try_send(EngineCommand::Shutdown);
    }
}

/// [EngineCommand] defines commands to other pchain-network peers which includes publishing messages
/// and shutting down the network when the peer is dropped.
pub(crate) enum EngineCommand {
    Publish(Topic, Message),
    Shutdown,
}

#[derive(Debug)]
pub enum EngineError {
    /// Failed to read from system configuration path
    SystemConfigError(std::io::Error),

    /// Failed to subscribe to a topic on gossipsub
    SubscriptionError(libp2p::gossipsub::SubscriptionError),

    /// Swarm failed to listen on an unsupported address
    UnsupportedAddressError(libp2p::TransportError<std::io::Error>),
}

impl From<std::io::Error> for EngineError {
    fn from(error: std::io::Error) -> EngineError {
        EngineError::SystemConfigError(error)
    }
}

impl From<libp2p::gossipsub::SubscriptionError> for EngineError {
    fn from(error: libp2p::gossipsub::SubscriptionError) -> EngineError {
        EngineError::SubscriptionError(error)
    }
}

impl From<libp2p::TransportError<std::io::Error>> for EngineError {
    fn from(error: libp2p::TransportError<std::io::Error>) -> EngineError {
        EngineError::UnsupportedAddressError(error)
    }
}
