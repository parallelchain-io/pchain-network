/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! (Internal) configuration of libp2p [network behaviors](https://docs.rs/libp2p/latest/libp2p/swarm/index.html#network-behaviour).
//! 
//! Library users use [configuration](crate::configuration) to configure `pchain-network`. In turn, `pchain-network`
//! uses [behaviour](crate::behaviour) to configure libp2p.

use std::time::Duration;
use libp2p::{
    gossipsub::{Gossipsub, GossipsubEvent, GossipsubConfigBuilder, IdentTopic, GossipsubMessage, MessageId},
    identity::Keypair,
    kad::{Kademlia, KademliaEvent, store::MemoryStore, KademliaConfig},
    ping::{Ping, PingEvent, PingConfig}, 
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    Multiaddr, NetworkBehaviour, PeerId,
};
use pchain_types::cryptography::PublicAddress;
use crate::messages::{Message, NetworkTopic};

/// Compose Network Behaviours:
/// It can do several things:
/// 1. Add or Remove a peer from DHT (Kademlia)
/// 2. Perform random walk in DHT
/// 3. Subscribe a Gossipsub Topic (see [crate::messages::NetworkTopic])
/// 4. Send Gossipsub message
#[derive(NetworkBehaviour)]
#[behaviour(out_event = "PeerNetworkEvent")]
pub(crate) struct PeerNetworkBehaviour {
    /// Public address of this network peer
    #[behaviour(ignore)] 
    id: PublicAddress,

    kad: Kademlia<MemoryStore>,
    gossip: Gossipsub,
    identify: Identify,
    ping: Ping,
}

impl PeerNetworkBehaviour {
    pub fn new(id: PublicAddress, local_key: &Keypair, heartbeat_secs: u64) -> Self {
        let proto_version = "/pchain_p2p/1.0.0";
        let local_peer_id = PeerId::from(local_key.public());

        // Configure Kad
        let kad_config = KademliaConfig::default()
            .set_protocol_name(proto_version.as_bytes())
            .to_owned();
        let kad = Kademlia::<MemoryStore>::with_config(
            local_peer_id, 
            MemoryStore::new(local_peer_id), 
            kad_config
        );

        // Configure Identify
        let identify_config = IdentifyConfig::new(
            proto_version.to_string(), 
            local_key.public()
        );
        let identify = Identify::new(identify_config);

        // Configure Gossip
        let message_id_fn = |message: &GossipsubMessage| {
            // message id is: source + topic + sequence number
            let mut id_str = message.topic.as_str().to_string();
            let src_str = match message.source {
                Some(src) => src.to_base58(),
                None => "none".to_string()
            };
            id_str.push_str(&src_str);
            id_str.push_str(&message.sequence_number.unwrap_or_default().to_string());
            MessageId::from(id_str.to_string())
        };

        let gossip = Gossipsub::new(
            libp2p::gossipsub::MessageAuthenticity::Signed(local_key.clone()), 
            GossipsubConfigBuilder::default()
                .max_transmit_size(4*1024*1024) // block size is limitted to 2 MB. Multiply by factor of safety = 2.
                .message_id_fn(message_id_fn)
                .heartbeat_interval(Duration::from_secs(heartbeat_secs))
                .build().unwrap()
        ).unwrap();

        // Configure Ping
        let ping =  Ping::new(PingConfig::new().with_keep_alive(true));

        Self { 
            id,
            gossip,
            kad,
            identify,
            ping
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

    /// Subscribes Gossipsub topics, and also subscribe a network topic that uses Base64 encoded 
    /// public address of this network peer as topic.
    pub fn subscribe(&mut self, topics: Vec<NetworkTopic>) -> Result<(), Box<dyn std::error::Error>> {
        for topic in topics {
            self.gossip.subscribe(&topic.into())?;
        }

        self.gossip.subscribe(&NetworkTopic::from(self.id).into())?;
        Ok(())
    }

    /// Sends Message to peer with specific public address
    pub fn send_to(&mut self, address: PublicAddress, msg: Message) -> Result<libp2p::gossipsub::MessageId, libp2p::gossipsub::error::PublishError> {
        let topic: IdentTopic = NetworkTopic::from(address).into();
        self.gossip.publish(topic, msg)
    }

    /// Broadcasts Messages to peers with specific topic
    pub fn broadcast(&mut self, topic: IdentTopic, msg: Message) -> Result<libp2p::gossipsub::MessageId, libp2p::gossipsub::error::PublishError> {
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
    Gossip(GossipsubEvent),
    Ping(PingEvent),
    Identify(IdentifyEvent),
}

impl From<GossipsubEvent> for PeerNetworkEvent {
    fn from(event: GossipsubEvent) -> Self {
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