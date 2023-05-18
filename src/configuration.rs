/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Configuration parameters provided by the library user.
//! 
//! Example:
//! 
//! ```no_run
//! let mut config = Config::from_keypair(keypair)
//!     .set_port(12345)
//!     .set_peer_discovery_interval(10);
//! ```

use libp2p::identity::{Keypair, ed25519};
use crate::peer_info::PeerInfo;

/// Configuration that specifies parameters such as network connection, node identification,
/// message buffer sizes and interval.
/// 
/// For convenience, default values are predefined when calling `new()` or `from_keypair()`
/// as following:
/// - Port number: 25519
/// - Buffer size of send commands: 10
/// - Buffer size of message: 10
/// - Buffer size of broadcast messages: 10
/// - Interval for peer discover: 10 secs
#[derive(Clone)]
pub struct Config {
    /// keypair used for identification of this network node
    pub(crate) keypair: Keypair,

    /// port number for TCP connection
    pub port: u16,

    /// bootstrap nodes for initial connection
    pub boot_nodes: Vec<PeerInfo>,

    /// buffer size of commands initiated from caller
    pub send_command_buffer_size: usize,

    /// buffer size of message that is sent to this network node. 
    pub private_msg_buffer_size: usize,

    /// buffer size of broadcast messages.
    pub broadcast_msg_buffer_size: usize,

    /// Interval in seconds for querying networking to discover peers.
    pub peer_discovery_interval: u64,
}

impl Config {
    /// Create config with an automatically generated keypair.
    pub fn new() -> Self {
        Self {
            keypair: Keypair::generate_ed25519(),
            port: 25519,
            boot_nodes: Vec::new(),

            send_command_buffer_size: 8,
            private_msg_buffer_size: 10,
            broadcast_msg_buffer_size: 10,
            peer_discovery_interval: 10, // secs 
        }
    }

    /// Create config with an existing keypair.
    pub fn new_with_keypair(mut keypair_bytes: Vec<u8>) -> Self {
        Self {
            keypair: Keypair::Ed25519(
                ed25519::Keypair::decode(&mut keypair_bytes).unwrap()
            ),
            port: 25519,
            boot_nodes: Vec::new(),

            send_command_buffer_size: 8,
            private_msg_buffer_size: 10,
            broadcast_msg_buffer_size: 10,
            peer_discovery_interval: 10, // secs 
        }
    }

    /// Set the port number used for networking
    pub fn set_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// buffer size of commands initiated from caller
    pub fn set_send_command_buffer_size(mut self, send_command_buffer_size: usize) -> Self {
        self.send_command_buffer_size = send_command_buffer_size;
        self
    }
    
    /// buffer size of message that is sent to this network node. 
    pub fn set_private_msg_buffer_size(mut self, private_msg_buffer_size: usize) -> Self {
        self.private_msg_buffer_size = private_msg_buffer_size;
        self
    }

    /// buffer size of broadcast messages.
    pub fn set_broadcast_msg_buffer_size(mut self, broadcast_msg_buffer_size: usize) -> Self {
        self.broadcast_msg_buffer_size = broadcast_msg_buffer_size;
        self
    }

    /// Interval in seconds for querying networking to discover peers.
    pub fn set_peer_discovery_interval(mut self, peer_discovery_interval: u64) -> Self {
        self.peer_discovery_interval = peer_discovery_interval;
        self
    }

    /// bootstrap nodes for initial connection
    pub fn set_boot_nodes(mut self, boot_nodes: Vec<PeerInfo>) -> Self {
        self.boot_nodes = boot_nodes;
        self
    }
}