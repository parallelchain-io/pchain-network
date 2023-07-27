/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! [start] a ParallelChain Network peer. This is the entrypoint to to this library.
//!
//! The process starts by loading network configuration to bring up P2P network. Then,
//! it spawns an asynchronous [tokio] task and enters the main event loop.
//!
//! In the event loop, it waits for:
//! - Libp2p events.
//! - [Commands](SendCommand) from application for sending message.
//! - Timeout of a periodic interval to discover peers in the network.
//!
//! ### Events Handling
//!
//! Upon receiving Libp2p Identify event, peer information is authenticated and recorded.
//! This record will be used to determine if received message should be proceed.
//!
//! Upon receiving Libp2p Gossipsub Message event, the message sent from authenticated sender
//! will be passed to the chain of Message Gates ([crate::messages::MessageGate]).
//!
//! Upon receiving commands from application, gossipsub message will be delivered to a
//! Gossipsub topic.

use std::{collections::HashMap, net::Ipv4Addr};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use libp2p::Transport;
use libp2p::dns::TokioDnsConfig;
use libp2p::swarm::SwarmBuilder;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport::Boxed},
    gossipsub::Event,
    identify::Event as IdentifyEvent,
    swarm::SwarmEvent,
    tcp::Config as TcpConfig,
    identity,
    noise,
    yamux,
    PeerId,
};
use libp2p_mplex::MplexConfig;
use futures::StreamExt;
use tokio::sync::Mutex;

use pchain_types::cryptography::PublicAddress;

use crate::messages::NetworkTopic;
use crate::network::Network;
use crate::{
    behaviour::{PeerNetworkBehaviour, PeerNetworkEvent},
    configuration::Config,
    conversions,
    messages::{Envelope, MessageGateChain},
    network::SendCommand,
};

/// start p2p networking and return the handle [Network] of this process.
pub async fn start(
    config: Config,
    subscribe_topics: Vec<NetworkTopic>,
    message_gates: MessageGateChain,
) -> Result<Network, Box<dyn Error>> {
    let local_public_address = conversions::public_address(&config.keypair.public()).unwrap();
    let local_peer_id = config.keypair.public().to_peer_id();
    let local_keypair = config.keypair;
    log::info!(
        "Local peer id: {:?} {:?}",
        local_peer_id,
        local_peer_id.to_bytes()
    );

    // 1. Instantiate Swarm
    let transport = build_transport(local_keypair.clone()).await?;
    let behaviour: PeerNetworkBehaviour =
        PeerNetworkBehaviour::new(local_public_address, &local_keypair, 10);
    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
    
    swarm.listen_on(conversions::multiaddr(
        Ipv4Addr::new(0, 0, 0, 0),
        config.port,
    ))?;

    // 2. Peer Discovery - connection to bootstrap nodes
    if !config.boot_nodes.is_empty() {
        config.boot_nodes.iter().for_each(|peer_info| {
            swarm.behaviour_mut().add_address(
                &peer_info.peer_id,
                conversions::multiaddr(peer_info.ip_address, peer_info.port),
            );
        });
    }

    // 3. Prepare Messaging Protocols
    swarm.behaviour_mut().subscribe(subscribe_topics)?;

    // 4. Start p2p networking
    let peer_lookup: Arc<Mutex<HashMap<PeerId, PublicAddress>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let peer_lookup_in_networking = peer_lookup.clone();
    let (sender, mut receiver) =
        tokio::sync::mpsc::channel::<SendCommand>(config.send_command_buffer_size);
    let mut discover_tick =
        tokio::time::interval(Duration::from_secs(config.peer_discovery_interval));

    let network_thread_handle = tokio::task::spawn(async move {
        loop {
            // 4.1 Wait until an Event comes
            let (send_command, event) = tokio::select! {
                biased;
                // Receive a Libp2p event
                event = swarm.select_next_some() => {
                    (None, Some(event))
                },
                // Receive a command from application
                send_command = receiver.recv() => {
                    (send_command, None)
                },
                // Time for network discovery
                _ = discover_tick.tick() => {
                    // Perform a random walk on DHT
                    swarm.behaviour_mut().random_walk();
                    (None, None)
                },
            };

            // 4.2 Deliver messages when received a Command from application
            if let Some(send_command) = send_command {
                match send_command {
                    SendCommand::SendTo(address, raw_message) => {
                        log::info!("SendTo: {}", conversions::base64_string(address).as_str());
                        if address == local_public_address {
                            // send to me by myself
                            let envelope = Envelope {
                                origin: local_public_address,
                                message: raw_message,
                            };
                            message_gates
                                .message_in(
                                    &NetworkTopic::from(local_public_address).hash(),
                                    envelope,
                                )
                                .await;
                        } else if let Err(e) = swarm.behaviour_mut().send_to(address, raw_message) {
                            log::error!("{:?}", e);
                        }
                    }
                    SendCommand::Broadcast(topic, msg) => {
                        log::info!("Broadcast (Topic: {:?})", topic);
                        if let Err(e) = swarm.behaviour_mut().broadcast(topic.into(), msg) {
                            log::debug!("{:?}", e);
                        }
                    }
                }
            }

            // 4.3 Deliver messages when received a Libp2p Event
            if let Some(event) = event {
                match event {
                    SwarmEvent::Behaviour(
                        PeerNetworkEvent::Gossip(
                            Event::Message { 
                                message,
                                ..
                    })) => {
                        if let Some(src_peer_id) = &message.source {
                            if let Some(public_addr) = peer_lookup_in_networking
                                .clone()
                                .lock()
                                .await
                                .get(src_peer_id)
                            {
                                if swarm.behaviour().is_subscribed(&message) {
                                    let envelope = Envelope {
                                        origin: *public_addr,
                                        message: message.data,
                                    };
                                    message_gates.message_in(&message.topic, envelope).await;
                                } else {
                                    log::debug!("Receive unknown gossip message");
                                }
                            } else {
                                log::debug!("Sender is not In List. {}", src_peer_id);
                            }
                        }
                    }
                    SwarmEvent::Behaviour(PeerNetworkEvent::Identify(
                        IdentifyEvent::Received { peer_id, info },
                    )) => {
                        // update routing table
                        info.listen_addrs.iter().for_each(|a| {
                            swarm.behaviour_mut().add_address(&peer_id, a.clone());
                        });

                        // save info.public_key to map
                        if let Some(identified_address) =
                            conversions::public_address(&info.public_key)
                        {
                            log::debug!(
                                "Identify PeerID: {} Address: {}",
                                peer_id,
                                conversions::base64_string(identified_address).as_str()
                            );
                            peer_lookup_in_networking
                                .clone()
                                .lock()
                                .await
                                .insert(peer_id, identified_address);
                        }
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        log::debug!("ConnectionClosed {}", peer_id);
                        swarm.behaviour_mut().remove_peer(&peer_id);
                        peer_lookup_in_networking
                            .clone()
                            .lock()
                            .await
                            .remove(&peer_id);
                    }
                    _ => {}
                }
            }
        }
    });

    Ok(Network {
        network_thread: network_thread_handle,
        peer_public_addrs: peer_lookup,
        sender,
    })
}

/// To build transport layer of the p2p network.
async fn build_transport(
    keypair: identity::Keypair,
) -> std::io::Result<Boxed<(PeerId, StreamMuxerBox)>> {
    let transport = {
        let tcp = libp2p::tcp::tokio::Transport::new(TcpConfig::new().nodelay(true));
        TokioDnsConfig::system(tcp)?
    };
    
    let upgrade = libp2p::core::upgrade::SelectUpgrade::new(
        yamux::Config::default(),
        MplexConfig::default(),
    );

    Ok(transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::Config::new(&keypair).unwrap())
        .multiplex(upgrade)
        .timeout(std::time::Duration::from_secs(20))
        .boxed())
}

#[cfg(test)]
mod tests {
    use super::build_transport;
    use crate::engine::start;
    use crate::{configuration::Config, messages::MessageGateChain};

    #[tokio::test]
    async fn network_test_start() {
        //initialize new test config
        let test_config = Config::new();

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
