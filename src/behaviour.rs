/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! (Internal) configuration of libp2p [Network Behaviors](https://docs.rs/libp2p/latest/libp2p/swarm/index.html#network-behaviour).
//!
//! `pchain_network` uses [behaviour](crate::behaviour) to configure libp2p protocols used:
//! Kademlia, Identify, Gossipsub, and Ping. It also defines functions for different behaviors
//! of the peer in the network. For instance, subscribe to a new topic, perform a random walk
//! in the network etc.

use crate::{
    conversions,
    messages::{Message, Topic},
};
use libp2p::{
    gossipsub::{self, ConfigBuilder, MessageAuthenticity, MessageId, PublishError, TopicHash},
    identify,
    identity::{
        ed25519::{Keypair, PublicKey},
        PeerId,
    },
    kad::{
        store::MemoryStore, Kademlia, KademliaConfig, KademliaEvent, KademliaStoreInserts, Mode,
    },
    ping,
    swarm::NetworkBehaviour,
    Multiaddr, StreamProtocol,
};
use pchain_types::cryptography::PublicAddress;
use std::{time::Duration, vec};

const MAX_TRANSMIT_SIZE: usize = 4;
const MEGABYTES: usize = 1048576;
const HEARTBEAT_INTERVAL: u64 = 10;

/// Defines behaviour of a node on pchain_network
/// 1. Add or Remove a peer from DHT (Kademlia)
/// 2. Perform random walk in DHT
/// 3. Subscribe to a gossipsub::Behaviour Topic (see [crate::messages::Topic])
/// 4. Publish a Gossipsub message
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "NetworkEvent")]
pub(crate) struct Behaviour {
    kad: Kademlia<MemoryStore>,
    gossip: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

impl Behaviour {
    pub fn new(id: PublicAddress, local_key: &Keypair, protocol_name: String) -> Self {
        let local_peer_id: PeerId = conversions::PublicAddress::new(id)
            .try_into()
            .expect("Invalid PublicAddress.");

        // Configure Kademlia
        let kad = Self::kad_config(local_peer_id, protocol_name.clone());

        // Configure Identify
        let identify = Self::identify_config(local_key.public(), protocol_name);

        // Configure Gossipsub - subscribe to the topic of its own the base64-encoded public address
        let mut gossip = Self::gossipsub_config(local_key);
        gossip.subscribe(&Topic::HotStuffRsSend(id).into()).unwrap();

        // Configure Ping
        let ping = ping::Behaviour::default();

        Self {
            gossip,
            kad,
            identify,
            ping,
        }
    }

    fn kad_config(peer_id: PeerId, protocol_name: String) -> Kademlia<MemoryStore> {
        let protocol_name = StreamProtocol::try_from_owned(protocol_name).unwrap();
        let kad_config = KademliaConfig::default()
            .set_protocol_names(vec![protocol_name])
            .set_record_filtering(KademliaStoreInserts::FilterBoth)
            .to_owned();

        let mut kad =
            Kademlia::<MemoryStore>::with_config(peer_id, MemoryStore::new(peer_id), kad_config);

        kad.set_mode(Some(Mode::Server));
        kad
    }

    fn identify_config(public_key: PublicKey, protocol_name: String) -> identify::Behaviour {
        let config = identify::Config::new(protocol_name, public_key.into());
        identify::Behaviour::new(config)
    }

    fn gossipsub_config(keypair: &Keypair) -> gossipsub::Behaviour {
        let gossip = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(keypair.clone().into()),
            ConfigBuilder::default()
                .max_transmit_size(MAX_TRANSMIT_SIZE * MEGABYTES) // block size is limitted to 2 MB. Multiply by factor of safety = 2.
                .heartbeat_interval(Duration::from_secs(HEARTBEAT_INTERVAL))
                .build()
                .unwrap(),
        )
        .unwrap();

        gossip
    }

    /// Add address to DHT
    pub fn add_address(&mut self, peer: &PeerId, address: Multiaddr) {
        self.kad.add_address(peer, address);
    }

    /// Remove a peer from DHT
    pub fn remove_peer(&mut self, peer: &PeerId) {
        self.kad.remove_peer(peer);
    }

    /// Query the network with random PeerId to discover peers in the network
    pub fn random_walk(&mut self) {
        self.kad.get_closest_peers(PeerId::random());
    }

    /// Subscribe to [Topic]
    pub fn subscribe(&mut self, topics: Vec<Topic>) -> Result<(), gossipsub::SubscriptionError> {
        for topic in topics {
            self.gossip.subscribe(&topic.into())?;
        }

        Ok(())
    }

    /// Unsubscribe from [Topic]
    pub fn unsubscribe(&mut self, topics: Vec<Topic>) -> Result<(), gossipsub::PublishError> {
        for topic in topics {
            self.gossip.unsubscribe(&topic.into())?;
        }

        Ok(())
    }

    /// Publish a [Message] to peers subscribed to the [Topic]
    pub fn publish(&mut self, topic: Topic, msg: Message) -> Result<MessageId, PublishError> {
        self.gossip.publish(topic.hash(), msg)
    }

    /// Check if the [gossipsub::TopicHash] is subscribed by this peer
    pub fn is_subscribed(&self, topic_hash: &TopicHash) -> bool {
        self.gossip.topics().any(|topic| topic_hash.eq(topic))
    }
}

/// The definition of Out-Event required by [Behaviour].
pub(crate) enum NetworkEvent {
    Kad(KademliaEvent),
    Gossip(gossipsub::Event),
    Ping(ping::Event),
    Identify(identify::Event),
}

impl From<gossipsub::Event> for NetworkEvent {
    fn from(event: gossipsub::Event) -> Self {
        Self::Gossip(event)
    }
}

impl From<KademliaEvent> for NetworkEvent {
    fn from(event: KademliaEvent) -> Self {
        Self::Kad(event)
    }
}

impl From<ping::Event> for NetworkEvent {
    fn from(event: ping::Event) -> Self {
        Self::Ping(event)
    }
}

impl From<identify::Event> for NetworkEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

#[cfg(test)]
mod test {
    use std::net::Ipv4Addr;

    use libp2p::{
        gossipsub,
        identity::{ed25519, PeerId},
        Multiaddr,
    };
    use pchain_types::cryptography::{PublicAddress, self};

    use crate::{
        config::Config,
        conversions,
        messages::{MessageTopicHash, Topic},
    };

    use super::Behaviour;

    struct Peer {
        public_address: PublicAddress,
        peer_id: PeerId,
        multi_addr: Multiaddr,
        behaviour: Behaviour,
    }

    fn create_new_peer() -> Peer {
        let keypair = ed25519::Keypair::generate();
        let local_keypair = cryptography::Keypair::from_keypair_bytes(&keypair.to_bytes()).unwrap();
        let config = Config {
            keypair: local_keypair,
            topics_to_subscribe: vec![Topic::HotStuffRsBroadcast],
            listening_port: 25519,
            boot_nodes: vec![],
            outgoing_msgs_buffer_capacity: 10,
            peer_discovery_interval: 10,
            kademlia_protocol_name: String::from("/test"),
        };

        let public_address = config.keypair.verifying_key().to_bytes();

        let behaviour = Behaviour::new(
            public_address,
            &keypair,
            config.kademlia_protocol_name,
        );

        let ip_addr = Ipv4Addr::new(127, 0, 0, 1);
        let multi_addr = format!("/ip4/{}/tcp/{}", ip_addr, config.listening_port)
            .parse()
            .unwrap();

        Peer {
            public_address,
            peer_id: conversions::PublicAddress::new(public_address)
                .try_into()
                .unwrap(),
            multi_addr,
            behaviour,
        }
    }

    #[test]
    fn test_add_and_remove_peer() {
        let mut peer1 = create_new_peer();
        let peer2 = create_new_peer();

        peer1
            .behaviour
            .add_address(&peer2.peer_id, peer2.multi_addr);

        let peer1_added_peer2 = peer1
            .behaviour
            .kad
            .kbuckets()
            .find(|entry| 
                entry.iter().find(|bucket| 
                    *bucket.node.key.preimage() == peer2.peer_id).is_some());
        
        assert!(peer1_added_peer2.is_some());
        
        peer1.behaviour.remove_peer(&peer2.peer_id);

        let peer1_removed_peer2 = peer1
            .behaviour
            .kad
            .kbuckets()
            .find(|entry| 
                entry.iter().find(|bucket| 
                    *bucket.node.key.preimage() == peer2.peer_id).is_some());

        assert!(peer1_removed_peer2.is_none());
    }

    #[test]
    fn test_subscribe_topics() {
        let mut peer = create_new_peer();

        // Peers subscribe to the hotstuff_rs topic with their public address during Behaviour configuration
        let mailbox_topic_hash = Topic::HotStuffRsSend(peer.public_address).hash();
        let mailbox_topic_msg = gossipsub::Message {
            source: None,
            data: vec![],
            sequence_number: None,
            topic: mailbox_topic_hash,
        };
        assert!(peer.behaviour.is_subscribed(&mailbox_topic_msg.topic));

        let hotstuff_rs_msg = gossipsub::Message {
            source: None,
            data: vec![],
            sequence_number: None,
            topic: Topic::HotStuffRsBroadcast.hash(),
        };
        let _ = peer.behaviour.subscribe(vec![Topic::HotStuffRsBroadcast]);

        let unsubscribed_msg = gossipsub::Message {
            source: None,
            data: vec![],
            sequence_number: None,
            topic: Topic::Mempool.hash(),
        };

        let subscribed_topics: Vec<&MessageTopicHash> = peer.behaviour.gossip.topics().collect();
        assert_eq!(subscribed_topics.len(), 2); //including the initial subscribed topic

        assert!(peer.behaviour.is_subscribed(&hotstuff_rs_msg.topic));
        assert!(!peer.behaviour.is_subscribed(&unsubscribed_msg.topic));
    }
}
