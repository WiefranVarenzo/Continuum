//! Continuum CP3: deterministic, local-first Continuity and Research Core.
//!
//! Repository intelligence, semantic AI, capture, and MCP intentionally live in
//! later checkpoints. This crate owns the stable Continuity foundation and the
//! deterministic Research Space domain introduced in CP3.

mod artifact;
mod error;
mod id;
mod manifest;
mod model;
mod research;
mod store;

pub use error::{CoreError, Result};
pub use id::new_id;
pub use manifest::ProjectManifest;
pub use model::*;
pub use research::*;
pub use store::ContinuityStore;

pub const CORE_SCHEMA_VERSION: u32 = 3;
pub const PROJECT_MANIFEST_VERSION: u32 = 1;
