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
use borsh::BorshDeserialize;

use crate::messages::{
    DroppedTxnMessage, 
    Message,
    Topic::{HotStuffRsBroadcast,HotStuffRsSend,Mempool,DroppedTxns}
};
use crate::config::fullnode_topics;


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
    type Error = PublicAddressTryFromPeerIdError;

    fn try_from(peer_id: PeerId) -> Result<Self, Self::Error> {
        let kp = identity::PublicKey::try_decode_protobuf(&peer_id.to_bytes())?;
        let ed25519_key = kp.try_into_ed25519()?;
        Ok(PublicAddress(ed25519_key.to_bytes()))
    }
}

impl TryFrom<PublicAddress> for PeerId {
    type Error = DecodingError;

    fn try_from(public_addr: PublicAddress) -> Result<Self, Self::Error> {
        let kp = ed25519::PublicKey::try_from_bytes(&public_addr.0)?;
        let public_key: identity::PublicKey = kp.into();
        Ok(public_key.to_peer_id())
    }
}

#[derive(Debug)]
pub enum PublicAddressTryFromPeerIdError {
    OtherVariantError(OtherVariantError),
    DecodingError(DecodingError),
}

impl From<OtherVariantError> for PublicAddressTryFromPeerIdError {
    fn from(error: OtherVariantError) -> PublicAddressTryFromPeerIdError {
        PublicAddressTryFromPeerIdError::OtherVariantError(error)
    }
}

impl From<DecodingError> for PublicAddressTryFromPeerIdError {
    fn from(error: DecodingError) -> PublicAddressTryFromPeerIdError {
        PublicAddressTryFromPeerIdError::DecodingError(error)
    }
}

impl TryFrom<(libp2p::gossipsub::Message, pchain_types::cryptography::PublicAddress)> for Message {
    type Error = MessageConversionError;

    fn try_from((message , local_public_address): (libp2p::gossipsub::Message, pchain_types::cryptography::PublicAddress)) 
    -> Result<Self, Self::Error> {
        let (topic_hash, data) = (message.topic, message.data);
        let mut data = data.as_slice();
        
        let topic = fullnode_topics(local_public_address)
            .into_iter()
            .find(|t| t.clone().hash() == topic_hash)
            .ok_or(InvalidTopicError)?;
        
        match topic {
            HotStuffRsBroadcast | HotStuffRsSend(_) => {
                let message = hotstuff_rs::messages::Message::deserialize(&mut data).map(Message::HotStuffRs)?;
                Ok(message)
            },
            Mempool => {
                let message = pchain_types::blockchain::TransactionV1::deserialize(&mut data).map(Message::Mempool)?;
                Ok(message)
            },
            DroppedTxns => {
                let message = DroppedTxnMessage::deserialize(&mut data).map(Message::DroppedTxns)?;
                Ok(message)
            }
        }
    }
}

#[derive(Debug)]
pub struct InvalidTopicError;

#[derive(Debug)]
pub enum MessageConversionError {
    DeserializeError(std::io::Error),
    InvalidTopicError(InvalidTopicError),
}

impl From<InvalidTopicError> for MessageConversionError {
    fn from(error: InvalidTopicError) -> MessageConversionError {
        MessageConversionError::InvalidTopicError(error)
    }
}

impl From<std::io::Error> for MessageConversionError {
    fn from(error: std::io::Error) -> MessageConversionError {
        MessageConversionError::DeserializeError(error)
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
        let result: Result<PeerId, DecodingError> = result.unwrap().try_into();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_peerid);
    }
}
