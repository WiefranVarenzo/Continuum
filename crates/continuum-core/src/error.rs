use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("unsupported schema version {found}; maximum supported is {supported}")]
    UnsupportedSchema { found: u32, supported: u32 },
    #[error("migration checksum mismatch for version {version}")]
    MigrationChecksum { version: u32 },
    #[error("artifact integrity error at {path}: {reason}")]
    ArtifactIntegrity { path: PathBuf, reason: String },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, CoreError>;
