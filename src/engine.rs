/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! The process starts by loading the network configuration to bring up the P2P network. Then,
//! it spawns an asynchronous [tokio] task and enters the main event loop.
//!
//! In the event loop, it waits for:
//! - [PeerNetworkEvent].
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
    behaviour::{Behaviour, PeerNetworkEvent},
    messages::{Envelope, Topic},
    peer::{EngineCommand, PeerBuilder, Peer}, conversions,
};

/// [start] p2p networking peer and return the handle [NetworkHandle] of this process.
pub(crate) async fn start(
    peer: PeerBuilder,
) -> Result<Peer, Box<dyn Error>> {

    let config = peer.config.unwrap(); 

    let local_public_address: PublicAddress = config.keypair.public().to_bytes();
    
    let local_peer_id = identity::Keypair::from(config.keypair).public().to_peer_id();
    let local_keypair = config.keypair;
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
        10,
        &config.kademlia_protocol_name, //TODO jonas
    );
    
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
    swarm.listen_on(multiaddr(Ipv4Addr::new(0, 0, 0, 0), config.listening_port))?;

    // 2. Connection to bootstrap nodes
    if !config.boot_nodes.is_empty() {
        config.boot_nodes.iter().for_each(|peer_info| {
            swarm.behaviour_mut().add_address(
                &peer_info.0,
                peer_info.1,
            );
        });
    }

    // 3. Subscribe to Topic
    //TODO jonas
    swarm.behaviour_mut().subscribe()?;

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
                                message: message.into(),
                            };
                            peer.handlers.into_iter().map(|handler| handler(local_public_address, message));
                            // TODO
                            // message_gates
                            //     .message_in(&Topic::HotStuffRsSend(local_public_address).hash(), envelope)
                            //     .await;
                        } else if let Err(e) = swarm.behaviour_mut().publish(topic.into(), message) {
                            log::debug!("{:?}", e);
                        }
                    }

                    EngineCommand::Shutdown => {
                        log::info!("Exiting out of Engine thread");
                        break
                    }
                }
            }

            // 4.3 Deliver messages when a PeerNetworkEvent is received
            if let Some(event) = event {
                match event {
                    SwarmEvent::Behaviour(PeerNetworkEvent::Gossip(
                        gossipsub::Event::Message { message, .. },
                    )) => {
                        if let Some(src_peer_id) = &message.source {
                            if let Ok(public_addr) =
                                conversions::PublicAddress::try_from(*src_peer_id)
                            {
                                let public_addr: PublicAddress = public_addr.into();
                                if swarm.behaviour().is_subscribed(&message) {
                                    // let envelope = Envelope {
                                    //     origin: public_addr,
                                    //     message: message.data.into(),
                                    // };
                                    // message_gates.message_in(&message.topic, envelope).await;

                                    // TODO jonas
                                    // The problem here is, Envelope need take in Message type, or in general, we need
                                    // to know the type of the Message to pass into different handlers.
                                    // So we need to convert Vec<u8> to pchain_network::Message. Instead of implementing
                                    // TryFrom trait for Vec<u8> to Message, implement a function that takes in the Message Topic to help
                                    // converting Vec<u8> to Message. You can refer to fullnode/mempool messagegate to see how to 
                                    // deserialise each Message type. 
                                } else {
                                    log::debug!("Receive unknown gossip message");
                                }
                            } else {
                                log::debug!("Received message from invalid PeerId.")
                            }
                        }
                    }
                    SwarmEvent::Behaviour(PeerNetworkEvent::Identify(
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

/// To build transport layer of the p2p network.
/// Create Multiaddr from IP address and port number.
fn multiaddr(ip_address: Ipv4Addr, port: u16) -> Multiaddr {
    format!("/ip4/{}/tcp/{}", ip_address, port).parse().unwrap()
}