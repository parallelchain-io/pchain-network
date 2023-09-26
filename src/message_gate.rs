use async_trait::async_trait;

use crate::messages::{Envelope, MessageTopicHash};

/// [MessageGate] is a message handler to process the message. The type of [Topic]
/// that the gate handles is specified in `topic()`. The message with the right
/// topic hash will be processed.
///
/// Macro `async_trait` has to be added for using this trait, Example:
///
/// ```no_run
/// struct MyGate {}
///
/// #[async_trait]
/// impl MessageGate for MyGate {
///     fn accepted(&self, topic_hash: &MessageTopicHash) -> bool {
///         Topic::Mempool.is(topic_hash)
///     }
///
///     async fn process(&self, envelope: Envelope) {
///         // ... process the envelope
///     }
/// }
/// ```
#[async_trait]
pub trait MessageGate: Send + Sync + 'static {
    /// Check if the message gate accept the incoming topic hash.
    fn accepted(&self, topic_hash: &MessageTopicHash) -> bool;
    /// Process the received message.
    async fn process(&self, envelope: Envelope);
}

/// List of [MessageGate], that consists of message handlers on different topics.
/// Each message handler [MessageGate] implements its own message processing logic.
///
/// ### Example
///
/// To add Gate to the chain:
/// ```no_run
/// let chain = MessageGateChain::new()
///     .append(message_gate_1)
///     .append(message_gate_2);
/// ```
#[derive(Default)]
pub struct MessageGateChain {
    gates: Vec<Box<dyn MessageGate>>,
}

impl MessageGateChain {
    pub fn new() -> Self {
        Self { gates: Vec::new() }
    }

    /// `append` adds a [MessageGate] to the list of handlers
    pub fn append(mut self, gate: impl MessageGate) -> Self {
        self.gates.push(Box::new(gate));
        self
    }

    /// `message_in` takes in the received message with the topic hash, and pick the gate with the same
    /// topic hash to process the message,
    pub(crate) async fn message_in(&self, topic_hash: &MessageTopicHash, envelope: Envelope) {
        if let Some(gate) = self.gates.iter().find(|gate| gate.accepted(topic_hash)) {
            gate.process(envelope).await
        };
    }
}
