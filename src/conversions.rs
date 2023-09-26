/*
    Copyright © 2023, ParallelChain Lab
    Licensed under the Apache License, Version 2.0: http://www.apache.org/licenses/LICENSE-2.0
*/

//! Data conversion functions that are used throughout this library.
//!
//! The following are implemented for converting between different types:
//!     - From<[PublicAddress]> for [ParallelChain PublicAddress](pchain_types::cryptography::PublicAddress)
//!     - TryFrom<identity::PublicKey> for [PublicAddress]
//!     - TryFrom<identity::PeerId> for [PublicAddress]
//!     - TryFrom<[PublicAddress]> for [identity::PeerId]

use libp2p::{
    identity::{self, ed25519, DecodingError, OtherVariantError},
    PeerId,
};

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

impl TryFrom<identity::PublicKey> for PublicAddress {
    type Error = ConversionError;

    fn try_from(public_key: identity::PublicKey) -> Result<Self, ConversionError> {
        let kp = public_key.clone().try_into_ed25519()?;
        Ok(PublicAddress(kp.to_bytes()))
    }
}

impl TryFrom<identity::PeerId> for PublicAddress {
    type Error = ConversionError;

    fn try_from(peer_id: PeerId) -> Result<Self, Self::Error> {
        let kp = identity::PublicKey::try_decode_protobuf(&peer_id.to_bytes())?;
        PublicAddress::try_from(kp)
    }
}

impl TryFrom<PublicAddress> for identity::PeerId {
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

#[cfg(test)]

mod test {
    use super::*;
    use libp2p::identity::{Keypair, PublicKey};

    #[test]
    fn test_public_key_and_public_address_conversion() {
        // Generate ed25529 keypair and obtain its PublicKey.
        let test_publickey = Keypair::generate_ed25519().public();

        // Convert to pchain_types::cryptography::PublicAddress
        let result = PublicAddress::try_from(test_publickey.clone());
        assert!(result.is_ok());
        let public_addr: pchain_types::cryptography::PublicAddress = result.unwrap().into();

        // Convert it back to PublicKey
        let result = ed25519::PublicKey::try_from_bytes(&public_addr);
        assert!(result.is_ok());
        let public_key: PublicKey = result.unwrap().into();
        assert_eq!(test_publickey, public_key);
    }

    #[test]
    fn test_peer_id_and_public_address_conversion() {
        // Generate ed25519 keypair and obtain its PeerID.
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
