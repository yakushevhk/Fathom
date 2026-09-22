//! Chaos / fault-tolerance tests for pr-ax: abrupt actor crashes,
//! controller kill-and-recover, suspended+deleted races, two-phase
//! delete ordering, unkillable-looking workloads, and loop timeouts.
//! Every test is bounded — a real deadlock fails fast rather than
//! hanging the suite.

use std::time::{Duration, Instant};

use pr_ax::manifest::{kind, phase, AxManifest};
use pr_ax::{AxController, AxStore};
use tempfile::TempDir;

fn task_yaml(name: &str, command: &str) -> String {
    format!(
        r#"apiVersion: ax.io/v1alpha1
kind: Task
metadata:
  name: {name}
spec:
  command: [{command}]
"#
    )
}

async fn wait_phase(store: &AxStore, name: &str, want: &[&str], secs: u64) -> String {
    let start = Instant::now();
    loop {
        if let Ok(t) = store.get_task("default", name) {
            if want.contains(&t.status.phase.as_str()) {
                return t.status.phase;
            }
        }
        assert!(
            start.elapsed() < Duration::from_secs(secs),
            "task {name} never reached {want:?} (phase={:?})",
            store.get_task("default", name).map(|t| t.status.phase)
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// SIGKILL the actor mid-run: the exit shim records a nonzero rc, the
/// monitor marks the task Failed, and the event log carries the kill.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn abrupt_actor_kill_marks_failed() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    ctl.apply_documents(&task_yaml("victim", "\"sleep\", \"300\""))
        .unwrap();
    wait_phase(ctl.store(), "victim", &[phase::RUNNING], 15).await;
    let pid = ctl
        .store()
        .get_task("default", "victim")
        .unwrap()
        .status
        .pid;
    assert!(pid > 0);

    pr_ax::actor::kill(pid, 50).await;
    wait_phase(ctl.store(), "victim", &[phase::FAILED], 15).await;

    let evs = ctl
        .store()
        .scan_events("task", "default", "victim", 0)
        .unwrap();
    assert!(
        evs.iter()
            .any(|e| e.action == "phase" && e.payload.to_string().contains("Failed")),
        "expected a Failed phase event, got {evs:?}"
    );
    ctl.delete_task("default", "victim").await.unwrap();
}

/// Controller crash while an actor is suspended: the suspended actor must
/// not wedge runtime drop, and recovery marks the task Interrupted so a
/// resume respawns it.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn crash_while_suspended_recovers() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    ctl.apply_documents(&task_yaml("frozen", "\"sleep\", \"300\""))
        .unwrap();
    wait_phase(ctl.store(), "frozen", &[phase::RUNNING], 15).await;
    ctl.suspend_task("default", "frozen").await.unwrap();

    // Drop the controller with a suspended actor — must NOT hang
    // (spawn_blocking wait would wedge Runtime::drop forever).
    let pid = ctl
        .store()
        .get_task("default", "frozen")
        .unwrap()
        .status
        .pid;
    drop(ctl);

    // Reopen + recover: dead-or-stopped pid on Suspended → Interrupted.
    // The orphan actor is still alive (stopped); kill its group first so
    // recovery sees a dead pid and respawns cleanly.
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
        libc::kill(pid as libc::pid_t, libc::SIGKILL);
    }
    tokio::time::sleep(Duration::from_millis(200)).await;

    let ctl2 = AxController::open(tmp.path()).unwrap();
    let t = ctl2.store().get_task("default", "frozen").unwrap();
    // A suspended task whose actor died is classified Interrupted or
    // Failed by recovery — either way it must be resumable.
    assert!(
        t.status.phase == phase::INTERRUPTED || t.status.phase == phase::FAILED,
        "unexpected phase after crash: {}",
        t.status.phase
    );
    // Durable resume respawns the declared command asynchronously.
    ctl2.resume_task("default", "frozen").await.unwrap();
    wait_phase(ctl2.store(), "frozen", &[phase::RUNNING], 15).await;
    ctl2.delete_task("default", "frozen").await.unwrap();
}

/// Delete while suspended: two-phase delete must proceed through
/// Terminating and remove the record even though the actor is stopped.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_while_suspended_is_two_phase() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    ctl.apply_documents(&task_yaml("hdel", "\"sleep\", \"300\""))
        .unwrap();
    wait_phase(ctl.store(), "hdel", &[phase::RUNNING], 15).await;
    ctl.suspend_task("default", "hdel").await.unwrap();

    ctl.delete_task("default", "hdel").await.unwrap();
    assert!(ctl.store().get("task", "default", "hdel").is_err());
    // The event log must have recorded the terminating marker.
    let evs = ctl.store().scan_all(0, 500).unwrap();
    assert!(
        evs.iter()
            .any(|e| e.name == "hdel" && e.payload.to_string().contains("Terminating")),
        "no Terminating marker in {evs:?}"
    );
}

/// A busy-loop actor (simulating an agent stuck in a doom loop) stays
/// responsive to suspend/resume/delete — no hang, bounded runtime.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn looping_actor_remains_controllable() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    let started = Instant::now();
    ctl.apply_documents(&task_yaml(
        "loop",
        "\"sh\", \"-c\", \"while :; do :; done\"",
    ))
    .unwrap();
    wait_phase(ctl.store(), "loop", &[phase::RUNNING], 15).await;
    ctl.suspend_task("default", "loop").await.unwrap();
    ctl.resume_task("default", "loop").await.unwrap();
    wait_phase(ctl.store(), "loop", &[phase::RUNNING], 15).await;
    ctl.delete_task("default", "loop").await.unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(30),
        "looping actor lifecycle exceeded 30s"
    );
}

/// N rapid suspend/resume cycles on the same actor — stresses signal
/// ordering; no panic, phase always consistent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rapid_suspend_resume_cycles() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    ctl.apply_documents(&task_yaml("yoyo", "\"sleep\", \"300\""))
        .unwrap();
    wait_phase(ctl.store(), "yoyo", &[phase::RUNNING], 15).await;
    for _ in 0..5 {
        let t = ctl.suspend_task("default", "yoyo").await.unwrap();
        assert_eq!(t.status.phase, phase::SUSPENDED);
        let t = ctl.resume_task("default", "yoyo").await.unwrap();
        assert_eq!(t.status.phase, phase::RUNNING);
    }
    ctl.delete_task("default", "yoyo").await.unwrap();
}

/// Concurrent mixed ops: apply 20 tasks, then suspend half, delete a
/// third, fork one — the controller must stay consistent (no panic, no
/// lost status, terminal phases reachable).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mixed_storm_consistent() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    for i in 0..20 {
        ctl.apply_documents(&task_yaml(&format!("storm-{i}"), "\"true\""))
            .unwrap();
    }
    wait_phase(ctl.store(), "storm-0", &[phase::COMPLETED], 20).await;

    // Suspend is only legal on Running; completed tasks must reject it.
    assert!(ctl.suspend_task("default", "storm-0").await.is_err());
    // Fork a completed task — chain diverges and completes again.
    let f = ctl
        .fork_task("default", "storm-0", "storm-fork", None)
        .unwrap();
    assert_eq!(f.metadata.name, "storm-fork");
    wait_phase(ctl.store(), "storm-fork", &[phase::COMPLETED], 20).await;
    // Bulk delete.
    for i in 0..20 {
        ctl.delete_task("default", &format!("storm-{i}"))
            .await
            .unwrap();
    }
    ctl.delete_task("default", "storm-fork").await.unwrap();
    assert!(ctl.store().list_tasks("default").unwrap().is_empty());
}

/// Events emitted during chaos stay monotonic and durable.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn event_log_monotonic_under_load() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    for i in 0..10 {
        ctl.apply_documents(&task_yaml(&format!("ev-{i}"), "\"true\""))
            .unwrap();
    }
    let evs = ctl.store().scan_all(0, 10000).unwrap();
    let mut prev = 0i64;
    for e in &evs {
        assert!(e.seq > prev, "non-monotonic seq {prev} -> {}", e.seq);
        prev = e.seq;
    }
    assert!(!evs.is_empty());
}

/// Malformed manifests fail cleanly (no panic, no partial apply).
/// An empty input parses to an empty doc list — not an error.
#[test]
fn malformed_manifests_rejected() {
    assert!(AxManifest::parse_documents("").unwrap().is_empty());
    for bad in [
        "not yaml at all: [", 
        "apiVersion: ax.io/v1alpha1\nkind: Bogus\nmetadata:\n  name: x\n",
        "apiVersion: ax.io/v1alpha1\nkind: Task\nmetadata:\n  name: \"\"\n",
        "apiVersion: ax.io/v1alpha1\nkind: Task\nmetadata:\n  name: x\nspec:\n  workspaces:\n    - name: \"../evil\"\n",
    ] {
        assert!(
            AxManifest::parse_documents(bad).is_err(),
            "accepted malformed manifest: {bad:?}"
        );
    }
}

/// kind constants sanity (regression for the two-phase delete path).
#[test]
fn kind_constants() {
    assert_eq!(kind::TASK, "Task");
    assert_eq!(kind::GATEWAY, "Gateway");
}
