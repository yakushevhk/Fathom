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
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
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

/// Maximum number of job records retained in the registry. Oldest
/// terminal-state jobs are evicted first past this cap.
const MAX_RETAINED_JOBS: usize = 4096;

/// Process-global manager for async background jobs.
///
/// Singleton. Jobs are created by `spawn_agent` with `background: true`.
/// The runtime registers a delivery sink per agent id; when a job finishes,
/// the result is pushed into that sink and the agent drains it at its next
/// turn boundary.
pub struct AsyncJobManager {
    jobs: Mutex<HashMap<JobId, JobInfo>>,
    abort_handles: Mutex<HashMap<JobId, tokio::task::AbortHandle>>,
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
            abort_handles: Mutex::new(HashMap::new()),
            sinks: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            max_running: AtomicU64::new(15),
        }
    }
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
        self.prune_jobs();
        id
    }

    /// Evict the oldest terminal jobs when the registry grows past
    /// `MAX_RETAINED_JOBS`, so a long-lived daemon cannot accumulate
    /// finished job records without bound.
    fn prune_jobs(&self) {
        let mut jobs = self.jobs.lock();
        if jobs.len() <= MAX_RETAINED_JOBS {
            return;
        }
        let mut terminal: Vec<JobId> = jobs
            .values()
            .filter(|j| {
                matches!(
                    j.status,
                    JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
                )
            })
            .map(|j| j.id)
            .collect();
        terminal.sort_unstable();
        for id in terminal
            .into_iter()
            .take(jobs.len().saturating_sub(MAX_RETAINED_JOBS))
        {
            jobs.remove(&id);
        }
    }

    /// Mark a job as running if a concurrency slot is available.
    /// Returns false when the configured limit is reached; callers should
    /// leave the job registered or queue it rather than exceeding the cap.
    pub fn mark_running(&self, id: JobId) -> bool {
        let mut jobs = self.jobs.lock();
        let running = jobs
            .values()
            .filter(|j| j.status == JobStatus::Running)
            .count() as u64;
        if running >= self.max_running.load(Ordering::Relaxed) {
            return false;
        }
        if let Some(job) = jobs.get_mut(&id) {
            job.status = JobStatus::Running;
            return true;
        }
        false
    }

    /// Associate an AbortHandle with a running job so cancel() can cleanly abort it.
    pub fn attach_abort_handle(&self, id: JobId, handle: tokio::task::AbortHandle) {
        self.abort_handles.lock().insert(id, handle);
    }
    /// Configure the process-wide running-job limit.
    pub fn set_max_running(&self, max: usize) {
        self.max_running.store(max.max(1) as u64, Ordering::Relaxed);
    }

    /// Complete a job with a result. The result is delivered to the owner's
    /// sink.
    pub fn complete(&self, id: JobId, result: Result<String, String>, tokens: u64) {
        self.abort_handles.lock().remove(&id);
        let label;
        let owner;
        {
            let mut jobs = self.jobs.lock();
            let Some(job) = jobs.get_mut(&id) else {
                return;
            };
            // A terminal record (e.g. a cancelled job) must not be
            // resurrected by a late completion.
            if matches!(
                job.status,
                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
            ) {
                return;
            }
            job.status = JobStatus::Completed;
            job.result = Some(result.clone());
            job.tokens = tokens;
            label = job.label.clone();
            owner = job.owner_id.clone();
        }

        // Deliver to the owner's sink.
        if let Some(tx) = self.sinks.lock().get(&owner.0).cloned() {
            let _ = tx.send(JobResult {
                label,
                result,
                tokens,
            });
        }
    }

    /// Mark a job as failed (without a delivery sink notification).
    pub fn fail(&self, id: JobId, error: String) {
        self.abort_handles.lock().remove(&id);
        if let Some(job) = self.jobs.lock().get_mut(&id) {
            if matches!(
                job.status,
                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
            ) {
                return;
            }
            job.status = JobStatus::Failed;
            job.result = Some(Err(error));
        }
    }

    /// Cancel a job.
    /// Cancel a job and abort its underlying Tokio task if an AbortHandle was registered.
    pub fn cancel(&self, id: JobId) {
        if let Some(handle) = self.abort_handles.lock().remove(&id) {
            handle.abort();
        }
        if let Some(job) = self.jobs.lock().get_mut(&id) {
            if matches!(job.status, JobStatus::Completed | JobStatus::Failed) {
                return;
            }
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

        assert!(mgr.mark_running(id));
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

    #[tokio::test]
    async fn test_cancelled_job_not_resurrected_by_late_complete() {
        let mgr = AsyncJobManager::new();
        let owner = AgentId::new();
        let mut rx = mgr.register_sink(&owner);
        let id = mgr.create_job(&owner, "dead-job");
        mgr.mark_running(id);
        mgr.cancel(id);
        // A late completion (e.g. the task finished just as cancel landed)
        // must not flip the record back or notify the sink.
        mgr.complete(id, Ok("too late".to_string()), 1);
        assert_eq!(mgr.get(id).unwrap().status, JobStatus::Cancelled);
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn test_completed_job_cannot_be_failed_or_cancelled() {
        let mgr = AsyncJobManager::new();
        let owner = AgentId::new();
        let id = mgr.create_job(&owner, "done-job");
        mgr.complete(id, Ok("ok".to_string()), 0);
        mgr.fail(id, "nope".to_string());
        mgr.cancel(id);
        assert_eq!(mgr.get(id).unwrap().status, JobStatus::Completed);
    }

    #[tokio::test]
    async fn test_complete_prunes_abort_handle() {
        let mgr = AsyncJobManager::new();
        let owner = AgentId::new();
        let id = mgr.create_job(&owner, "h");
        let jh = tokio::spawn(async {});
        mgr.attach_abort_handle(id, jh.abort_handle());
        assert_eq!(mgr.abort_handles.lock().len(), 1);
        mgr.complete(id, Ok("x".to_string()), 0);
        assert!(mgr.abort_handles.lock().is_empty());
    }

    #[test]
    fn test_prune_evicts_oldest_terminal_jobs() {
        let mgr = AsyncJobManager::new();
        let owner = AgentId::new();
        for _ in 0..MAX_RETAINED_JOBS {
            let id = mgr.create_job(&owner, "j");
            mgr.complete(id, Ok("x".to_string()), 0);
        }
        assert_eq!(mgr.snapshot().len(), MAX_RETAINED_JOBS);
        // The next job pushes us past the cap: the oldest terminal
        // record is evicted while running/registered jobs stay.
        let running = mgr.create_job(&owner, "still-running");
        assert!(mgr.snapshot().len() <= MAX_RETAINED_JOBS);
        assert!(mgr.get(1).is_none());
        assert_eq!(mgr.get(running).unwrap().status, JobStatus::Registered);
    }
}
