//! Process-global agent message bus (`IrcBus`) and agent registry
//! (`AgentRegistry`).
//!
//! # IrcBus
//!
//! A singleton message bus that lets agents send messages to each other by
//! ID, broadcast to all peers, or wait for incoming messages. Messages are
//! delivered to the target's mailbox (if it is not listening) or directly to
//! a registered waiter.
//!
//! # AgentRegistry
//!
//! A singleton registry of every live agent in the process. Tracks status
//! (Idle / Running / Parked / Aborted), role, parent, activity description,
//! and timestamps.

use crate::agent::AgentRole;
use crate::ids::AgentId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::LazyLock;
use parking_lot::Mutex;
use tokio::sync::{mpsc, oneshot};

// ─── PeerStatus ────────────────────────────────────────────────────────────

/// Runtime status of an agent in the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeerStatus {
    /// Agent is alive and waiting for work (or between turns).
    Idle,
    /// Agent is actively processing a turn.
    Running,
    /// Agent's session was unloaded but can be revived.
    Parked,
    /// Agent was hard-killed and cannot be revived.
    Aborted,
}

impl std::fmt::Display for PeerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Running => write!(f, "running"),
            Self::Parked => write!(f, "parked"),
            Self::Aborted => write!(f, "aborted"),
        }
    }
}

// ─── AgentRef ──────────────────────────────────────────────────────────────

/// A snapshot of an agent's public metadata in the registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRef {
    pub id: AgentId,
    pub role: AgentRole,
    pub parent_id: Option<AgentId>,
    pub status: PeerStatus,
    /// Short human-readable description of what this agent is doing.
    pub activity: String,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
}

// ─── IrcMessage ────────────────────────────────────────────────────────────

/// A message sent between agents over the IrcBus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IrcMessage {
    /// Sender agent id.
    pub from: AgentId,
    /// Recipient agent id, or `None` for broadcast.
    pub to: Option<AgentId>,
    /// Message body.
    pub content: String,
    /// Unique message id (for dedup / reply tracking).
    pub id: String,
    /// If `true`, the sender is blocking for a reply.
    pub expects_reply: bool,
    /// If this is a reply, the id of the original message.
    pub reply_to: Option<String>,
}

/// Outcome of [`IrcBus::send`].
#[derive(Debug, Clone)]
pub enum DeliveryReceipt {
    /// Delivered to the agent's mailbox or its active receiver.
    Delivered,
    /// Delivered to a waiter that was blocking for this message.
    WaiterDelivered,
    /// Agent is unknown (not registered); message was dropped.
    AgentNotFound,
    /// Broadcast to N peers.
    Broadcast(usize),
}

// ─── Waiter ────────────────────────────────────────────────────────────────

struct Waiter {
    from: Option<AgentId>,
    tx: oneshot::Sender<IrcMessage>,
}

// ─── IrcBus ────────────────────────────────────────────────────────────────

/// Process-global message bus for inter-agent communication.
///
/// Every agent registers its id + a sender half of an mpsc channel. The bus
/// delivers messages directly to the channel when the agent is alive, or to
/// a mailbox when it is not listening. `wait` creates a one-shot waiter that
/// takes priority over the mailbox.
pub struct IrcBus {
    /// Registered agents: id -> sender.
    agents: Mutex<HashMap<String, mpsc::UnboundedSender<IrcMessage>>>,
    /// Mailboxes for agents that are not currently listening.
    mailboxes: Mutex<HashMap<String, Vec<IrcMessage>>>,
    /// Waiters blocking for a specific (or any) sender.
    waiters: Mutex<Vec<Waiter>>,
    /// Monotonic message counter.
    next_id: AtomicU64,
}

impl IrcBus {
    fn new() -> Self {
        Self {
            agents: Mutex::new(HashMap::new()),
            mailboxes: Mutex::new(HashMap::new()),
            waiters: Mutex::new(Vec::new()),
            next_id: AtomicU64::new(1),
        }
    }

    /// Access the process-global singleton.
    pub fn global() -> &'static Self {
        static BUS: LazyLock<IrcBus> = LazyLock::new(IrcBus::new);
        &BUS
    }

    /// Register an agent so it can receive messages. Returns the receiver
    /// half that the agent should poll.
    pub fn register(&self, id: &AgentId) -> mpsc::UnboundedReceiver<IrcMessage> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.agents.lock().insert(id.0.clone(), tx);
        rx
    }

    /// Unregister an agent. Pending mailboxes are cleaned up.
    pub fn unregister(&self, id: &AgentId) {
        let mut agents = self.agents.lock();
        agents.remove(&id.0);
        drop(agents);
        self.mailboxes.lock().remove(&id.0);
    }

    /// Send a message to one agent or broadcast to all.
    ///
    /// Priority: waiters (blocking `wait` calls) → agent channel → mailbox.
    pub fn send(&self, msg: IrcMessage) -> DeliveryReceipt {
        // Try waiters first (highest priority).
        {
            let mut waiters = self.waiters.lock();
            if let Some(pos) = waiters.iter().position(|w| {
                w.from
                    .as_ref()
                    .map(|f| msg.from.0 == f.0)
                    .unwrap_or(true)
            }) {
                let waiter = waiters.remove(pos);
                let _ = waiter.tx.send(msg);
                return DeliveryReceipt::WaiterDelivered;
            }
        }

        if let Some(ref to) = msg.to {
            // Direct message to a specific agent.
            let tx = self.agents.lock().get(&to.0).cloned();
            match tx {
                Some(tx) => {
                    if tx.send(msg).is_err() {
                        // Channel closed — agent died without unregistering.
                        DeliveryReceipt::AgentNotFound
                    } else {
                        DeliveryReceipt::Delivered
                    }
                }
                None => {
                    // Agent not currently registered — put in mailbox.
                    self.mailboxes
                        .lock()
                        .entry(to.0.clone())
                        .or_default()
                        .push(msg);
                    DeliveryReceipt::Delivered
                }
            }
        } else {
            // Broadcast to all registered agents.
            let agents = self.agents.lock().clone();
            let count = agents.len();
            for (_, tx) in agents {
                let _ = tx.send(msg.clone());
            }
            DeliveryReceipt::Broadcast(count)
        }
    }

    /// Register a one-shot waiter. When a message arrives (optionally
    /// filtered by `from`), it is delivered to this channel instead of the
    /// agent's mailbox.
    pub fn register_waiter(&self, from: Option<AgentId>, tx: oneshot::Sender<IrcMessage>) {
        self.waiters.lock().push(Waiter { from, tx });
    }

    /// Drain the mailbox for an agent (non-blocking inbox read).
    pub fn drain_mailbox(&self, id: &AgentId) -> Vec<IrcMessage> {
        self.mailboxes.lock().remove(&id.0).unwrap_or_default()
    }

    /// Generate a unique message id.
    pub fn next_msg_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("irc_msg_{n}")
    }

    /// Check whether a given agent is currently registered (alive).
    pub fn is_registered(&self, id: &AgentId) -> bool {
        self.agents.lock().contains_key(&id.0)
    }
}

// ─── AgentRegistry ─────────────────────────────────────────────────────────

/// Process-global registry of every live agent.
///
/// Agents register on creation and unregister on termination. The registry
/// is the source of truth for `hub list` and for lifecycle management
/// (park/revive).
pub struct AgentRegistry {
    agents: Mutex<HashMap<String, AgentRef>>,
}

impl AgentRegistry {
    fn new() -> Self {
        Self {
            agents: Mutex::new(HashMap::new()),
        }
    }

    /// Access the process-global singleton.
    pub fn global() -> &'static Self {
        static REG: LazyLock<AgentRegistry> = LazyLock::new(AgentRegistry::new);
        &REG
    }

    /// Register an agent.
    pub fn register(&self, r#ref: AgentRef) {
        self.agents
            .lock()
            .insert(r#ref.id.0.clone(), r#ref);
    }

    /// Unregister an agent by id.
    pub fn unregister(&self, id: &AgentId) {
        self.agents.lock().remove(&id.0);
    }

    /// Get a snapshot of one agent.
    pub fn get(&self, id: &AgentId) -> Option<AgentRef> {
        self.agents.lock().get(&id.0).cloned()
    }

    /// List all registered agents.
    pub fn list(&self) -> Vec<AgentRef> {
        self.agents.lock().values().cloned().collect()
    }

    /// Update an agent's status.
    pub fn update_status(&self, id: &AgentId, status: PeerStatus) {
        if let Some(r#ref) = self.agents.lock().get_mut(&id.0) {
            r#ref.status = status;
            r#ref.last_activity = Utc::now();
        }
    }

    /// Update an agent's activity description.
    pub fn update_activity(&self, id: &AgentId, activity: String) {
        if let Some(r#ref) = self.agents.lock().get_mut(&id.0) {
            r#ref.activity = activity;
            r#ref.last_activity = Utc::now();
        }
    }

    /// Count of registered agents.
    pub fn count(&self) -> usize {
        self.agents.lock().len()
    }
}

// ─── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentRole;

    #[test]
    fn test_agent_status_display() {
        assert_eq!(PeerStatus::Idle.to_string(), "idle");
        assert_eq!(PeerStatus::Running.to_string(), "running");
        assert_eq!(PeerStatus::Parked.to_string(), "parked");
        assert_eq!(PeerStatus::Aborted.to_string(), "aborted");
    }

    #[test]
    fn test_agent_registry_register_list() {
        let reg = AgentRegistry::new();
        let id = AgentId::new();
        let r#ref = AgentRef {
            id: id.clone(),
            role: AgentRole::Researcher,
            parent_id: None,
            status: PeerStatus::Idle,
            activity: "researching".to_string(),
            created_at: Utc::now(),
            last_activity: Utc::now(),
        };
        reg.register(r#ref);
        assert_eq!(reg.count(), 1);
        let list = reg.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id.0, id.0);

        reg.unregister(&id);
        assert_eq!(reg.count(), 0);
    }

    #[test]
    fn test_agent_registry_update_status() {
        let reg = AgentRegistry::new();
        let id = AgentId::new();
        reg.register(AgentRef {
            id: id.clone(),
            role: AgentRole::Coordinator,
            parent_id: None,
            status: PeerStatus::Running,
            activity: "planning".to_string(),
            created_at: Utc::now(),
            last_activity: Utc::now(),
        });
        reg.update_status(&id, PeerStatus::Idle);
        assert_eq!(reg.get(&id).unwrap().status, PeerStatus::Idle);
    }

    #[tokio::test]
    async fn test_irc_bus_send_receive() {
        let bus = IrcBus::new();
        let alice = AgentId::new();
        let bob = AgentId::new();

        let mut rx = bus.register(&bob);

        let msg = IrcMessage {
            from: alice.clone(),
            to: Some(bob.clone()),
            content: "hello".to_string(),
            id: bus.next_msg_id(),
            expects_reply: false,
            reply_to: None,
        };

        match bus.send(msg) {
            DeliveryReceipt::Delivered => {}
            other => panic!("expected Delivered, got {other:?}"),
        }

        let received = rx.recv().await.unwrap();
        assert_eq!(received.content, "hello");
        assert_eq!(received.from.0, alice.0);
    }

    #[tokio::test]
    async fn test_irc_bus_wait_delivery() {
        let bus = IrcBus::new();
        let alice = AgentId::new();
        let bob = AgentId::new();

        bus.register(&bob);

        let (tx, rx) = oneshot::channel();
        bus.register_waiter(Some(alice.clone()), tx);

        let msg = IrcMessage {
            from: alice.clone(),
            to: Some(bob.clone()),
            content: "direct to waiter".to_string(),
            id: bus.next_msg_id(),
            expects_reply: false,
            reply_to: None,
        };

        match bus.send(msg) {
            DeliveryReceipt::WaiterDelivered => {}
            other => panic!("expected WaiterDelivered, got {other:?}"),
        }

        let received = rx.await.unwrap();
        assert_eq!(received.content, "direct to waiter");
    }

    #[tokio::test]
    async fn test_irc_bus_mailbox() {
        let bus = IrcBus::new();
        let alice = AgentId::new();
        let bob = AgentId::new();

        // Send to bob before bob registers — goes to mailbox.
        let msg = IrcMessage {
            from: alice.clone(),
            to: Some(bob.clone()),
            content: "mailbox test".to_string(),
            id: bus.next_msg_id(),
            expects_reply: false,
            reply_to: None,
        };
        bus.send(msg);

        // Bob registers and drains mailbox.
        bus.register(&bob);
        let msgs = bus.drain_mailbox(&bob);
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "mailbox test");
    }

    #[tokio::test]
    async fn test_irc_bus_broadcast() {
        let bus = IrcBus::new();
        let alice = AgentId::new();
        let bob = AgentId::new();
        let charlie = AgentId::new();

        let mut rx_bob = bus.register(&bob);
        let mut rx_charlie = bus.register(&charlie);

        let msg = IrcMessage {
            from: alice,
            to: None, // broadcast
            content: "broadcast!".to_string(),
            id: bus.next_msg_id(),
            expects_reply: false,
            reply_to: None,
        };

        match bus.send(msg) {
            DeliveryReceipt::Broadcast(2) => {}
            other => panic!("expected Broadcast(2), got {other:?}"),
        }

        assert_eq!(rx_bob.recv().await.unwrap().content, "broadcast!");
        assert_eq!(rx_charlie.recv().await.unwrap().content, "broadcast!");
    }

    #[test]
    fn test_agent_registry_activity() {
        let reg = AgentRegistry::new();
        let id = AgentId::new();
        reg.register(AgentRef {
            id: id.clone(),
            role: AgentRole::Writer,
            parent_id: None,
            status: PeerStatus::Running,
            activity: "writing report".to_string(),
            created_at: Utc::now(),
            last_activity: Utc::now(),
        });
        reg.update_activity(&id, "proofreading".to_string());
        assert_eq!(reg.get(&id).unwrap().activity, "proofreading");
    }
}