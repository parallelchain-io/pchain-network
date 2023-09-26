/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Defines useful constants which are used in pchain_network.
//!
// Default pchain_network configuration
pub const PORT_NUMBER: u16 = 25519;
pub const SEND_COMMAND_BUFFER_SIZE: usize = 8;
pub const PRIVATE_MSG_BUFFER_SIZE: usize = 10;
pub const BRAODCAST_MSG_BUFFER_SIZE: usize = 10;
pub const PEER_DISCOVERY_INTERVAL: u64 = 10;

// Swarm configuration
pub const HEARTBEAT_SECS: u64 = 10;

// PeerBehaviour configuration
pub const PROTOCOL_NAME: &str = "/pchain_p2p/1.0.0";

// Overriding GossipSub configuration
pub const MAX_TRANSMIT_SIZE: usize = 4;
pub const MEGABYTES: usize = 1048576;
