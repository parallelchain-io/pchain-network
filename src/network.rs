/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! [Network], the handle type you use to send messages.
//! 
//! Network is returned from [engine::start](crate::engine::start). It keeps the thread operating the
//! peer alive--the peer stops working when it is dropped.
//! 
//! To send a message using Network, call its [sender](Network::sender) method to get a sender, then
//! call `.send()` on the sender passing in a [SendCommand].

use std::{sync::Arc, collections::HashMap};
use futures::{Future, FutureExt};
use tokio::{task::{JoinError, JoinHandle}, sync::Mutex};
use libp2p::PeerId;
use pchain_types::cryptography::PublicAddress;
use crate::messages::{Message, NetworkTopic};

/// Network is the handle returned by [crate::engine::start]. It provides
/// Inter-process messaging between application and p2p network.
pub struct Network {
    /// Network handle for the [tokio::task] which is the main thread for
    /// the p2p network (see [crate::engine]).
    pub(crate) network_thread: JoinHandle<()>,

    /// Mapping between identified peers and their public address
    pub(crate) peer_public_addrs: Arc<Mutex<HashMap<PeerId, PublicAddress>>>,

    /// mpsc sender for delivering message to p2p network
    pub(crate) sender: tokio::sync::mpsc::Sender<SendCommand>,
}

impl Network {
    /// sender is the channel for intake of SendCommand so that message can be sent to network by the Engine.
    pub fn sender(&self) -> tokio::sync::mpsc::Sender<SendCommand> {
        self.sender.clone()
    }

    /// Get list of the discovered and identified addresses in the network
    pub async fn list_addresses(&self) -> Vec<PublicAddress>{
        self.peer_public_addrs.lock().await.values().copied().collect()
    }

    /// abort the networking process
    /// 
    /// ### Example: 
    /// ```no_run
    /// let network = pchain_network::start(config, gates).await.unwrap();
    /// // ...
    /// network.stop().await;
    /// ```
    pub async fn stop(self) {
        self.network_thread.abort();
        log::debug!("pchain network stop!");
    }
}

impl Future for Network {
    type Output = Result<(), JoinError>;
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut s = self;
        s.network_thread.poll_unpin(cx)
    }
}

/// A command to send a message either to a specific peer ([SendTo](SendCommand::SendTo)), or to all subscribers
/// of a network topic ([Broadcast](SendTo::Broadcast)).
pub enum SendCommand {
    /// expects a peer with specific PublicAddress would be interested in
    SendTo(PublicAddress, Message),

    /// does not care which peer would be interested in
    Broadcast(NetworkTopic, Message)
}