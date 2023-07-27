/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! (Internal) configuration of libp2p [network behaviors](https://docs.rs/libp2p/latest/libp2p/swarm/index.html#network-behaviour).
//!
//! Library users use [configuration](crate::configuration) to configure `pchain-network`. In turn, `pchain-network`
//! uses [behaviour](crate::behaviour) to configure libp2p.

use crate::messages::{Message, NetworkTopic};
use libp2p::{
    gossipsub::{Event, ConfigBuilder, IdentTopic, Message as GossipsubMessage, MessageId},
    identity::{Keypair, PublicKey, ed25519},
    kad::{Kademlia, KademliaEvent, store::MemoryStore, KademliaConfig},
    ping::{Behaviour as Ping, Event as PingEvent, Config as PingConfig}, 
    identify::{Behaviour as Identify, Config as  IdentifyConfig, Event as IdentifyEvent},
    Multiaddr, PeerId, StreamProtocol
};

use libp2p::swarm::NetworkBehaviour;
use pchain_types::cryptography::PublicAddress;
use std::{time::Duration, vec};

/// Compose Network Behaviours:
/// It can do several things:
/// 1. Add or Remove a peer from DHT (Kademlia)
/// 2. Perform random walk in DHT
/// 3. Subscribe a gossipsub::Behaviour Topic (see [crate::messages::NetworkTopic])
/// 4. Send Gossipsub message
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "PeerNetworkEvent")]
pub(crate) struct PeerNetworkBehaviour {
    kad: Kademlia<MemoryStore>,
    gossip: libp2p::gossipsub::Behaviour,
    identify: Identify,
    ping: Ping,
}

impl PeerNetworkBehaviour {
    pub fn new(id: PublicAddress, local_key: &Keypair, heartbeat_secs: u64) -> Self {
        let proto_version = "/pchain_p2p/1.0.0";
        let public_key: PublicKey = ed25519::PublicKey::try_from_bytes(&id).expect("Invalid public key to setup peer newtork.").into();
        let local_peer_id = public_key.to_peer_id();

        // Configure Kad
        let proto_names= vec![StreamProtocol::new(proto_version)];
        let kad_config = KademliaConfig::default()
            .set_protocol_names(proto_names)
            .to_owned();
        let mut kad = Kademlia::<MemoryStore>::with_config(
            local_peer_id,
            MemoryStore::new(local_peer_id),
            kad_config,
        );

        kad.set_mode(Some(libp2p::kad::Mode::Server));

        // Configure Identify
        let identify_config = IdentifyConfig::new(proto_version.to_string(), local_key.public());
        let identify = Identify::new(identify_config);

        // Configure Gossip
        let message_id_fn = |message: &GossipsubMessage| {
            // message id is: source + topic + sequence number
            let mut id_str = message.topic.as_str().to_string();
            let src_str = match message.source {
                Some(src) => base64url::encode(src.to_bytes()),
                None => "none".to_string(),
            };
            id_str.push_str(&src_str);
            id_str.push_str(&message.sequence_number.unwrap_or_default().to_string());
            MessageId::from(id_str.to_string())
        };

        let mut gossip = libp2p::gossipsub::Behaviour::new(
            libp2p::gossipsub::MessageAuthenticity::Signed(local_key.clone()), 
            ConfigBuilder::default()
                .max_transmit_size(4*1024*1024) // block size is limitted to 2 MB. Multiply by factor of safety = 2.
                .message_id_fn(message_id_fn)
                .heartbeat_interval(Duration::from_secs(heartbeat_secs))
                .build()
                .unwrap(),
        )
        .unwrap();
        // subscribe a network topic that uses Base64 encoded public address of this network peer as topic.
        gossip.subscribe(&NetworkTopic::from(id).into()).unwrap();

        // Configure Ping
        let ping = Ping::new(PingConfig::new());

        Self { 
            gossip,
            kad,
            identify,
            ping,
        }
    }
}

impl PeerNetworkBehaviour {
    /// Adds address to DHT
    pub fn add_address(&mut self, peer: &PeerId, address: Multiaddr) {
        self.kad.add_address(peer, address);
    }
    /// Removes a peer from DHT
    pub fn remove_peer(&mut self, peer: &PeerId) {
        self.kad.remove_peer(peer);
    }
    /// Query the network with random PeerId so as to discover
    /// peers in the network.
    pub fn random_walk(&mut self) {
        self.kad.get_closest_peers(PeerId::random());
    }

    /// Subscribes libp2p::gossipsub::Behaviour topics
    pub fn subscribe(
        &mut self,
        topics: Vec<NetworkTopic>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        for topic in topics {
            self.gossip.subscribe(&topic.into())?;
        }

        Ok(())
    }

    /// Sends Message to peer with specific public address
    pub fn send_to(&mut self, address: PublicAddress, msg: Message) -> Result<libp2p::gossipsub::MessageId, libp2p::gossipsub::PublishError> {
        let topic: IdentTopic = NetworkTopic::from(address).into();
        self.gossip.publish(topic, msg)
    }

    /// Broadcasts Messages to peers with specific topic
    pub fn broadcast(&mut self, topic: IdentTopic, msg: Message) -> Result<libp2p::gossipsub::MessageId, libp2p::gossipsub::PublishError> {
        self.gossip.publish(topic, msg)
    }

    /// Check if the GossipsubMessage belongs to subscribed message topics
    pub fn is_subscribed(&self, message: &GossipsubMessage) -> bool {
        self.gossip.topics().any(|topic| message.topic.eq(topic))
    }
}

/// The definition of Out-Event required by [PeerNetworkBehaviour].

pub(crate) enum PeerNetworkEvent {
    Kad(KademliaEvent),
    Gossip(Event),
    Ping(PingEvent),
    Identify(IdentifyEvent),
}

impl From<Event> for PeerNetworkEvent {
    fn from(event: Event) -> Self {
        Self::Gossip(event)
    }
}

impl From<KademliaEvent> for PeerNetworkEvent {
    fn from(event: KademliaEvent) -> Self {
        Self::Kad(event)
    }
}

impl From<PingEvent> for PeerNetworkEvent {
    fn from(event: PingEvent) -> Self {
        Self::Ping(event)
    }
}

impl From<IdentifyEvent> for PeerNetworkEvent {
    fn from(event: IdentifyEvent) -> Self {
        Self::Identify(event)
    }
}

#[cfg(test)]

mod test {
    use std::net::Ipv4Addr;

    use super::PeerNetworkBehaviour;
    use crate::{conversions, messages::NetworkTopic, configuration::Config};

    use libp2p::{
        gossipsub::{Message as GossipsubMessage, TopicHash},
        Multiaddr, PeerId,
    };
    use pchain_types::cryptography::PublicAddress;

    struct PeerInfo {
        public_key: PublicAddress, 
        peer_id: PeerId,
        multi_addr: Multiaddr
    }

    fn setup_network_behaviour() -> PeerNetworkBehaviour {
        let config = Config::new();
        let local_public_address = conversions::public_address(&config.keypair.public()).unwrap();
        
        PeerNetworkBehaviour::new(
            local_public_address,
            &config.keypair,
            config.peer_discovery_interval
        )
    }

    fn create_new_peer() -> PeerInfo {
        let peer_config = Config::new();
        let peer_id = peer_config.keypair.public().to_peer_id();
        let peer_ip_addr = Ipv4Addr::new(127, 0, 0, 1);
        let peer_multiaddr = conversions::multiaddr(peer_ip_addr, peer_config.port);

        PeerInfo { 
            public_key: conversions::public_address(&peer_config.keypair.public()).unwrap(),
            peer_id,
            multi_addr: peer_multiaddr
        }
    }


    #[test]
    fn test_init_network_behaviour() {
        let network_behaviour = setup_network_behaviour();

        let subscribed_topics: Vec<&TopicHash> = network_behaviour.gossip.topics().collect();
        assert_eq!(subscribed_topics.len(), 1);
    }

    #[test]
    fn test_add_and_remove_peer() {
        let mut network_behaviour = setup_network_behaviour();
        
        let peer1 = create_new_peer();
        let peer2 = create_new_peer();

        network_behaviour.add_address(&peer1.peer_id, peer1.multi_addr);
        network_behaviour.add_address(&peer2.peer_id, peer2.multi_addr);

        let peer_num: usize = network_behaviour.kad.kbuckets().map(|x| x.num_entries()).sum();
        assert_eq!(peer_num, 2);

        network_behaviour.remove_peer(&peer1.peer_id);

        let peer_num: usize = network_behaviour.kad.kbuckets().map(|x| x.num_entries()).sum();
        assert_eq!(peer_num, 1);
    }

    #[test]
    fn test_subscribe_topics() {
        let mut network_behaviour = setup_network_behaviour();

        let peer1 = create_new_peer();

        // create new NetworkTopic
        let topic1 = NetworkTopic::new("hello world".to_string());
        let topic1_hash = topic1.hash().clone();

        // create NetworkTopic from PublicAddress
        let topic2 = NetworkTopic::from(peer1.public_key);
        let topic2_hash = topic2.hash().clone();

        let topics = vec![topic1, topic2];
        
        let _ = network_behaviour.subscribe(topics);
        let subscribed_topics: Vec<&TopicHash> = network_behaviour.gossip.topics().collect();
        assert_eq!(subscribed_topics.len(), 3); //including the initial subscribed topic
        
        let topic1_message = GossipsubMessage {
            source: None,
            data: vec![],
            sequence_number: None,
            topic: topic1_hash,
        };

        let topic2_message = GossipsubMessage {
            source: None,
            data: vec![],
            sequence_number: None,
            topic: topic2_hash,
        };
        assert!(network_behaviour.is_subscribed(&topic1_message));
        assert!(network_behaviour.is_subscribed(&topic2_message));
    }
}
