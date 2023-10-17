use std::{net::Ipv4Addr, sync::mpsc, time::Duration};

use borsh::BorshSerialize;
use hotstuff_rs::messages::SyncRequest;
use libp2p::identity::ed25519::{Keypair, self};
use pchain_network::peer::PeerBuilder;
use pchain_network::{
    config::Config,
    messages::{Topic, Message},
    peer::Peer,
};
use pchain_types::{blockchain::TransactionV1, cryptography::PublicAddress};

fn base_tx(signer: PublicAddress) -> TransactionV1 {
    TransactionV1 {
        signer,
        nonce: 0,
        commands: vec![],
        gas_limit: 200000,
        max_base_fee_per_gas: 8,
        priority_fee_per_gas: 0,
        hash: [0u8; 32],
        signature: [0u8; 64],
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
// - Node1: Set Node2 as bootnode, keep broadcasting Mempool topic message
// - Node2: set Node1 as bootnode, listens to subscribed topics
#[tokio::test]
async fn test_broadcast() {
    let keypair_1 = ed25519::Keypair::generate();
    let address_1 = keypair_1.public().to_bytes();

    let keypair_2 = ed25519::Keypair::generate();
    let address_2 = keypair_2.public().to_bytes();

    let (node_1, _message_receiver_1) = node(
        keypair_1, 
        30001, 
        vec![(address_2, Ipv4Addr::new(127, 0, 0, 1), 30002)], 
        vec![]
    ).await;

    let (_node_2, message_receiver_2) = node(
        keypair_2,
        30002,
        vec![(address_1, Ipv4Addr::new(127, 0, 0, 1), 30001)],
        vec![Topic::Mempool]
    ).await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = Message::Mempool(base_tx(address_1));

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.broadcast_mempool_msg(base_tx(address_1));
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node2_received = message_receiver_2.try_recv();
                if node2_received.is_ok() {
                    let (msg_origin, msg) = node2_received.unwrap();
                    let msg_vec: Vec<u8> = msg.into();
                    assert_eq!(msg_vec, Vec::from(message.clone()));
                    assert_eq!(msg_origin, address_1);
                    return
                } 
            }
        }
    }
    panic!("Timeout! Failed to receive message");
}

// - Network: Node1, Node2
// - Node1: set Node2 as bootnode, keep sending message to Node2 only
// - Node2: set Node1 as bootnode, listens to subscribed topics
#[tokio::test]
async fn test_send_to() {
    let keypair_1 = ed25519::Keypair::generate();
    let address_1 = keypair_1.public().to_bytes();

    let keypair_2 = ed25519::Keypair::generate();
    let address_2 = keypair_2.public().to_bytes();

    let (node_1, _message_receiver_1) = node(
        keypair_1, 
        30003, 
        vec![(address_2, Ipv4Addr::new(127, 0, 0, 1), 30004)], 
        vec![]
    ).await;

    let (_node_2, message_receiver_2) = node(
        keypair_2,
        30004,
        vec![(address_1, Ipv4Addr::new(127, 0, 0, 1), 30003)],
        vec![]
    ).await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = create_sync_req(1);

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.send_hotstuff_rs_msg(address_2, message.clone());
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node2_received = message_receiver_2.try_recv();
                if node2_received.is_ok() {
                    let (msg_orgin, msg) = node2_received.unwrap();
                    let msg_vec: Vec<u8> = msg.into();
                    assert_eq!(msg_vec, message.try_to_vec().unwrap());
                    assert_eq!(msg_orgin, address_1);
                    return
                }        
            }
        }
    }
    panic!("Timeout! Failed to receive message");
}

// - Network: Node1, Node2, Node3
// - Node1: set Node2 and Node3 as bootnode, keep sending message to Node2 only
// - Node2: set Node1 and Node3 as bootnode, listens to subscribed topics
// - Node3: set Node1 and Node2 as bootnode, should not process any message
#[tokio::test]
async fn test_send_to_only_specific_receiver() {
    let keypair_1 = ed25519::Keypair::generate();
    let address_1 = keypair_1.public().to_bytes();

    let keypair_2 = ed25519::Keypair::generate();
    let address_2 = keypair_2.public().to_bytes();

    let keypair_3 = ed25519::Keypair::generate();
    let address_3 = keypair_3.public().to_bytes(); 

    let (node_1, _message_receiver_1) = node(
        keypair_1,
        30005, 
        vec![(address_2, Ipv4Addr::new(127, 0, 0, 1), 30006), 
             (address_3, Ipv4Addr::new(127, 0, 0, 1), 30007)],
        vec![]
    ).await;

    let (_node_2, _message_receiver_2) = node(
        keypair_2,
        30006,
        vec![(address_1, Ipv4Addr::new(127, 0, 0, 1), 30005),
             (address_3, Ipv4Addr::new(127, 0, 0, 1), 30007)],
        vec![]
    )
    .await;

    let (_node_3, message_receiver_3) = node(
        keypair_3,
        30007,
        vec![(address_1, Ipv4Addr::new(127, 0, 0, 1), 30005),
             (address_2, Ipv4Addr::new(127, 0, 0, 1), 30006)],
        vec![]
    )
    .await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.send_hotstuff_rs_msg(address_2, create_sync_req(1));

                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node3_received = message_receiver_3.try_recv().is_ok();
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
    let keypair_1 = ed25519::Keypair::generate();
    let address_1 = keypair_1.public().to_bytes();

    let keypair_2 = ed25519::Keypair::generate();
    let address_2 = keypair_2.public().to_bytes();

    let keypair_3 = ed25519::Keypair::generate();
    let address_3 = keypair_3.public().to_bytes(); 

    let (node_1, message_receiver_1) = node(
        keypair_1,
        30008, 
        vec![], 
        vec![]
    ).await;

    let (_node_2, _message_receiver_2) = node(
        keypair_2,
        30009,
        vec![(address_1, Ipv4Addr::new(127, 0, 0, 1), 30008)],
        vec![]
    )
    .await;

    let (node_3, message_receiver_3) = node(
        keypair_3,
        30010,
        vec![(address_2, Ipv4Addr::new(127, 0, 0, 1), 30009)],
        vec![]
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
                node_1.send_hotstuff_rs_msg(address_3, message_to_node3.clone());
                node_3.send_hotstuff_rs_msg(address_1, message_to_node1.clone());

                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node1_received = message_receiver_1.try_recv();
                let node3_received = message_receiver_3.try_recv();
                if node3_received.is_ok() && node1_received.is_ok() {
                    let (node1_message_origin, node1_message) = node1_received.unwrap();
                    let (node3_message_origin, node3_message) = node3_received.unwrap();
                    let node1_message_vec: Vec<u8> = node1_message.into();
                    let node3_message_vec: Vec<u8> = node3_message.into();
                    assert_eq!(node1_message_vec, message_to_node1.try_to_vec().unwrap());
                    assert_eq!(node3_message_vec, message_to_node3.try_to_vec().unwrap());
                    assert_eq!(node1_message_origin, address_3);
                    assert_eq!(node3_message_origin, address_1);
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
    let keypair_1 = ed25519::Keypair::generate();
    let address_1 = keypair_1.public().to_bytes();

    let (node_1, message_receiver_1) = node(
        keypair_1,
        30013, 
        vec![],
        vec![]
    ).await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = create_sync_req(1);

    loop {
        tokio::select! {
            //broadcast does not send to self
            _ = sending_tick.tick() => {
                node_1.send_hotstuff_rs_msg(address_1, message.clone());
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                let node1_received = message_receiver_1.try_recv();
                if node1_received.is_ok() {
                    let (msg_orgin, msg) = node1_received.unwrap();
                    let msg_vec: Vec<u8> = msg.into();
                    assert_eq!(msg_vec, message.try_to_vec().unwrap());
                    assert_eq!(msg_orgin, address_1);
                    return
                }
            }
        }
    }
    panic!("Timeout! Failed to receive message.");
}

// - Network: Node1, Node2
// - Node1: set Node2 as bootnode, keep broadcasting message with topic that is not subscribed by Node2
// - Node2: set Node1 as bootnode, should not receive anything from Node1
#[tokio::test]
async fn test_broadcast_different_topics() {
    let keypair_1 = ed25519::Keypair::generate();
    let address_1 = keypair_1.public().to_bytes();

    let keypair_2 = ed25519::Keypair::generate();
    let address_2 = keypair_2.public().to_bytes();

    let (node_1, _message_receiver_1) = node(
        keypair_1,
        30014, 
        vec![(address_2, Ipv4Addr::new(127, 0, 0, 1), 30015)],
        vec![Topic::Mempool]
    ).await;

    let (_node_2, message_receiver_2) = node(
        keypair_2,
        30015,
        vec![(address_1, Ipv4Addr::new(127, 0, 0, 1), 30014)],
        vec![Topic::HotStuffRsBroadcast],
    ).await;

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.broadcast_mempool_msg(base_tx(address_1));
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                if message_receiver_2.try_recv().is_ok() {
                    panic!("Received messages that are not subscribed");
                }
            }
        }
    }
}

// - Network: Node1, Node2
// - Node1: set Node2 as bootnode, keep sending messages to Node2 only
// - Node2: set Node1 as bootnode, the handle is being dropped, should not receive message
#[tokio::test]
async fn test_stopped_node() {
    let keypair_1 = ed25519::Keypair::generate();
    let address_1 = keypair_1.public().to_bytes();

    let keypair_2 = ed25519::Keypair::generate();
    let address_2 = keypair_2.public().to_bytes();

    let (node_1, _message_receiver_1) = node(
        keypair_1, 
        30016, 
        vec![(address_2, Ipv4Addr::new(127, 0, 0, 1), 30017)], 
        vec![]
    ).await;

    let (node_2, message_receiver_2) = node(
        keypair_2,
        30017,
        vec![(address_1, Ipv4Addr::new(127, 0, 0, 1), 30016)],
        vec![]
    ).await;

    // Stop node by EngineCommand::Shutdown
    drop(node_2);

    let mut sending_limit = 10;
    let mut sending_tick = tokio::time::interval(Duration::from_secs(1));
    let mut receiving_tick = tokio::time::interval(Duration::from_secs(2));

    let message = create_sync_req(1);

    loop {
        tokio::select! {
            _ = sending_tick.tick() => {
                node_1.send_hotstuff_rs_msg(address_2, message.clone());
                if sending_limit == 0 { break }
                sending_limit -= 1;
            }
            _ = receiving_tick.tick() => {
                if message_receiver_2.try_recv().is_ok() {
                    panic!("node 2 should not receive messages!")
                }        
            }
        }
    }
}


pub async fn node(
    keypair: Keypair,
    listening_port: u16,
    boot_nodes: Vec<([u8;32], Ipv4Addr, u16)>,
    topics_to_subscribe: Vec<Topic>
) -> (Peer, std::sync::mpsc::Receiver<(PublicAddress, Message)>) {

    let config = Config {
        keypair,
        topics_to_subscribe,
        listening_port,
        boot_nodes,
        outgoing_msgs_buffer_capacity: 8,
        incoming_msgs_buffer_capacity: 10,
        peer_discovery_interval: 10,
        kademlia_protocol_name: String::from("/pchain_p2p/1.0.0")
    };

    let(tx,rx) = mpsc::channel();

    let message_sender = tx.clone();
    let message_handler = move |msg_origin: [u8;32], msg: Message| {
        let _ = message_sender.send((msg_origin, msg));
    };

    let peer = PeerBuilder::new(config)
    .on_receive_msg(message_handler)
    .build()
    .await
    .unwrap();

    (peer, rx)
}
