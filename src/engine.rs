/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! The process starts by loading the network configuration to bring up the P2P network. Then,
//! it spawns an asynchronous [tokio] task and enters the main event loop.
//!
//! In the event loop, it waits for:
//! 1. [NetworkEvent]
//! 2. [Commands](EngineCommand) from application for sending messages or termination
//! 3. a periodic interval to discover peers in the network
//!
//! ### 1. NetworkEvent Handling
//!
//! Upon receiving the Identify event, the information of the new peer will be added to the
//! routing table.
//!
//! Upon receiving the Gossip event, the message will be deserailized to (Message)[crate::messages::Message]
//! if the peer is subscribed to the topic. The message will then be passed in the message handlers defined
//! by the user.
//!
//! Upon receiving the ConnectionClosed event, peer information will be removed from the
//! routing table.
//!
//! ### 2. EngineCommand Handling
//!
//! Upon receiving a (Publish)[EngineCommand::Publish] command, the peer will publish the message to its
//! connected peers. The peer will process the message directly if it is a [Topic::HotStuffRsSend] message
//! directed to the peer's public address.
//!
//! Upon receiving a (Shutdown)[EngineCommand::Shutdown] command, the process will exit the loop and terminate
//! the thread.
//!

use futures::StreamExt;
use tokio::task::JoinHandle;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport::Boxed},
    dns::TokioDnsConfig,
    gossipsub,
    identify, identity, noise,
    swarm::{SwarmBuilder, SwarmEvent},
    tcp, yamux, PeerId, Transport,
};
use libp2p_mplex::MplexConfig;
use pchain_types::cryptography::PublicAddress;
use std::net::Ipv4Addr;
use std::time::Duration;

use crate::{
    behaviour::{Behaviour, NetworkEvent},
    conversions,
    messages::{Message, Topic},
    peer::EngineCommand,
    config::Config,
};


/// [start] p2p networking peer and return the handle of this process.
pub(crate) async fn start(config: Config, message_handlers: Vec<Box<dyn Fn(PublicAddress, Message) + Send>>) 
    -> Result<(JoinHandle<()>,tokio::sync::mpsc::Sender<EngineCommand>), EngineStartError> {
    let local_keypair = config.keypair;
    let local_public_address: PublicAddress = local_keypair.public().to_bytes();
    let local_peer_id = identity::Keypair::from(local_keypair.clone())
        .public()
        .to_peer_id();

    log::info!("Local PeerId: {:?}", local_peer_id);

    // 1. Instantiate Swarm
    let transport = build_transport(local_keypair.clone()).await?;
    let behaviour = Behaviour::new(
        local_public_address,
        &local_keypair,
        config.kademlia_protocol_name,
    );

    let mut swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, local_peer_id).build();
    let multiaddr = conversions::multi_addr(
        Ipv4Addr::new(0, 0, 0, 0),
        config.listening_port
    );
    swarm.listen_on(multiaddr)?;

    // 2. Connection to bootstrap nodes
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

    // 3. Subscribe to Topic
    swarm
        .behaviour_mut()
        .subscribe(config.topics_to_subscribe.clone())?;

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
                // Receive an EngineCommand from application
                engine_command = receiver.recv() => {
                    (engine_command, None)
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

            // 4.2 Deliver messages when an EngineCommand::Publish from the application is received
            // and shutdown engine when an EngineCommand::Shutdown from the application is received
            if let Some(engine_command) = engine_command {
                match engine_command {
                    EngineCommand::Publish(topic, message) => {
                        log::info!("Publishing (Topic: {:?})", topic);
                        if topic == Topic::HotStuffRsSend(local_public_address) {
                            // send to myself
                            message_handlers.iter()
                            .for_each(|handler| handler(local_public_address, message.clone()));
                        } 
                        else if let Err(e) = swarm.behaviour_mut().publish(topic, message) {
                            log::debug!("Failed to publish the message. {:?}", e);
                        }
                    }
                    EngineCommand::Shutdown => {
                        log::info!("Shutting down the engine...");
                        break;
                    }
                }
            }

            // 4.3 Deliver messages when a NetworkEvent is received
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
                                if swarm.behaviour().is_subscribed(&message) {
                                    // Send it to ourselves if we subscribed to this topic
                                    if let Ok(pchain_message) =
                                        Message::try_from((message, local_public_address))
                                    {
                                        message_handlers.iter().for_each(|handler| {
                                            handler(public_addr, pchain_message.clone())
                                        });
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
                    }
                    SwarmEvent::ConnectionClosed { peer_id, .. } => {
                        swarm.behaviour_mut().remove_peer(&peer_id);
                    }
                    _ => {}
                }
            }
        }
    });

    Ok(
        (network_thread_handle, sender)
    )
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

#[derive(Debug)]
pub enum EngineStartError {
    /// Failed to read from system configuration path
    SystemConfigError(std::io::Error),

    /// Failed to subscribe to a topic on gossipsub
    SubscriptionError(libp2p::gossipsub::SubscriptionError),

    /// Swarm failed to listen on an unsupported address
    UnsupportedAddressError(libp2p::TransportError<std::io::Error>),
}

impl From<std::io::Error> for EngineStartError {
    fn from(error: std::io::Error) -> EngineStartError {
        EngineStartError::SystemConfigError(error)
    }
}

impl From<libp2p::gossipsub::SubscriptionError> for EngineStartError {
    fn from(error: libp2p::gossipsub::SubscriptionError) -> EngineStartError {
        EngineStartError::SubscriptionError(error)
    }
}

impl From<libp2p::TransportError<std::io::Error>> for EngineStartError {
    fn from(error: libp2p::TransportError<std::io::Error>) -> EngineStartError {
        EngineStartError::UnsupportedAddressError(error)
    }
}
