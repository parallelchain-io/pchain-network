/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Data conversion functions that are used throughout this library.
//!
//! - ([base64_string]) Base64 URL safe string (RFC4648) conversion without padding (see [base64url]).
//! - ([public_address]) Conversion from an Ed25519 Public Key ([PublicKey]) to public address ([PublicAddress]).
//! - ([multiaddr]) MultiAddr is IPv4 under TCP.

use libp2p::{identity::PublicKey, Multiaddr};
use pchain_types::cryptography::PublicAddress;
use std::net::Ipv4Addr;

/// Base64 Encoding for arbitrary bytes. This method ensures the string is URL Safe and without padding.
pub fn base64_string<T: AsRef<[u8]>>(bytes: T) -> String {
    base64url::encode(bytes)
}

/// Convert PublicKey in libp2p to PublicAddress in ParallelChain Mainnet. The PublicKey must be an
/// Ed25519 key, otherwise the method returns None.
pub fn public_address(public_key: &PublicKey) -> Option<PublicAddress> {
    match public_key.clone().try_into_ed25519() {
        Ok(kp) => Some(kp.to_bytes()),
        _=> None
    }
}

/// Create Multiaddr from IP address and port number. This method ParallelChain Network
pub fn multiaddr(ip_address: Ipv4Addr, port: u16) -> Multiaddr {
    format!("/ip4/{}/tcp/{}", ip_address, port).parse().unwrap()
}

#[cfg(test)]

mod test {
    use super::*;
    use libp2p::identity::Keypair;
    #[test]
    fn test_base64_string() {
        //generate test bytes
        let test_keypair = Keypair::generate_ed25519();
        let test_keypair_bytes = test_keypair.public().to_peer_id().to_bytes();
        //put test bytes through base64_string conversion
        let result = base64_string(&test_keypair_bytes);
        //test len() of result
        assert_eq!(result.len(), 51)
    }

    #[test]
    fn test_public_address_conversion() {
        //generate test keypair
        let test_keypair = Keypair::generate_ed25519();
        //get public keypair
        let test_keypair_public = test_keypair.public();
        //put public keypair public address conversion to get Parallelchain Mainnet Public Address type
        let result = public_address(&test_keypair_public);

        //check result and len() is correct.
        assert_eq!(result.is_some(), true);
        assert_eq!(result.unwrap().len(), 32);
    }

    #[test]
    fn test_create_multiaddress() {
        //generate test IP address and port
        let test_ip = Ipv4Addr::new(127, 0, 0, 1);
        let test_port = 4;

        //test create multiaddress
        let result = multiaddr(test_ip, test_port);
        let expected_result: String = String::from("/ip4/127.0.0.1/tcp/4");

        //check expected result
        assert_eq!(result.to_string(), expected_result);
    }
}
