/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! The process starts by loading the network configuration to bring up the P2P network. Then,
//! it spawns an asynchronous [tokio] task and enters the main event loop.
//!
//! In the event loop, it waits for:
//! - [PeerNetworkEvent].
//! - [Commands](EngineCommand) from application for sending message.
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
    behaviour::{PeerBehaviour, PeerNetworkEvent},
    Config,
    constants, conversions,
    message_gate::MessageGateChain,
    messages::{Envelope, Topic},
    network_handle::EngineCommand,
    Peer,
};

/// [start] p2p networking peer and return the handle [NetworkHandle] of this process.
pub(crate) async fn start(
    config: Config,
    subscribe_topics: Vec<Topic>,
    message_gates: MessageGateChain,
) -> Result<Peer, Box<dyn Error>> {
    let local_public_address: PublicAddress =
        conversions::PublicAddress::try_from(config.keypair.public())
            .expect("Invalid PublicKey from configuration")
            .into();
    let local_peer_id = config.keypair.public().to_peer_id();
    let local_keypair = config.keypair;
    log::info!(
        "Local peer id: {:?} {:?}",
        local_peer_id,
        local_peer_id.to_bytes()
    );

    // 1. Instantiate Swarm
    let transport = build_transport(local_keypair.clone()).await?;
    let behaviour = PeerBehaviour::new(
        local_public_address,
        &local_keypair,
        constants::HEARTBEAT_SECS,
        &config.protocol_name,
    );
    
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
    swarm.listen_on(multiaddr(Ipv4Addr::new(0, 0, 0, 0), config.port))?;

    // 2. Connection to bootstrap nodes
    if !config.boot_nodes.is_empty() {
        config.boot_nodes.iter().for_each(|peer_info| {
            swarm.behaviour_mut().add_address(
                &peer_info.peer_id,
                multiaddr(peer_info.ip_address, peer_info.port),
            );
        });
    }

    // 3. Subscribe to Topic
    swarm.behaviour_mut().subscribe(subscribe_topics)?;

    // 4. Start p2p networking
    let (sender, mut receiver) =
        tokio::sync::mpsc::channel::<EngineCommand>(config.send_command_buffer_size);
    let mut discover_tick =
        tokio::time::interval(Duration::from_secs(config.peer_discovery_interval));

    let network_thread_handle = tokio::task::spawn(async move {
        loop {
            // 4.1 Wait for the following events:
            let (engine_command, event) = tokio::select! {
                biased;
                // Receive a (EngineCommand) from application
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
                    EngineCommand::SendTo(recipient, raw_message) => {
                        log::info!("SendTo: {}", base64url::encode(recipient).as_str());
                        if recipient == local_public_address {
                            // send to myself
                            let envelope = Envelope {
                                origin: local_public_address,
                                message: raw_message.into(),
                            };
                            message_gates
                                .message_in(&Topic::Mailbox(local_public_address).hash(), envelope)
                                .await;
                        } else if let Err(e) = swarm.behaviour_mut().send_to(recipient, raw_message)
                        {
                            log::error!("{:?}", e);
                        }
                    }
                    EngineCommand::Broadcast(topic, msg) => {
                        log::info!("Broadcast (Topic: {:?})", topic);
                        if let Err(e) = swarm.behaviour_mut().broadcast(topic.into(), msg) {
                            log::debug!("{:?}", e);
                        }
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
                                    let envelope = Envelope {
                                        origin: public_addr,
                                        message: message.data,
                                    };
                                    message_gates.message_in(&message.topic, envelope).await;
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
    keypair: identity::Keypair,
) -> std::io::Result<Boxed<(PeerId, StreamMuxerBox)>> {
    let transport = {
        let tcp = libp2p::tcp::tokio::Transport::new(tcp::Config::new().nodelay(true));
        TokioDnsConfig::system(tcp)?
    };

    let upgrade =
        libp2p::core::upgrade::SelectUpgrade::new(yamux::Config::default(), MplexConfig::default());

    Ok(transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&keypair).unwrap())
        .multiplex(upgrade)
        .timeout(std::time::Duration::from_secs(20))
        .boxed())
}

/// To build transport layer of the p2p network.
/// Create Multiaddr from IP address and port number.
fn multiaddr(ip_address: Ipv4Addr, port: u16) -> Multiaddr {
    format!("/ip4/{}/tcp/{}", ip_address, port).parse().unwrap()
}

#[cfg(test)]
mod tests {
    use super::build_transport;
    use crate::engine::start;
    use crate::{Config, message_gate::MessageGateChain};

    #[tokio::test]
    async fn network_test_start() {
        //initialize new test config
        let test_config = Config::default();

        //create new test vector
        let test_subscribe_topics = vec![];

        //create new message gate chain
        let test_gates = MessageGateChain::new();

        //start test network and check for error
        let result = start(test_config, test_subscribe_topics, test_gates).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_build_transport() {
        //generate keypair
        let test_keypair = libp2p::identity::Keypair::generate_ed25519();

        //test build transport function
        let result = build_transport(test_keypair).await;

        //test output
        assert!(result.is_ok());
    }
}
