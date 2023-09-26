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
    blockchain::TransactionV2, cryptography::PublicAddress, serialization::Serializable,
};

/// Hash of the message topic.
pub type MessageTopicHash = libp2p::gossipsub::TopicHash;

/// [Topic] defines the topics available for subscribing.
#[derive(Debug, Clone)]
pub enum Topic {
    HotstuffRS,
    Mempool,
    Mailbox(PublicAddress),
}

impl Topic {
    pub fn is(self, topic_hash: &MessageTopicHash) -> bool {
        topic_hash == &self.hash()
    }

    pub fn is_mailbox(topic_hash: &MessageTopicHash, node_address: PublicAddress) -> bool {
        topic_hash == &Topic::Mailbox(node_address).hash()
    }

    pub fn hash(&self) -> MessageTopicHash {
        IdentTopic::from(self.to_owned()).hash()
    }
}

impl From<Topic> for IdentTopic {
    fn from(topic: Topic) -> Self {
        let str = match topic {
            Topic::HotstuffRS => "hotstuff_rs".to_string(),
            Topic::Mempool => "mempool".to_string(),
            Topic::Mailbox(addr) => format!("hotstuff_rs/{}", base64url::encode(addr)),
        };
        IdentTopic::new(str)
    }
}

/// [Message] contains the three types of messages that are allowed to be sent or received within the network.
#[derive(Clone)]
pub enum Message {
    HotstuffRS(hotstuff_rs::messages::Message),
    Mempool(TransactionV2),
}

impl From<Message> for Vec<u8> {
    fn from(msg: Message) -> Self {
        match msg {
            Message::HotstuffRS(msg) => msg.try_to_vec().unwrap(),
            Message::Mempool(txn) => Serializable::serialize(&txn),
        }
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
        let hotstuff_topic = Topic::HotstuffRS;

        let ident_topic = IdentTopic::new("hotstuff_rs".to_string());
        assert_eq!(hotstuff_topic.hash(), ident_topic.hash());

        // Mailbox Topic
        let addr = [1u8;32];
        let mailbox_topic = Topic::Mailbox(addr);

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
        let test_topic = Topic::Mailbox(test_public_address);

        let expected_topic = IdentTopic::new(format!(
            "hotstuff_rs/{}",
            base64url::encode(test_public_address)
        ));
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

        let hotstuff_msg_hash = Topic::HotstuffRS.hash();
        assert!(Topic::HotstuffRS.is(&hotstuff_msg_hash));
        assert!(!Topic::is_mailbox(&hotstuff_msg_hash, test_public_address));

        let mailbox_topic_hash = Topic::Mailbox(test_public_address).hash();
        assert!(Topic::is_mailbox(&mailbox_topic_hash, test_public_address));
    }
}
