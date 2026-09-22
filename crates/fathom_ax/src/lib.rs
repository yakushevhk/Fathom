//! pr-ax — a standalone Rust implementation of the google/ax runtime model
//! for Fathom: declarative `ax.io/v1alpha1` manifests (Task/Gateway/
//! Workspace/Model), a single-writer controller over a durable SQLite/WAL
//! event log, and process-isolated task actors with suspend/resume,
//! resumption after controller restarts, and execution forking at event
//! sequence numbers.
//!
//! See `crates/fathom_ax/README.md` for the architecture and CLI usage.

pub mod actor;
pub mod bench;
pub mod controller;
pub mod error;
pub mod manifest;
pub mod ops;
pub mod store;

pub use controller::{AxController, TaskEvent};
pub use error::{AxError, AxResult};
pub use manifest::{
    kind, phase, AxManifest, Condition, Gateway, Model, ObjectMeta, Task, TaskSpec, TaskStatus,
    Workspace, API_VERSION, DEFAULT_ATESPACE,
};
pub use store::{AxStore, Event};
