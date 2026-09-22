use thiserror::Error;

#[derive(Debug, Error)]
pub enum AxError {
    #[error("manifest parse error: {0}")]
    Manifest(String),

    #[error("manifest validation error: {0}")]
    Validation(String),

    #[error("{kind} {name} not found in atespace '{atespace}'")]
    NotFound {
        kind: &'static str,
        name: String,
        atespace: String,
    },

    #[error("referenced {kind} '{name}' does not exist in atespace '{atespace}'")]
    MissingRef {
        kind: &'static str,
        name: String,
        atespace: String,
    },

    #[error("store error: {0}")]
    Store(String),

    #[error("actor error: {0}")]
    Actor(String),

    #[error("task '{0}' is in phase {1}; this operation requires {2}")]
    InvalidPhase(String, String, &'static str),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type AxResult<T> = Result<T, AxError>;
