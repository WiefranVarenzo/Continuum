use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::new_id;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Space {
    Research,
    Development,
}

impl Space {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Development => "development",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointScope {
    Research,
    Development,
    Integrated,
    Core,
}

impl CheckpointScope {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Development => "development",
            Self::Integrated => "integrated",
            Self::Core => "core",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OriginKind {
    User,
    Deterministic,
    Import,
    External,
    Legacy,
    AiProposal,
    Unknown,
}

impl OriginKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Deterministic => "deterministic",
            Self::Import => "import",
            Self::External => "external",
            Self::Legacy => "legacy",
            Self::AiProposal => "ai_proposal",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorKind {
    User,
    System,
    Import,
    AiProposal,
}

impl ActorKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
            Self::Import => "import",
            Self::AiProposal => "ai_proposal",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActorRef {
    pub kind: ActorKind,
    pub id: String,
}

impl ActorRef {
    pub fn system(component: impl Into<String>) -> Self {
        Self {
            kind: ActorKind::System,
            id: component.into(),
        }
    }

    pub fn user(id: impl Into<String>) -> Self {
        Self {
            kind: ActorKind::User,
            id: id.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandContext {
    pub command_id: String,
    pub command_type_version: u32,
    pub actor: ActorRef,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub payload_schema_version: u32,
    pub issued_at: String,
}

impl CommandContext {
    pub fn new(actor: ActorRef) -> Self {
        let command_id = new_id();
        Self {
            idempotency_key: command_id.clone(),
            command_id,
            command_type_version: 1,
            actor,
            correlation_id: new_id(),
            causation_id: None,
            payload_schema_version: 1,
            issued_at: Utc::now().to_rfc3339(),
        }
    }

    pub fn system() -> Self {
        Self::new(ActorRef::system("continuum-core"))
    }

    pub fn system_with_id(command_id: impl Into<String>) -> Self {
        let mut context = Self::system();
        let command_id = command_id.into();
        context.command_id = command_id.clone();
        context.idempotency_key = command_id.clone();
        context.correlation_id = command_id;
        context
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewEntity {
    pub entity_type: String,
    pub schema_version: u32,
    pub title: String,
    pub origin: OriginKind,
    #[serde(default)]
    pub metadata: Value,
    #[serde(default)]
    pub data: Value,
}

impl NewEntity {
    pub fn authored(entity_type: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            entity_type: entity_type.into(),
            schema_version: 1,
            title: title.into(),
            origin: OriginKind::User,
            metadata: json!({}),
            data: json!({}),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    pub id: String,
    pub project_id: String,
    pub entity_type: String,
    pub schema_version: i64,
    pub title: String,
    pub status: String,
    pub version: i64,
    pub origin: String,
    pub metadata: Value,
    pub data: Value,
    pub created_at: String,
    pub created_by: String,
    pub updated_at: String,
    pub updated_by: String,
    pub archived_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityUpdate {
    pub title: String,
    pub status: String,
    pub metadata: Value,
    pub data: Value,
    pub expected_version: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipReviewState {
    Unreviewed,
    Accepted,
    Rejected,
}

impl RelationshipReviewState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unreviewed => "unreviewed",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewRelationship {
    pub relation_type: String,
    pub relation_version: u32,
    pub source_entity_id: String,
    pub target_entity_id: String,
    pub origin: OriginKind,
    pub confidence: Option<f64>,
    pub review_state: RelationshipReviewState,
    #[serde(default)]
    pub direct_source_ids: Vec<String>,
    pub supersedes_id: Option<String>,
}

pub trait RelationshipPolicy: Send + Sync {
    fn validate_pair(
        &self,
        source_entity_type: &str,
        relation_type: &str,
        target_entity_type: &str,
    ) -> std::result::Result<(), String>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CoreRelationshipPolicy;

impl RelationshipPolicy for CoreRelationshipPolicy {
    fn validate_pair(
        &self,
        source_entity_type: &str,
        relation_type: &str,
        target_entity_type: &str,
    ) -> std::result::Result<(), String> {
        if source_entity_type.is_empty()
            || relation_type.is_empty()
            || target_entity_type.is_empty()
        {
            return Err("relationship type and both endpoint types are required".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Relationship {
    pub id: String,
    pub project_id: String,
    pub relation_type: String,
    pub relation_version: i64,
    pub source_entity_id: String,
    pub source_entity_type: String,
    pub target_entity_id: String,
    pub target_entity_type: String,
    pub status: String,
    pub origin: String,
    pub actor_id: String,
    pub confidence: Option<f64>,
    pub review_state: String,
    pub state_version: i64,
    pub annotation: String,
    pub direct_source_ids: Vec<String>,
    pub supersedes_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub reviewed_at: Option<String>,
    pub reviewed_by: Option<String>,
    pub retired_at: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactClassification {
    Public,
    Internal,
    Confidential,
    Secret,
    NeverSend,
}

impl ArtifactClassification {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Confidential => "confidential",
            Self::Secret => "secret",
            Self::NeverSend => "never_send",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactAvailability {
    Available,
    Unavailable,
    PurgedPayload,
}

impl ArtifactAvailability {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::PurgedPayload => "purged_payload",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactRecord {
    pub id: String,
    pub project_id: String,
    pub sha256: String,
    pub byte_size: i64,
    pub media_type: String,
    pub relative_path: String,
    pub classification: String,
    pub availability: String,
    pub origin: String,
    pub metadata: Value,
    pub unavailable_reason: Option<String>,
    pub created_at: String,
    pub created_by: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Checkpoint {
    pub id: String,
    pub project_id: String,
    pub scope: String,
    pub ledger_sequence: i64,
    pub summary: Value,
    pub created_at: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Job {
    pub id: String,
    pub project_id: String,
    pub job_type: String,
    pub state: String,
    pub attempts: i64,
    pub max_attempts: i64,
    pub payload_json: String,
    pub progress_current: i64,
    pub progress_total: Option<i64>,
    pub progress_message: Option<String>,
    pub cancellation_requested: bool,
    pub created_by: String,
    pub updated_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutboxMessage {
    pub id: String,
    pub project_id: String,
    pub audit_event_id: String,
    pub topic: String,
    pub payload_json: String,
    pub attempts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub id: String,
    pub project_id: String,
    pub ledger_sequence: i64,
    pub command_id: String,
    pub aggregate_id: Option<String>,
    pub event_type: String,
    pub event_version: i64,
    pub actor_kind: String,
    pub actor_id: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub payload_json: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PageRequest {
    pub limit: u32,
    pub offset: u64,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EntityPage {
    pub items: Vec<Entity>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RelationshipPage {
    pub items: Vec<Relationship>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegrityIssue {
    pub code: String,
    pub path_or_id: String,
    pub guidance: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct IntegrityReport {
    pub checked_artifacts: usize,
    pub issues: Vec<IntegrityIssue>,
}

impl IntegrityReport {
    pub fn is_healthy(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectSummary {
    pub project_id: String,
    pub name: String,
    pub status: String,
    pub ledger_sequence: i64,
}
