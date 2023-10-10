//! Usage of PeerBuilder
//! 
//! let peer = PeerBuilder::new()
//!     .configuration(Configuration::fullnode(...))
//!     .on_receive_msg(msg_handler)
//!     .start();

use libp2p::PeerId;
use pchain_types::blockchain::TransactionV1;
use pchain_types::cryptography::PublicAddress;
use tokio::task::JoinHandle;

use crate::messages::{Topic, Message, DroppedTxMessage};
use crate::config::Config;

pub struct PeerBuilder {
    pub(crate) config: Option<Config>,

}

impl PeerBuilder {
    pub fn new() -> PeerBuilder {
        PeerBuilder {
            config: None,
        }
    }

    pub fn configuration(&mut self, config: Config) -> PeerBuilder{
        self.config = Some(config);
    }

    fn on_receive_msg(self, handler: impl Fn(PeerId, Message)) -> PeerBuilder {

    }


    pub fn start(self) -> Peer {
        crate::engine::start(self).await.unwrap()
    }
}



pub struct Peer {
    pub(crate) engine: JoinHandle<()>,
    to_engine: tokio::sync::mpsc::Sender<EngineCommand>,
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

pub(crate) enum EngineCommand {
    Publish(Topic, Message),
    Shutdown,
}