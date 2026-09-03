//! Continuum CP4: deterministic, local-first Continuity, Research, and Development Core.
//!
//! Repository intelligence, semantic AI, capture, and MCP intentionally live in
//! later checkpoints. This crate owns the stable Continuity foundation and the
//! deterministic Research Space plus read-only Git-backed Development Space.

mod artifact;
mod development;
mod error;
mod id;
mod manifest;
mod model;
mod research;
mod store;

pub use development::*;
pub use error::{CoreError, Result};
pub use id::new_id;
pub use manifest::ProjectManifest;
pub use model::*;
pub use research::*;
pub use store::ContinuityStore;

pub const CORE_SCHEMA_VERSION: u32 = 4;
pub const PROJECT_MANIFEST_VERSION: u32 = 1;
