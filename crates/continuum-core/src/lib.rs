//! Continuum CP5: deterministic, local-first Continuity, Research, Development,
//! and Code Intelligence Core.
//!
//! Semantic AI, capture, full provenance traversal, and MCP intentionally live in
//! later checkpoints. This crate owns the stable Continuity foundation, the
//! deterministic Research Space, and read-only Git-backed Development and Code
//! Intelligence capabilities.

mod artifact;
mod code_intelligence;
mod development;
mod error;
mod id;
mod manifest;
mod model;
mod research;
mod store;

pub use code_intelligence::*;
pub use development::*;
pub use error::{CoreError, Result};
pub use id::new_id;
pub use manifest::ProjectManifest;
pub use model::*;
pub use research::*;
pub use store::ContinuityStore;

pub const CORE_SCHEMA_VERSION: u32 = 5;
pub const PROJECT_MANIFEST_VERSION: u32 = 1;
