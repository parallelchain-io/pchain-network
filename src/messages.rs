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
use libp2p::gossipsub::{IdentTopic, TopicHash};
use pchain_types::{
    blockchain::TransactionV1,
    cryptography::{PublicAddress, Sha256Hash},
    serialization::Serializable,
};

use crate::{config,
    messages::Topic::{HotStuffRsBroadcast,HotStuffRsSend,Mempool,DroppedTxns}
};

/// Hash of the message topic.
pub type MessageTopicHash = libp2p::gossipsub::TopicHash;

/// [Topic] defines the topics of the messages in `pchain-network`.
#[derive(PartialEq, Debug, Clone)]
pub enum Topic {
    HotStuffRsBroadcast,
    HotStuffRsSend(PublicAddress),
    Mempool,
    DroppedTxns,
}

impl Topic {
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


/// [Message] are structured messages that are sent between ParallelChain Network Peers.
#[derive(Clone, BorshSerialize, BorshDeserialize)]
pub enum Message {
    HotStuffRs(hotstuff_rs::messages::Message),
    Mempool(TransactionV1),
    DroppedTxns(DroppedTxnMessage),
}

impl From<Message> for Vec<u8> {
    fn from(msg: Message) -> Self {
        match msg {
            Message::HotStuffRs(msg) => msg.try_to_vec().unwrap(),
            Message::Mempool(txn) => Serializable::serialize(&txn),
            Message::DroppedTxns(msg) => msg.try_to_vec().unwrap(),
        }
    }
}

impl TryFrom<(libp2p::gossipsub::Message, pchain_types::cryptography::PublicAddress)> for Message {
    type Error = MessageConversionError;

    fn try_from((message , local_public_address): (libp2p::gossipsub::Message, pchain_types::cryptography::PublicAddress)) 
    -> Result<Self, Self::Error> {
        let (topic_hash, data) = (message.topic, message.data);
        let mut data = data.as_slice();
        
        let topic = identify_topics(topic_hash, local_public_address)?;
        
        match topic {
            HotStuffRsBroadcast | HotStuffRsSend(_) => {
                let message = hotstuff_rs::messages::Message::deserialize(&mut data).map(Message::HotStuffRs)?;
                Ok(message)
            },
            Mempool => {
                let message = pchain_types::blockchain::TransactionV1::deserialize(&mut data).map(Message::Mempool)?;
                Ok(message)
            },
            DroppedTxns => {
                let message = DroppedTxnMessage::deserialize(&mut data).map(Message::DroppedTxns)?;
                Ok(message)
            }
        }
    }
}

fn identify_topics(topic_hash: TopicHash, addr: PublicAddress) -> Result<Topic, InvalidTopicError> {
    let topic = config::fullnode_topics(addr)
        .into_iter()
        .find(|t| t.clone().hash() == topic_hash)
        .ok_or(InvalidTopicError)?;
    Ok(topic)
}

#[derive(Debug)]
pub struct InvalidTopicError;

#[derive(Debug)]
pub enum MessageConversionError {
    DeserializeError(std::io::Error),
    InvalidTopicError(InvalidTopicError),
}

impl From<InvalidTopicError> for MessageConversionError {
    fn from(error: InvalidTopicError) -> MessageConversionError {
        MessageConversionError::InvalidTopicError(error)
    }
}

impl From<std::io::Error> for MessageConversionError {
    fn from(error: std::io::Error) -> MessageConversionError {
        MessageConversionError::DeserializeError(error)
    }
}

/// [DroppedTxnMessage] defines message content for [Message::DroppedTxns].
#[derive(Clone, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum DroppedTxnMessage {
    MempoolDroppedTx {
        txn: TransactionV1,
        status_code: DroppedTxnStatusCode,
    },
    ExecutorDroppedTx {
        tx_hash: Sha256Hash,
        status_code: DroppedTxnStatusCode,
    },
}

#[derive(Clone)]
pub enum DroppedTxnStatusCode {
    Invalid,
    NonceTooLow,
    NonceInaccessible,
}

impl From<&DroppedTxnStatusCode> for u16 {
    fn from(status_code: &DroppedTxnStatusCode) -> Self {
        match status_code {
            DroppedTxnStatusCode::Invalid => 0x515_u16,
            DroppedTxnStatusCode::NonceTooLow => 0x516_u16,
            DroppedTxnStatusCode::NonceInaccessible => 0x517_u16,
        }
    }
}

impl borsh::BorshSerialize for DroppedTxnStatusCode {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let status_code: u16 = self.into();
        status_code.serialize(writer)
    }
}

impl borsh::BorshDeserialize for DroppedTxnStatusCode {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let status_code = match u16::deserialize_reader(reader) {
            Ok(0x515_u16) => DroppedTxnStatusCode::Invalid,
            Ok(0x516_u16) => DroppedTxnStatusCode::NonceTooLow,
            Ok(0x517_u16) => DroppedTxnStatusCode::NonceInaccessible,
            _ => panic!("Invalid droppedTx status code."),
        };
        Ok(status_code)
    }
}

#[cfg(test)]

mod test {
    use libp2p::gossipsub::IdentTopic;

    use super::Topic;

    #[test]
    fn test_message_topic() {
        let hotstuff_broadcast_topic = Topic::HotStuffRsBroadcast;
        let ident_topic = IdentTopic::new("consensus".to_string());
        assert_eq!(hotstuff_broadcast_topic.hash(), ident_topic.hash());

        let hotstuff_send_topic = Topic::HotStuffRsSend([1u8; 32]);
        let ident_topic = IdentTopic::new(base64url::encode([1u8; 32]));
        assert_eq!(hotstuff_send_topic.hash(), ident_topic.hash());

        let mempool_topic = Topic::Mempool;
        let ident_topic = IdentTopic::new("mempool".to_string());
        assert_eq!(mempool_topic.hash(), ident_topic.hash());

        let droppedtxn_topic = Topic::DroppedTxns;
        let ident_topic = IdentTopic::new("droppedTx".to_string());
        assert_eq!(droppedtxn_topic.hash(), ident_topic.hash());
    }
}
