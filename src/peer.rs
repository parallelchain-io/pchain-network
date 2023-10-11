//! Usage of PeerBuilder
//! 
//! let peer = PeerBuilder::new()
//!     .configuration(Configuration::fullnode(...))
//!     .on_receive_msg(msg_handler)
//!     .start();

use pchain_types::blockchain::TransactionV1;
use pchain_types::cryptography::PublicAddress;
use tokio::task::JoinHandle;

use crate::messages::{Topic, Message, DroppedTxMessage};
use crate::config::Config;

pub struct PeerBuilder {
    pub config: Option<Config>,
    pub handlers: Option<Box<dyn Fn(PublicAddress, Message) + Send>>,
}

impl PeerBuilder {
    pub fn new() -> PeerBuilder {
        PeerBuilder {
            config: None,
            handlers: None,
        }
    }

    pub fn configuration(&mut self, config: Config) -> &mut Self{
        self.config = Some(config);
        self
    }

    pub fn on_receive_msg(&mut self, handlers: impl Fn(PublicAddress, Message) + Send + 'static) -> &mut Self {
        self.handlers = Some(Box::new(handlers));
        self
    }

    pub async fn start(self) -> Peer {
        crate::engine::start(self).await.unwrap()
    }
}

pub struct Peer {
    pub(crate) engine: JoinHandle<()>,
    pub(crate) to_engine: tokio::sync::mpsc::Sender<EngineCommand>,
}

impl Peer {
    pub fn broadcast_mempool_msg(&self, txn: TransactionV1) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::Mempool,
            Message::Mempool(txn)
        ));
    }

    pub fn broadcast_dropped_tx_msg(&self, msg: DroppedTxMessage) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::DroppedTxns,
            Message::DroppedTx(msg)
        ));
    }

    pub fn broadcast_hotstuff_rs_msg(&self, msg: hotstuff_rs::messages::Message) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::HotStuffRsBroadcast,
            Message::Consensus(msg)
        ));
    }

    pub fn send_hotstuff_rs_msg(&self, address: PublicAddress, msg: hotstuff_rs::messages::Message) {
        let _ = self.to_engine.try_send(EngineCommand::Publish(
            Topic::HotStuffRsSend(address),
            Message::Consensus(msg)
        ));
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.to_engine.try_send(EngineCommand::Shutdown);
    }
}

pub(crate) enum EngineCommand {
    Publish(Topic, Message),
    Shutdown,
}