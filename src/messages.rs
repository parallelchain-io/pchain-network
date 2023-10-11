
use borsh::BorshSerialize;
use libp2p::gossipsub::IdentTopic;
use pchain_types::{blockchain::TransactionV1, cryptography::{Sha256Hash, PublicAddress}, serialization::Serializable};

/// Hash of the message topic.
pub type MessageTopicHash = libp2p::gossipsub::TopicHash;

#[derive(PartialEq, Debug, Clone)]
pub enum Topic {
    HotStuffRsBroadcast,
    HotStuffRsSend(PublicAddress),
    Mempool,
    DroppedTxns,
}

impl Topic {
    pub fn hash(self) -> MessageTopicHash {
        IdentTopic::from(self).hash()
    }
}

impl From<Topic> for IdentTopic {
    fn from(topic: Topic) -> Self {
        let str = match topic {
            Topic::HotStuffRsBroadcast => "consensus".to_string(),
            Topic::Mempool => "mempool".to_string(),
            Topic::DroppedTxns => "droppedTx".to_string(),
            Topic::HotStuffRsSend(addr) => base64url::encode(addr),
        };
        IdentTopic::new(str)
    }
}

#[derive(Clone)]
pub enum Message {
    Consensus(hotstuff_rs::messages::Message),
    Mempool(TransactionV1),
    DroppedTx(DroppedTxMessage),
}

impl From<Message> for Vec<u8> {
    fn from(msg: Message) -> Self {
        match msg {
            Message::Consensus(msg) => msg.try_to_vec().unwrap(),
            Message::Mempool(txn) => Serializable::serialize(&txn),
            Message::DroppedTx(msg) => msg.try_to_vec().unwrap(),
        }
    }
}

/// [Envelope] encapsulates the message received from the p2p network with it's sender address.
#[derive(Clone)]
pub struct Envelope {
    /// The origin of the message
    pub origin: PublicAddress,

    /// The message encapsulated
    pub message: Message,
}

pub struct MempoolMessage(TransactionV1);

// [DroppedTxMessage] defines data content for Message::DroppedTx.
#[derive(Clone, borsh::BorshSerialize, borsh::BorshDeserialize)]
pub enum DroppedTxMessage {
    MempoolDroppedTx {
        txn: TransactionV1,
        status_code: DroppedTxStatusCode,
    },
    ExecutorDroppedTx {
        tx_hash: Sha256Hash,
        status_code: DroppedTxStatusCode,
    },
}

#[derive(Clone)]
pub enum DroppedTxStatusCode {
    Invalid,
    NonceTooLow,
    NonceInaccessible,
}

impl From<&DroppedTxStatusCode> for u16 {
    fn from(status_code: &DroppedTxStatusCode) -> Self {
        match status_code {
            DroppedTxStatusCode::Invalid => 0x515_u16,
            DroppedTxStatusCode::NonceTooLow => 0x516_u16,
            DroppedTxStatusCode::NonceInaccessible => 0x517_u16,
        }
    }
}

impl borsh::BorshSerialize for DroppedTxStatusCode {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        let status_code: u16 = self.into();
        status_code.serialize(writer)
    }
}

impl borsh::BorshDeserialize for DroppedTxStatusCode {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let status_code = match u16::deserialize_reader(reader) {
            Ok(0x515_u16) => DroppedTxStatusCode::Invalid,
            Ok(0x516_u16) => DroppedTxStatusCode::NonceTooLow,
            Ok(0x517_u16) => DroppedTxStatusCode::NonceInaccessible,
            _ => panic!("Invalid droppedTx status code."),
        };
        Ok(status_code)
    }
}