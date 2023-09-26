/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This is the entry point to the pchain_network library. It starts a ParallelChain Network peer
//! and keeps the peer alive -- the peer stops working when the thread is dropped.
//!
//! To send a message in the P2P network, call `.broadcast_mempool_tx_msg()`, `broadcast_dropped_tx_msg`,
//! `broadcast_consensus_msg` or `.send_to()` directly.

use crate::Config;
use crate::engine;
use crate::message_gate::MessageGateChain;
use crate::messages::{Message, Topic};
use pchain_types::{blockchain::TransactionV2, cryptography::PublicAddress};

/// [NetworkHandle] provides inter-process messaging between application and the p2p
/// network. It started the main thread for the p2p network and handles for the [tokio::task]
/// by calling [crate::engine::start].
#[derive(Clone)]
pub struct NetworkHandle {
    /// mpsc sender for delivering message to p2p network
    pub(crate) sender: tokio::sync::mpsc::Sender<SendCommand>,
}

impl NetworkHandle {
    pub async fn start(
        config: Config,
        subscribe_topics: Vec<Topic>,
        message_gates: MessageGateChain,
    ) -> Self {
        let handle = engine::start(config, subscribe_topics, message_gates)
            .await
            .unwrap();
        Self {
            sender: handle.sender,
        }
    }

    pub fn broadcast_mempool_tx_msg(&self, content: &TransactionV2) {
        let _ = self.sender.try_send(SendCommand::Broadcast(
            Topic::Mempool,
            Message::Mempool(content.clone()),
        ));
    }

    pub fn broadcast_hotstuff_rs_msg(&self, content: hotstuff_rs::messages::Message) {
        let _ = self.sender.try_send(SendCommand::Broadcast(
            Topic::HotstuffRS,
            Message::HotstuffRS(content),
        ));
    }

    pub fn send_to(&self, address: PublicAddress, content: hotstuff_rs::messages::Message) {
        let _ = self
            .sender
            .try_send(SendCommand::SendTo(address, Message::HotstuffRS(content)));
    }
}

/// A command to send a message either to a specific peer ([SendTo](SendCommand::SendTo)), or to all subscribers
/// of a network topic ([Broadcast](SendTo::Broadcast)).
pub enum SendCommand {
    /// send to a specific peer
    SendTo(PublicAddress, Message),

    /// broadcast to all peers
    Broadcast(Topic, Message),
}
