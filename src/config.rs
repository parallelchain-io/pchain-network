/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Configuration parameters provided by the library user.
//!
//! Example:
//!
//! ```no_run
//! let mut config = Config::with_keypair(keypair)
//!     .set_port(12345)
//!     .set_peer_discovery_interval(10);
//! ```

use std::net::Ipv4Addr;

use crate::constants::{
    BRAODCAST_MSG_BUFFER_SIZE, PEER_DISCOVERY_INTERVAL, PORT_NUMBER, PRIVATE_MSG_BUFFER_SIZE,
    SEND_COMMAND_BUFFER_SIZE, PROTOCOL_NAME
};
use crate::conversions;
use libp2p::identity::{ed25519, Keypair};
use libp2p::PeerId;
use pchain_types::cryptography::PublicAddress;

/// Configuration that specifies parameters such as network connection, node identification,
/// message buffer sizes and interval.
///
/// For convenience, default values are predefined when calling `default()` or `with_keypair()`
/// as following:
/// - Port number: 25519
/// - Buffer size of send commands: 8
/// - Buffer size of message: 10
/// - Buffer size of broadcast messages: 10
/// - Interval for peer discover: 10 secs
/// - Protocol name: "/pchain_p2p/1.0.0"
#[derive(Clone)]
pub struct Config {
    /// keypair used for identification of this network node
    pub(crate) keypair: Keypair,

    /// port number for TCP connection
    pub port: u16,

    /// bootstrap nodes for initial connection
    pub boot_nodes: Vec<Peer>,

    /// buffer size of commands initiated from caller
    pub send_command_buffer_size: usize,

    /// buffer size of message that is sent to this network node
    pub private_msg_buffer_size: usize,

    /// buffer size of broadcast messages
    pub broadcast_msg_buffer_size: usize,

    /// interval in seconds for querying networking to discover peers
    pub peer_discovery_interval: u64,

    /// Protocol name to communicate with other Kademlia nodes
    pub protocol_name: String,
}

/// Create default configuration with an automatically generated keypair.
impl Default for Config {
    fn default() -> Self {
        Self {
            keypair: Keypair::generate_ed25519(),
            port: PORT_NUMBER,
            boot_nodes: Vec::new(),
            send_command_buffer_size: SEND_COMMAND_BUFFER_SIZE,
            private_msg_buffer_size: PRIVATE_MSG_BUFFER_SIZE,
            broadcast_msg_buffer_size: BRAODCAST_MSG_BUFFER_SIZE,
            peer_discovery_interval: PEER_DISCOVERY_INTERVAL, // secs
            protocol_name: PROTOCOL_NAME.to_string(),
        }
    }
}

impl Config {
    /// Create default configuration with an existing keypair.
    pub fn with_keypair(mut keypair_bytes: Vec<u8>) -> Self {
        Config {
            keypair: ed25519::Keypair::try_from_bytes(&mut keypair_bytes)
                .expect("Invalid ed25519 keypair")
                .into(),
            ..Default::default()
        }
    }

    /// Set the port number used for networking.
    pub fn set_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the buffer size of send commands.
    pub fn set_send_command_buffer_size(mut self, send_command_buffer_size: usize) -> Self {
        self.send_command_buffer_size = send_command_buffer_size;
        self
    }

    /// Set the buffer size of messages that are sent to this network peer.
    pub fn set_private_msg_buffer_size(mut self, private_msg_buffer_size: usize) -> Self {
        self.private_msg_buffer_size = private_msg_buffer_size;
        self
    }

    /// Set the buffer size of broadcast messages.
    pub fn set_broadcast_msg_buffer_size(mut self, broadcast_msg_buffer_size: usize) -> Self {
        self.broadcast_msg_buffer_size = broadcast_msg_buffer_size;
        self
    }

    /// Set the interval for querying networking to discover peers in seconds .
    pub fn set_peer_discovery_interval(mut self, peer_discovery_interval: u64) -> Self {
        self.peer_discovery_interval = peer_discovery_interval;
        self
    }

    /// Set the bootstrap nodes for initial connection.
    pub fn set_boot_nodes(mut self, boot_nodes: Vec<Peer>) -> Self {
        self.boot_nodes = boot_nodes;
        self
    }

    /// Set the protocol name for Kademlia communication
    pub fn set_protocol_name(mut self, protocol_name: String) -> Self {
        self.protocol_name = protocol_name;
        self
    }
}

/// [Peer] consists of required information to identify an entity in the network, such as
/// PeerId, IPv4 Address and port number.
#[derive(Clone)]
pub struct Peer {
    /// Peer ID in the P2P network
    pub peer_id: PeerId,

    /// IP address (v4) of connection
    pub ip_address: Ipv4Addr,

    /// Port number of connection
    pub port: u16,
}

impl Peer {
    /// Instantiation of [Peer]. It is used in bootstrap nodes in [crate::configuration::Config].
    ///
    /// ## Panics
    /// Panics if address is not a valid Ed25519 public key.
    pub fn new(address: PublicAddress, ip_address: Ipv4Addr, port: u16) -> Self {
        let peer_id: PeerId = conversions::PublicAddress::new(address)
            .try_into()
            .expect("Invalid PublicKey");
        Self {
            peer_id,
            ip_address,
            port,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use libp2p::identity::ed25519;
    use pchain_types::cryptography::PublicAddress;

    use super::*;
    use crate::conversions;

    #[test]
    fn create_test_config_auto_keypair() {
        let test_config = Config::default();
        //change keypair to PublicAddress type in Parallelchain Mainnet.
        let test_config_public_address =
            conversions::PublicAddress::try_from(test_config.keypair.public());
        assert!(test_config_public_address.is_ok());
        //test new config variables
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

        //create config with existing keypair
        let test_config = Config::with_keypair(existing_keypair.to_bytes().to_vec());

        let test_config_public_address =
            conversions::PublicAddress::try_from(test_config.keypair.public());
        assert!(test_config_public_address.is_ok());
        //test new config variables
        assert_eq!(test_config.boot_nodes.len(), 0);
        assert_eq!(test_config.port, 25519);
        assert_eq!(test_config.send_command_buffer_size, 8);
        assert_eq!(test_config.private_msg_buffer_size, 10);
        assert_eq!(test_config.broadcast_msg_buffer_size, 10);
        assert_eq!(test_config.peer_discovery_interval, 10);
    }

    #[test]
    fn test_set_port() {
        let new_port = 12345;
        let test_config = Config::default().set_port(new_port);
        assert_eq!(test_config.port, new_port);
    }

    #[test]
    fn test_set_send_command_buffer_size() {
        let new_send_command_buffer_size = 10;
        let test_config =
            Config::default().set_send_command_buffer_size(new_send_command_buffer_size);
        assert_eq!(
            test_config.send_command_buffer_size,
            new_send_command_buffer_size
        );
    }

    #[test]
    fn test_set_private_msg_buffer_size() {
        let new_private_msg_buffer_size = 15;
        let test_config =
            Config::default().set_private_msg_buffer_size(new_private_msg_buffer_size);
        assert_eq!(
            test_config.private_msg_buffer_size,
            new_private_msg_buffer_size
        );
    }

    #[test]
    fn test_set_broadcast_msg_buffer_size() {
        let new_broadcast_msg_buffer_size = 15;
        let test_config =
            Config::default().set_broadcast_msg_buffer_size(new_broadcast_msg_buffer_size);
        assert_eq!(
            test_config.broadcast_msg_buffer_size,
            new_broadcast_msg_buffer_size
        );
    }

    #[test]
    fn test_set_peer_discovery_interval() {
        let new_peer_discovery_interval = 15;
        let test_config =
            Config::default().set_peer_discovery_interval(new_peer_discovery_interval);
        assert_eq!(
            test_config.peer_discovery_interval,
            new_peer_discovery_interval
        );
    }

    #[test]
    fn test_set_boot_nodes() {
        let boot_node_public_key = Keypair::generate_ed25519().public();
        let boot_node_public_address: PublicAddress =
            conversions::PublicAddress::try_from(boot_node_public_key)
                .unwrap()
                .into();
        let boot_node_ip_address = Ipv4Addr::new(127, 0, 0, 1);
        let boot_node_port: u16 = 1;

        let boot_node = Peer::new(
            boot_node_public_address,
            boot_node_ip_address,
            boot_node_port,
        );

        let new_boot_nodes = vec![boot_node.clone()];
        let test_config = Config::default().set_boot_nodes(new_boot_nodes);
        assert_eq!(test_config.boot_nodes.len(), 1);
        assert_eq!(test_config.boot_nodes[0].peer_id.clone(), boot_node.peer_id);
    }

    #[test]
    fn test_new_peer_info() {
        //generate keypair
        let test_keypair = Keypair::generate_ed25519();

        //get public keypair and convert to pchain_types::cryptography::PublicAddress
        let test_public_address: PublicAddress =
            conversions::PublicAddress::try_from(test_keypair.public())
                .unwrap()
                .into();

        //get test ip address and port
        let ip_address = Ipv4Addr::new(127, 0, 0, 1);
        let port: u16 = 1;

        //create new instance of Peer Info
        let peer_info = Peer::new(test_public_address, ip_address, port);

        //test new instance of Peer Info
        assert_eq!(&peer_info.ip_address.to_string(), "127.0.0.1");
        assert_eq!(peer_info.peer_id, test_keypair.public().to_peer_id());
        assert_eq!(peer_info.port, 1);
    }

    #[test]
    fn test_new_protocol_name() {
        let new_protocol_name = "/parallelchain_mainnet/v0.5".to_string();
        let test_config =
            Config::default().set_protocol_name(new_protocol_name.clone());
        assert_eq!(
            test_config.protocol_name,
            new_protocol_name
        );
    }
}
