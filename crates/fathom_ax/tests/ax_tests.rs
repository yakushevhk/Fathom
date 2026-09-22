//! Integration tests for the pr-ax runtime: manifest parsing/validation,
//! the durable event log, actor lifecycle (spawn → suspend → resume →
//! exit), controller-crash resumption, and execution forking.

use std::sync::Arc;
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

fn multi_doc() -> String {
    r#"apiVersion: ax.io/v1alpha1
kind: Workspace
metadata:
  name: my-ws
spec:
  git:
    - name: repo
      repo: https://github.com/example/repo.git
      depth: 1
---
apiVersion: ax.io/v1alpha1
kind: Gateway
metadata:
  name: my-gw
spec:
  listeners:
    - name: https
      port: 443
      protocol: https
  egress:
    allowlist:
      hosts:
        - host: api.openai.com
          port: 443
---
apiVersion: ax.io/v1alpha1
kind: Model
metadata:
  name: default-model
spec:
  provider: openai
  model: gpt-4o
  secretKey:
    name: OPENAI_API_KEY
  parameters:
    temperature: 0.7
---
apiVersion: ax.io/v1alpha1
kind: Task
metadata:
  name: hello
spec:
  command: ["echo", "hi"]
  workspaces:
    - name: my-ws
      path: work
      goal: "do the thing"
  gateway:
    name: my-gw
"#
    .to_string()
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

// ── Manifest parsing / validation ────────────────────────────────────────

#[test]
fn parses_multi_doc_manifests() {
    let docs = AxManifest::parse_documents(&multi_doc()).unwrap();
    assert_eq!(docs.len(), 4);
    assert_eq!(docs[0].kind(), kind::WORKSPACE);
    assert_eq!(docs[1].kind(), kind::GATEWAY);
    assert_eq!(docs[2].kind(), kind::MODEL);
    assert_eq!(docs[3].kind(), kind::TASK);
    if let AxManifest::Model(m) = &docs[2] {
        assert_eq!(m.spec.provider, "openai");
        assert_eq!(m.spec.secret_key.as_ref().unwrap().name, "OPENAI_API_KEY");
        assert_eq!(m.spec.parameters["temperature"], serde_json::json!(0.7));
    } else {
        panic!("expected Model");
    }
}

#[test]
fn rejects_wrong_api_version_and_unknown_fields() {
    let bad = "apiVersion: v1\nkind: Task\nmetadata:\n  name: x\n";
    assert!(AxManifest::parse_documents(bad).is_err());
    let extra = "apiVersion: ax.io/v1alpha1\nkind: Task\nmetadata:\n  name: x\nspec:\n  bogus: 1\n";
    assert!(AxManifest::parse_documents(extra).is_err());
    let no_name = "apiVersion: ax.io/v1alpha1\nkind: Task\nmetadata:\n  name: \"\"\n";
    assert!(AxManifest::parse_documents(no_name).is_err());
}

#[test]
fn camel_case_roundtrip() {
    let docs = AxManifest::parse_documents(&multi_doc()).unwrap();
    let AxManifest::Gateway(g) = &docs[1] else {
        panic!()
    };
    let yaml = serde_yaml::to_string(&g).unwrap();
    assert!(yaml.contains("apiVersion:"));
    assert!(yaml.contains("creationTimestamp") || !yaml.contains("creation_timestamp"));
}

// ── Store ────────────────────────────────────────────────────────────────

#[test]
fn store_upsert_get_list_delete_and_events() {
    let tmp = TempDir::new().unwrap();
    let store = AxStore::open(tmp.path()).unwrap();
    let docs = AxManifest::parse_documents(&multi_doc()).unwrap();
    for d in &docs {
        store.upsert(d).unwrap();
    }
    // get + list
    assert!(store.get("workspace", "default", "my-ws").is_ok());
    assert_eq!(store.list("task", "").unwrap().len(), 1);
    assert_eq!(store.list("gateway", "default").unwrap().len(), 1);
    // events
    let meta = docs[3].metadata().clone();
    let s1 = store
        .append_event("task", &meta, "upsert", &serde_json::json!({}))
        .unwrap();
    let s2 = store
        .append_event(
            "task",
            &meta,
            "phase",
            &serde_json::json!({"phase":"Running"}),
        )
        .unwrap();
    assert!(s2 > s1);
    let evs = store.scan_events("task", "default", "hello", 0).unwrap();
    assert_eq!(evs.len(), 2);
    assert_eq!(evs[1].payload["phase"], "Running");
    // delete
    assert!(store.delete("task", "default", "hello").unwrap());
    assert!(store.get("task", "default", "hello").is_err());
}

// ── Controller lifecycle ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_task_completes_and_logs() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    let yaml = task_yaml("hello", "\"sh\", \"-c\", \"echo ax-output\"");
    ctl.apply_documents(&yaml).unwrap();
    assert_eq!(
        wait_phase(ctl.store(), "hello", &[phase::COMPLETED], 15).await,
        phase::COMPLETED
    );
    let t = ctl.store().get_task("default", "hello").unwrap();
    let log = std::fs::read_to_string(&t.status.log_path).unwrap();
    assert!(log.contains("ax-output"));
    // Event chain present.
    let evs = ctl
        .store()
        .scan_events("task", "default", "hello", 0)
        .unwrap();
    assert!(evs.iter().any(|e| e.action == "upsert"));
    assert!(evs.iter().any(|e| e.payload["phase"] == "Running"));
    assert!(evs.iter().any(|e| e.payload["phase"] == "Completed"));
    assert!(t.status.last_seq > 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failing_command_marks_failed() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    let yaml = task_yaml("boom", "\"sh\", \"-c\", \"exit 3\"");
    ctl.apply_documents(&yaml).unwrap();
    assert_eq!(
        wait_phase(ctl.store(), "boom", &[phase::FAILED], 15).await,
        phase::FAILED
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suspend_resume_and_delete() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    let yaml = task_yaml("sleeper", "\"sleep\", \"60\"");
    ctl.apply_documents(&yaml).unwrap();
    wait_phase(ctl.store(), "sleeper", &[phase::RUNNING], 15).await;

    let t = ctl.suspend_task("default", "sleeper").await.unwrap();
    assert_eq!(t.status.phase, phase::SUSPENDED);
    // SIGSTOP actually froze the process.
    let pid = t.status.pid;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap();
    let state = stat
        .rsplit(')')
        .next()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_string();
    assert_eq!(state, "T");

    let t = ctl.resume_task("default", "sleeper").await.unwrap();
    assert_eq!(t.status.phase, phase::RUNNING);

    ctl.delete_task("default", "sleeper").await.unwrap();
    assert!(ctl.store().get("task", "default", "sleeper").is_err());
    // Actor process group is dead.
    assert!(!pr_ax::actor::alive(pid));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suspended_manifest_never_spawns() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    let yaml = r#"apiVersion: ax.io/v1alpha1
kind: Task
metadata:
  name: held
spec:
  suspend: true
  command: ["sleep", "60"]
"#;
    ctl.apply_documents(yaml).unwrap();
    wait_phase(ctl.store(), "held", &[phase::SUSPENDED], 15).await;
    let t = ctl.store().get_task("default", "held").unwrap();
    assert_eq!(t.status.pid, 0);
    // resume respawns it.
    ctl.resume_task("default", "held").await.unwrap();
    wait_phase(ctl.store(), "held", &[phase::RUNNING], 15).await;
    ctl.delete_task("default", "held").await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restart_marks_lost_actor_interrupted_and_resume_respawns() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    ctl.apply_documents(&task_yaml("crashy", "\"sleep\", \"60\""))
        .unwrap();
    wait_phase(ctl.store(), "crashy", &[phase::RUNNING], 15).await;
    let pid = ctl
        .store()
        .get_task("default", "crashy")
        .unwrap()
        .status
        .pid;

    // Simulate the crash: kill the actor, wait for the monitor to record
    // the failure, then rewind the stored phase to Running (as it would be
    // if the controller had died before the monitor fired).
    pr_ax::actor::kill(pid, 100).await;
    wait_phase(ctl.store(), "crashy", &[phase::FAILED], 15).await;
    let mut t = ctl.store().get_task("default", "crashy").unwrap();
    t.status.phase = phase::RUNNING.into();
    t.status.pid = pid;
    ctl.store().save_status(&t).unwrap();
    // Drop every reference so the "old controller" is gone.
    drop(ctl);
    tokio::time::sleep(Duration::from_millis(300)).await;

    // Reopen: recovery sees a dead pid on a Running task → Interrupted.
    let ctl2 = AxController::open(tmp.path()).unwrap();
    let t2 = ctl2.store().get_task("default", "crashy").unwrap();
    assert_eq!(t2.status.phase, phase::INTERRUPTED);

    // Durable resume: respawns the command under a new actor id.
    ctl2.resume_task("default", "crashy").await.unwrap();
    wait_phase(ctl2.store(), "crashy", &[phase::RUNNING], 15).await;
    let t3 = ctl2.store().get_task("default", "crashy").unwrap();
    assert_ne!(t3.status.pid, pid);
    assert!(pr_ax::actor::alive(t3.status.pid));
    ctl2.delete_task("default", "crashy").await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fork_creates_sibling_at_seq() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    ctl.apply_documents(&task_yaml("parent", "\"sh\", \"-c\", \"echo parent-run\""))
        .unwrap();
    wait_phase(ctl.store(), "parent", &[phase::COMPLETED], 15).await;
    let src = ctl.store().get_task("default", "parent").unwrap();
    let at_seq = src.status.last_seq;

    let fork = ctl
        .fork_task("default", "parent", "child", Some(at_seq))
        .unwrap();
    assert_eq!(fork.status.forked_from, "default/parent");
    assert_eq!(fork.status.fork_seq, at_seq);
    wait_phase(ctl.store(), "child", &[phase::COMPLETED], 15).await;
    let child = ctl.store().get_task("default", "child").unwrap();
    assert_eq!(child.status.forked_from, "default/parent");
    // Fork event recorded on the child's chain.
    let evs = ctl
        .store()
        .scan_events("task", "default", "child", 0)
        .unwrap();
    assert!(evs.iter().any(|e| e.action == "fork"));
    // And the child really re-ran the command.
    let log = std::fs::read_to_string(&child.status.log_path).unwrap();
    assert!(log.contains("parent-run"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_workspace_ref_fails_cleanly() {
    let tmp = TempDir::new().unwrap();
    let ctl = AxController::open(tmp.path()).unwrap();
    let yaml = r#"apiVersion: ax.io/v1alpha1
kind: Task
metadata:
  name: need-ws
spec:
  command: ["true"]
  workspaces:
    - name: nope
"#;
    ctl.apply_documents(yaml).unwrap();
    wait_phase(ctl.store(), "need-ws", &[phase::FAILED], 15).await;
    let t = ctl.store().get_task("default", "need-ws").unwrap();
    assert!(t
        .status
        .conditions
        .iter()
        .any(|c| c.reason == "WorkspaceNotFound"));
}

// Ensure Arc'd controller is Send+Sync (single-writer across threads).
#[test]
fn controller_is_send_sync() {
    fn assert<T: Send + Sync>() {}
    assert::<Arc<AxController>>();
    assert::<AxStore>();
}
