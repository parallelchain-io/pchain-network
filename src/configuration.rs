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

use crate::peer_info::PeerInfo;
use libp2p::identity::{Keypair, ed25519};

/// Configuration that specifies parameters such as network connection, node identification,
/// message buffer sizes and interval.
///
/// For convenience, default values are predefined when calling `new()` or `from_keypair()`
/// as following:
/// - Port number: 25519
/// - Buffer size of send commands: 8
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
            keypair: ed25519::Keypair::try_from_bytes(&mut keypair_bytes).expect("Invalid ed25519 keypair").into(),
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

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use libp2p::identity::ed25519;

    use super::*;
    use crate::conversions;

    #[test]
    fn create_test_config_auto_keypair() {
        let test_config = Config::new();
        //change keypair to PublicAddress type in Parallelchain Mainnet.
        let test_config_public_address = conversions::public_address(&test_config.keypair.public());
        //test new config variables
        assert!(test_config_public_address.is_some());
        assert_eq!(test_config_public_address.unwrap().len(), 32);
        assert_eq!(test_config.boot_nodes.len(), 0);
        assert_eq!(test_config.port, 25519);
        assert_eq!(test_config.send_command_buffer_size, 8);
        assert_eq!(test_config.private_msg_buffer_size, 10);
        assert_eq!(test_config.broadcast_msg_buffer_size, 10);
        assert_eq!(test_config.peer_discovery_interval, 10);
    }

    #[test]
    fn create_test_config_existing_keypair() {
        //generate keypair
        let existing_keypair = ed25519::Keypair::generate();

        //encode keypair
        let encoded_existing_keypair = existing_keypair.to_bytes();

        //create config with existing keypair
        let result = Config::new_with_keypair(encoded_existing_keypair.to_vec());

        //change keypair to PublicAddress type in Parallelchain Mainnet.
        let test_config_public_address = conversions::public_address(&result.keypair.public());

        //test new config variables
        assert!(test_config_public_address.is_some());
        assert_eq!(test_config_public_address.unwrap().len(), 32);
        assert_eq!(result.boot_nodes.len(), 0);
        assert_eq!(result.port, 25519);
        assert_eq!(result.send_command_buffer_size, 8);
        assert_eq!(result.private_msg_buffer_size, 10);
        assert_eq!(result.broadcast_msg_buffer_size, 10);
        assert_eq!(result.peer_discovery_interval, 10);
    }

    #[test]
    fn test_set_port() {
        let new_port = 12345;
        let test_config = Config::set_port(Config::new(), new_port);
        assert_eq!(test_config.port, new_port);
    }

    #[test]
    fn test_set_send_command_buffer_size() {
        let new_send_command_buffer_size = 10;
        let test_config =
            Config::set_send_command_buffer_size(Config::new(), new_send_command_buffer_size);
        assert_eq!(
            test_config.send_command_buffer_size,
            new_send_command_buffer_size
        );
    }

    #[test]
    fn test_set_private_msg_buffer_size() {
        let new_private_msg_buffer_size = 15;
        let test_config =
            Config::set_private_msg_buffer_size(Config::new(), new_private_msg_buffer_size);
        assert_eq!(
            test_config.private_msg_buffer_size,
            new_private_msg_buffer_size
        );
    }

    #[test]
    fn test_set_broadcast_msg_buffer_size() {
        let new_broadcast_msg_buffer_size = 15;
        let test_config =
            Config::set_broadcast_msg_buffer_size(Config::new(), new_broadcast_msg_buffer_size);
        assert_eq!(
            test_config.broadcast_msg_buffer_size,
            new_broadcast_msg_buffer_size
        );
    }

    #[test]
    fn test_set_peer_discovery_interval() {
        let new_peer_discovery_interval = 15;
        let test_config =
            Config::set_peer_discovery_interval(Config::new(), new_peer_discovery_interval);
        assert_eq!(
            test_config.peer_discovery_interval,
            new_peer_discovery_interval
        );
    }

    #[test]
    fn test_set_boot_nodes() {
        let boot_node_public_key = Keypair::generate_ed25519().public();
        let boot_node_public_key_bytes = conversions::public_address(&boot_node_public_key).unwrap();
        let boot_node_ip_address = Ipv4Addr::new(127, 0, 0, 1);
        let boot_node_port: u16 = 1;

        let boot_node = PeerInfo::new(boot_node_public_key_bytes, boot_node_ip_address, boot_node_port);
        
        let new_boot_nodes = vec![boot_node.clone()];
        let test_config = Config::set_boot_nodes(Config::new(), new_boot_nodes);
        assert_eq!(test_config.boot_nodes.len(), 1);
        assert_eq!(test_config.boot_nodes[0].peer_id.clone(), boot_node.peer_id);
    }
}
