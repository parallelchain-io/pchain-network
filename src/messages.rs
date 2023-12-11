/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This module defines two main message-related types:
//! - [Topic]: topics of the messages in the network.
//! - [Message]: data to be sent in the network.
//!
//! `pchain-network` only accepts messages with the topics defined in [Topic]. Each topic corresponds
//! to a variant in [Message], which is an encapsulation of different types of data to be sent
//! in the pchain-network.
//!

use borsh::{BorshSerialize, BorshDeserialize};
use libp2p::gossipsub::IdentTopic;
use pchain_types::{
    cryptography::PublicAddress,
    rpc::TransactionV1OrV2,
};

/// Hash of the message topic.
pub type MessageTopicHash = libp2p::gossipsub::TopicHash;

/// [Topic] defines the topics of the messages in `pchain-network`.
#[derive(PartialEq, Debug, Clone)]
pub enum Topic {
    HotStuffRsBroadcast,
    HotStuffRsSend(PublicAddress),
    Mempool,
}

impl Topic {
    pub fn hash(self) -> MessageTopicHash {
        IdentTopic::from(self).hash()
    }
}

impl From<Topic> for IdentTopic {
    fn from(topic: Topic) -> Self {
        let str = match topic {
            Topic::HotStuffRsBroadcast => "hotstuff_rs".to_string(),
            Topic::HotStuffRsSend(addr) => String::from("hotstuff_rs/") + &base64url::encode(addr),
            Topic::Mempool => "mempool".to_string(),
        };
        IdentTopic::new(str)
    }
}


/// [Message] are structured messages that are sent between ParallelChain Network Peers.
#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub enum Message {
    HotStuffRs(hotstuff_rs::messages::Message),
    Mempool(TransactionV1OrV2),
}

impl From<Message> for Vec<u8> {
    fn from(msg: Message) -> Self {
        match msg {
            Message::HotStuffRs(msg) => msg.try_to_vec().unwrap(),
            Message::Mempool(txn) => {
                let mut data: Vec<u8> = Vec::new();
                TransactionV1OrV2::serialize(&txn, &mut data).unwrap();
                data
            }
        }
    }
}

#[cfg(test)]

mod test {
    use libp2p::gossipsub::IdentTopic;

    use super::Topic;

    #[test]
    fn test_message_topic() {
        let hotstuff_broadcast_topic = Topic::HotStuffRsBroadcast;
        let ident_topic = IdentTopic::new("hotstuff_rs".to_string());
        assert_eq!(hotstuff_broadcast_topic.hash(), ident_topic.hash());

        let hotstuff_send_topic = Topic::HotStuffRsSend([1u8; 32]);
        let ident_topic = IdentTopic::new(String::from("hotstuff_rs/") + &base64url::encode([1u8; 32]));
        assert_eq!(hotstuff_send_topic.hash(), ident_topic.hash());

        let mempool_topic = Topic::Mempool;
        let ident_topic = IdentTopic::new("mempool".to_string());
        assert_eq!(mempool_topic.hash(), ident_topic.hash());
    }
}
