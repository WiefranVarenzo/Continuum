//! Continuum CP12: deterministic, local-first R&D continuity with immutable
//! semantic Checkpoints, bounded Context Packs, and a permissioned AI interface.
//!
//! This crate owns canonical provenance, bounded graph validation, provider-neutral
//! semantic candidates, renderer-neutral Human Documents, bounded capture
//! provenance, resumable Current Project State, and review-gated MCP access.

mod artifact;
mod capture;
mod code_intelligence;
mod context;
mod development;
mod error;
mod human_document;
mod id;
mod manifest;
mod mcp;
mod model;
mod provenance;
mod research;
mod semantic;
mod store;
mod workspace_content;

pub use capture::*;
pub use code_intelligence::*;
pub use context::*;
pub use development::*;
pub use error::{CoreError, Result};
pub use human_document::*;
pub use id::new_id;
pub use manifest::ProjectManifest;
pub use mcp::*;
pub use model::*;
pub use provenance::*;
pub use research::*;
pub use semantic::*;
pub use store::ContinuityStore;
pub use workspace_content::*;

pub const CORE_SCHEMA_VERSION: u32 = 15;
pub const PROJECT_MANIFEST_VERSION: u32 = 1;
