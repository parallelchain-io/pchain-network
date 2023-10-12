/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! The process starts by loading the network configuration to bring up the P2P network. Then,
//! it spawns an asynchronous [tokio] task and enters the main event loop.
//!
//! In the event loop, it waits for:
//! - [NetworkEvent].
//! - [Commands](SendCommand) from application for sending message.
//! - Timeout of a periodic interval to discover peers in the network.
//!
//! ### Events Handling
//!
//! Upon receiving the Libp2p Identify event, peer information is added to the routing table.
//!
//! Upon receiving the Libp2p Gossipsub Message event, the message received will be passed to
//! the chain of message handlers ([crate::messages::MessageGate]).
//!
//! Upon receiving the Libp2p ConnectionClosed event, peer information will be removed from the
//! routing table.
//!
//! Upon receiving commands from application, gossipsub message will be delivered to a
//! Gossipsub topic.

use borsh::BorshDeserialize;
use futures::StreamExt;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport::Boxed},
    dns::TokioDnsConfig,
    gossipsub, identify, identity, noise,
    swarm::{SwarmBuilder, SwarmEvent},
    tcp, yamux, Multiaddr, PeerId, Transport,
};
use libp2p_mplex::MplexConfig;
use pchain_types::cryptography::PublicAddress;
use std::error::Error;
use std::net::Ipv4Addr;
use std::time::Duration;

use crate::{
    config,
    conversions,
    behaviour::{Behaviour, NetworkEvent},
    messages::{DroppedTxnMessage, Envelope, Topic, Message, self}, 
    messages::Topic::{DroppedTxns, HotStuffRsBroadcast, HotStuffRsSend, Mempool},
    peer::{EngineCommand, PeerBuilder, Peer},
};

/// [start] p2p networking peer and return the handle [NetworkHandle] of this process.
pub(crate) async fn start(
    peer: PeerBuilder,
) -> Result<Peer, Box<dyn Error>> {

    let config = peer.config.unwrap(); 
    let local_keypair = config.keypair;

    let local_public_address: PublicAddress = local_keypair.public().to_bytes();
    
    let local_peer_id = identity::Keypair::from(local_keypair.clone()).public().to_peer_id();
    log::info!(
        "Local peer id: {:?} {:?}",
        local_peer_id,
        local_peer_id.to_bytes()
    );

    // 1. Instantiate Swarm
    let transport = build_transport(local_keypair.clone()).await?;
    let behaviour = Behaviour::new(
        local_public_address,
        &local_keypair,
        config.kademlia_protocol_name
    );
    
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
    swarm.listen_on(multiaddr(Ipv4Addr::new(0, 0, 0, 0), config.listening_port))?;

    // 2. Connection to bootstrap nodes
    if !config.boot_nodes.is_empty() {
        config.boot_nodes.iter().for_each(|peer_info| {
            swarm.behaviour_mut().add_address(
                &peer_info.0,
                peer_info.1.clone(),
            );
        });
    }

    // 3. Subscribe to Topic
    swarm.behaviour_mut().subscribe(config.topics_to_subscribe.clone())?;

    // 4. Start p2p networking
    let (sender, mut receiver) =
        tokio::sync::mpsc::channel::<EngineCommand>(config.outgoing_msgs_buffer_capacity);
    let mut discover_tick =
        tokio::time::interval(Duration::from_secs(config.peer_discovery_interval));

    let network_thread_handle = tokio::task::spawn(async move {
        loop {
            // 4.1 Wait for the following events:
            let (engine_command, event) = tokio::select! {
                biased;
                // Receive a SendCommand from application
                engine_command = receiver.recv() => {
                    (engine_command, None)
                },
                // Receive a PeerNetworkEvent
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

            // 4.2 Deliver messages when a EngineCommand from application is received
            if let Some(engine_command) = engine_command {
                match engine_command {
                    EngineCommand::Publish(topic, message) => {
                        log::info!("Publish (Topic: {:?})", topic);
                        if topic == Topic::HotStuffRsSend(local_public_address) {
                            // send to myself
                            let envelope = Envelope {
                                origin: local_public_address,
                                message: message.clone().into(),
                            };
                            let _ = peer.handlers.iter().map(|handler| handler(local_public_address, message.clone()));
                        } else if let Err(e) = swarm.behaviour_mut().publish(topic.into(), message) {
                            log::debug!("{:?}", e);
                        }
                    }

                    EngineCommand::Shutdown => {
                        log::info!("Shutting down the engine...");
                        break
                    }
                }
            }

            // 4.3 Deliver messages when a PeerNetworkEvent is received
            if let Some(event) = event {
                match event {
                    SwarmEvent::Behaviour(NetworkEvent::Gossip(
                        gossipsub::Event::Message { message, .. },
                    )) => {
                        if let Some(src_peer_id) = &message.source {
                            if let Ok(public_addr) =
                                conversions::PublicAddress::try_from(*src_peer_id)
                            {
                                let public_addr: PublicAddress = public_addr.into();
                                if swarm.behaviour().is_subscribed(&message) {
                                    // Send it to ourselves if we subscribed to this topic  
                                    let topic = identify_topic(message.topic, public_addr.clone());
                                    
                                    match topic {
                                        Some(t) => {
                                            let message = deserialize_message(message.data, t).unwrap();
                                            let _ = peer.handlers.iter().map(|handler| handler(local_public_address, message.clone()));
                                        }
                                        None => continue,
                                    }
                                } else {
                                    log::debug!("Received unknown gossip message");
                                }
                            } else {
                                log::debug!("Received message from invalid PeerId.");
                            }
                        }
                    }
                    SwarmEvent::Behaviour(NetworkEvent::Identify(
                        identify::Event::Received { peer_id, info },
                    )) => {
                        // update routing table
                        info.listen_addrs.iter().for_each(|a| {
                            swarm.behaviour_mut().add_address(&peer_id, a.clone());
                        });
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        log::debug!("ConnectionClosed {}", peer_id);
                        swarm.behaviour_mut().remove_peer(&peer_id);
                    }
                    _ => {}
                }
            }
        }
    });

    Ok(Peer {
        engine: network_thread_handle,
        to_engine: sender
    })
}

async fn build_transport(
    keypair: identity::ed25519::Keypair,
) -> std::io::Result<Boxed<(PeerId, StreamMuxerBox)>> {
    let transport = {
        let tcp = libp2p::tcp::tokio::Transport::new(tcp::Config::new().nodelay(true));
        TokioDnsConfig::system(tcp)?
    };

    let upgrade =
        libp2p::core::upgrade::SelectUpgrade::new(yamux::Config::default(), MplexConfig::default());

    Ok(transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&keypair.into()).unwrap())
        .multiplex(upgrade)
        .timeout(std::time::Duration::from_secs(20))
        .boxed())
}

fn identify_topic(hash: gossipsub::TopicHash, public_addr: PublicAddress) -> Option<Topic>{
    config::fullnode_topics(public_addr)
    .into_iter()
    .find(|t| t.clone().hash() == hash)
}

fn deserialize_message(data: Vec<u8>, topic: Topic) -> Result<Message, std::io::Error> {
    let mut data = data.as_slice();

    match topic {
        HotStuffRsBroadcast => {
            hotstuff_rs::messages::Message::deserialize(&mut data)
            .map(Message::HotStuffRs)            
        },
        HotStuffRsSend(address) => {
            hotstuff_rs::messages::Message::deserialize(&mut data)
            .map(Message::HotStuffRs)
        },
        Mempool => {
            pchain_types::blockchain::TransactionV1::deserialize(&mut data)
            .map(Message::Mempool)
        },
        DroppedTxns => {
            DroppedTxnMessage::deserialize(&mut data)
            .map(Message::DroppedTxns)
        },
    }
} 


/// To build transport layer of the p2p network.
/// Create Multiaddr from IP address and port number.
fn multiaddr(ip_address: Ipv4Addr, port: u16) -> Multiaddr {
    format!("/ip4/{}/tcp/{}", ip_address, port).parse().unwrap()
}