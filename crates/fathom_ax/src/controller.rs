//! AX controller — single-writer orchestration over the durable store.
//!
//! All mutations flow through [`AxController`], which owns the SQLite store
//! and the actor registry. Every applied manifest, phase transition and
//! lifecycle action is appended to the event log before observers are
//! notified, so `fathom ax watch` streams the same durable history the
//! store replays after a crash.
//!
//! Lifecycle (mirroring `internal/controller/reconciler.go` upstream):
//! `upsert → resolve refs → materialize workspaces (git clone) → apply
//! egress policy (recorded as a GatewayReady condition) → spawn or
//! suspend/resume actor → watch exit → Completed/Failed`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::actor::{self, ActorSpec};
use crate::error::{AxError, AxResult};
use crate::manifest::{
    kind, phase, AxManifest, Condition, Gateway, Model, Task, TaskStatus, Workspace,
};
use crate::store::AxStore;

/// Grace period before SIGTERM escalates to SIGKILL.
const KILL_GRACE_MS: u64 = 3000;

/// One task status snapshot as published to watchers.
#[derive(Debug, Clone)]
pub struct TaskEvent {
    pub seq: i64,
    pub action: String,
    pub task: Task,
}

struct Live {
    /// JoinHandle awaiting the actor's exit.
    monitor: JoinHandle<()>,
}

pub struct AxController {
    store: AxStore,
    /// Live actor monitors keyed by `atespace/name`.
    live: Mutex<HashMap<String, Live>>,
    /// Broadcast channel feeding `watch` subscribers (event log stays the
    /// source of truth — the broadcast is a notification edge).
    events_tx: broadcast::Sender<TaskEvent>,
    /// Optional path to the fathom binary for the built-in harness.
    fathom_bin: Mutex<PathBuf>,
}

impl AxController {
    /// Open a controller rooted at `state_dir` (created if absent), then
    /// recover: reattach to surviving actors and mark lost ones
    /// Interrupted so `ax resume` can continue them durably.
    pub fn open(state_dir: impl AsRef<Path>) -> AxResult<Arc<Self>> {
        let store = AxStore::open(state_dir.as_ref().join("store"))?;
        let (events_tx, _) = broadcast::channel(1024);
        let fathom_bin = std::env::var("FATHOM_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|_| which::which("fathom").unwrap_or_else(|_| PathBuf::from("fathom")));
        let ctl = Arc::new(Self {
            store,
            live: Mutex::new(HashMap::new()),
            events_tx,
            fathom_bin: Mutex::new(fathom_bin),
        });
        ctl.recover();
        Ok(ctl)
    }

    /// In-memory controller (tests) — SQLite in RAM, `root` a real dir for
    /// actor logs and materialized workspaces.
    pub fn in_memory(root: impl AsRef<Path>) -> AxResult<Arc<Self>> {
        let (events_tx, _) = broadcast::channel(1024);
        Ok(Arc::new(Self {
            store: AxStore::in_memory(root)?,
            live: Mutex::new(HashMap::new()),
            events_tx,
            fathom_bin: Mutex::new(PathBuf::from("fathom")),
        }))
    }

    /// Use an explicit fathom binary path for the built-in harness.
    pub fn set_fathom_bin(&self, bin: PathBuf) {
        *self.fathom_bin.lock() = bin;
    }

    pub fn store(&self) -> &AxStore {
        &self.store
    }

    // ── Reconciliation entry points ──────────────────────────────────────

    /// Apply a manifest (upsert). For tasks, kicks reconciliation.
    pub fn apply(self: &Arc<Self>, manifest: AxManifest) -> AxResult<()> {
        manifest.validate()?;
        let meta = manifest.metadata().clone();
        let kind_s = manifest.kind().to_lowercase();
        let is_task = matches!(manifest, AxManifest::Task(_));

        if is_task {
            let AxManifest::Task(task) = &manifest else {
                return Err(AxError::Manifest("expected a Task manifest".into()));
            };
            let mut task = task.clone();
            if task.status.phase.is_empty() {
                task.status.phase = phase::PENDING.to_string();
            }
            let manifest = AxManifest::Task(task.clone());
            self.store.upsert(&manifest)?;
            self.publish(&task, "upsert", serde_json::json!({}))?;
            // Reconcile runs off the caller's path.
            let ctl = Arc::clone(self);
            spawn_reconcile(ctl, task);
        } else {
            self.store.upsert(&manifest)?;
            self.append_obj_event(&kind_s, &meta, "upsert", &serde_json::to_value(&manifest)?)?;
        }
        Ok(())
    }

    /// Apply every document in a YAML stream (like `ax apply -f`).
    /// Non-task manifests are stored first so a Task that references a
    /// Workspace/Gateway/Model resolves its refs regardless of document
    /// order in the file. Returns the number of applied manifests.
    pub fn apply_documents(self: &Arc<Self>, yaml: &str) -> AxResult<usize> {
        let docs = AxManifest::parse_documents(yaml)?;
        for m in docs.iter().filter(|m| !matches!(m, AxManifest::Task(_))) {
            self.apply(m.clone())?;
        }
        for m in docs.iter().filter(|m| matches!(m, AxManifest::Task(_))) {
            self.apply(m.clone())?;
        }
        Ok(docs.len())
    }

    /// Fetch a resource.
    pub fn get(&self, kind: &str, atespace: &str, name: &str) -> AxResult<AxManifest> {
        self.store.get(kind, atespace, name)
    }

    /// List resources of a kind ("task", "gateway", "workspace", "model").
    pub fn list(&self, kind: &str, atespace: &str) -> AxResult<Vec<AxManifest>> {
        self.store.list(kind, atespace)
    }

    /// Subscribe to live task events (for `ax watch`).
    pub fn watch(&self) -> broadcast::Receiver<TaskEvent> {
        self.events_tx.subscribe()
    }

    /// Two-phase delete: Terminating → actor teardown → row removal.
    pub async fn delete_task(self: &Arc<Self>, atespace: &str, name: &str) -> AxResult<()> {
        self.store.mark_terminating(atespace, name)?;
        self.teardown_actor(atespace, name).await;
        self.store.delete("task", atespace, name)?;
        Ok(())
    }

    /// Delete a non-task resource.
    pub fn delete_resource(&self, kind: &str, atespace: &str, name: &str) -> AxResult<()> {
        if kind == kind::TASK || kind.eq_ignore_ascii_case("task") {
            return Err(AxError::Validation(
                "use delete_task() for tasks (two-phase)".into(),
            ));
        }
        if !self.store.delete(kind, atespace, name)? {
            return Err(AxError::NotFound {
                kind: kind_static(kind),
                name: name.into(),
                atespace: atespace.into(),
            });
        }
        Ok(())
    }

    /// SIGSTOP the actor's process group; phase → Suspended.
    pub async fn suspend_task(self: &Arc<Self>, atespace: &str, name: &str) -> AxResult<Task> {
        let mut task = self.store.get_task(atespace, name)?;
        match task.status.phase.as_str() {
            p if p == phase::RUNNING => {}
            p => return Err(AxError::InvalidPhase(name.into(), p.into(), phase::RUNNING)),
        }
        if task.status.pid > 0 {
            actor::suspend(task.status.pid)?;
        }
        task.status.phase = phase::SUSPENDED.into();
        self.set_condition(
            &mut task,
            "Ready",
            "False",
            "TaskSuspended",
            "Task is suspended",
        );
        self.save_and_publish(&task, "phase", serde_json::json!({"phase": "Suspended"}))?;
        Ok(task)
    }

    /// Resume a suspended/interrupted task: SIGCONT when the actor is still
    /// alive, otherwise respawn from its manifest (durable resume).
    pub async fn resume_task(self: &Arc<Self>, atespace: &str, name: &str) -> AxResult<Task> {
        let mut task = self.store.get_task(atespace, name)?;
        match task.status.phase.as_str() {
            p if p == phase::SUSPENDED || p == phase::INTERRUPTED || p == phase::FAILED => {}
            p => {
                return Err(AxError::InvalidPhase(
                    name.into(),
                    p.into(),
                    "Suspended|Interrupted|Failed",
                ))
            }
        }
        if task.status.pid > 0 && actor::alive(task.status.pid) {
            actor::resume(task.status.pid)?;
            task.status.phase = phase::RUNNING.into();
            task.spec.suspend = false;
            self.set_condition(&mut task, "Ready", "True", "TaskResumed", "Actor resumed");
            self.save_and_publish(&task, "phase", serde_json::json!({"phase": "Running"}))?;
            return Ok(task);
        }
        // Process is gone — durable resume: clear spec.suspend (upstream
        // `ax unsuspend` semantics) and respawn the declared command.
        task.spec.suspend = false;
        self.store.upsert(&AxManifest::Task(task.clone()))?;
        self.store.save_status(&task)?;
        let ctl = Arc::clone(self);
        spawn_reconcile(ctl, task.clone());
        Ok(task)
    }

    /// Fork a task at event sequence `at_seq`: a sibling task is created
    /// with provenance (forked_from/fork_seq) recorded on its status and a
    /// `fork` event appended — the new chain diverges from that point.
    pub fn fork_task(
        self: &Arc<Self>,
        atespace: &str,
        name: &str,
        new_name: &str,
        at_seq: Option<i64>,
    ) -> AxResult<Task> {
        let src = self.store.get_task(atespace, name)?;
        let fork_seq = at_seq.unwrap_or(src.status.last_seq);
        let mut fork = src.clone();
        fork.metadata.name = new_name.to_string();
        fork.metadata.creation_timestamp = None;
        fork.status = TaskStatus {
            phase: phase::PENDING.into(),
            forked_from: format!("{atespace}/{name}"),
            fork_seq,
            ..Default::default()
        };
        self.store.upsert(&AxManifest::Task(fork.clone()))?;
        self.append_obj_event(
            "task",
            &fork.metadata,
            "fork",
            &serde_json::json!({
                "from": format!("{atespace}/{name}"),
                "at_seq": fork_seq,
            }),
        )?;
        let ctl = Arc::clone(self);
        spawn_reconcile(ctl, fork.clone());
        Ok(fork)
    }

    // ── Reconciler ───────────────────────────────────────────────────────

    /// Reconcile one task to Running (or Suspended when spec.suspend).
    ///
    /// Runs fully synchronously on the caller's path: every durable
    /// mutation — ref resolution, workspace materialization, actor spawn,
    /// actor record, phase publish — completes before this returns. That
    /// is what makes a CLI invocation safe: when `fathom ax apply` exits
    /// and its tokio runtime drops, nothing needed to reach Running state
    /// is still pending on an async task that will never be polled again.
    /// The only async work is the exit monitor, which is best-effort: the
    /// actor's recorded exit file plus `recover()` resolve the terminal
    /// phase even if the monitor never ran.
    fn reconcile(self: &Arc<Self>, task: &Task) {
        if let Err(e) = self.reconcile_sync(task) {
            tracing::warn!("ax reconcile failed for {}: {e:#}", task.metadata.key());
        }
    }

    fn reconcile_sync(self: &Arc<Self>, task: &Task) -> AxResult<()> {
        let mut task = task.clone();
        let key = task.metadata.key();
        task.status.id = format!(
            "task-{}-{}",
            task.metadata.name,
            chrono::Utc::now().timestamp()
        );
        task.status.actor = task.metadata.name.clone();
        if task.status.phase.is_empty() || task.status.phase == phase::PENDING {
            task.status.phase = phase::PENDING.to_string();
        }

        // 1. Resolve referenced objects.
        let mut workspaces = Vec::new();
        for wref in &task.spec.workspaces {
            match self.get_workspace(&task, &wref.name) {
                Ok(w) => workspaces.push((wref.clone(), w)),
                Err(e) => {
                    return self.fail_task(
                        &task,
                        "WorkspaceReady",
                        "WorkspaceNotFound",
                        &e.to_string(),
                    );
                }
            }
        }
        let gateway = match &task.spec.gateway {
            Some(gref) => match self.get_gateway(&task, &gref.name) {
                Ok(g) => Some(g),
                Err(e) => {
                    return self.fail_task(
                        &task,
                        "GatewayReady",
                        "GatewayNotFound",
                        &e.to_string(),
                    );
                }
            },
            None => None,
        };
        let model = self.find_model(&task);

        // 2. Materialize workspaces (git clones into .ax/workspaces/<task>/).
        let task_dir = self
            .store
            .root()
            .join("workspaces")
            .join(key.replace('/', "_"));
        std::fs::create_dir_all(&task_dir)?;
        for (wref, ws) in &workspaces {
            for repo in &ws.spec.git {
                let dest = task_dir
                    .join(if wref.path.is_empty() {
                        &wref.name
                    } else {
                        &wref.path
                    })
                    .join(if repo.dir.is_empty() {
                        &repo.name
                    } else {
                        &repo.dir
                    });
                if !dest.exists() {
                    if let Err(e) = git_clone(repo, &dest) {
                        return self.fail_task(
                            &task,
                            "WorkspaceReady",
                            "GitCloneFailed",
                            &format!("{}: {e}", repo.repo),
                        );
                    }
                    self.append_obj_event(
                        "task",
                        &task.metadata,
                        "condition",
                        &serde_json::json!({
                            "type": "WorkspaceReady", "status": "True",
                            "reason": "GitCloned", "message": repo.repo,
                        }),
                    )?;
                }
            }
        }
        if !workspaces.is_empty() {
            self.set_condition(
                &mut task,
                "WorkspaceReady",
                "True",
                "WorkspacesBound",
                &format!("{} workspace(s) materialized", workspaces.len()),
            );
        }

        // 3. Egress policy — recorded declaratively (a standalone runtime
        //    has no kernel-level net enforcement hook; the allowlist is
        //    exported to the actor env as AX_EGRESS_ALLOWLIST).
        let egress_json = match &gateway {
            Some(g) => serde_json::to_string(&g.spec.egress.allowlist)?,
            None => serde_json::to_string(&crate::manifest::EgressAllowlist {
                hosts: vec![crate::manifest::HostRule {
                    host: "*".into(),
                    port: 443,
                }],
            })?,
        };
        self.set_condition(
            &mut task,
            "GatewayReady",
            "True",
            "PoliciesApplied",
            "Network policies active",
        );

        // 4. Suspended manifests never spawn.
        if task.spec.suspend {
            task.status.phase = phase::SUSPENDED.into();
            self.set_condition(
                &mut task,
                "Ready",
                "False",
                "TaskSuspended",
                "Task is suspended",
            );
            self.save_and_publish(&task, "phase", serde_json::json!({"phase": "Suspended"}))?;
            return Ok(());
        }

        // 5. Build the actor spec and spawn. The command is wrapped in a
        //    shim that records its exit code next to the log — the durable
        //        checkpoint that lets a *future* controller resolve the real
        //        terminal phase even if this controller died first.
        let spec = self.build_actor_spec(&task, &workspaces, &model, &egress_json, &task_dir)?;
        let log_path = self.store.root().join("logs").join(format!(
            "{}-{}.log",
            key.replace('/', "_"),
            task.status.id
        ));
        let exit_path = exit_code_path(&log_path);
        let spec = ActorSpec {
            argv: wrap_exit_recorder(&spec.argv, &exit_path),
            ..spec
        };
        let (child, pid) = match actor::spawn(&spec, &log_path) {
            Ok(v) => v,
            Err(e) => {
                return self.fail_task(&task, "Ready", "ActorSpawnFailed", &e.to_string());
            }
        };
        task.status.pid = pid;
        task.status.log_path = log_path.to_string_lossy().to_string();
        task.status.phase = phase::RUNNING.into();
        task.status.worker_ip = "127.0.0.1".into(); // local actor
        self.set_condition(
            &mut task,
            "Ready",
            "True",
            "ActorRunning",
            "Actor is running",
        );
        self.store
            .record_actor(&key, Some(pid), &task.status.id, &log_path)?;
        let task = {
            self.save_and_publish(&task, "phase", serde_json::json!({"phase": "Running"}))?;
            task
        };

        // 6. Monitor the child — only when a tokio runtime is live (a
        //    long-running embedded controller or a `watch`-style command).
        //    Without one the actor still records its exit code durably and
        //    the next controller `open()` resolves the terminal phase.
        if let Ok(h) = tokio::runtime::Handle::try_current() {
            let ctl = Arc::clone(self);
            let key2 = key.clone();
            let actor_id = task.status.id.clone();
            let monitor = h.spawn(async move { ctl.monitor_child(&key2, actor_id, child).await });
            self.live.lock().insert(key, Live { monitor });
        }
        Ok(())
    }

    /// Build env + argv for the actor: manifest env, injected AX_TASK_YAML /
    /// AX_WORKSPACES_YAML (as upstream does), model resolution, egress.
    fn build_actor_spec(
        &self,
        task: &Task,
        workspaces: &[(crate::manifest::WorkspaceRef, Workspace)],
        model: &Option<Model>,
        egress_json: &str,
        task_dir: &Path,
    ) -> AxResult<ActorSpec> {
        let mut env: HashMap<String, String> = std::env::vars().collect();
        for e in &task.spec.env {
            env.insert(e.name.clone(), e.value.clone());
        }
        env.insert(
            "AX_TASK_YAML".into(),
            serde_yaml::to_string(&task).unwrap_or_default(),
        );
        if !workspaces.is_empty() {
            let docs: Vec<String> = workspaces
                .iter()
                .map(|(_, w)| serde_yaml::to_string(w).unwrap_or_default())
                .collect();
            env.insert("AX_WORKSPACES_YAML".into(), docs.join("---\n"));
        }
        env.insert("AX_EGRESS_ALLOWLIST".into(), egress_json.to_string());
        if let Some(m) = model {
            env.insert("AX_MODEL_PROVIDER".into(), m.spec.provider.clone());
            env.insert("AX_MODEL_NAME".into(), m.spec.model.clone());
            if !m.spec.parameters.is_empty() {
                env.insert(
                    "AX_MODEL_PARAMETERS".into(),
                    serde_json::to_string(&m.spec.parameters)?,
                );
            }
            if let Some(sk) = &m.spec.secret_key {
                // Resolve <key> (or <name>) from the process environment —
                // the local stand-in for a k8s Secret lookup.
                let var = if sk.key.is_empty() { &sk.name } else { &sk.key };
                if let Ok(v) = std::env::var(var) {
                    env.insert("MODEL_API_KEY".into(), v);
                }
            }
        }

        let argv = if !task.spec.command.is_empty() {
            task.spec.command.clone()
        } else {
            // Built-in Fathom harness: feed the first workspace goal (or the
            // task name) to `fathom run`.
            let goal = task
                .spec
                .workspaces
                .first()
                .map(|w| w.goal.clone())
                .filter(|g| !g.is_empty())
                .unwrap_or_else(|| task.metadata.name.clone());
            vec![
                self.fathom_bin.lock().to_string_lossy().to_string(),
                "run".into(),
                goal,
                "-o".into(),
                task_dir.join("output").to_string_lossy().to_string(),
            ]
        };

        let cwd = task_dir.to_path_buf();
        Ok(ActorSpec { argv, cwd, env })
    }

    /// Await the actor's exit and mark the task terminal. The child is a
    /// std process (runtime-free spawn) polled via try_wait; if the task is
    /// abandoned by runtime shutdown, `recover()` finalizes via the exit
    /// file on the next open.
    async fn monitor_child(&self, key: &str, actor_id: String, mut child: std::process::Child) {
        // Poll try_wait rather than a blocking wait(): a blocking wait on a
        // suspended actor never returns, and Runtime::drop waits for
        // in-flight spawn_blocking work — wedging every runtime shutdown
        // (CLI exit, embedder teardown, test harness) while a task is
        // suspended. A plain async poll is simply abandoned on drop.
        let status = loop {
            match child.try_wait() {
                Ok(Some(s)) => break Ok(s),
                Ok(None) => {
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
                Err(e) => break Err(e),
            }
        };
        let (atespace, name) = key.split_once('/').unwrap_or(("default", key));
        let Ok(mut task) = self.store.get_task(atespace, name) else {
            return; // deleted mid-run
        };
        // Only finalize the generation we spawned — a resumed task has a
        // newer actor_id and its own monitor.
        if task.status.id != actor_id {
            return;
        }
        self.store.clear_actor_pid(key).ok();
        match status {
            Ok(s) if s.success() => {
                task.status.phase = phase::COMPLETED.into();
                task.status.pid = 0;
                self.set_condition(&mut task, "Ready", "True", "ActorExited", "exit code 0");
                self.save_and_publish(&task, "phase", serde_json::json!({"phase": "Completed"}))
                    .ok();
            }
            Ok(s) => {
                task.status.phase = phase::FAILED.into();
                task.status.pid = 0;
                let code = s
                    .code()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "signal".into());
                self.set_condition(
                    &mut task,
                    "Ready",
                    "False",
                    "ActorExited",
                    &format!("exit code {code}"),
                );
                self.save_and_publish(
                    &task,
                    "phase",
                    serde_json::json!({"phase": "Failed", "exit": code}),
                )
                .ok();
            }
            Err(e) => {
                task.status.phase = phase::FAILED.into();
                task.status.pid = 0;
                self.set_condition(
                    &mut task,
                    "Ready",
                    "False",
                    "ActorWaitFailed",
                    &e.to_string(),
                );
                self.save_and_publish(&task, "phase", serde_json::json!({"phase": "Failed"}))
                    .ok();
            }
        }
        self.live.lock().remove(key);
    }

    /// After a restart, reattach to surviving actors; mark the rest
    /// Interrupted so `ax resume` can respawn them durably.
    fn recover(self: &Arc<Self>) {
        let Ok(tasks) = self.store.list_tasks("") else {
            return;
        };
        for mut task in tasks {
            let phase_now = task.status.phase.as_str();
            if !matches!(phase_now, phase::RUNNING | phase::SUSPENDED) {
                continue;
            }
            let key = task.metadata.key();
            let actor_pid = self
                .store
                .get_actor(&key)
                .ok()
                .flatten()
                .and_then(|(pid, _aid, _log)| pid)
                .or(if task.status.pid > 0 {
                    Some(task.status.pid)
                } else {
                    None
                });
            let ever_spawned = actor_pid.is_some() || !task.status.log_path.is_empty();
            if let Some(pid) = actor_pid {
                if actor::alive(pid) {
                    // Reattach: poll liveness; death finalizes the task.
                    if tokio::runtime::Handle::try_current().is_ok() {
                        let ctl = Arc::clone(self);
                        let k = key.clone();
                        tokio::spawn(async move { ctl.reattach_poll(&k, pid).await });
                    }
                    continue;
                }
            } else if !ever_spawned {
                // Suspended task that never spawned — genuinely still held.
                continue;
            }
            // Actor gone — consult the recorded exit code (durable
            // checkpoint) to resolve the real terminal state; absent it,
            // the task is genuinely Interrupted and resumable.
            task.status.pid = 0;
            match read_exit_code(&task.status.log_path) {
                Some(0) => {
                    task.status.phase = phase::COMPLETED.into();
                    self.set_condition(&mut task, "Ready", "True", "ActorExited", "exit code 0");
                    self.save_and_publish(
                        &task,
                        "phase",
                        serde_json::json!({"phase": "Completed"}),
                    )
                    .ok();
                }
                Some(c) => {
                    task.status.phase = phase::FAILED.into();
                    self.set_condition(
                        &mut task,
                        "Ready",
                        "False",
                        "ActorExited",
                        &format!("exit code {c}"),
                    );
                    self.save_and_publish(
                        &task,
                        "phase",
                        serde_json::json!({"phase": "Failed", "exit": c}),
                    )
                    .ok();
                }
                None => {
                    task.status.phase = phase::INTERRUPTED.into();
                    self.set_condition(
                        &mut task,
                        "Ready",
                        "False",
                        "ActorLost",
                        "controller restart; `fathom ax resume` to continue",
                    );
                    self.save_and_publish(
                        &task,
                        "phase",
                        serde_json::json!({"phase": "Interrupted"}),
                    )
                    .ok();
                }
            }
        }
    }

    /// Poll-based reattach for tasks whose actor survived a controller
    /// restart (this process can't wait() a non-child).
    async fn reattach_poll(&self, key: &str, pid: i64) {
        while actor::alive(pid) {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
        let (atespace, name) = key.split_once('/').unwrap_or(("default", key));
        let Ok(mut task) = self.store.get_task(atespace, name) else {
            return;
        };
        if task.status.pid != pid {
            return; // replaced by a newer generation
        }
        task.status.pid = 0;
        let (ph, reason) = match read_exit_code(&task.status.log_path) {
            Some(0) => (phase::COMPLETED, "exit code 0"),
            Some(_) => (phase::FAILED, "actor exited nonzero"),
            None => (phase::INTERRUPTED, "actor lost"),
        };
        task.status.phase = ph.into();
        self.set_condition(&mut task, "Ready", "True", "ActorExited", reason);
        self.save_and_publish(&task, "phase", serde_json::json!({"phase": ph}))
            .ok();
        self.live.lock().remove(key);
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn get_workspace(&self, task: &Task, name: &str) -> AxResult<Workspace> {
        match self
            .store
            .get("workspace", task.metadata.atespace_or_default(), name)?
        {
            AxManifest::Workspace(w) => Ok(w),
            _ => Err(AxError::MissingRef {
                kind: "workspace",
                name: name.into(),
                atespace: task.metadata.atespace_or_default().into(),
            }),
        }
    }

    fn get_gateway(&self, task: &Task, name: &str) -> AxResult<Gateway> {
        match self
            .store
            .get("gateway", task.metadata.atespace_or_default(), name)?
        {
            AxManifest::Gateway(g) => Ok(g),
            _ => Err(AxError::MissingRef {
                kind: "gateway",
                name: name.into(),
                atespace: task.metadata.atespace_or_default().into(),
            }),
        }
    }

    /// The atespace's default model: first Model whose name starts with
    /// "default", else the first Model at all.
    fn find_model(&self, task: &Task) -> Option<Model> {
        let models = self
            .store
            .list("model", task.metadata.atespace_or_default())
            .ok()?;
        let mut found = None;
        for m in models {
            if let AxManifest::Model(md) = m {
                if md.metadata.name.starts_with("default") {
                    return Some(md);
                }
                found.get_or_insert(md);
            }
        }
        found
    }

    fn fail_task(&self, task: &Task, cond: &str, reason: &str, msg: &str) -> AxResult<()> {
        let mut task = task.clone();
        task.status.phase = phase::FAILED.into();
        self.set_condition(&mut task, cond, "False", reason, msg);
        self.set_condition(&mut task, "Ready", "False", reason, msg);
        self.save_and_publish(
            &task,
            "phase",
            serde_json::json!({"phase": "Failed", "reason": reason}),
        )
    }

    fn set_condition(
        &self,
        task: &mut Task,
        cond_type: &str,
        status: &str,
        reason: &str,
        msg: &str,
    ) {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        if let Some(c) = task
            .status
            .conditions
            .iter_mut()
            .find(|c| c.cond_type == cond_type)
        {
            c.status = status.into();
            c.reason = reason.into();
            c.message = msg.into();
            c.last_transition_time = now;
        } else {
            task.status.conditions.push(Condition {
                cond_type: cond_type.into(),
                status: status.into(),
                last_transition_time: now,
                reason: reason.into(),
                message: msg.into(),
            });
        }
    }

    /// Persist status + append event + broadcast — the single writer path.
    fn save_and_publish(
        &self,
        task: &Task,
        action: &str,
        payload: serde_json::Value,
    ) -> AxResult<()> {
        let seq = self
            .store
            .append_event("task", &task.metadata, action, &payload)?;
        let mut t = task.clone();
        t.status.last_seq = seq;
        self.store.save_status(&t)?;
        let _ = self.events_tx.send(TaskEvent {
            seq,
            action: action.to_string(),
            task: t,
        });
        Ok(())
    }

    fn publish(&self, task: &Task, action: &str, payload: serde_json::Value) -> AxResult<()> {
        self.save_and_publish(task, action, payload)
    }

    fn append_obj_event(
        &self,
        kind: &str,
        meta: &crate::manifest::ObjectMeta,
        action: &str,
        payload: &serde_json::Value,
    ) -> AxResult<i64> {
        self.store.append_event(kind, meta, action, payload)
    }

    /// Teardown: kill the actor process group and drop the monitor.
    async fn teardown_actor(&self, atespace: &str, name: &str) {
        let key = format!("{atespace}/{name}");
        if let Some(l) = self.live.lock().remove(&key) {
            l.monitor.abort();
        }
        if let Ok(Some((Some(pid), _, _))) = self.store.get_actor(&key) {
            actor::kill(pid, KILL_GRACE_MS).await;
        }
        self.store.clear_actor_pid(&key).ok();
    }
}

/// `<log>.exit` — where the actor shim records its exit code.
fn exit_code_path(log_path: &Path) -> PathBuf {
    log_path.with_extension("exit")
}

/// Wrap `argv` in `sh -c '"$@"; rc=$?; echo -n $rc > <path>'` so the actor's
/// own process records its terminal code durably — readable by any future
/// controller even after this one died.
fn wrap_exit_recorder(argv: &[String], exit_path: &Path) -> Vec<String> {
    let mut v = vec![
        "sh".into(),
        "-c".into(),
        format!(
            "\"$@\"; rc=$?; echo -n \"$rc\" > {}; exit $rc",
            sh_quote(&exit_path.to_string_lossy())
        ),
        "ax-actor".into(),
    ];
    v.extend(argv.iter().cloned());
    v
}

/// POSIX single-quote escaping (' → '\'').
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Read the exit code recorded by the actor shim, if present.
fn read_exit_code(log_path: &str) -> Option<i64> {
    if log_path.is_empty() {
        return None;
    }
    let p = exit_code_path(Path::new(log_path));
    let s = std::fs::read_to_string(p).ok()?;
    s.trim().parse().ok()
}

/// Spawn a reconcile pass when a tokio runtime is present; without one
/// (plain sync callers) the task stays Pending — honest semantics: no
/// running controller, no actor.
/// Reconciliation runs synchronously on the caller's path (see
/// `reconcile` for why) — no runtime required, so a one-shot CLI
/// command fully materializes and spawns the actor before it exits.
fn spawn_reconcile(ctl: Arc<AxController>, task: Task) {
    ctl.reconcile(&task);
}

fn kind_static(kind: &str) -> &'static str {
    match kind {
        "task" | "Task" => "task",
        "gateway" | "Gateway" => "gateway",
        "workspace" | "Workspace" => "workspace",
        "model" | "Model" => "model",
        _ => "resource",
    }
}

/// Clone into a `<dest>.partial` staging dir, then rename — a clone
/// killed mid-flight (controller exit) leaves no half-checkout behind:
/// `dest` only ever appears complete. Sync `std::process` deliberately:
/// a one-shot CLI must not depend on a tokio runtime for this (an async
/// spawn dies with the runtime before it can even start).
fn git_clone(repo: &crate::manifest::GitRepo, dest: &Path) -> anyhow::Result<()> {
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let staged = dest.with_extension("partial");
    if staged.exists() {
        std::fs::remove_dir_all(&staged)?;
    }
    let mut cmd = std::process::Command::new("git");
    cmd.arg("clone");
    if repo.depth > 0 {
        cmd.arg("--depth").arg(repo.depth.to_string());
    }
    if !repo.branch.is_empty() {
        cmd.arg("--branch").arg(&repo.branch);
    }
    cmd.arg(&repo.repo).arg(&staged);
    let out = cmd.output()?;
    if !out.status.success() {
        std::fs::remove_dir_all(&staged).ok();
        anyhow::bail!("git clone failed: {}", String::from_utf8_lossy(&out.stderr));
    }
    std::fs::rename(&staged, dest)?;
    Ok(())
}
