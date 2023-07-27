/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Messages that can be sent using Gossipsub, as well as chainable "gates" to process them on receipt.
//!
//! This module defines four main types:
//! - [Message]: arbitrary data that can be sent over Gossipsub.
//! - [Envelope]: a wrapper over message which contains its origin.
//! - [MessageGateChain]: a chain of [MessageGate]s.
//! - [NetworkTopic]: the pub/sub topic of a message.
//!
//! Message flow starts with a Message received from the network. This message is passed as Envelope into
//! chain of [MessageGate]. Each gate examinates the message's [NetworkTopic] to decide whether it should be
//! proceed. If the gate is not interested in the topic, the message is directly passed to the next gate. Otherwise,
//! the message is proceeded and then the gate can decide whether the message should be passed to next gate, or
//! terminate this message flow.

use async_trait::async_trait;
use libp2p::gossipsub::IdentTopic;
use pchain_types::cryptography::PublicAddress;

use crate::conversions;

/// The arbitrary user-defined data that is being transmitted within the network.
pub type Message = Vec<u8>;

/// Envelope encapsulates the message received from the p2p network with the network
/// information such as sender address.
#[derive(Clone)]
pub struct Envelope {
    /// The origin of the received message
    pub origin: PublicAddress,

    /// The message encapsulated
    pub message: Message,
}

/// MessageGate is a message handler to proceed the message. It is used
/// with [MessageGateChain] to pass the message to other gate along the chain,
/// as well as stop passing message to the next gate.
///
/// Macro `async_trait` has to be added for using this trait, Example:
///
/// ```no_run
/// struct MyGate {}
///
/// #[async_trait]
/// impl MessageGate for MyGate {
///     async fn can_proceed(&self, topic_hash: &NetworkTopicHash) -> bool {
///         // ... topic filtering
///         true
///     }
///     async fn proceed(&self, envelope: Envelope) -> bool {
///         // ... do something with envelope
///         false // pass the message to next gate
///     }
/// }
/// ```
#[async_trait]
pub trait MessageGate: Send + Sync + 'static {
    /// check if the message type can be accepted to be proceed
    async fn can_proceed(&self, topic_hash: &NetworkTopicHash) -> bool;

    /// proceed the message and return true if the chain should be terminated
    async fn proceed(&self, envelope: Envelope) -> bool;
}

/// Chain of MessageGate. It consists of a sequence of message handlers.
/// Each message handler (Gate) implements its own message processing logic. Hence,
/// the chain of gates defines a complete flow of message processing.
///
/// ### Example
///
/// To add Gate to the chain:
/// ```no_run
/// let chain = MessageGateChain::new()
///     .chain(message_handler_1)
///     .chain(message_handler_2);
/// ```
#[derive(Default)]
pub struct MessageGateChain {
    gates: Vec<Box<dyn MessageGate>>,
}

impl MessageGateChain {
    pub fn new() -> Self {
        Self { gates: Vec::new() }
    }

    /// append a Message Gate at the end of the chain
    pub fn chain(mut self, gate: impl MessageGate) -> Self {
        self.gates.push(Box::new(gate));
        self
    }

    /// message_in inputs the received message and pass it to the chain of MessageGate
    pub(crate) async fn message_in(&self, topic_hash: &NetworkTopicHash, envelope: Envelope) {
        for gate in &self.gates {
            if gate.can_proceed(topic_hash).await && gate.proceed(envelope.clone()).await {
                break;
            }
        }
    }
}

#[derive(Debug)]
/// The Topic of the gossipsub message in the network. It basically wraps over [IdentTopic].
pub struct NetworkTopic(IdentTopic);

/// Hash of the Network message topic.
pub type NetworkTopicHash = libp2p::gossipsub::TopicHash;

impl From<NetworkTopic> for IdentTopic {
    fn from(topic: NetworkTopic) -> Self {
        topic.0
    }
}

impl From<PublicAddress> for NetworkTopic {
    fn from(address: PublicAddress) -> Self {
        Self(IdentTopic::new(conversions::base64_string(address)))
    }
}

impl NetworkTopic {
    pub fn new(topic: String) -> Self {
        NetworkTopic(IdentTopic::new(topic))
    }

    pub fn hash(&self) -> libp2p::gossipsub::TopicHash {
        self.0.hash()
    }
}

#[cfg(test)]

mod test {
    use super::*;
    use libp2p::identity::Keypair;

    #[test]
    fn test_network_topic() {
        // Create new Network topic
        let network_topic = NetworkTopic::new(String::from("new topic!"));
        let topic_hash = network_topic.0.hash();
        let ident_topic = IdentTopic::from(network_topic);
        assert_eq!(ident_topic.hash(), topic_hash);

        // Create new Network topic with NetworkTopic::from() should result in same hash as creating with a public address string
        let test_public_address =
            conversions::public_address(&Keypair::generate_ed25519().public()).unwrap();
        let network_topic_from_address = NetworkTopic::from(test_public_address);
        let network_topic = NetworkTopic::new(String::from(base64url::encode(test_public_address)));
        assert_eq!(network_topic_from_address.hash(), network_topic.hash());
    }
}
