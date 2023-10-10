/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! (Internal) configuration of libp2p [Network Behaviors](https://docs.rs/libp2p/latest/libp2p/swarm/index.html#network-behaviour).
//!
//! Library users use [configuration](crate::config) to configure `pchain_network`. In turn, `pchain_network`
//! uses [behaviour](crate::behaviour) to configure libp2p.

use crate::{messages::{Message, Topic}, conversions};
use libp2p::{
    gossipsub::{self, ConfigBuilder, MessageAuthenticity, MessageId, PublishError},
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

const MAX_TRANSMIT_SIZE: usize = 4;
const MEGABYTES: usize = 1048576;

/// Defines behaviour of a node on pchain_network
/// 1. Add or Remove a peer from DHT (Kademlia)
/// 2. Perform random walk in DHT
/// 3. Subscribe to a gossipsub::Behaviour Topic (see [crate::messages::Topic])
/// 4. Send or Broadcast Gossipsub message
#[derive(NetworkBehaviour)]
#[behaviour(to_swarm = "PeerNetworkEvent")]
pub(crate) struct Behaviour {
    kad: Kademlia<MemoryStore>,
    gossip: gossipsub::Behaviour,
    identify: identify::Behaviour,
    ping: ping::Behaviour,
}

impl Behaviour {
    pub fn new(id: PublicAddress, local_key: &Keypair, heartbeat_secs: u64, protocol_name: &str) -> Self {
        let local_peer_id: PeerId = conversions::PublicAddress::new(id)
            .try_into()
            .expect("Invalid PublicAddress.");

        // Configure Kademlia
        let kad = Self::kad_config(local_peer_id, protocol_name);

        // Configure Identify
        let identify = Self::identify_config(local_key.public(), protocol_name);

        // Configure Gossipsub - subscribe to the topic of its own the base64-encoded public address
        let mut gossip = Self::gossipsub_config(local_key, heartbeat_secs);
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

    fn kad_config(peer_id: PeerId, protocol_name: &str) -> Kademlia<MemoryStore> {
        let protocol_name = StreamProtocol::try_from_owned(protocol_name.to_string()).unwrap();
        let kad_config = KademliaConfig::default()
            .set_protocol_names(vec![protocol_name])
            .set_record_filtering(KademliaStoreInserts::FilterBoth)
            .to_owned();

        let mut kad =
            Kademlia::<MemoryStore>::with_config(peer_id, MemoryStore::new(peer_id), kad_config);

        kad.set_mode(Some(Mode::Server));
        kad
    }

    fn identify_config(public_key: PublicKey, protocol_ver: &str) -> identify::Behaviour {
        let config = identify::Config::new(protocol_ver.to_string(), public_key);
        identify::Behaviour::new(config)
    }

    fn gossipsub_config(keypair: &Keypair, heartbeat_secs: u64) -> gossipsub::Behaviour {
        let build_msg_id = |msg: &gossipsub::Message| {
            let mut id_str = msg.topic.to_string();
            let src_peer_id = match msg.source {
                Some(src) => base64url::encode(src.to_bytes()),
                None => "none".to_string(),
            };
            id_str.push_str(&src_peer_id);
            id_str.push_str(&msg.sequence_number.unwrap_or_default().to_string());
            MessageId::from(id_str)
        };

        let gossip = gossipsub::Behaviour::new(
            MessageAuthenticity::Signed(keypair.clone()),
            ConfigBuilder::default()
                .max_transmit_size(MAX_TRANSMIT_SIZE * MEGABYTES) // block size is limitted to 2 MB. Multiply by factor of safety = 2.
                .message_id_fn(build_msg_id)
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

    // /// Send [Message] to peer with the given address
    // pub fn send_to(
    //     &mut self,
    //     address: PublicAddress,
    //     msg: Message,
    // ) -> Result<MessageId, PublishError> {
    //     let topic = Topic::HotStuffRsSend(address).hash();
    //     self.gossip.publish(topic, msg)
    // }

    // /// Broadcast [Message] with a specific topic
    // pub fn broadcast(
    //     &mut self,
    //     topic: IdentTopic,
    //     msg: Message,
    // ) -> Result<MessageId, PublishError> {
    //     self.gossip.publish(topic, msg)
    // }

    pub fn publish(
        &mut self,
        topic: Topic,
        msg: Message,
    ) -> Result<MessageId, PublishError> {
        self.gossip.publish(topic.hash(), msg)
    }

    /// Check if the [gossipsub::Message] is subscribed by this peer
    pub fn is_subscribed(&self, message: &gossipsub::Message) -> bool {
        self.gossip.topics().any(|topic| message.topic.eq(topic))
    }
}

/// The definition of Out-Event required by [Behaviour].
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

