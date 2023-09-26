/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! (Internal) configuration of libp2p [Network Behaviors](https://docs.rs/libp2p/latest/libp2p/swarm/index.html#network-behaviour).
//!
//! Library users use [configuration](crate::config) to configure `pchain_network`. In turn, `pchain_network`
//! uses [behaviour](crate::behaviour) to configure libp2p.

use crate::constants::{self, MAX_TRANSMIT_SIZE, MEGABYTES};
use crate::conversions;
use crate::messages::{Message, Topic};
use libp2p::{
    gossipsub::{self, ConfigBuilder, IdentTopic, MessageAuthenticity, MessageId, PublishError},
    identify,
    identity::{Keypair, PublicKey},
    kad::{
        store::MemoryStore, Kademlia, KademliaConfig, KademliaEvent, KademliaStoreInserts, Mode,
    },
    ping,
    swarm::NetworkBehaviour,
    Multiaddr, PeerId, StreamProtocol,
};

use pchain_types::cryptography::PublicAddress;
use std::{time::Duration, vec};

/// Defines behaviour of a node on pchain_network
/// 1. Add or Remove a peer from DHT (Kademlia)
/// 2. Perform random walk in DHT
/// 3. Subscribe to a gossipsub::Behaviour Topic (see [crate::messages::Topic])
/// 4. Send or Broadcast Gossipsub message
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "PeerNetworkEvent")]
pub(crate) struct PeerBehaviour {
    kad: Kademlia<MemoryStore>,
    gossip: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

impl PeerBehaviour {
    pub fn new(id: PublicAddress, local_key: &Keypair, heartbeat_secs: u64) -> Self {
        let local_peer_id: PeerId = conversions::PublicAddress::new(id)
            .try_into()
            .expect("Invalid PublicAddress.");

        // Configure Kademlia
        let kad = Self::kad_config(local_peer_id);

        // Configure Identify
        let identify = Self::identify_config(local_key.public());

        // Configure Gossipsub - subscribe to the topic of its own the base64-encoded public address
        let mut gossip = Self::gossipsub_config(local_key, heartbeat_secs);
        gossip.subscribe(&Topic::Mailbox(id).into()).unwrap();

        // Configure Ping
        let ping = ping::Behaviour::default();

        Self {
            gossip,
            kad,
            identify,
            ping,
        }
    }

    fn kad_config(peer_id: PeerId) -> Kademlia<MemoryStore> {
        let kad_config = KademliaConfig::default()
            .set_protocol_names(vec![StreamProtocol::new(constants::PROTOCOL_NAME)])
            .set_record_filtering(KademliaStoreInserts::FilterBoth)
            .to_owned();

        let mut kad =
            Kademlia::<MemoryStore>::with_config(peer_id, MemoryStore::new(peer_id), kad_config);

        kad.set_mode(Some(Mode::Server));
        kad
    }

    fn identify_config(public_key: PublicKey) -> identify::Behaviour {
        let config = identify::Config::new(constants::PROTOCOL_NAME.to_string(), public_key);
        identify::Behaviour::new(config)
    }

    fn gossipsub_config(keypair: &Keypair, heartbeat_secs: u64) -> gossipsub::Behaviour {
        let gossip = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(keypair.clone()),
            ConfigBuilder::default()
                .max_transmit_size(MAX_TRANSMIT_SIZE * MEGABYTES) // block size is limitted to 2 MB. Multiply by factor of safety = 2.
                .heartbeat_interval(Duration::from_secs(heartbeat_secs))
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
    pub fn subscribe(&mut self, topics: Vec<Topic>) -> Result<(), Box<dyn std::error::Error>> {
        for topic in topics {
            self.gossip.subscribe(&topic.into())?;
        }

        Ok(())
    }

    /// Unsubscribe from [BroadcastTopic]
    pub fn unsubscribe(&mut self, topics: Vec<Topic>) -> Result<(), Box<dyn std::error::Error>> {
        for topic in topics {
            self.gossip.unsubscribe(&topic.into())?;
        }

        Ok(())
    }

    /// Send [Message] to peers subscribed to given address
    pub fn send_to(
        &mut self,
        address: PublicAddress,
        msg: Message,
    ) -> Result<MessageId, PublishError> {
        let topic = Topic::Mailbox(address).hash();
        let content: Vec<u8> = msg.into();
        self.gossip.publish(topic, content)
    }

    /// Broadcast [Message] with a specific topic
    pub fn broadcast(
        &mut self,
        topic: IdentTopic,
        msg: Message,
    ) -> Result<MessageId, PublishError> {
        let content: Vec<u8> = msg.into();
        self.gossip.publish(topic, content)
    }

    /// Check if the [gossipsub::TopicHash] is subscribed by this peer
    pub fn is_subscribed(&self, topic_hash: &gossipsub::TopicHash) -> bool {
        self.gossip.topics().any(|topic| topic_hash.eq(topic))
    }
}

/// The definition of Out-Event required by [PeerBehaviour].
pub(crate) enum PeerNetworkEvent {
    Kad(KademliaEvent),
    Gossip(gossipsub::Event),
    Ping(ping::Event),
    Identify(identify::Event),
}

impl From<gossipsub::Event> for PeerNetworkEvent {
    fn from(event: gossipsub::Event) -> Self {
        Self::Gossip(event)
    }
}

impl From<KademliaEvent> for PeerNetworkEvent {
    fn from(event: KademliaEvent) -> Self {
        Self::Kad(event)
    }
}

impl From<ping::Event> for PeerNetworkEvent {
    fn from(event: ping::Event) -> Self {
        Self::Ping(event)
    }
}

impl From<identify::Event> for PeerNetworkEvent {
    fn from(event: identify::Event) -> Self {
        Self::Identify(event)
    }
}

#[cfg(test)]

mod test {
    use std::net::Ipv4Addr;

    use super::PeerBehaviour;
    use crate::{Config, conversions, messages::{Topic, MessageTopicHash}};

    use libp2p::{gossipsub, Multiaddr, PeerId};
    use pchain_types::cryptography::PublicAddress;

    struct PeerInfo {
        public_address: PublicAddress,
        peer_id: PeerId,
        multi_addr: Multiaddr,
        behaviour: PeerBehaviour,
    }

    fn create_new_peer() -> PeerInfo {
        let peer_config = Config::default();
        let peer_id = peer_config.keypair.public().to_peer_id();
        let peer_public_address = conversions::PublicAddress::try_from(peer_id)
            .unwrap()
            .into();
        let peer_ip_addr = Ipv4Addr::new(127, 0, 0, 1);
        let peer_multiaddr = format!("/ip4/{}/tcp/{}", peer_ip_addr, peer_config.port)
            .parse()
            .unwrap();

        let behaviour = PeerBehaviour::new(
            peer_public_address,
            &peer_config.keypair,
            peer_config.peer_discovery_interval,
        );

        PeerInfo {
            public_address: peer_public_address,
            peer_id,
            multi_addr: peer_multiaddr,
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

        let peer_num: usize = peer1
            .behaviour
            .kad
            .kbuckets()
            .map(|x| x.num_entries())
            .sum();
        assert_eq!(peer_num, 1);

        peer1.behaviour.remove_peer(&peer2.peer_id);

        let peer_num: usize = peer1
            .behaviour
            .kad
            .kbuckets()
            .map(|x| x.num_entries())
            .sum();
        assert_eq!(peer_num, 0);
    }

    #[test]
    fn test_subscribe_topics() {
        let mut peer1 = create_new_peer();

        let self_topic_hash = Topic::Mailbox(peer1.public_address).hash();

        let self_topic_message = gossipsub::Message {
            source: None,
            data: vec![],
            sequence_number: None,
            topic: self_topic_hash,
        };
        assert!(peer1.behaviour.is_subscribed(&self_topic_message.topic));

        // create new Message with Topic::Consensus and subscribe
        let hotstuff_msg = gossipsub::Message {
            source: None,
            data: vec![],
            sequence_number: None,
            topic: Topic::HotstuffRS.hash(),
        };
        let _ = peer1.behaviour.subscribe(vec![Topic::HotstuffRS]);

        // create Message with unsubscribed topic
        let unsubscribed_msg = gossipsub::Message {
            source: None,
            data: vec![],
            sequence_number: None,
            topic: Topic::Mempool.hash(),
        };

        let subscribed_topics: Vec<&MessageTopicHash> = peer1.behaviour.gossip.topics().collect();
        assert_eq!(subscribed_topics.len(), 2); //including the initial subscribed topic

        assert!(peer1.behaviour.is_subscribed(&hotstuff_msg.topic));
        assert!(!peer1.behaviour.is_subscribed(&unsubscribed_msg.topic));
    }
}
