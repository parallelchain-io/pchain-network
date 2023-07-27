/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Information which identifies a peer ([PeerInfo]).

use std::net::Ipv4Addr;

use libp2p::{PeerId, identity::{ed25519, PublicKey}};
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
        let ed25519_pk: PublicKey = ed25519::PublicKey::try_from_bytes(&address).unwrap().into();
        Self {
            peer_id: ed25519_pk.to_peer_id(),
            ip_address,
            port,
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::conversions;
    use libp2p::identity::Keypair;
    use std::net::Ipv4Addr;

    #[test]
    fn test_new_peer_info() {
        //generate random keypair
        let test_keypair = Keypair::generate_ed25519();

        //get public keypair and convert to bytes
        let test_public_key_bytes = conversions::public_address(&test_keypair.public()).unwrap();

        //get test ip address and port
        let test_ip_address = Ipv4Addr::new(127, 0, 0, 1);
        let test_port: u16 = 1;

        //create new instance of Peer Info
        let test_peer_info = PeerInfo::new(test_public_key_bytes, test_ip_address, test_port);

        //test new instance of Peer Info
        //testing length of ip address: 127.0.0.1
        assert_eq!(test_peer_info.ip_address.to_string().len(), 9);
        //testing length of peer id
        assert_eq!(test_peer_info.peer_id.to_string().len(), 52);
        //test port
        assert_eq!(test_peer_info.port, 1);
    }
}
