/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Information which identifies a peer ([PeerInfo]).

use std::net::Ipv4Addr;

use libp2p::PeerId;
use pchain_types::cryptography::PublicAddress;

/// PeerInfo consists of required information to identify an entity in the network, such as 
/// Peer Id, IPv4 Address and port number.
#[derive(Clone)]
pub struct PeerInfo {
    /// Peer id in the p2p network
    pub peer_id: PeerId,

    /// IP address (v4) of connection
    pub ip_address: Ipv4Addr,

    /// port number of connection
    pub port: u16,
}

impl PeerInfo {

    /// Instantiation of PeerInfo. It is used in bootstrap nodes in [crate::configuration::Config].
    /// 
    /// ## Panics 
    /// Panics if address is not a valid Ed25519 public key.
    pub fn new(address: PublicAddress, ip_address: Ipv4Addr, port: u16) -> Self {
        let ed25519_pk = libp2p::identity::ed25519::PublicKey::decode(&address).unwrap();
        Self {
            peer_id: PeerId::from_public_key(&libp2p::identity::PublicKey::Ed25519(ed25519_pk)),
            ip_address,
            port
        }
    }
}