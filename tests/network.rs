use std::{net::Ipv4Addr, sync::Arc, time::Duration};

use async_trait::async_trait;
use borsh::BorshSerialize;
use futures::lock::Mutex;
use hotstuff_rs::messages::SyncRequest;
use libp2p::{identity::{Keypair, PublicKey}, gossipsub::TopicHash};
use pchain_network::{
    config::{Config, Peer},
    message_gate::{MessageGate, MessageGateChain},
    messages::{Envelope, Message, Topic},
    NetworkHandle,
};
use pchain_types::{blockchain::TransactionV2, cryptography::PublicAddress};

fn base_tx(signer: PublicAddress) -> TransactionV2 {
    TransactionV2 {
        signer,
        nonce: 0,
        commands: vec![],
        gas_limit: 200000,
        max_base_fee_per_gas: 8,
        priority_fee_per_gas: 0,
        signature: [0u8; 64],
        hash: [0u8; 32],
    }
}

fn create_sync_req(start_height: u64) -> hotstuff_rs::messages::Message {
    let test_message = hotstuff_rs::messages::SyncMessage::SyncRequest(SyncRequest {
        start_height,
        limit: 5,
    });
    hotstuff_rs::messages::Message::SyncMessage(test_message)
}

// - Network: Node1, Node2
// - Node1: keep broadcasting Mempool topic message
// - Node2: set Node1 as bootnode, listens to subscribed topics
#[tokio::test]
async fn test_broadcast() {
    let (address_1, node_1, _) = node(30001, vec![], None, vec![]).await;
    let (_address_2, _node_2, receiver_gate) = node(
        30002,
        vec![Peer::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30001)],
        Some(Topic::Mempool),
        vec![Topic::Mempool],
    )
    .await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = Message::Mempool(base_tx(address_1));

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.broadcast_mempool_tx_msg(&base_tx(address_1));
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                if receiver_gate.received().await {
                    assert_eq!(receiver_gate.get_message().await, Vec::from(message));
                    assert_eq!(receiver_gate.get_origin().await, address_1);
                    return;
                }
            }
        }
    }
    panic!("Timeout! Failed to receive message");
}

// - Network: Node1, Node2
// - Node1: keep sending message to Node2 only
// - Node2: set Node1 as bootnode, listens to subscribed topics
#[tokio::test]
async fn test_send_to() {
    let (address_1, node_1, _) = node(30003, vec![], None, vec![]).await;

    let (address_2, _node_2, receiver_gate) = node(
        30004,
        vec![Peer::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30003)],
        None,
        vec![],
    )
    .await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = create_sync_req(1);

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.send_to(address_2, message.clone());
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node2_received = receiver_gate.received().await;
                if node2_received  {
                    assert_eq!(receiver_gate.get_message().await, message.try_to_vec().unwrap());
                    assert_eq!(receiver_gate.get_origin().await, address_1);
                    return;
                }
            }
        }
    }
    panic!("Timeout! Failed to receive message");
}

// - Network: Node1, Node2, Node3
// - Node1: keep sending message of Node2 Mailbox topic
// - Node2: set Node1 as bootnode, receives message from Node 1
// - Node3: set Node2 as bootnode, should subscribe to Node2's mailbox topic and receive corresponding message
#[tokio::test]
async fn test_send_to_only_specific_receiver() {
    let (address_1, node_1, _) = node(30005, vec![], None, vec![]).await;

    let (address_2, _node_2, receiver_gate_2) = node(
        30006,
        vec![Peer::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30005)],
        None,
        vec![],
    )
    .await;

    let (_address_3, _node_3, receiver_gate_3) = node(
        30007,
        vec![Peer::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30005)],
        None,
        vec![],
    )
    .await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = create_sync_req(1);

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.send_to(address_2, message.clone());

                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node3_received = receiver_gate_3.received().await;
                let node2_received = receiver_gate_2.received().await;
                if node3_received && node2_received{
                    assert_eq!(receiver_gate_3.get_message().await, message.try_to_vec().unwrap());
                    assert_eq!(receiver_gate_3.get_origin().await, address_1);
                    return
                }
            }
        }
    }
}

// - Network: Node1, Node2, Node3
// - Node1: keep sending message to Node3 only
// - Node2: set Node1 as bootnode, listens to subscribed topics
// - Node3: set Node2 as bootnode, keep sending message to Node1 only
// - Node1 and Node3 should receive message from each other
#[tokio::test]
async fn test_sparse_messaging() {
    let (address_1, node_1, receiver_gate_1) = node(30008, vec![], None, vec![]).await;

    let (address_2, _node_2, _) = node(
        30009,
        vec![Peer::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30008)],
        None,
        vec![],
    )
    .await;

    let (address_3, node_3, receiver_gate_3) = node(
        30010,
        vec![Peer::new(address_2, Ipv4Addr::new(127, 0, 0, 1), 30009)],
        None,
        vec![],
    )
    .await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message_to_node3 = create_sync_req(1);
    let message_to_node1 = create_sync_req(2);

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.send_to(address_3, message_to_node3.clone());
                node_3.send_to(address_1, message_to_node1.clone());

                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node1_received = receiver_gate_1.received().await;
                let node3_received = receiver_gate_3.received().await;
                if node3_received && node1_received {
                    assert_eq!(receiver_gate_1.get_message().await, message_to_node1.try_to_vec().unwrap());
                    assert_eq!(receiver_gate_3.get_message().await, message_to_node3.try_to_vec().unwrap());
                    assert_eq!(receiver_gate_1.get_origin().await, address_3);
                    assert_eq!(receiver_gate_3.get_origin().await, address_1);
                    return;
                }
            }
        }
    }
    panic!("Timeout! Failed to receive message");
}

// - Network: Node1
// - Node1: keep sending message to itself only
#[tokio::test]
async fn test_send_to_self() {
    let (address_1, node_1, receiver_gate) = node(30013, vec![], None, vec![]).await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = create_sync_req(1);

    loop {
        tokio::select! {
            //broadcast does not send to self
            _ = sending_tick.tick() => {
                node_1.send_to(address_1, message.clone());
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node1_received = receiver_gate.received().await;
                if node1_received {
                    assert_eq!(receiver_gate.get_message().await, message.try_to_vec().unwrap());
                    assert_eq!(receiver_gate.get_origin().await, address_1);
                    return
                }
            }
        }
    }
    panic!("Timeout! Failed to receive message.");
}

// - Network: Node1, Node2
// - Node1: keep broadcasting messages whose topic is not subscribed by Node2
// - Node2: set Node1 as bootnode, should not receive anything from Node1
#[tokio::test]
async fn test_broadcast_different_topics() {
    let (address_1, node_1, _) = node(30014, vec![], None, vec![]).await;

    let (_address_2, _node_2, receiver_gate) = node(
        30015,
        vec![Peer::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30014)],
        Some(Topic::Mempool),
        vec![Topic::HotstuffRS],
    )
    .await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.broadcast_mempool_tx_msg(&base_tx(address_1));
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                if receiver_gate.received().await {
                    panic!("Received messages that are not subscribed");
                }
            }
        }
    }
}

pub async fn node(
    port: u16,
    boot_nodes: Vec<Peer>,
    gate_topic: Option<Topic>,
    subscribe_topics: Vec<Topic>,
) -> (PublicAddress, NetworkHandle, MessageCounts) {
    let keypair: Keypair = Keypair::generate_ed25519();
    let address = public_address(&keypair.public());
    let config = Config::with_keypair(
        keypair
            .try_into_ed25519()
            .unwrap()
            .to_bytes()
            .try_into()
            .unwrap(),
    )
    .set_port(port)
    .set_boot_nodes(boot_nodes);

    let gate = if !subscribe_topics.is_empty() {
        MessageCounts::new(gate_topic.unwrap())
    } else {
        MessageCounts::new(Topic::Mailbox(address))
    };
    let message_chain = MessageGateChain::new().append(gate.clone());

    let node = pchain_network::NetworkHandle::start(config, subscribe_topics, message_chain).await;

    (address, node, gate)
}

pub fn public_address(public_key: &PublicKey) -> PublicAddress {
    let kp = public_key.clone().try_into_ed25519().unwrap();
    kp.to_bytes()
}

#[derive(Clone)]
pub struct MessageCounts {
    topic: Topic,
    /// number of calls to proceed()
    count_proceed: Arc<Mutex<usize>>,

    /// actual message received
    message_received: Arc<Mutex<Vec<u8>>>,

    /// source of message
    origin: Arc<Mutex<PublicAddress>>,
}

impl MessageCounts {
    fn new(topic: Topic) -> Self {
        Self {
            topic,
            count_proceed: Arc::new(Mutex::new(usize::default())),
            message_received: Arc::new(Mutex::new(Vec::default())),
            origin: Arc::new(Mutex::new(PublicAddress::default())),
        }
    }

    async fn received(&self) -> bool {
        *self.count_proceed.lock().await > 0
    }

    async fn get_message(&self) -> Vec<u8> {
        self.message_received.lock().await.to_vec()
    }

    async fn get_origin(&self) -> PublicAddress {
        self.origin.lock().await.to_owned()
    }
}

#[async_trait]
impl MessageGate for MessageCounts {
    fn accepted(&self, topic_hash: &TopicHash) -> bool {
        self.topic.clone().is(topic_hash)
    }
    async fn process(&self, envelope: Envelope) {
        *self.count_proceed.lock().await += 1;
        *self.message_received.lock().await = envelope.message;
        *self.origin.lock().await = envelope.origin;
    }
}
