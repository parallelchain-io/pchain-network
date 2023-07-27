use std::{net::Ipv4Addr, sync::Arc, time::Duration};

use async_trait::async_trait;
use futures::lock::Mutex;
use libp2p::identity::{Keypair, PublicKey};
use pchain_network::{
    configuration::Config,
    messages::{Envelope, MessageGate, MessageGateChain, NetworkTopic, NetworkTopicHash},
    network::Network,
    peer_info::PeerInfo,
};
use pchain_types::cryptography::PublicAddress;


// - Network: Node1, Node2
// - Node1: keep broadcasting message
// - Node2: set Node1 as bootnode, listens to subscribed topics
#[tokio::test]
async fn test_broadcast() {
    let message_chain_1 = MessageGateChain::new();
    let (address_1, node_1) = node(30001, vec![], message_chain_1).await;
    let sender_1 = node_1.sender();

    let message_receiver_2 = MessageCounts::default();
    let message_chain_2 = MessageGateChain::new().chain(message_receiver_2.clone());
    let (_address_2, _node_2) = node(
        30002,
        vec![PeerInfo::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30001)],
        message_chain_2,
    )
    .await;

    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut sending_limit = 10;
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = vec![1,2];

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                let _ = sender_1.send(pchain_network::network::SendCommand::Broadcast(NetworkTopic::new("topic".to_string()), message.clone())).await;
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                if message_receiver_2.received().await {
                    let message_received = message_receiver_2.get_message().await;
                    assert_eq!(message_received, message);
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
    let message_chain_1 = MessageGateChain::new();
    let (address_1, node_1) = node(30003, vec![], message_chain_1).await;
    let sender_1 = node_1.sender();

    let message_receiver_2 = MessageCounts::default();
    let message_chain_2 = MessageGateChain::new().chain(message_receiver_2.clone());
    let (address_2, _node_2) = node(
        30004,
        vec![PeerInfo::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30003)],
        message_chain_2,
    )
    .await;

    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut sending_limit = 10;
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = vec![1,2];

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                let _ = sender_1.send(pchain_network::network::SendCommand::SendTo(address_2, message.clone())).await;
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node2_received = message_receiver_2.received().await;
                let received_message = message_receiver_2.get_message().await;
                if node2_received  {
                    assert_eq!(received_message, message);
                    return;
                }
            }
        }
    }
    panic!("Timeout! Failed to receive message");
}


// - Network: Node1, Node2, Node3
// - Node1: keep sending message to Node2 only
// - Node2: set Node1 as bootnode, listens to subscribed topics
// - Node3: set Node1 as bootnode, should not receive any message
#[tokio::test]
async fn test_send_to_only_specific_receiver() {
    let message_chain_1 = MessageGateChain::new();
    let (address_1, node_1) = node(30005, vec![], message_chain_1).await;
    let sender_1 = node_1.sender();

    let message_receiver_2 = MessageCounts::default();
    let message_chain_2 = MessageGateChain::new().chain(message_receiver_2.clone());
    let (address_2, _node_2) = node(
        30006,
        vec![PeerInfo::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30005)],
        message_chain_2,
    )
    .await;

    let message_receiver_3 = MessageCounts::default();
    let message_chain_3 = MessageGateChain::new().chain(message_receiver_3.clone());
    let (_address_3, _node_3) = node(
        30007,
        vec![PeerInfo::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30005)],
        message_chain_3,
    )
    .await;

    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut sending_limit = 10;
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                let _ = sender_1.send(pchain_network::network::SendCommand::SendTo(address_2, Vec::new())).await;
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node3_received = message_receiver_3.received().await;
                if node3_received {
                    panic!("Wrong recipient");
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
    let message_receiver_1 = MessageCounts::default();
    let message_chain_1 = MessageGateChain::new().chain(message_receiver_1.clone());
    let (address_1, node_1) = node(30008, vec![], message_chain_1).await;
    let sender_1 = node_1.sender();

    let message_receiver_2 = MessageCounts::default();
    let message_chain_2 = MessageGateChain::new().chain(message_receiver_2.clone());
    let (address_2, _node_2) = node(
        30009,
        vec![PeerInfo::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30008)],
        message_chain_2,
    )
    .await;

    let message_receiver_3 = MessageCounts::default();
    let message_chain_3 = MessageGateChain::new().chain(message_receiver_3.clone());
    let (address_3, node_3) = node(
        30010,
        vec![PeerInfo::new(address_2, Ipv4Addr::new(127, 0, 0, 1), 30009)],
        message_chain_3,
    )
    .await;
    let sender_3 = node_3.sender();

    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut sending_limit = 10;
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message1 = vec![1,2];
    let message2 = vec![3,4];

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                let _ = sender_1.send(pchain_network::network::SendCommand::SendTo(address_3, message1.clone())).await;
                let _ = sender_3.send(pchain_network::network::SendCommand::SendTo(address_1, message2.clone())).await;
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node1_received = message_receiver_1.received().await;
                let node3_received = message_receiver_3.received().await;
                let received_by_node1 = message_receiver_1.get_message().await;
                let received_by_node3 = message_receiver_3.get_message().await;
                if node3_received && node1_received {
                    assert_eq!(received_by_node1, message2);
                    assert_eq!(received_by_node3, message1);
                    return;
                }
            }
        }
    }
    panic!("Timeout! Failed to receive message");
}


// - Network: Node1, Node2
// - Node1: stopped, should not receive message from Node2
// - Node2: set Node1 as bootnode, keep sending message to Node1
#[tokio::test]
async fn test_stopped_node() {
    let message_receiver_1 = MessageCounts::default();
    let message_chain_1 = MessageGateChain::new().chain(message_receiver_1.clone());
    let (address_1, node_1) = node(30011, vec![], message_chain_1).await;
    let _stopped_node_1 = node_1.stop().await;

    let message_receiver_2 = MessageCounts::default();
    let message_chain_2 = MessageGateChain::new().chain(message_receiver_2.clone());
    let (_address_2, node_2) = node(
        30012,
        vec![PeerInfo::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30011)],
        message_chain_2,
    )
    .await;
    let sender_2 = node_2.sender();

    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut sending_limit = 10;
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                let _ = sender_2.send(pchain_network::network::SendCommand::SendTo(address_1, Vec::new())).await;
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node1_received = message_receiver_1.received().await;
                if node1_received {
                    panic!("node 1 not stopped!");
                }
            }
        }
    }
}


// - Network: Node1
// - Node1: keep sending message itself only
#[tokio::test]
async fn test_send_to_self() {
    let message_receiver_1 = MessageCounts::default();
    let message_chain_1 = MessageGateChain::new().chain(message_receiver_1.clone());
    let (address_1, node_1) = node(30013, vec![], message_chain_1).await;
    let sender_1 = node_1.sender();

    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut sending_limit = 10;
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = vec![1,2];

    loop {
        tokio::select! {
            //broadcast does not send to self
            _ = sending_tick.tick() => {
                let _ = sender_1.send(pchain_network::network::SendCommand::SendTo(address_1, message.clone())).await;
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node1_received = message_receiver_1.received().await;
                let received_message = message_receiver_1.get_message().await;
                if node1_received {
                    assert_eq!(received_message, message);
                    return
                }
            }
        }
    }
    panic!("Timeout! Failed to receive message.");
}


// - Network: Node1, Node2
// - Node1: keep broadcast topic that is not subscribed by Node2
// - Node2: set Node1 as bootnode, should not receive anything from Node1
#[tokio::test]
async fn test_broadcast_different_topics() {
    let message_chain_1 = MessageGateChain::new();
    let (address_1, node_1) = node(30014, vec![], message_chain_1).await;
    let sender_1 = node_1.sender();

    let message_receiver_2 = MessageCounts::default();
    let message_chain_2 = MessageGateChain::new().chain(message_receiver_2.clone());
    let (_address_2, _node_2) = node(
        30015,
        vec![PeerInfo::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30014)],
        message_chain_2,
    )
    .await;

    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut sending_limit = 10;
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = vec![1,2];

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                let _ = sender_1.send(pchain_network::network::SendCommand::Broadcast(NetworkTopic::new("another_topic".to_string()), message.clone())).await;
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                if message_receiver_2.received().await {
                    panic!("Received toipcs that are not subscribed");
                }
            }
        }
    }
}


// - Network: Node1, Node2
// - Node1: keep broadcast topic that is not subscribed by Node2
// - Node2: set Node1 as bootnode, should not receive anything from Node1
#[tokio::test]
async fn test_list_addresses() {
    let message_chain_1 = MessageGateChain::new();
    let (address_1, node_1) = node(30016, vec![], message_chain_1).await;
    let _sender_1 = node_1.sender();

    let message_receiver_2 = MessageCounts::default();
    let message_chain_2 = MessageGateChain::new().chain(message_receiver_2.clone());
    let (address_2, _node_2) = node(
        30017,
        vec![PeerInfo::new(address_1, Ipv4Addr::new(127, 0, 0, 1), 30016)],
        message_chain_2,
    )
    .await;

    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    let node_1_peers = node_1.list_addresses().await;
    assert!(node_1_peers.contains(&address_2));
}

pub async fn node(
    port: u16,
    boot_nodes: Vec<PeerInfo>,
    message_chain: MessageGateChain,
) -> (PublicAddress, Network) {
    let keypair: Keypair = Keypair::generate_ed25519();
    let address = public_address(&keypair.public()).unwrap();
    let config = Config::new_with_keypair(
        keypair
            .try_into_ed25519()
            .unwrap()
            .to_bytes()
            .try_into()
            .unwrap(),
    )
    .set_port(port)
    .set_boot_nodes(boot_nodes);
    let node = pchain_network::engine::start(
        config,
        vec![NetworkTopic::new("topic".to_string())],
        message_chain,
    )
    .await
    .unwrap();

    (address, node)
}

pub fn public_address(public_key: &PublicKey) -> Option<PublicAddress> {
    public_key
        .clone()
        .try_into_ed25519()
        .map(|kp| kp.to_bytes())
        .ok()
}

#[derive(Default, Clone)]
struct MessageCounts {
    /// number of calls to can_proceed()
    count_can_proceed: Arc<Mutex<usize>>,

    /// number of calls to proceed()
    count_proceed: Arc<Mutex<usize>>,

    /// actual message received
    message_received: Arc<Mutex<Vec<u8>>>,
}

impl MessageCounts {
    async fn counts(&self) -> (usize, usize) {
        let c1 = *self.count_can_proceed.lock().await;
        let c2 = *self.count_proceed.lock().await;
        (c1, c2)
    }

    async fn received(&self) -> bool {
        let (c1, c2) = self.counts().await;
        c1 > 0 && c2 > 0
    }

    async fn get_message(&self) -> Vec<u8> {
        self.message_received.lock().await.to_vec()
    }
}

#[async_trait]
impl MessageGate for MessageCounts {
    /// check if the message type can be accepted to be proceed
    async fn can_proceed(&self, _topic_hash: &NetworkTopicHash) -> bool {
        *self.count_can_proceed.lock().await += 1;
        true
    }

    /// proceed the message and return true if the chain should be terminated
    async fn proceed(&self, envelope: Envelope) -> bool {
        *self.count_proceed.lock().await += 1;
        *self.message_received.lock().await = envelope.message;
        true
    }
}
