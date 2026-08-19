//! Operator control plane: mid-run questions and approvals.
//!
//! Agents interact with a human operator through two request channels
//! (wired by the host — TUI, HTTP server, or nothing in headless runs):
//!
//! - **questions** — the `question` tool blocks until the operator answers
//!   (or a timeout returns an "operator unavailable" notice);
//! - **approvals** — tools listed in `[agent] approval_tools` block until
//!   the operator allows or denies the call (timeout falls back to
//!   `[agent] approval_fallback`).
//!
//! Every request carries a oneshot reply channel; dropping the receiver is
//! treated as "operator went away" and handled by the caller's fallback.

use pr_core::AgentId;
use tokio::sync::{mpsc, oneshot};

/// A question an agent asks the operator mid-run.
pub struct QuestionRequest {
    pub agent_id: AgentId,
    /// Correlation id surfaced in events / HTTP endpoints.
    pub request_id: String,
    pub question: String,
    pub reply: oneshot::Sender<String>,
}

/// An approval request for a side-effect tool.
pub struct ApprovalRequest {
    pub agent_id: AgentId,
    pub request_id: String,
    pub tool: String,
    /// Short human-readable preview of the call arguments.
    pub args_preview: String,
    /// true = allow, false = deny.
    pub reply: oneshot::Sender<bool>,
}

pub type QuestionTx = mpsc::UnboundedSender<QuestionRequest>;
pub type ApprovalTx = mpsc::UnboundedSender<ApprovalRequest>;

/// How an approval gate resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalVerdict {
    Allowed,
    Denied,
}
