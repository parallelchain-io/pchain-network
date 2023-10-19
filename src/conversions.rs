/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! This module defines the data conversion functions that are used throughout the pchain-network library.
//!
//! The following are implemented for converting between different types:
//!     - From<[PublicAddress]> for [ParallelChain PublicAddress](pchain_types::cryptography::PublicAddress)
//!     - TryFrom<PeerId> for [PublicAddress]
//!     - TryFrom<[PublicAddress]> for [PeerId]

use libp2p::identity::{self, ed25519, DecodingError, OtherVariantError, PeerId};
use libp2p::Multiaddr;
use std::net::Ipv4Addr;

/// PublicAddress(PublicAddress) is wrapper around [PublicAddress](pchain_types::cryptography::PublicAddress).
pub struct PublicAddress(pchain_types::cryptography::PublicAddress);

impl PublicAddress {
    pub fn new(addr: pchain_types::cryptography::PublicAddress) -> Self {
        PublicAddress(addr)
    }
}

impl From<PublicAddress> for pchain_types::cryptography::PublicAddress {
    fn from(peer: PublicAddress) -> pchain_types::cryptography::PublicAddress {
        peer.0
    }
}

impl TryFrom<PeerId> for PublicAddress {
    type Error = ConversionError;

    fn try_from(peer_id: PeerId) -> Result<Self, Self::Error> {
        let kp = identity::PublicKey::try_decode_protobuf(&peer_id.to_bytes())?;
        Ok(PublicAddress(kp.try_into_ed25519().unwrap().to_bytes()))
    }
}

impl TryFrom<PublicAddress> for PeerId {
    type Error = ConversionError;

    fn try_from(public_addr: PublicAddress) -> Result<Self, Self::Error> {
        let kp = ed25519::PublicKey::try_from_bytes(&public_addr.0)?;
        let public_key: identity::PublicKey = kp.into();
        Ok(public_key.to_peer_id())
    }
}

#[derive(Debug)]
pub enum ConversionError {
    OtherVariantError(OtherVariantError),
    DecodingError(DecodingError),
}

impl From<OtherVariantError> for ConversionError {
    fn from(error: OtherVariantError) -> ConversionError {
        ConversionError::OtherVariantError(error)
    }
}

impl From<DecodingError> for ConversionError {
    fn from(error: DecodingError) -> ConversionError {
        ConversionError::DecodingError(error)
    }
}

/// Convert ip address [std::net::Ipv4Addr] and port [u16] into MultiAddr [libp2p::Multiaddr] type
pub fn multi_addr(ip_address: Ipv4Addr, port: u16) -> Multiaddr {
    format!("/ip4/{}/tcp/{}", ip_address, port).parse().unwrap()
}

#[cfg(test)]

mod test {
    use super::*;
    use identity::Keypair;

    #[test]
    fn test_peer_id_and_public_address_conversion() {
        // Generate ed25519 keypair and obtain the corresponding PeerId.
        let test_keypair = Keypair::generate_ed25519();
        let test_peerid = test_keypair.public().to_peer_id();

        // Convert to pchain_types::cryptography::PublicAddress
        let result = PublicAddress::try_from(test_peerid);
        assert!(result.is_ok());

        // Convert it back to PeerId
        let result: Result<PeerId, ConversionError> = result.unwrap().try_into();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_peerid);
    }
}
