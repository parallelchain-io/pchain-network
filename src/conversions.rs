/*
    Copyright © 2023, ParallelChain Lab 
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Data conversion functions that are used throughout this library.
//! 
//! - ([base64_string]) Base64 URL safe string (RFC4648) conversion without padding (see [base64url]).
//! - ([public_address]) Conversion from an Ed25519 Public Key ([PublicKey]) to public address ([PublicAddress]). 
//! - ([multiaddr]) MultiAddr is IPv4 under TCP.

use std::net::Ipv4Addr;
use libp2p::{identity::PublicKey, Multiaddr};
use pchain_types::cryptography::PublicAddress;

/// Base64 Encoding for arbitrary bytes. This method ensures the string is URL Safe and without padding.
pub fn base64_string<T: AsRef<[u8]>>(bytes: T) -> String {
    base64url::encode(bytes)
}

/// Convert PublicKey in libp2p to PublicAddress in ParallelChain Mainnet. The PublicKey must be an
/// Ed25519 key, otherwise the method returns None.
pub fn public_address(public_key: &PublicKey) -> Option<PublicAddress> {
    match public_key {
        libp2p::identity::PublicKey::Ed25519(kp) => {
            Some(kp.encode())
        },
        _=> None
    }
}

/// Create Multiaddr from IP address and port number. This method ParallelChain Network 
pub fn multiaddr(ip_address: Ipv4Addr, port: u16) -> Multiaddr {
    format!("/ip4/{}/tcp/{}", ip_address, port).parse().unwrap()
}