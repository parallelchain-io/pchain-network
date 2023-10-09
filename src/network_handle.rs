/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This is the entry point to the pchain_network library. It starts a ParallelChain Network peer
//! and keeps the peer alive -- the peer stops working when the thread is dropped.
//!
//! To send a message in the P2P network, call `.broadcast_mempool_msg()`, `broadcast_dropped_txn_msg`,
//! `broadcast_hotstuff_rs_msg` or `.send_hotstuff_rs_msg()` directly.

use crate::Config;
use crate::engine;
use crate::message_gate::MessageGateChain;
use crate::messages::{DroppedTxMessage, Message, Topic};
use pchain_types::{blockchain::TransactionV1, cryptography::PublicAddress};
use tokio::task::JoinHandle;

/// [Peer] provides inter-process messaging between application and the p2p
/// network. It started the main thread for the p2p network and handles for the [tokio::task]
/// by calling [crate::engine::start].
pub struct Peer {
    /// Engine handle for the [tokio::task] which is the main thread for
    /// the p2p network (see[crate::engine])
    pub(crate) engine: JoinHandle<()>,
    /// mpsc sender for delivering message to p2p network
    pub(crate) to_engine: tokio::sync::mpsc::Sender<EngineCommand>,
}

impl Peer {
    pub async fn start(
        config: Config,
        subscribe_topics: Vec<Topic>,
        message_gates: MessageGateChain,
    ) -> Self {
        return engine::start(config, subscribe_topics, message_gates)
            .await
            .unwrap();

    }

    pub fn broadcast_mempool_msg(&self, content: &TransactionV1) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::Mempool,
            Message::Mempool(content.clone()),
        ));
    }

    pub fn broadcast_dropped_txn_msg(&self, content: DroppedTxMessage) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::DroppedTxns,
            Message::DroppedTx(content),
        ));
    }

    pub fn broadcast_hotstuff_rs_msg(&self, content: hotstuff_rs::messages::Message) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::HotStuffRsBroadcast,
            Message::Consensus(content),
        ));
    }

    pub fn send_hotstuff_rs_msg(&self, address: PublicAddress, content: hotstuff_rs::messages::Message) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::HotStuffRsSend(address),
            Message::Consensus(content)
        ));
    }
}

/// A command to send messages to all peers subscribed to the topic ([Publish](EngineCommand::Publish))
/// Or a command to gracefully shutdown the engine thread ([Shutdown](EngineCommand::Shutdown)).
pub enum EngineCommand {
    /// Publish to all peers with subscribed topic
    Publish(Topic, Message),

    /// Shuts down the Engine thread
    Shutdown, 
}
