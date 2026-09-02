//! Continuum CP2: deterministic, local-first Continuity Core.
//!
//! Research workflows, repository intelligence, semantic AI, capture, and MCP
//! intentionally live in later checkpoints. This crate owns the stable identity,
//! persistence, artifact, audit, job, capability, and checkpoint foundations.

mod artifact;
mod error;
mod id;
mod manifest;
mod model;
mod store;

pub use error::{CoreError, Result};
pub use id::new_id;
pub use manifest::ProjectManifest;
pub use model::*;
pub use store::ContinuityStore;

pub const CORE_SCHEMA_VERSION: u32 = 2;
pub const PROJECT_MANIFEST_VERSION: u32 = 1;
