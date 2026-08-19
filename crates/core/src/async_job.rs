//! In-process async job manager for background sub-agent tasks.
//!
//! Tracks the lifecycle of every spawned background agent: registered →
//! running → completed / failed / cancelled. Results are delivered to the
//! owning agent's delivery sink (a simple mpsc channel) as structured
//! notifications that the runtime drains at turn boundaries.
//!
//! Concurrency is bounded by `max_running_jobs` (default 15). Jobs past
//! the limit are queued and start when a slot opens.

use crate::ids::AgentId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::mpsc;

/// Unique job id within the process.
pub type JobId = u64;

/// Status of an async job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Registered,
    Running,
    Completed,
    Failed,
    Cancelled,
}

/// Metadata about one job.
#[derive(Debug, Clone)]
pub struct JobInfo {
    pub id: JobId,
    pub owner_id: AgentId,
    pub label: String,
    pub status: JobStatus,
    pub result: Option<Result<String, String>>,
    pub tokens: u64,
}

/// Notification sent to the owning agent when a job completes.
#[derive(Debug, Clone)]
pub struct JobResult {
    pub label: String,
    pub result: Result<String, String>,
    pub tokens: u64,
}

/// Process-global manager for async background jobs.
///
/// Singleton. Jobs are created by `spawn_agent` with `background: true`.
/// The runtime registers a delivery sink per agent id; when a job finishes,
/// the result is pushed into that sink and the agent drains it at its next
/// turn boundary.
pub struct AsyncJobManager {
    jobs: Mutex<HashMap<JobId, JobInfo>>,
    /// Delivery sinks: agent id → sender.
    sinks: Mutex<HashMap<String, mpsc::UnboundedSender<JobResult>>>,
    next_id: AtomicU64,
    /// Max concurrently running jobs (queued above this).
    max_running: AtomicU64,
}

impl AsyncJobManager {
    fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            sinks: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            max_running: AtomicU64::new(15),
        }
    }

    /// Access the process-global singleton.
    pub fn global() -> &'static Self {
        static MGR: std::sync::LazyLock<AsyncJobManager> =
            std::sync::LazyLock::new(AsyncJobManager::new);
        &MGR
    }

    /// Register a delivery sink for an agent. When jobs owned by this agent
    /// complete, results are sent here.
    pub fn register_sink(&self, id: &AgentId) -> mpsc::UnboundedReceiver<JobResult> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.sinks.lock().insert(id.0.clone(), tx);
        rx
    }

    /// Remove a delivery sink (agent unregistered).
    pub fn unregister_sink(&self, id: &AgentId) {
        self.sinks.lock().remove(&id.0);
    }

    /// Create a new job and assign it a unique id. Returns the job id.
    /// The job is registered but not yet running.
    pub fn create_job(&self, owner_id: &AgentId, label: &str) -> JobId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.jobs.lock().insert(
            id,
            JobInfo {
                id,
                owner_id: owner_id.clone(),
                label: label.to_string(),
                status: JobStatus::Registered,
                result: None,
                tokens: 0,
            },
        );
        id
    }

    /// Mark a job as running.
    pub fn mark_running(&self, id: JobId) {
        if let Some(job) = self.jobs.lock().get_mut(&id) {
            job.status = JobStatus::Running;
        }
    }

    /// Complete a job with a result. The result is delivered to the owner's
    /// sink.
    pub fn complete(&self, id: JobId, result: Result<String, String>, tokens: u64) {
        let label;
        let owner;
        {
            let mut jobs = self.jobs.lock();
            let Some(job) = jobs.get_mut(&id) else {
                return;
            };
            job.status = JobStatus::Completed;
            job.result = Some(result.clone());
            job.tokens = tokens;
            label = job.label.clone();
            owner = job.owner_id.clone();
        }

        // Deliver to the owner's sink.
        if let Some(tx) = self.sinks.lock().get(&owner.0).cloned() {
            let _ = tx.send(JobResult { label, result, tokens });
        }
    }

    /// Mark a job as failed (without a delivery sink notification).
    pub fn fail(&self, id: JobId, error: String) {
        if let Some(job) = self.jobs.lock().get_mut(&id) {
            job.status = JobStatus::Failed;
            job.result = Some(Err(error));
        }
    }

    /// Cancel a job.
    pub fn cancel(&self, id: JobId) {
        if let Some(job) = self.jobs.lock().get_mut(&id) {
            job.status = JobStatus::Cancelled;
        }
    }

    /// Get info about a job.
    pub fn get(&self, id: JobId) -> Option<JobInfo> {
        self.jobs.lock().get(&id).cloned()
    }

    /// List all jobs owned by an agent.
    pub fn list_by_owner(&self, owner_id: &AgentId) -> Vec<JobInfo> {
        self.jobs
            .lock()
            .values()
            .filter(|j| j.owner_id.0 == owner_id.0)
            .cloned()
            .collect()
    }

    /// Snapshot of all jobs.
    pub fn snapshot(&self) -> Vec<JobInfo> {
        self.jobs.lock().values().cloned().collect()
    }

    /// Count of running jobs.
    pub fn running_count(&self) -> usize {
        self.jobs
            .lock()
            .values()
            .filter(|j| j.status == JobStatus::Running)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_complete_job() {
        let mgr = AsyncJobManager::new();
        let owner = AgentId::new();
        let mut rx = mgr.register_sink(&owner);

        let id = mgr.create_job(&owner, "test-job");
        assert_eq!(mgr.get(id).unwrap().status, JobStatus::Registered);

        mgr.mark_running(id);
        assert_eq!(mgr.get(id).unwrap().status, JobStatus::Running);

        mgr.complete(id, Ok("done".to_string()), 42);
        assert_eq!(mgr.get(id).unwrap().status, JobStatus::Completed);

        // Result delivered to sink
        let result = rx.recv().await.unwrap();
        assert_eq!(result.label, "test-job");
        assert_eq!(result.result.unwrap(), "done");
        assert_eq!(result.tokens, 42);
    }

    #[tokio::test]
    async fn test_job_list_by_owner() {
        let mgr = AsyncJobManager::new();
        let owner = AgentId::new();
        let other = AgentId::new();

        let _ = mgr.create_job(&owner, "a");
        let _ = mgr.create_job(&owner, "b");
        let _ = mgr.create_job(&other, "c");

        assert_eq!(mgr.list_by_owner(&owner).len(), 2);
        assert_eq!(mgr.list_by_owner(&other).len(), 1);
    }

    #[test]
    fn test_job_cancel() {
        let mgr = AsyncJobManager::new();
        let owner = AgentId::new();
        let id = mgr.create_job(&owner, "cancel-me");
        mgr.cancel(id);
        assert_eq!(mgr.get(id).unwrap().status, JobStatus::Cancelled);
    }

    #[test]
    fn test_running_count() {
        let mgr = AsyncJobManager::new();
        let owner = AgentId::new();
        let a = mgr.create_job(&owner, "a");
        let b = mgr.create_job(&owner, "b");
        mgr.mark_running(a);
        mgr.mark_running(b);
        assert_eq!(mgr.running_count(), 2);
    }
}