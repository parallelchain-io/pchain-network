/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Entry point to the pchain_network library.
//!
//! It starts a ParallelChain Network peer and keeps the peer alive -- the peer stops working when the
//! thread is dropped.
//!
//! To start a pchain-network peer, users pass a [Config] instance and the message handler into Peer::start().
//! [Config] contains the peer's keypair, or other deployment-specific parameters, such as listening ports, bootstrap nodes etc. 
//! Users need to define the message handler for processing the [Message]. 
//! Starting Peer will return a Sender for delivering PeerCommand to the thread. 
//! 
//! Example:
//! 
//! 1. Define the configurations
//! ```
//! let config = Config {...}
//! ```
//! // 2. Define the message handler 
//! ```
//! let(tx,rx) = mpsc::channel(100);
//! let message_sender = tx.clone();
//! let message_handler = move |msg_origin: [u8;32], msg: Message| {
//!     let _ = message_sender.send((msg_origin, msg));
//! };
//!  ```
//! 3. Start the peer
//!  ```
//! let peer = Peer::start(config, message_handler).await.unwrap();
//!  ```
//! 4. Send PeerCommand
//!  ```
//! peer.broadcast_mempool_msg(txn);
//!  ```

use futures::StreamExt;
use tokio::task::JoinHandle;
use libp2p::{
    kad::KBucketKey,
    gossipsub,
    identify, identity::{self, ed25519::Keypair, PeerId}, noise,
    SwarmBuilder,
    swarm::SwarmEvent,
    tcp,
};

use libp2p_mplex::MplexConfig;
use pchain_types::{cryptography::PublicAddress, rpc::TransactionV1OrV2};
use std::{net::Ipv4Addr,time::Duration};

use crate::{
    behaviour::{Behaviour, NetworkEvent},
    conversions,
    messages::{Message, Topic},
    config::Config,
};

const MAX_REPLICA_DISTANCE: u32 = 255;

pub struct Peer {
    /// Network handle for the [tokio::task] which is the main thread for the p2p network 
    pub(crate) handle: JoinHandle<()>,

    /// mpsc sender for delivering [PeerCommand] to the internal thread, commands are used to 
    /// publish messages with specific [Topic] to the p2p network.
    pub(crate) sender: tokio::sync::mpsc::Sender<PeerCommand>,
}

impl Peer {
/// Constructs a [Peer] from the given configuration and handler, starting the thread for the p2p network 
/// 1. Load network configuration to set up transport for the P2P network. 
/// 2. Peer starts listening on the given config address
/// 3. Establishes connection to the network by adding bootnodes and subscribing to message [Topic]. 
/// 4. Spawns an asynchronous [tokio] task and enters the event handling loop, returning a Sender used for sending 
/// [PeerCommand] to the internal thread.
    pub async fn start(config: Config, handler: Box<dyn FnMut(PublicAddress, Message) + Send>) -> Result<Peer, PeerStartError> {

        let mut swarm = set_up_transport(&config)
        .await
        .map_err(PeerStartError::BuildTransportError)?;

        swarm.listen_on(conversions::multi_addr(
                Ipv4Addr::new(0, 0, 0, 0), 
                config.listening_port
            ))
            .map_err(PeerStartError::UnsupportedAddressError)?;

        swarm = establish_network_connections(swarm, &config)
        .map_err(PeerStartError::SubscriptionError)?;

        let (handle, sender) = start_event_handling(swarm, &config, handler);
        Ok(
            Peer {
                handle,
                sender
            }
        )
    }

    pub fn broadcast_mempool_msg(&self, txn: TransactionV1OrV2) {
        let _ = self.sender.try_send(PeerCommand::Publish(
            Topic::Mempool,
            Message::Mempool(txn),
        ));
    }

    pub fn broadcast_hotstuff_rs_msg(&self, msg: hotstuff_rs::messages::Message) {
        let _ = self.sender.try_send(PeerCommand::Publish(
            Topic::HotStuffRsBroadcast,
            Message::HotStuffRs(msg),
        ));
    }

    pub fn send_hotstuff_rs_msg(
        &self,
        address: PublicAddress,
        msg: hotstuff_rs::messages::Message,
    ) {
        let _ = self.sender.try_send(PeerCommand::Publish(
            Topic::HotStuffRsSend(address),
            Message::HotStuffRs(msg),
        ));
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        let _ = self.sender.try_send(PeerCommand::Shutdown);
    }
}

/// [PeerCommand] defines commands to the internal thread which includes publishing messages
/// and shutting down the network when the peer is dropped.
pub(crate) enum PeerCommand {
    Publish(Topic, Message),
    Shutdown,
}

/// Loads the network configuration from [Config] and build the transport for the P2P network
async fn set_up_transport(config: &Config) -> Result<libp2p::Swarm<Behaviour>,libp2p::noise::Error> {
    // Read network configuration 
    let local_keypair = &config.keypair;
    let local_public_address: PublicAddress = local_keypair.verifying_key().to_bytes();
    let local_libp2p_keypair = Keypair::try_from_bytes(config.keypair.to_keypair_bytes().as_mut_slice()).unwrap();
    let local_peer_id = identity::Keypair::from(local_libp2p_keypair.clone())
        .public()
        .to_peer_id();

    log::info!("Local PeerId: {:?}", local_peer_id);

    // Instantiate Swarm
    let behaviour = Behaviour::new(
        local_public_address,
        &local_libp2p_keypair,
        config.kademlia_protocol_name.clone(),
    );
    let swarm = SwarmBuilder::with_existing_identity(local_libp2p_keypair.into())
        .with_tokio()
        .with_tcp(
            tcp::Config::new().nodelay(true), 
            noise::Config::new,
            (MplexConfig::default, libp2p::yamux::Config::default)
        )?
        .with_behaviour(|_| behaviour).unwrap()
        .with_swarm_config(|config| config.with_idle_connection_timeout(Duration::from_secs(20)))
        .build();
    Ok(swarm)
}

/// Peer establishes network specific connections: 
/// 1. Adds network bootnodes to local routing table
/// 2. Subscribes to network specific message [Topic] 
fn establish_network_connections(mut swarm: libp2p::Swarm<Behaviour> , config: &Config) -> Result<libp2p::Swarm<Behaviour>,gossipsub::SubscriptionError> {
    // Connection to bootstrap nodes
    if !config.boot_nodes.is_empty() {
        config.boot_nodes.iter().for_each(|peer_info| {
            let multiaddr = conversions::multi_addr(peer_info.1, peer_info.2);
            if let Ok(peer_id) = &conversions::PublicAddress::new(peer_info.0).try_into() {
                swarm
                .behaviour_mut()
                .add_address(peer_id, multiaddr);
            }
        });
    }

    // Subscribe to Topic
    swarm
        .behaviour_mut()
        .subscribe(config.topics_to_subscribe.clone())?;
    Ok(swarm)
}

/// Spawns the Main Event Handling loop for p2p network. It waits for:
/// 1. [NetworkEvent]
/// 2. [Commands](PeerCommand) from application for sending messages or termination
/// 3. a periodic interval to discover peers in the network
///
/// #### 1. [NetworkEvent] Handling
///
/// Upon receiving the Identify event, the information of the new peer will be added to the
/// routing table.
///
/// Upon receiving the Gossip event, the message will be deserailized to (Message)[crate::messages::Message]
/// if the peer is subscribed to the topic. The message will then be passed in the message handlers defined
/// by the user.
///
/// Upon receiving the ConnectionClosed event, peer information will be removed from the
/// routing table.
///
/// #### 2. [PeerCommand] Handling
///
/// Upon receiving a (Publish)[PeerCommand::Publish] command, the peer will publish the message to its
/// connected peers. The peer will process the message directly if it is a [Topic::HotStuffRsSend] message
/// directed to the peer's public address.
///
/// Upon receiving a (Shutdown)[PeerCommand::Shutdown] command, the process will exit the loop and terminate
/// the thread.
/// 
fn start_event_handling(mut swarm: libp2p::Swarm<Behaviour>, config: &Config, mut message_handler: Box<dyn FnMut(PublicAddress, Message) + Send>) -> 
    (JoinHandle<()>,tokio::sync::mpsc::Sender<PeerCommand>) {
    // 4. Start p2p networking
    let local_keypair = &config.keypair;
    let local_public_address: PublicAddress = local_keypair.verifying_key().to_bytes();
    let local_libp2p_keypair = Keypair::try_from_bytes(config.keypair.to_keypair_bytes().as_mut_slice()).unwrap();
    let local_peer_id = identity::Keypair::from(local_libp2p_keypair.clone())
        .public()
        .to_peer_id();

    let (sender, mut receiver) =
        tokio::sync::mpsc::channel::<PeerCommand>(config.outgoing_msgs_buffer_capacity);
    let mut discover_tick =
        tokio::time::interval(Duration::from_secs(config.peer_discovery_interval));

    let network_thread_handle = tokio::task::spawn(async move {
        loop {
            // 1. Wait for the following events:
            let (peer_command, event) = tokio::select! {
                biased;
                // Receive a PeerCommand from application
                peer_command = receiver.recv() => {
                    (peer_command, None)
                },
                // Receive a NetworkEvent
                event = swarm.select_next_some() => {
                    (None, Some(event))
                },
                // Time for network discovery
                _ = discover_tick.tick() => {
                    // Perform a random walk on DHT
                    swarm.behaviour_mut().random_walk();
                    (None, None)
                },
            };

            // 2. Deliver messages when a PeerCommand::Publish from the application is received
            // and shutdown Peer when a PeerCommand::Shutdown from the application is received
            if let Some(peer_command) = peer_command {
                match peer_command {
                    PeerCommand::Publish(topic, message) => {
                        log::info!("Publishing (Topic: {:?})", topic);
                        if swarm.behaviour().is_subscribed(&topic.clone().hash()) {
                            // Send it to ourselves if we subscribed to this topic
                            message_handler(local_public_address, message.clone());
                        } 
                        if let Err(e) = swarm.behaviour_mut().publish(topic, message) {
                            log::debug!("Failed to publish the message. {:?}", e);
                        }
                    }
                    PeerCommand::Shutdown => {
                        log::info!("Shutting down the Peer...");
                        break;
                    }
                }
            }

            // 3. Deliver messages when a NetworkEvent is received
            if let Some(event) = event {
                match event {
                    SwarmEvent::Behaviour(NetworkEvent::Gossip(gossipsub::Event::Message {
                        message,
                        ..
                    })) => {
                        if let Some(src_peer_id) = &message.source {
                            if let Ok(public_addr) =
                                conversions::PublicAddress::try_from(*src_peer_id)
                            {
                                let public_addr: PublicAddress = public_addr.into();
                                if swarm.behaviour().is_subscribed(&message.topic) {
                                    // Send it to ourselves if we subscribed to this topic
                                    if let Ok(message) =
                                        conversions::filter_gossipsub_messages(message, local_public_address)
                                    {
                                        message_handler(public_addr, message.clone())
                                    }                                  
                                }
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NetworkEvent::Identify(identify::Event::Received {
                        peer_id,
                        info,
                    })) => {
                        // Update routing table
                        info.listen_addrs.iter().for_each(|a| {
                            swarm.behaviour_mut().add_address(&peer_id, a.clone());
                        });

                        // subscribe to individual topic of closest replicas when added to Kademlia Kbucket
                        if let Ok(addr) = conversions::PublicAddress::try_from(peer_id) {
                            let public_addr: PublicAddress = addr.into();
                            let topic = Topic::HotStuffRsSend(public_addr);
                            if !swarm.behaviour().is_subscribed(&topic.clone().hash())
                                && is_close_peer(&local_peer_id, &peer_id)
                            {
                                let _ = swarm.behaviour_mut().subscribe(vec![topic]);
                            }
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        swarm.behaviour_mut().remove_peer(&peer_id);

                        // unsubscribe from individual topic of replicas if disconnected
                        if let Ok(addr) = conversions::PublicAddress::try_from(peer_id) {
                            let public_addr: PublicAddress = addr.into();
                            let topic = Topic::HotStuffRsSend(public_addr);
                            if swarm.behaviour().is_subscribed(&topic.clone().hash()) {
                                let _ = swarm.behaviour_mut().unsubscribe(vec![topic]);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    (network_thread_handle, sender)
}

/// Check the distance between 2 peers. Subscribe to new peer's individual topic
/// if the distance is below [MAX_REPLICA_DISTANCE]
fn is_close_peer(peer_1: &PeerId, peer_2: &PeerId) -> bool {
    let peer_1_key = KBucketKey::from(*peer_1);
    let peer_2_key = KBucketKey::from(*peer_2);
    // returns the distance in base2 logarithm ranging from 0 - 256
    let distance = KBucketKey::distance(&peer_1_key, &peer_2_key)
        .ilog2()
        .unwrap_or(0);
    distance < MAX_REPLICA_DISTANCE
}

#[derive(Debug)]
pub enum PeerStartError {
    /// Error building TCP transport
    BuildTransportError(libp2p::noise::Error),

    /// Failed to subscribe to a topic on gossipsub
    SubscriptionError(libp2p::gossipsub::SubscriptionError),

    /// Swarm failed to listen on an unsupported address
    UnsupportedAddressError(libp2p::TransportError<std::io::Error>),
}

