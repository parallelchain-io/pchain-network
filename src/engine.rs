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

use borsh::BorshDeserialize;
use futures::StreamExt;
use libp2p::{
    core::{muxing::StreamMuxerBox, transport::Boxed},
    dns::TokioDnsConfig,
    gossipsub::{self, TopicHash},
    Multiaddr,
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
    config, conversions,
    messages::Topic::{DroppedTxns, HotStuffRsBroadcast, HotStuffRsSend, Mempool},
    messages::{DroppedTxnMessage, Message, Topic},
    peer::{EngineCommand, EngineError, Peer, PeerBuilder},
};

/// [start] p2p networking peer and return the handle [Peer] of this process.
pub(crate) async fn start(peer: PeerBuilder) -> Result<Peer, EngineError> {
    let config = peer.config;

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
    let multiaddr = format!(
        "/ip4/{}/tcp/{}",
        Ipv4Addr::new(0, 0, 0, 0),
        config.listening_port
    )
    .parse()
    .unwrap();
    swarm.listen_on(multiaddr)?;

    // 2. Connection to bootstrap nodes
    if !config.boot_nodes.is_empty() {
        config.boot_nodes.iter().for_each(|peer_info| {
            let multiaddr = multi_addr(peer_info.1, peer_info.2);
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
                // Receive a EngineCommand from application
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

            // 4.2 Deliver messages when a EngineCommand from an application is received
            if let Some(engine_command) = engine_command {
                match engine_command {
                    EngineCommand::Publish(topic, message) => {
                        log::info!("Publishing (Topic: {:?})", topic);
                        if topic == Topic::HotStuffRsSend(local_public_address) {
                            // send to myself
                            peer.handlers.iter()
                            .for_each(|handler| handler(local_public_address, message.clone()));
                        } 
                        else if let Err(e) = swarm.behaviour_mut().publish(topic, message) {
                            log::debug!("Failed to pulish the message. {:?}", e);
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

                                    if let Some(topic) = identify_topic(message.topic, local_public_address)
                                    {
                                        if let Ok(message) =
                                            deserialize_message(message.data, topic)
                                        {
                                            let _ = peer.handlers.iter().for_each(|handler| {
                                                handler(public_addr, message.clone())
                                            });
                                        }
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

    Ok(Peer {
        engine: network_thread_handle,
        to_engine: sender,
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

/// Identify the [crate::messages::Topic] of the message
fn identify_topic(topic_hash: TopicHash, local_public_address: PublicAddress) -> Option<Topic> {
    //address for fullnode_topics should be the local peer address
    config::fullnode_topics(local_public_address)
        .into_iter()
        .find(|t| t.clone().hash() == topic_hash)
}

/// Deserialize [libp2p::gossipsub::Message] into [crate::messages::Message]
fn deserialize_message(data: Vec<u8>, topic: Topic) -> Result<Message, std::io::Error> {
    let mut data = data.as_slice();

    match topic {
        HotStuffRsBroadcast | HotStuffRsSend(_) => {
            hotstuff_rs::messages::Message::deserialize(&mut data).map(Message::HotStuffRs)
        }
        Mempool => {
            pchain_types::blockchain::TransactionV1::deserialize(&mut data).map(Message::Mempool)
        }
        DroppedTxns => DroppedTxnMessage::deserialize(&mut data).map(Message::DroppedTxns),
    }
}

/// Convert [std::net::Ipv4Addr] and port [u16] into MultiAddr[libp2p::Multiaddr] type
fn multi_addr(ip_address: Ipv4Addr, port: u16) -> Multiaddr {
    format!("/ip4/{}/tcp/{}", ip_address, port).parse().unwrap()
}
