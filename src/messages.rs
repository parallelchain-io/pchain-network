/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Messages that can be sent over Gossipsub.
//!
//! This module defines three main message-related types:
//! - [Topic]: topic of the gossipsub message in the network.
//! - [Message]: data that can be sent over Gossipsub.
//! - [Envelope]: a wrapper over the message and its origin.
//!
//! Message flow starts with a [Message] received from the network. This message is passed as [Envelope] into
//! [MessageGateChain] and is processed by the [MessageGate].

use borsh::BorshSerialize;
use libp2p::gossipsub::IdentTopic;
use pchain_types::{
    blockchain::TransactionV1,
    cryptography::{PublicAddress, Sha256Hash},
    serialization::Serializable,
};

/// Hash of the message topic.
pub type MessageTopicHash = libp2p::gossipsub::TopicHash;

/// [Topic] defines the topics available for subscribing.
#[derive(Debug, Clone, PartialEq)]
pub enum Topic {
    HotStuffRsBroadcast,
    HotStuffRsSend(PublicAddress),
    Mempool,
    DroppedTxns,
    
}

impl Topic {
    pub fn is(self, topic_hash: &MessageTopicHash) -> bool {
        topic_hash == &self.hash()
    }

    pub fn is_mailbox(topic_hash: &MessageTopicHash, node_address: PublicAddress) -> bool {
        topic_hash == &Topic::HotStuffRsSend(node_address).hash()
    }

    pub fn hash(self) -> MessageTopicHash {
        IdentTopic::from(self).hash()
    }
}

impl From<Topic> for IdentTopic {
    fn from(topic: Topic) -> Self {
        let str = match topic {
            Topic::HotStuffRsBroadcast => "consensus".to_string(),
            Topic::HotStuffRsSend(addr) => base64url::encode(addr),
            Topic::Mempool => "mempool".to_string(),
            Topic::DroppedTxns => "droppedTx".to_string(),
        };
        IdentTopic::new(str)
    }
}

/// [Message] contains the three types of messages that are allowed to be sent or received within the network.
#[derive(Clone)]
pub enum Message {
    Consensus(hotstuff_rs::messages::Message),
    Mempool(TransactionV1),
    DroppedTx(DroppedTxMessage),
}

impl From<Message> for Vec<u8> {
    fn from(msg: Message) -> Self {
        match msg {
            Message::Consensus(msg) => msg.try_to_vec().unwrap(),
            Message::Mempool(txn) => Serializable::serialize(&txn),
            Message::DroppedTx(msg) => msg.try_to_vec().unwrap(),
        }
    }
}

// [DroppedTxMessage] defines data content for Message::DroppedTx.
#[derive(Clone, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum DroppedTxMessage {
    MempoolDroppedTx {
        txn: TransactionV1,
        status_code: DroppedTxStatusCode,
    },
    ExecutorDroppedTx {
        tx_hash: Sha256Hash,
        status_code: DroppedTxStatusCode,
    },
}

#[derive(Clone)]
pub enum DroppedTxStatusCode {
    Invalid,
    NonceTooLow,
    NonceInaccessible,
}

impl From<&DroppedTxStatusCode> for u16 {
    fn from(status_code: &DroppedTxStatusCode) -> Self {
        match status_code {
            DroppedTxStatusCode::Invalid => 0x515_u16,
            DroppedTxStatusCode::NonceTooLow => 0x516_u16,
            DroppedTxStatusCode::NonceInaccessible => 0x517_u16,
        }
    }
}

impl borsh::BorshSerialize for DroppedTxStatusCode {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let status_code: u16 = self.into();
        status_code.serialize(writer)
    }
}

impl borsh::BorshDeserialize for DroppedTxStatusCode {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let status_code = match u16::deserialize_reader(reader) {
            Ok(0x515_u16) => DroppedTxStatusCode::Invalid,
            Ok(0x516_u16) => DroppedTxStatusCode::NonceTooLow,
            Ok(0x517_u16) => DroppedTxStatusCode::NonceInaccessible,
            _ => panic!("Invalid droppedTx status code."),
        };
        Ok(status_code)
    }
}

/// [Envelope] encapsulates the message received from the p2p network with it's sender address.
#[derive(Clone)]
pub struct Envelope {
    /// The origin of the message
    pub origin: PublicAddress,

    /// The message encapsulated
    pub message: Vec<u8>,
}

#[cfg(test)]

mod test {
    use crate::conversions;

    use super::*;
    use libp2p::identity::Keypair;

    #[test]
    fn test_broadcast_topic() {
        // Mempool topic
        let mempool_topic = Topic::Mempool;

        let ident_topic = IdentTopic::new("mempool".to_string());
        assert_eq!(mempool_topic.hash(), ident_topic.hash());

        // Consensus topic
        let consensus_topic = Topic::HotStuffRsBroadcast;

        let ident_topic = IdentTopic::new("consensus".to_string());
        assert_eq!(consensus_topic.hash(), ident_topic.hash());

        // Dropped Tx Topic
        let droppedtx_topic = Topic::DroppedTxns;

        let ident_topic = IdentTopic::new("droppedTx".to_string());
        assert_eq!(droppedtx_topic.hash(), ident_topic.hash());

        // Mailbox Topic
        let addr = [1u8;32];
        let mailbox_topic = Topic::HotStuffRsSend(addr);

        let ident_topic = IdentTopic::new(base64url::encode(addr));
        assert_eq!(mailbox_topic.hash(), ident_topic.hash())
    }

    #[test]
    fn test_mailbox_topic() {
        // Create new Network topic with MessageTopic::from() should result in same hash as creating with a public address string
        let test_public_address: PublicAddress =
            conversions::PublicAddress::try_from(Keypair::generate_ed25519().public())
                .unwrap()
                .into();
        let test_topic = Topic::HotStuffRsSend(test_public_address);

        let expected_topic = IdentTopic::new(String::from(base64url::encode(test_public_address)));
        assert_eq!(test_topic.hash(), expected_topic.hash());
    }

    #[test]
    fn test_classify_topic() {
        let test_public_address: PublicAddress =
            conversions::PublicAddress::try_from(Keypair::generate_ed25519().public())
                .unwrap()
                .into();
        let mempool_msg_hash = Topic::Mempool.hash();
        assert!(Topic::Mempool.is(&mempool_msg_hash));
        assert!(!Topic::is_mailbox(&mempool_msg_hash, test_public_address));

        let consensus_msg_hash = Topic::HotStuffRsBroadcast.hash();
        assert!(Topic::HotStuffRsBroadcast.is(&consensus_msg_hash));
        assert!(!Topic::is_mailbox(&consensus_msg_hash, test_public_address));

        let droppedtx_msg_hash = Topic::DroppedTxns.hash();
        assert!(Topic::DroppedTxns.is(&droppedtx_msg_hash));
        assert!(!Topic::is_mailbox(&droppedtx_msg_hash, test_public_address));

        let mailbox_topic_hash = Topic::HotStuffRsSend(test_public_address).hash();
        assert!(Topic::is_mailbox(&mailbox_topic_hash, test_public_address));
    }
}
