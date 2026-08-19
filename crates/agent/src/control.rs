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

#[cfg(test)]
mod tests {
    use super::*;
    use pr_core::AgentId;
    use tokio::sync::oneshot;

    // -----------------------------------------------------------------------
    // ApprovalVerdict
    // -----------------------------------------------------------------------

    #[test]
    fn approval_verdict_debug() {
        assert_eq!(format!("{:?}", ApprovalVerdict::Allowed), "Allowed");
        assert_eq!(format!("{:?}", ApprovalVerdict::Denied), "Denied");
    }

    #[test]
    fn approval_verdict_clone_copy_eq() {
        let a = ApprovalVerdict::Allowed;
        let b = a;
        assert_eq!(a, b);
        let c = ApprovalVerdict::Denied;
        assert_ne!(a, c);
        // copy
        let _ = (a, b, c);
    }

    // -----------------------------------------------------------------------
    // QuestionRequest
    // -----------------------------------------------------------------------

    #[test]
    fn question_request_fields() {
        let (tx, rx) = oneshot::channel();
        let req = QuestionRequest {
            agent_id: AgentId("agent-1".into()),
            request_id: "req-1".into(),
            question: "What is the capital of France?".into(),
            reply: tx,
        };
        assert_eq!(req.agent_id.0, "agent-1");
        assert_eq!(req.request_id, "req-1");
        assert_eq!(req.question, "What is the capital of France?");
        // Sending a reply should succeed
        req.reply.send("Paris".into()).ok();
        assert_eq!(rx.blocking_recv(), Ok("Paris".into()));
    }

    #[test]
    fn question_request_dropped_receiver() {
        // When the receiver is dropped, sending should fail
        let (tx, rx) = oneshot::channel();
        let req = QuestionRequest {
            agent_id: AgentId("a1".into()),
            request_id: "r1".into(),
            question: "q?".into(),
            reply: tx,
        };
        drop(rx);
        // This should return an error since the receiver is gone
        assert!(req.reply.send("answer".into()).is_err());
    }

    // -----------------------------------------------------------------------
    // ApprovalRequest
    // -----------------------------------------------------------------------

    #[test]
    fn approval_request_fields() {
        let (tx, rx) = oneshot::channel();
        let req = ApprovalRequest {
            agent_id: AgentId("agent-2".into()),
            request_id: "req-2".into(),
            tool: "shell".into(),
            args_preview: "ls -la".into(),
            reply: tx,
        };
        assert_eq!(req.agent_id.0, "agent-2");
        assert_eq!(req.request_id, "req-2");
        assert_eq!(req.tool, "shell");
        assert_eq!(req.args_preview, "ls -la");
        req.reply.send(true).ok();
        assert_eq!(rx.blocking_recv(), Ok(true));
    }

    #[test]
    fn approval_request_deny() {
        let (tx, rx) = oneshot::channel();
        let req = ApprovalRequest {
            agent_id: AgentId("a3".into()),
            request_id: "r3".into(),
            tool: "delete".into(),
            args_preview: "rm -rf /".into(),
            reply: tx,
        };
        req.reply.send(false).ok();
        assert_eq!(rx.blocking_recv(), Ok(false));
    }

    #[test]
    fn approval_request_dropped_receiver() {
        let (tx, rx) = oneshot::channel();
        let req = ApprovalRequest {
            agent_id: AgentId("a4".into()),
            request_id: "r4".into(),
            tool: "write".into(),
            args_preview: "file".into(),
            reply: tx,
        };
        drop(rx);
        assert!(req.reply.send(false).is_err());
    }

    // -----------------------------------------------------------------------
    // Channel type aliases
    // -----------------------------------------------------------------------

    #[test]
    fn question_tx_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<QuestionTx>();
    }

    #[test]
    fn approval_tx_is_send() {
        fn assert_send<T: Send>() {}
        assert_send::<ApprovalTx>();
    }

    // -----------------------------------------------------------------------
    // Integration: send QuestionRequest through channel
    // -----------------------------------------------------------------------

    #[test]
    fn send_question_request_through_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<QuestionRequest>();

        let (reply_tx, reply_rx) = oneshot::channel();
        let req = QuestionRequest {
            agent_id: AgentId("a5".into()),
            request_id: "r5".into(),
            question: "Continue?".into(),
            reply: reply_tx,
        };
        tx.send(req).ok();
        let received = rx.blocking_recv().expect("should receive");
        assert_eq!(received.question, "Continue?");
        received.reply.send("yes".into()).ok();
        assert_eq!(reply_rx.blocking_recv(), Ok("yes".into()));
    }

    #[test]
    fn send_approval_request_through_channel() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<ApprovalRequest>();

        let (reply_tx, reply_rx) = oneshot::channel();
        let req = ApprovalRequest {
            agent_id: AgentId("a6".into()),
            request_id: "r6".into(),
            tool: "deploy".into(),
            args_preview: "deploy to prod".into(),
            reply: reply_tx,
        };
        tx.send(req).ok();
        let received = rx.blocking_recv().expect("should receive");
        assert_eq!(received.tool, "deploy");
        received.reply.send(true).ok();
        assert_eq!(reply_rx.blocking_recv(), Ok(true));
    }
}
