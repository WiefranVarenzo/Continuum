use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::development::inspect_live_state;
use crate::store::{
    append_event_with_context, bounded_json, prior_result, record_command_with_context,
    validate_command_context, validate_nonempty,
};
use crate::{
    Checkpoint, CheckpointScope, CommandContext, ContinuityStore, CoreError, DataClassification,
    IntegrityIssue, IntegrityReport, PageRequest, Result, new_id,
};

pub const CONTEXT_CONTRACT_VERSION: u32 = 1;
pub const CONTEXT_PRIVACY_POLICY_VERSION: &str = "context-privacy-v1";
const MAX_CHECKPOINT_SOURCES: usize = 2_000;
const MAX_CHECKPOINT_TEXT_CHARS: usize = 20_000;
const MAX_CONTEXT_ITEMS: usize = 500;
const MAX_CONTEXT_ITEM_BYTES: usize = 64 * 1024;
const MAX_CONTEXT_PACK_BYTES: usize = 4 * 1024 * 1024;
const MAX_CONTEXT_TOKENS: u32 = 128_000;
const DEFAULT_SOFT_TOKENS: u32 = 16_000;
const DEFAULT_HARD_TOKENS: u32 = 32_000;
const DEFAULT_BYTE_BUDGET: u32 = 512 * 1024;
const MAX_COMPARISON_EVENTS: usize = 500;

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($variant),+ }

        impl $name {
            pub fn as_str(self) -> &'static str {
                match self { $(Self::$variant => $value),+ }
            }
        }
    };
}

string_enum!(CheckpointTrigger {
    Manual => "manual",
    Interruption => "interruption",
    Handoff => "handoff",
    Milestone => "milestone",
    Policy => "policy",
});

string_enum!(ContextAudience {
    LocalUser => "local_user",
    ExternalAi => "external_ai",
    PublicPortable => "public_portable",
});

string_enum!(FreshnessRequirement {
    Current => "current",
    AllowStaleWithWarning => "allow_stale_with_warning",
});

string_enum!(RetrievalProfile {
    Resume => "resume",
    Research => "research",
    Development => "development",
    Integrated => "integrated",
    Custom => "custom",
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SemanticCheckpointInput {
    pub scope: CheckpointScope,
    pub trigger: CheckpointTrigger,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
    pub semantic_candidate_id: Option<String>,
    pub supersedes_checkpoint_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckpointEnvelope {
    pub checkpoint: Checkpoint,
    pub schema_version: u32,
    pub trigger: String,
    pub deterministic: Value,
    pub semantic_candidate_id: Option<String>,
    pub semantic_snapshot: Option<Value>,
    pub privacy_policy_version: String,
    pub privacy_snapshot: Value,
    pub repository_snapshot: Value,
    pub capture_snapshot: Value,
    pub material_fingerprint: String,
    pub supersedes_checkpoint_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CheckpointFreshness {
    pub checkpoint_id: String,
    pub fresh: bool,
    pub evaluated_at_ledger_sequence: i64,
    pub events_since_checkpoint: u64,
    pub relevant_events_truncated: bool,
    pub changed_source_ids: Vec<String>,
    pub missing_source_ids: Vec<String>,
    pub changed_artifact_ids: Vec<String>,
    pub repository_diverged_ids: Vec<String>,
    pub repository_unavailable_ids: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckpointEventDelta {
    pub ledger_sequence: i64,
    pub event_type: String,
    pub aggregate_id: Option<String>,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckpointComparison {
    pub older_checkpoint_id: String,
    pub newer_checkpoint_id: String,
    pub older_ledger_sequence: i64,
    pub newer_ledger_sequence: i64,
    pub added_source_ids: Vec<String>,
    pub removed_source_ids: Vec<String>,
    pub changed_source_ids: Vec<String>,
    pub intervening_events: Vec<CheckpointEventDelta>,
    pub events_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CurrentProjectStateRequest {
    pub scope: CheckpointScope,
    pub checkpoint_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CurrentProjectState {
    pub project_id: String,
    pub scope: String,
    pub as_of_ledger_sequence: i64,
    pub computed_at: String,
    pub checkpoint: Option<CheckpointEnvelope>,
    pub freshness: Option<CheckpointFreshness>,
    pub then: Value,
    pub since: Vec<CheckpointEventDelta>,
    pub since_truncated: bool,
    pub now: Value,
    pub next_actions: Vec<String>,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextBudget {
    pub soft_tokens: u32,
    pub hard_tokens: u32,
    pub max_bytes: u32,
    pub max_items: u32,
    pub max_item_bytes: u32,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            soft_tokens: DEFAULT_SOFT_TOKENS,
            hard_tokens: DEFAULT_HARD_TOKENS,
            max_bytes: DEFAULT_BYTE_BUDGET,
            max_items: 200,
            max_item_bytes: 16 * 1024,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextPackRequest {
    pub task: String,
    pub audience: ContextAudience,
    pub consumer_target: String,
    pub scope: CheckpointScope,
    pub checkpoint_id: Option<String>,
    pub freshness_requirement: FreshnessRequirement,
    pub retrieval_profile: RetrievalProfile,
    #[serde(default)]
    pub root_entity_ids: Vec<String>,
    #[serde(default)]
    pub exclude_source_ids: Vec<String>,
    pub include_artifact_content: bool,
    pub budget: ContextBudget,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextPackItem {
    pub ordinal: u32,
    pub tier: u8,
    pub source_kind: String,
    pub source_id: String,
    pub source_version: String,
    pub classification: DataClassification,
    pub relevance_basis: Vec<String>,
    pub content: Value,
    pub content_bytes: u32,
    pub estimated_tokens: u32,
    pub content_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContextOmission {
    pub source_id: Option<String>,
    pub reason: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ContextPack {
    pub id: String,
    pub schema_version: u32,
    pub project_id: String,
    pub scope: String,
    pub task: String,
    pub audience: String,
    pub consumer_target: String,
    pub freshness_requirement: String,
    pub retrieval_profile: String,
    pub checkpoint_id: Option<String>,
    pub source_ledger_sequence: i64,
    pub generated_at: String,
    pub freshness: Option<CheckpointFreshness>,
    pub privacy_policy_version: String,
    pub privacy_summary: Value,
    pub budget: ContextBudget,
    pub included_bytes: u32,
    pub estimated_tokens: u32,
    pub estimator_id: String,
    pub estimator_uncertainty: String,
    pub items: Vec<ContextPackItem>,
    pub omissions: Vec<ContextOmission>,
    pub unavailable_sources: Vec<String>,
    pub request_fingerprint: String,
    pub content_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SavedContextPack {
    pub pack: ContextPack,
    pub generated_artifact_id: String,
    pub created_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CheckpointEnvelopePage {
    pub items: Vec<CheckpointEnvelope>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedContextPackSummary {
    pub id: String,
    pub scope: String,
    pub checkpoint_id: Option<String>,
    pub source_ledger_sequence: i64,
    pub task: String,
    pub audience: String,
    pub included_bytes: u32,
    pub estimated_tokens: u32,
    pub content_fingerprint: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SavedContextPackPage {
    pub items: Vec<SavedContextPackSummary>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone)]
struct ScopedEntity {
    id: String,
    entity_type: String,
    title: String,
    status: String,
    version: i64,
    metadata: Value,
    data: Value,
    updated_at: String,
    created_in_space: Option<String>,
    classification: DataClassification,
}

#[derive(Debug, Clone)]
struct CandidateItem {
    tier: u8,
    score: i64,
    source_kind: String,
    source_id: String,
    source_version: String,
    classification: DataClassification,
    relevance_basis: Vec<String>,
    content: Value,
}

#[derive(Debug, Clone, Serialize)]
struct ArtifactSnapshot {
    id: String,
    sha256: String,
    availability: String,
    classification: String,
}

impl ContinuityStore {
    pub fn create_semantic_checkpoint(
        &self,
        command: &CommandContext,
        input: SemanticCheckpointInput,
    ) -> Result<CheckpointEnvelope> {
        validate_command_context(command)?;
        validate_checkpoint_input(&input)?;
        let mut connection = self.connection()?;
        require_checkpoint_scope(&connection, &self.manifest().project_id, input.scope)?;

        let semantic_snapshot = if let Some(candidate_id) = &input.semantic_candidate_id {
            let freshness = self.semantic_candidate_freshness(candidate_id)?;
            if !freshness.fresh {
                return Err(CoreError::Conflict(
                    "stale AI candidate cannot be attached to a Checkpoint".into(),
                ));
            }
            Some(load_reviewed_semantic_candidate(
                &connection,
                &self.manifest().project_id,
                candidate_id,
            )?)
        } else {
            None
        };

        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "CreateSemanticCheckpoint",
        )? {
            tx.commit()?;
            return self.get_checkpoint_envelope(&id);
        }
        if let Some(supersedes) = &input.supersedes_checkpoint_id {
            let prior_scope: Option<String> = tx
                .query_row(
                    "SELECT scope FROM checkpoints WHERE id=?1 AND project_id=?2",
                    params![supersedes, self.manifest().project_id],
                    |row| row.get(0),
                )
                .optional()?;
            let prior_scope = prior_scope
                .ok_or_else(|| CoreError::Validation("superseded checkpoint is invalid".into()))?;
            if prior_scope != input.scope.as_str() {
                return Err(CoreError::Validation(
                    "a semantic checkpoint can only supersede the same scope".into(),
                ));
            }
        }

        let entities = load_scoped_entities(&tx, &self.manifest().project_id, input.scope)?;
        if entities.len() > MAX_CHECKPOINT_SOURCES {
            return Err(CoreError::Conflict(format!(
                "checkpoint source set has {} entities; narrow or archive state before exceeding {MAX_CHECKPOINT_SOURCES}",
                entities.len()
            )));
        }
        let source_ids = entities
            .iter()
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        let artifact_sources = load_linked_artifact_snapshots(
            &tx,
            &self.manifest().project_id,
            &source_ids,
            MAX_CHECKPOINT_SOURCES,
        )?;
        let sequence: i64 = tx.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        let repository_snapshot =
            repository_snapshot(&tx, &self.manifest().project_id, input.scope)?;
        let capture_snapshot = capture_snapshot(&tx, &self.manifest().project_id, input.scope)?;
        let deterministic = deterministic_checkpoint_state(
            &entities,
            &input,
            sequence,
            &repository_snapshot,
            &capture_snapshot,
        );
        let privacy_snapshot = checkpoint_privacy_snapshot(&entities, &artifact_sources);
        let summary = json!({
            "schema_version": CONTEXT_CONTRACT_VERSION,
            "scope": input.scope.as_str(),
            "source_ledger_sequence": sequence,
            "note": input.note,
            "blockers": input.blockers,
            "risks": input.risks,
            "next_actions": input.next_actions,
            "deterministic_state": deterministic,
            "semantic_candidate_id": input.semantic_candidate_id,
            "complete": false
        });
        let summary_json = bounded_json(&summary, 2 * 1024 * 1024, "checkpoint summary")?;
        let material_fingerprint = hash_value(&json!({
            "contract_version": CONTEXT_CONTRACT_VERSION,
            "project_id": self.manifest().project_id,
            "scope": input.scope.as_str(),
            "ledger_sequence": sequence,
            "sources": entities.iter().map(|item| (&item.id,item.version)).collect::<Vec<_>>(),
            "artifacts": artifact_sources,
            "repository": repository_snapshot,
            "capture": capture_snapshot,
            "privacy": privacy_snapshot,
            "semantic": semantic_snapshot
        }))?;
        let checkpoint_id = new_id();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO checkpoints(id,project_id,scope,ledger_sequence,summary_json,created_at,created_by)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                checkpoint_id,
                self.manifest().project_id,
                input.scope.as_str(),
                sequence,
                summary_json,
                now,
                command.actor.id
            ],
        )?;
        for entity in &entities {
            tx.execute(
                "INSERT INTO checkpoint_sources(checkpoint_id,source_entity_id,source_version)
                 VALUES(?1,?2,?3)",
                params![checkpoint_id, entity.id, entity.version],
            )?;
        }
        for artifact in &artifact_sources {
            tx.execute(
                "INSERT INTO checkpoint_artifact_sources(
                    checkpoint_id,artifact_id,sha256,availability,classification
                 ) VALUES(?1,?2,?3,?4,?5)",
                params![
                    checkpoint_id,
                    artifact.id,
                    artifact.sha256,
                    artifact.availability,
                    artifact.classification
                ],
            )?;
        }
        tx.execute(
            "INSERT INTO checkpoint_envelopes(
                checkpoint_id,project_id,schema_version,trigger_type,deterministic_json,
                semantic_candidate_id,semantic_snapshot_json,privacy_policy_version,
                privacy_snapshot_json,repository_snapshot_json,capture_snapshot_json,
                material_fingerprint,supersedes_checkpoint_id,created_at,created_by
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
            params![
                checkpoint_id,
                self.manifest().project_id,
                CONTEXT_CONTRACT_VERSION,
                input.trigger.as_str(),
                bounded_json(&deterministic, 2 * 1024 * 1024, "deterministic checkpoint")?,
                input.semantic_candidate_id,
                semantic_snapshot
                    .as_ref()
                    .map(|value| bounded_json(value, 2 * 1024 * 1024, "semantic snapshot"))
                    .transpose()?,
                CONTEXT_PRIVACY_POLICY_VERSION,
                bounded_json(&privacy_snapshot, 256 * 1024, "checkpoint privacy snapshot")?,
                bounded_json(&repository_snapshot, 512 * 1024, "repository snapshot")?,
                bounded_json(&capture_snapshot, 512 * 1024, "capture snapshot")?,
                material_fingerprint,
                input.supersedes_checkpoint_id,
                now,
                command.actor.id
            ],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&checkpoint_id),
            "context.checkpoint.created",
            &json!({
                "checkpoint_id": checkpoint_id,
                "scope": input.scope.as_str(),
                "source_ledger_sequence": sequence,
                "source_count": entities.len(),
                "artifact_source_count": artifact_sources.len(),
                "trigger": input.trigger.as_str()
            }),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "CreateSemanticCheckpoint",
            Some(&checkpoint_id),
            None,
            &json!({
                "scope": input.scope.as_str(),
                "trigger": input.trigger.as_str(),
                "source_count": entities.len(),
                "artifact_source_count": artifact_sources.len()
            }),
        )?;
        tx.commit()?;
        self.get_checkpoint_envelope(&checkpoint_id)
    }

    pub fn get_checkpoint_envelope(&self, id: &str) -> Result<CheckpointEnvelope> {
        let checkpoint = self.get_checkpoint(id)?;
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT schema_version,trigger_type,deterministic_json,semantic_candidate_id,
                        semantic_snapshot_json,privacy_policy_version,privacy_snapshot_json,
                        repository_snapshot_json,capture_snapshot_json,material_fingerprint,
                        supersedes_checkpoint_id
                 FROM checkpoint_envelopes WHERE checkpoint_id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<String>>(10)?,
                    ))
                },
            )
            .optional()?;
        if let Some(raw) = raw {
            Ok(CheckpointEnvelope {
                checkpoint,
                schema_version: raw.0,
                trigger: raw.1,
                deterministic: serde_json::from_str(&raw.2)?,
                semantic_candidate_id: raw.3,
                semantic_snapshot: raw
                    .4
                    .map(|value| serde_json::from_str(&value))
                    .transpose()?,
                privacy_policy_version: raw.5,
                privacy_snapshot: serde_json::from_str(&raw.6)?,
                repository_snapshot: serde_json::from_str(&raw.7)?,
                capture_snapshot: serde_json::from_str(&raw.8)?,
                material_fingerprint: raw.9,
                supersedes_checkpoint_id: raw.10,
            })
        } else {
            Ok(legacy_checkpoint_envelope(checkpoint)?)
        }
    }

    pub fn list_checkpoint_envelopes(
        &self,
        scope: Option<CheckpointScope>,
        page: PageRequest,
    ) -> Result<CheckpointEnvelopePage> {
        validate_page(page, "checkpoint")?;
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("checkpoint page offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id FROM checkpoints WHERE project_id=?1
             AND (?2 IS NULL OR scope=?2)
             ORDER BY ledger_sequence DESC,created_at DESC,id DESC LIMIT ?3 OFFSET ?4",
        )?;
        let ids = statement
            .query_map(
                params![
                    self.manifest().project_id,
                    scope.map(|value| value.as_str()),
                    page.limit + 1,
                    offset
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = ids.len() > page.limit as usize;
        let items = ids
            .into_iter()
            .take(page.limit as usize)
            .map(|id| self.get_checkpoint_envelope(&id))
            .collect::<Result<Vec<_>>>()?;
        Ok(CheckpointEnvelopePage {
            next_offset: has_more.then_some(page.offset + items.len() as u64),
            items,
        })
    }

    pub fn checkpoint_freshness(&self, checkpoint_id: &str) -> Result<CheckpointFreshness> {
        let envelope = self.get_checkpoint_envelope(checkpoint_id)?;
        let connection = self.connection()?;
        checkpoint_freshness_with_connection(&connection, &self.manifest().project_id, &envelope)
    }

    pub fn compare_checkpoints(
        &self,
        first_checkpoint_id: &str,
        second_checkpoint_id: &str,
    ) -> Result<CheckpointComparison> {
        let first = self.get_checkpoint(first_checkpoint_id)?;
        let second = self.get_checkpoint(second_checkpoint_id)?;
        if first.scope != second.scope {
            return Err(CoreError::Validation(
                "Checkpoint comparison requires the same scope".into(),
            ));
        }
        let (older, newer) =
            if (first.ledger_sequence, &first.id) <= (second.ledger_sequence, &second.id) {
                (first, second)
            } else {
                (second, first)
            };
        let connection = self.connection()?;
        let older_sources = checkpoint_source_versions(&connection, &older.id)?;
        let newer_sources = checkpoint_source_versions(&connection, &newer.id)?;
        let older_ids = older_sources.keys().cloned().collect::<BTreeSet<_>>();
        let newer_ids = newer_sources.keys().cloned().collect::<BTreeSet<_>>();
        let added_source_ids = newer_ids.difference(&older_ids).cloned().collect();
        let removed_source_ids = older_ids.difference(&newer_ids).cloned().collect();
        let changed_source_ids = older_ids
            .intersection(&newer_ids)
            .filter(|id| older_sources.get(*id) != newer_sources.get(*id))
            .cloned()
            .collect();
        let (intervening_events, events_truncated) = relevant_event_delta(
            &connection,
            &self.manifest().project_id,
            older.ledger_sequence,
            newer.ledger_sequence,
            parse_checkpoint_scope(&older.scope)?,
            MAX_COMPARISON_EVENTS,
        )?;
        Ok(CheckpointComparison {
            older_checkpoint_id: older.id,
            newer_checkpoint_id: newer.id,
            older_ledger_sequence: older.ledger_sequence,
            newer_ledger_sequence: newer.ledger_sequence,
            added_source_ids,
            removed_source_ids,
            changed_source_ids,
            intervening_events,
            events_truncated,
        })
    }

    pub fn current_project_state(
        &self,
        request: CurrentProjectStateRequest,
    ) -> Result<CurrentProjectState> {
        let connection = self.connection()?;
        require_checkpoint_scope(&connection, &self.manifest().project_id, request.scope)?;
        let checkpoint = if let Some(id) = &request.checkpoint_id {
            let envelope = self.get_checkpoint_envelope(id)?;
            if envelope.checkpoint.scope != request.scope.as_str() {
                return Err(CoreError::Validation(
                    "requested checkpoint does not match Current Project State scope".into(),
                ));
            }
            Some(envelope)
        } else {
            let latest: Option<String> = connection
                .query_row(
                    "SELECT id FROM checkpoints WHERE project_id=?1 AND scope=?2
                     ORDER BY ledger_sequence DESC,created_at DESC,id DESC LIMIT 1",
                    params![self.manifest().project_id, request.scope.as_str()],
                    |row| row.get(0),
                )
                .optional()?;
            latest
                .as_deref()
                .map(|id| self.get_checkpoint_envelope(id))
                .transpose()?
        };
        let current_sequence: i64 = connection.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        let entities =
            load_scoped_entities(&connection, &self.manifest().project_id, request.scope)?;
        let repository =
            repository_snapshot(&connection, &self.manifest().project_id, request.scope)?;
        let capture = capture_snapshot(&connection, &self.manifest().project_id, request.scope)?;
        let now_state = deterministic_current_state(&entities, &repository, &capture);
        let (freshness, since, since_truncated, then, next_actions) =
            if let Some(envelope) = &checkpoint {
                let freshness = checkpoint_freshness_with_connection(
                    &connection,
                    &self.manifest().project_id,
                    envelope,
                )?;
                let (events, truncated) = relevant_event_delta(
                    &connection,
                    &self.manifest().project_id,
                    envelope.checkpoint.ledger_sequence,
                    current_sequence,
                    request.scope,
                    100,
                )?;
                (
                    Some(freshness),
                    events,
                    truncated,
                    envelope.deterministic.clone(),
                    json_string_list(&envelope.checkpoint.summary, "next_actions"),
                )
            } else {
                (None, Vec::new(), false, json!(null), Vec::new())
            };
        Ok(CurrentProjectState {
            project_id: self.manifest().project_id.clone(),
            scope: request.scope.as_str().into(),
            as_of_ledger_sequence: current_sequence,
            computed_at: Utc::now().to_rfc3339(),
            checkpoint,
            freshness,
            then,
            since,
            since_truncated,
            now: now_state,
            next_actions,
            source_ids: entities.into_iter().map(|entity| entity.id).collect(),
        })
    }

    pub fn build_context_pack(&self, request: ContextPackRequest) -> Result<ContextPack> {
        validate_context_request(&request)?;
        let connection = self.connection()?;
        require_checkpoint_scope(&connection, &self.manifest().project_id, request.scope)?;
        let state = self.current_project_state(CurrentProjectStateRequest {
            scope: request.scope,
            checkpoint_id: request.checkpoint_id.clone(),
        })?;
        if request.freshness_requirement == FreshnessRequirement::Current
            && state
                .freshness
                .as_ref()
                .is_some_and(|freshness| !freshness.fresh)
        {
            return Err(CoreError::Conflict(
                "the selected checkpoint is stale; request current state or explicitly allow stale context with a warning"
                    .into(),
            ));
        }

        let entities =
            load_scoped_entities(&connection, &self.manifest().project_id, request.scope)?;
        let entity_by_id = entities
            .iter()
            .map(|entity| (entity.id.as_str(), entity))
            .collect::<HashMap<_, _>>();
        for root in &request.root_entity_ids {
            let entity = entity_by_id.get(root.as_str()).ok_or_else(|| {
                CoreError::Validation(format!("context root {root} is invalid for this scope"))
            })?;
            if request.exclude_source_ids.contains(root) {
                return Err(CoreError::Validation(format!(
                    "context root {root} cannot also be excluded"
                )));
            }
            if !audience_allows(request.audience, entity.classification)
                || external_secret_scan_blocks(request.audience, entity)
            {
                return Err(CoreError::Validation(format!(
                    "explicit context root {root} is denied by the audience privacy policy"
                )));
            }
        }

        let checkpoint_sources = state
            .checkpoint
            .as_ref()
            .map(|value| checkpoint_source_versions(&connection, &value.checkpoint.id))
            .transpose()?
            .unwrap_or_default();
        let task_terms = normalized_terms(&request.task);
        let roots = request
            .root_entity_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let excluded = request
            .exclude_source_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let mut omissions = Vec::new();
        let mut candidates = Vec::new();

        let (state_content, state_classification) =
            if request.audience == ContextAudience::PublicPortable {
                (
                    public_project_state_content(&state, &entities),
                    DataClassification::Public,
                )
            } else {
                (
                    filtered_project_state_content(&state, &entities, request.audience),
                    DataClassification::Internal,
                )
            };
        candidates.push(CandidateItem {
            tier: 1,
            score: i64::MAX,
            source_kind: "project_state".into(),
            source_id: self.manifest().project_id.clone(),
            source_version: state.as_of_ledger_sequence.to_string(),
            classification: state_classification,
            relevance_basis: vec!["current_project_state".into()],
            content: state_content,
        });
        if let Some(checkpoint) = &state.checkpoint {
            let checkpoint_classification =
                highest_entity_classification_for_ids(&entities, checkpoint_sources.keys());
            let checkpoint_secret_blocked = request.audience != ContextAudience::LocalUser
                && entities.iter().any(|entity| {
                    checkpoint_sources.contains_key(&entity.id)
                        && external_secret_scan_blocks(request.audience, entity)
                });
            if audience_allows(request.audience, checkpoint_classification)
                && !checkpoint_secret_blocked
            {
                candidates.push(CandidateItem {
                    tier: 1,
                    score: i64::MAX - 1,
                    source_kind: "checkpoint".into(),
                    source_id: checkpoint.checkpoint.id.clone(),
                    source_version: checkpoint.material_fingerprint.clone(),
                    classification: checkpoint_classification,
                    relevance_basis: vec!["selected_checkpoint".into()],
                    content: safe_checkpoint_content(checkpoint, request.audience, &entities),
                });
            } else {
                omissions.push(ContextOmission {
                    source_id: Some(checkpoint.checkpoint.id.clone()),
                    reason: "privacy_policy".into(),
                    detail: "Checkpoint narrative was withheld because one or more sources exceed the requested audience.".into(),
                });
            }
        }

        for entity in &entities {
            if excluded.contains(&entity.id) {
                omissions.push(ContextOmission {
                    source_id: Some(entity.id.clone()),
                    reason: "user_excluded".into(),
                    detail: "Source was removed in the Context Pack preview request.".into(),
                });
                continue;
            }
            if !audience_allows(request.audience, entity.classification) {
                omissions.push(ContextOmission {
                    source_id: Some(entity.id.clone()),
                    reason: "privacy_policy".into(),
                    detail: format!(
                        "{} content is not permitted for {}.",
                        entity.classification.as_str(),
                        request.audience.as_str()
                    ),
                });
                continue;
            }
            if external_secret_scan_blocks(request.audience, entity) {
                omissions.push(ContextOmission {
                    source_id: Some(entity.id.clone()),
                    reason: "secret_pattern_detected".into(),
                    detail: "Potential credential material was excluded before external context composition."
                        .into(),
                });
                continue;
            }
            let explicit = roots.contains(&entity.id);
            let checkpoint_source = checkpoint_sources.contains_key(&entity.id);
            let term_matches = relevance_matches(entity, &task_terms);
            let active = entity_is_active(entity);
            if !explicit && !checkpoint_source && term_matches == 0 && !active {
                continue;
            }
            let mut basis = Vec::new();
            if explicit {
                basis.push("explicit_root".into());
            }
            if checkpoint_source {
                basis.push("checkpoint_source".into());
            }
            if term_matches > 0 {
                basis.push(format!("task_terms:{term_matches}"));
            }
            if active {
                basis.push("active_or_unresolved".into());
            }
            let tier = if explicit || checkpoint_source { 2 } else { 3 };
            let score = (if explicit { 1_000_000 } else { 0 })
                + (if checkpoint_source { 100_000 } else { 0 })
                + i64::from(term_matches) * 1_000
                + if active { 100 } else { 0 };
            candidates.push(CandidateItem {
                tier,
                score,
                source_kind: "entity".into(),
                source_id: entity.id.clone(),
                source_version: entity.version.to_string(),
                classification: entity.classification,
                relevance_basis: basis,
                content: entity_context_content(entity, request.budget.max_item_bytes as usize),
            });
        }

        let selected_entity_ids = candidates
            .iter()
            .filter(|item| item.source_kind == "entity")
            .map(|item| item.source_id.clone())
            .collect::<HashSet<_>>();
        candidates.extend(relationship_candidates(
            &connection,
            &self.manifest().project_id,
            &selected_entity_ids,
            &excluded,
        )?);
        if request.include_artifact_content {
            candidates.extend(artifact_candidates(
                self,
                &connection,
                &selected_entity_ids,
                request.audience,
                request.budget.max_item_bytes as usize,
                &excluded,
                &mut omissions,
            )?);
        }

        candidates.sort_by_key(|item| {
            (
                item.tier,
                Reverse(item.score),
                item.source_kind.clone(),
                item.source_id.clone(),
            )
        });
        candidates.dedup_by(|left, right| {
            left.source_kind == right.source_kind && left.source_id == right.source_id
        });
        let mut items = Vec::new();
        let mut included_bytes = 0_u32;
        let mut estimated_tokens = 0_u32;
        for mut candidate in candidates {
            if !audience_allows(request.audience, candidate.classification) {
                omissions.push(ContextOmission {
                    source_id: Some(candidate.source_id),
                    reason: "privacy_policy".into(),
                    detail: format!(
                        "{} source is not permitted for {}.",
                        candidate.classification.as_str(),
                        request.audience.as_str()
                    ),
                });
                continue;
            }
            if request.audience != ContextAudience::LocalUser
                && serde_json::to_string(&candidate.content)
                    .is_ok_and(|value| has_secret_marker(&value))
            {
                omissions.push(ContextOmission {
                    source_id: Some(candidate.source_id),
                    reason: "secret_pattern_detected".into(),
                    detail: "Potential credential material was excluded before external context composition."
                        .into(),
                });
                continue;
            }
            if items.len() >= request.budget.max_items as usize {
                omissions.push(ContextOmission {
                    source_id: Some(candidate.source_id),
                    reason: "item_budget".into(),
                    detail: "The maximum Context Pack item count was reached.".into(),
                });
                continue;
            }
            candidate.content =
                fit_context_content(candidate.content, request.budget.max_item_bytes as usize)?;
            let content_json = bounded_json(
                &candidate.content,
                request.budget.max_item_bytes as usize,
                "context item",
            )?;
            let bytes = u32::try_from(content_json.len())
                .map_err(|_| CoreError::Validation("context item size overflow".into()))?;
            let tokens = conservative_token_estimate(bytes);
            let over_hard = included_bytes.saturating_add(bytes) > request.budget.max_bytes
                || estimated_tokens.saturating_add(tokens) > request.budget.hard_tokens;
            let over_soft = estimated_tokens.saturating_add(tokens) > request.budget.soft_tokens;
            if over_hard || (over_soft && candidate.tier > 2) {
                omissions.push(ContextOmission {
                    source_id: Some(candidate.source_id),
                    reason: if over_hard {
                        "hard_budget".into()
                    } else {
                        "soft_budget".into()
                    },
                    detail: "Source remains available through progressive retrieval.".into(),
                });
                continue;
            }
            included_bytes = included_bytes.saturating_add(bytes);
            estimated_tokens = estimated_tokens.saturating_add(tokens);
            items.push(ContextPackItem {
                ordinal: items.len() as u32,
                tier: candidate.tier,
                source_kind: candidate.source_kind,
                source_id: candidate.source_id,
                source_version: candidate.source_version,
                classification: candidate.classification,
                relevance_basis: candidate.relevance_basis,
                content: candidate.content,
                content_bytes: bytes,
                estimated_tokens: tokens,
                content_fingerprint: hash_bytes(content_json.as_bytes()),
            });
        }
        if items.is_empty() {
            return Err(CoreError::Conflict(
                "the Context Pack budget is too small for required project identity".into(),
            ));
        }
        let request_fingerprint = context_request_fingerprint(
            &request,
            state.as_of_ledger_sequence,
            state
                .checkpoint
                .as_ref()
                .map(|value| value.material_fingerprint.as_str()),
        )?;
        let content_fingerprint = hash_value(&json!({
            "contract_version": CONTEXT_CONTRACT_VERSION,
            "request_fingerprint": request_fingerprint,
            "items": items.iter().map(|item| (&item.source_kind,&item.source_id,&item.source_version,&item.content_fingerprint)).collect::<Vec<_>>(),
            "omissions": omissions,
            "ledger_sequence": state.as_of_ledger_sequence
        }))?;
        let privacy_summary = context_privacy_summary(&items, &omissions, request.audience);
        let unavailable_sources = items
            .iter()
            .filter(|item| {
                item.source_kind == "artifact"
                    && item
                        .content
                        .get("availability")
                        .and_then(Value::as_str)
                        .is_some_and(|value| value != "available")
            })
            .map(|item| item.source_id.clone())
            .collect();
        Ok(ContextPack {
            id: new_id(),
            schema_version: CONTEXT_CONTRACT_VERSION,
            project_id: self.manifest().project_id.clone(),
            scope: request.scope.as_str().into(),
            task: request.task,
            audience: request.audience.as_str().into(),
            consumer_target: request.consumer_target,
            freshness_requirement: request.freshness_requirement.as_str().into(),
            retrieval_profile: request.retrieval_profile.as_str().into(),
            checkpoint_id: request.checkpoint_id.or_else(|| {
                state
                    .checkpoint
                    .as_ref()
                    .map(|value| value.checkpoint.id.clone())
            }),
            source_ledger_sequence: state.as_of_ledger_sequence,
            generated_at: Utc::now().to_rfc3339(),
            freshness: state.freshness,
            privacy_policy_version: CONTEXT_PRIVACY_POLICY_VERSION.into(),
            privacy_summary,
            budget: request.budget,
            included_bytes,
            estimated_tokens,
            estimator_id: "utf8_bytes_div_3_ceiling_v1".into(),
            estimator_uncertainty: "medium".into(),
            items,
            omissions,
            unavailable_sources,
            request_fingerprint,
            content_fingerprint,
        })
    }

    pub fn save_context_pack(
        &self,
        command: &CommandContext,
        pack: &ContextPack,
    ) -> Result<SavedContextPack> {
        validate_command_context(command)?;
        validate_materialized_pack(pack, &self.manifest().project_id)?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(Some(id)) =
            prior_result(&tx, &self.manifest().project_id, command, "SaveContextPack")?
        {
            tx.commit()?;
            return self.get_saved_context_pack(&id);
        }
        let current_sequence: i64 = tx.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        if current_sequence != pack.source_ledger_sequence {
            return Err(CoreError::Conflict(
                "project state changed after Context Pack preview; rebuild before saving".into(),
            ));
        }
        if let Some(checkpoint_id) = &pack.checkpoint_id {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM checkpoints WHERE id=?1 AND project_id=?2)",
                params![checkpoint_id, self.manifest().project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(CoreError::Validation(
                    "Context Pack checkpoint is invalid".into(),
                ));
            }
        }
        for item in &pack.items {
            verify_pack_source_snapshot(&tx, &self.manifest().project_id, item)?;
        }
        let content = serde_json::to_value(pack)?;
        let content_json = bounded_json(&content, MAX_CONTEXT_PACK_BYTES, "saved Context Pack")?;
        let budget_json = bounded_json(
            &serde_json::to_value(pack.budget)?,
            64 * 1024,
            "Context Pack budget",
        )?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO context_packs(id,project_id,scope,source_ledger_sequence,budget_json,content_json,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                pack.id,
                self.manifest().project_id,
                pack.scope,
                pack.source_ledger_sequence,
                budget_json,
                content_json,
                now
            ],
        )?;
        tx.execute(
            "INSERT INTO context_pack_records(
                context_pack_id,project_id,checkpoint_id,schema_version,request_fingerprint,
                task,audience,consumer_target,privacy_policy_version,freshness_requirement,
                retrieval_profile,estimator_id,estimator_uncertainty,included_bytes,
                estimated_tokens,content_fingerprint,omissions_json,unavailable_sources_json,
                source_snapshot_json,created_by
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20)",
            params![
                pack.id,
                self.manifest().project_id,
                pack.checkpoint_id,
                pack.schema_version,
                pack.request_fingerprint,
                pack.task,
                pack.audience,
                pack.consumer_target,
                pack.privacy_policy_version,
                pack.freshness_requirement,
                pack.retrieval_profile,
                pack.estimator_id,
                pack.estimator_uncertainty,
                pack.included_bytes,
                pack.estimated_tokens,
                pack.content_fingerprint,
                bounded_json(
                    &serde_json::to_value(&pack.omissions)?,
                    512 * 1024,
                    "Context Pack omissions"
                )?,
                bounded_json(
                    &serde_json::to_value(&pack.unavailable_sources)?,
                    256 * 1024,
                    "unavailable Context Pack sources"
                )?,
                bounded_json(
                    &serde_json::to_value(
                        pack.items
                            .iter()
                            .map(|item| json!({
                                "source_kind":item.source_kind,
                                "source_id":item.source_id,
                                "source_version":item.source_version,
                                "content_fingerprint":item.content_fingerprint
                            }))
                            .collect::<Vec<_>>()
                    )?,
                    1024 * 1024,
                    "Context Pack source snapshot"
                )?,
                command.actor.id
            ],
        )?;
        for item in &pack.items {
            tx.execute(
                "INSERT INTO context_pack_sources(
                    context_pack_id,ordinal,tier,source_kind,source_id,source_version,
                    classification,content_fingerprint
                 ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    pack.id,
                    item.ordinal,
                    item.tier,
                    item.source_kind,
                    item.source_id,
                    item.source_version,
                    item.classification.as_str(),
                    item.content_fingerprint
                ],
            )?;
        }
        let generated_artifact_id = new_id();
        tx.execute(
            "INSERT INTO generated_artifacts(
                id,project_id,kind,lifecycle,source_json,artifact_id,created_at,updated_at
             ) VALUES(?1,?2,'context_pack','current',?3,NULL,?4,?4)",
            params![
                generated_artifact_id,
                self.manifest().project_id,
                bounded_json(
                    &json!({
                        "context_pack_id":pack.id,
                        "content_fingerprint":pack.content_fingerprint,
                        "source_ledger_sequence":pack.source_ledger_sequence,
                        "schema_version":pack.schema_version
                    }),
                    64 * 1024,
                    "GeneratedArtifact source"
                )?,
                now
            ],
        )?;
        tx.execute(
            "INSERT INTO context_pack_generated_artifacts(context_pack_id,generated_artifact_id)
             VALUES(?1,?2)",
            params![pack.id, generated_artifact_id],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&pack.id),
            "context.pack.saved",
            &json!({
                "context_pack_id":pack.id,
                "generated_artifact_id":generated_artifact_id,
                "source_ledger_sequence":pack.source_ledger_sequence,
                "item_count":pack.items.len(),
                "included_bytes":pack.included_bytes,
                "estimated_tokens":pack.estimated_tokens
            }),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "SaveContextPack",
            Some(&pack.id),
            None,
            &json!({
                "content_fingerprint":pack.content_fingerprint,
                "item_count":pack.items.len()
            }),
        )?;
        tx.commit()?;
        self.get_saved_context_pack(&pack.id)
    }

    pub fn get_saved_context_pack(&self, id: &str) -> Result<SavedContextPack> {
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT p.content_json,r.created_by,g.generated_artifact_id
                 FROM context_packs p
                 JOIN context_pack_records r ON r.context_pack_id=p.id
                 JOIN context_pack_generated_artifacts g ON g.context_pack_id=p.id
                 WHERE p.id=?1 AND p.project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        let pack: ContextPack = serde_json::from_str(&raw.0)?;
        validate_materialized_pack(&pack, &self.manifest().project_id)?;
        Ok(SavedContextPack {
            pack,
            generated_artifact_id: raw.2,
            created_by: raw.1,
        })
    }

    pub fn list_saved_context_packs(&self, page: PageRequest) -> Result<SavedContextPackPage> {
        validate_page(page, "Context Pack")?;
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("Context Pack page offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT p.id,p.scope,r.checkpoint_id,p.source_ledger_sequence,r.task,r.audience,
                    r.included_bytes,r.estimated_tokens,r.content_fingerprint,p.created_at
             FROM context_packs p JOIN context_pack_records r ON r.context_pack_id=p.id
             WHERE p.project_id=?1 ORDER BY p.created_at DESC,p.id DESC LIMIT ?2 OFFSET ?3",
        )?;
        let rows = statement
            .query_map(
                params![self.manifest().project_id, page.limit + 1, offset],
                |row| {
                    Ok(SavedContextPackSummary {
                        id: row.get(0)?,
                        scope: row.get(1)?,
                        checkpoint_id: row.get(2)?,
                        source_ledger_sequence: row.get(3)?,
                        task: row.get(4)?,
                        audience: row.get(5)?,
                        included_bytes: row.get(6)?,
                        estimated_tokens: row.get(7)?,
                        content_fingerprint: row.get(8)?,
                        created_at: row.get(9)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = rows.len() > page.limit as usize;
        let items = rows
            .into_iter()
            .take(page.limit as usize)
            .collect::<Vec<_>>();
        Ok(SavedContextPackPage {
            next_offset: has_more.then_some(page.offset + items.len() as u64),
            items,
        })
    }
}

fn validate_checkpoint_input(input: &SemanticCheckpointInput) -> Result<()> {
    if input.note.chars().count() > MAX_CHECKPOINT_TEXT_CHARS {
        return Err(CoreError::Validation(format!(
            "checkpoint note exceeds {MAX_CHECKPOINT_TEXT_CHARS} characters"
        )));
    }
    validate_text_list(&input.blockers, "checkpoint blockers")?;
    validate_text_list(&input.risks, "checkpoint risks")?;
    validate_text_list(&input.next_actions, "checkpoint next actions")?;
    if let Some(id) = &input.semantic_candidate_id {
        crate::store::validate_id(id, "semantic_candidate_id")?;
    }
    if let Some(id) = &input.supersedes_checkpoint_id {
        crate::store::validate_id(id, "supersedes_checkpoint_id")?;
    }
    Ok(())
}

fn validate_text_list(values: &[String], label: &str) -> Result<()> {
    if values.len() > 100 {
        return Err(CoreError::Validation(format!("{label} exceeds 100 items")));
    }
    let mut unique = HashSet::new();
    for value in values {
        validate_nonempty(value, 2_000, label)?;
        if !unique.insert(value.trim()) {
            return Err(CoreError::Validation(format!(
                "{label} contains duplicates"
            )));
        }
    }
    Ok(())
}

fn require_checkpoint_scope(
    connection: &Connection,
    project_id: &str,
    scope: CheckpointScope,
) -> Result<()> {
    let capability = |space: &str| -> Result<bool> {
        connection
            .query_row(
                "SELECT enabled FROM space_capabilities WHERE project_id=?1 AND space=?2",
                params![project_id, space],
                |row| row.get(0),
            )
            .map_err(Into::into)
    };
    match scope {
        CheckpointScope::Research if !capability("research")? => Err(CoreError::Validation(
            "Research Space must be enabled for a Research Checkpoint".into(),
        )),
        CheckpointScope::Development if !capability("development")? => Err(CoreError::Validation(
            "Development Space must be enabled for a Development Checkpoint".into(),
        )),
        CheckpointScope::Integrated if !capability("research")? || !capability("development")? => {
            Err(CoreError::Validation(
                "Integrated R&D Checkpoints require both Research and Development Spaces".into(),
            ))
        }
        _ => Ok(()),
    }
}

fn load_reviewed_semantic_candidate(
    connection: &Connection,
    project_id: &str,
    candidate_id: &str,
) -> Result<Value> {
    let raw = connection
        .query_row(
            "SELECT c.output_json,c.source_fingerprint,c.candidate_version,c.reviewed_at,c.reviewed_by,
                    t.task_type
             FROM ai_candidates c JOIN ai_semantic_tasks t ON t.id=c.task_id
             WHERE c.id=?1 AND c.project_id=?2 AND c.review_state='accepted'",
            params![candidate_id, project_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| {
            CoreError::Validation(
                "semantic checkpoint narrative must reference a reviewed accepted candidate".into(),
            )
        })?;
    Ok(json!({
        "candidate_id":candidate_id,
        "task_type":raw.5,
        "candidate_version":raw.2,
        "source_fingerprint":raw.1,
        "reviewed_at":raw.3,
        "reviewed_by":raw.4,
        "output":serde_json::from_str::<Value>(&raw.0)?
    }))
}

fn load_scoped_entities(
    connection: &Connection,
    project_id: &str,
    scope: CheckpointScope,
) -> Result<Vec<ScopedEntity>> {
    let mut statement = connection.prepare(
        "SELECT e.id,e.entity_type,e.title,e.status,e.version,e.metadata_json,e.data_json,
                e.updated_at,req.created_in_space,COALESCE(c.classification,'internal')
         FROM entities e
         LEFT JOIN requirements req ON req.entity_id=e.id
         LEFT JOIN ai_entity_classification c ON c.entity_id=e.id AND c.project_id=e.project_id
         WHERE e.project_id=?1 AND e.status<>'archived'
         ORDER BY e.entity_type,e.updated_at DESC,e.id LIMIT ?2",
    )?;
    let rows = statement
        .query_map(
            params![project_id, (MAX_CHECKPOINT_SOURCES + 1) as i64],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                ))
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut result = Vec::new();
    for raw in rows {
        if !entity_type_matches_scope(&raw.1, raw.8.as_deref(), scope) {
            continue;
        }
        result.push(ScopedEntity {
            id: raw.0,
            entity_type: raw.1,
            title: raw.2,
            status: raw.3,
            version: raw.4,
            metadata: serde_json::from_str(&raw.5)?,
            data: serde_json::from_str(&raw.6)?,
            updated_at: raw.7,
            created_in_space: raw.8,
            classification: DataClassification::parse(&raw.9)?,
        });
    }
    Ok(result)
}

pub(crate) fn entity_type_matches_scope(
    entity_type: &str,
    created_in_space: Option<&str>,
    scope: CheckpointScope,
) -> bool {
    let research = matches!(
        entity_type,
        "research_session"
            | "research_question"
            | "evidence"
            | "experiment"
            | "result"
            | "finding"
            | "decision"
    ) || (entity_type == "requirement" && created_in_space == Some("research"));
    let development = matches!(
        entity_type,
        "repository"
            | "repository_baseline"
            | "commit_observation"
            | "change_set"
            | "analysis_run"
            | "code_entity"
            | "test"
            | "test_run"
    ) || (entity_type == "requirement"
        && created_in_space == Some("development"));
    match scope {
        CheckpointScope::Research => research,
        CheckpointScope::Development => development,
        CheckpointScope::Integrated => {
            research || development || entity_type == "learning_feedback"
        }
        CheckpointScope::Core => true,
    }
}

fn load_linked_artifact_snapshots(
    connection: &Connection,
    project_id: &str,
    source_ids: &[String],
    max: usize,
) -> Result<Vec<ArtifactSnapshot>> {
    let mut artifacts = BTreeMap::new();
    let mut statement = connection.prepare(
        "SELECT a.id,a.sha256,a.availability,a.classification
         FROM entity_artifacts l JOIN artifacts a ON a.id=l.artifact_id
         WHERE l.entity_id=?1 AND a.project_id=?2 ORDER BY a.id",
    )?;
    for source_id in source_ids {
        let rows = statement
            .query_map(params![source_id, project_id], |row| {
                Ok(ArtifactSnapshot {
                    id: row.get(0)?,
                    sha256: row.get(1)?,
                    availability: row.get(2)?,
                    classification: row.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for artifact in rows {
            artifacts.entry(artifact.id.clone()).or_insert(artifact);
            if artifacts.len() > max {
                return Err(CoreError::Conflict(format!(
                    "checkpoint artifact source set exceeds {max} records"
                )));
            }
        }
    }
    Ok(artifacts.into_values().collect())
}

fn repository_snapshot(
    connection: &Connection,
    project_id: &str,
    scope: CheckpointScope,
) -> Result<Value> {
    if !matches!(
        scope,
        CheckpointScope::Development | CheckpointScope::Integrated | CheckpointScope::Core
    ) {
        return Ok(json!({"repositories":[]}));
    }
    let mut statement = connection.prepare(
        "SELECT r.entity_id,r.root_path,r.root_fingerprint,
                b.entity_id,b.head_oid,b.head_ref,b.branch_name,b.worktree_fingerprint,b.observed_at
         FROM repositories r
         LEFT JOIN repository_reconciliations rr ON rr.id=(
             SELECT x.id FROM repository_reconciliations x WHERE x.repository_id=r.entity_id
             ORDER BY x.observed_at DESC,x.id DESC LIMIT 1)
         LEFT JOIN repository_baselines b ON b.entity_id=rr.current_baseline_id
         WHERE r.project_id=?1 ORDER BY r.entity_id",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, Option<String>>(8)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut repositories = Vec::new();
    for row in rows {
        let live = inspect_live_state(Path::new(&row.1));
        let live_json = match live {
            Ok(value) => json!({
                "available":true,
                "head_oid":value.head_oid,
                "head_ref":value.head_ref,
                "branch_name":value.branch_name,
                "worktree_fingerprint":value.worktree_fingerprint,
                "worktree_entry_count":value.status.len()
            }),
            Err(error) => json!({
                "available":false,
                "error_code":repository_error_code(&error)
            }),
        };
        repositories.push(json!({
            "repository_id":row.0,
            "root_fingerprint":row.2,
            "baseline_id":row.3,
            "observed_head_oid":row.4,
            "observed_head_ref":row.5,
            "observed_branch_name":row.6,
            "observed_worktree_fingerprint":row.7,
            "observed_at":row.8,
            "live":live_json
        }));
    }
    Ok(json!({"repositories":repositories}))
}

fn repository_error_code(error: &CoreError) -> &'static str {
    match error {
        CoreError::NotFound(_) => "not_found",
        CoreError::Conflict(_) => "unstable_or_conflict",
        CoreError::Validation(_) => "invalid_or_unavailable",
        CoreError::Io(_) => "io_error",
        _ => "inspection_failed",
    }
}

fn capture_snapshot(
    connection: &Connection,
    project_id: &str,
    scope: CheckpointScope,
) -> Result<Value> {
    if !matches!(
        scope,
        CheckpointScope::Research | CheckpointScope::Integrated | CheckpointScope::Core
    ) {
        return Ok(json!({"sessions":[]}));
    }
    let mut statement = connection.prepare(
        "SELECT c.id,c.state,c.state_version,c.recoverable,c.failure_code,c.updated_at,
                (SELECT MAX(s.sequence) FROM capture_segments s WHERE s.capture_session_id=c.id),
                (SELECT MAX(s.end_offset_ms) FROM capture_segments s WHERE s.capture_session_id=c.id),
                (SELECT COUNT(*) FROM capture_markers m WHERE m.capture_session_id=c.id)
         FROM capture_sessions c WHERE c.project_id=?1
         ORDER BY c.updated_at DESC,c.id DESC LIMIT 100",
    )?;
    let sessions = statement
        .query_map([project_id], |row| {
            Ok(json!({
                "capture_session_id":row.get::<_,String>(0)?,
                "state":row.get::<_,String>(1)?,
                "state_version":row.get::<_,i64>(2)?,
                "recoverable":row.get::<_,bool>(3)?,
                "failure_code":row.get::<_,Option<String>>(4)?,
                "updated_at":row.get::<_,String>(5)?,
                "latest_segment_sequence":row.get::<_,Option<i64>>(6)?,
                "latest_persisted_offset_ms":row.get::<_,Option<i64>>(7)?,
                "marker_count":row.get::<_,i64>(8)?
            }))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(json!({"sessions":sessions,"truncated":sessions.len()==100}))
}

fn deterministic_checkpoint_state(
    entities: &[ScopedEntity],
    input: &SemanticCheckpointInput,
    ledger_sequence: i64,
    repository_snapshot: &Value,
    capture_snapshot: &Value,
) -> Value {
    let state = deterministic_current_state(entities, repository_snapshot, capture_snapshot);
    json!({
        "source_ledger_sequence":ledger_sequence,
        "trigger":input.trigger.as_str(),
        "note":input.note,
        "state":state,
        "blockers":input.blockers,
        "risks":input.risks,
        "next_actions":input.next_actions
    })
}

fn deterministic_current_state(
    entities: &[ScopedEntity],
    repository_snapshot: &Value,
    capture_snapshot: &Value,
) -> Value {
    let mut counts = BTreeMap::<String, usize>::new();
    let mut active = Vec::new();
    let mut unresolved = Vec::new();
    for entity in entities {
        *counts.entry(entity.entity_type.clone()).or_default() += 1;
        if entity_is_active(entity) && active.len() < 100 {
            active.push(json!({
                "id":entity.id,"type":entity.entity_type,"title":entity.title,
                "status":entity.status,"version":entity.version
            }));
        }
        if entity_is_unresolved(entity) && unresolved.len() < 100 {
            unresolved.push(json!({
                "id":entity.id,"type":entity.entity_type,"title":entity.title,
                "status":entity.status,"version":entity.version
            }));
        }
    }
    json!({
        "counts":counts,
        "active_items":active,
        "active_items_truncated":entities.iter().filter(|item|entity_is_active(item)).count()>active.len(),
        "unresolved_items":unresolved,
        "unresolved_items_truncated":entities.iter().filter(|item|entity_is_unresolved(item)).count()>unresolved.len(),
        "repositories":repository_snapshot,
        "capture":capture_snapshot
    })
}

fn checkpoint_privacy_snapshot(entities: &[ScopedEntity], artifacts: &[ArtifactSnapshot]) -> Value {
    let mut entity_counts = BTreeMap::<String, usize>::new();
    for entity in entities {
        *entity_counts
            .entry(entity.classification.as_str().into())
            .or_default() += 1;
    }
    let mut artifact_counts = BTreeMap::<String, usize>::new();
    for artifact in artifacts {
        let normalized = if artifact.classification == "confidential" {
            "sensitive"
        } else {
            artifact.classification.as_str()
        };
        *artifact_counts.entry(normalized.into()).or_default() += 1;
    }
    json!({
        "policy_version":CONTEXT_PRIVACY_POLICY_VERSION,
        "entity_classification_counts":entity_counts,
        "artifact_classification_counts":artifact_counts,
        "payloads_embedded":false
    })
}

fn entity_is_active(entity: &ScopedEntity) -> bool {
    !matches!(
        entity.status.as_str(),
        "completed"
            | "closed"
            | "resolved"
            | "rejected"
            | "superseded"
            | "archived"
            | "validated"
            | "passed"
    )
}

fn entity_is_unresolved(entity: &ScopedEntity) -> bool {
    matches!(
        entity.status.as_str(),
        "draft"
            | "active"
            | "open"
            | "blocked"
            | "in_progress"
            | "candidate"
            | "challenged"
            | "failed"
            | "observed"
            | "linked"
    )
}

fn legacy_checkpoint_envelope(checkpoint: Checkpoint) -> Result<CheckpointEnvelope> {
    let fingerprint = hash_value(&json!({
        "legacy":true,
        "checkpoint_id":checkpoint.id,
        "scope":checkpoint.scope,
        "ledger_sequence":checkpoint.ledger_sequence,
        "summary":checkpoint.summary
    }))?;
    Ok(CheckpointEnvelope {
        deterministic: checkpoint.summary.clone(),
        checkpoint,
        schema_version: 1,
        trigger: "manual".into(),
        semantic_candidate_id: None,
        semantic_snapshot: None,
        privacy_policy_version: "legacy-unspecified".into(),
        privacy_snapshot: json!({"policy_version":"legacy-unspecified"}),
        repository_snapshot: json!({"repositories":[]}),
        capture_snapshot: json!({"sessions":[]}),
        material_fingerprint: fingerprint,
        supersedes_checkpoint_id: None,
    })
}

fn checkpoint_source_versions(
    connection: &Connection,
    checkpoint_id: &str,
) -> Result<BTreeMap<String, i64>> {
    let mut statement = connection.prepare(
        "SELECT source_entity_id,source_version FROM checkpoint_sources
         WHERE checkpoint_id=?1 ORDER BY source_entity_id",
    )?;
    statement
        .query_map([checkpoint_id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<std::result::Result<BTreeMap<_, _>, _>>()
        .map_err(Into::into)
}

fn checkpoint_freshness_with_connection(
    connection: &Connection,
    project_id: &str,
    envelope: &CheckpointEnvelope,
) -> Result<CheckpointFreshness> {
    let current_sequence: i64 = connection.query_row(
        "SELECT ledger_sequence FROM projects WHERE id=?1",
        [project_id],
        |row| row.get(0),
    )?;
    let sources = checkpoint_source_versions(connection, &envelope.checkpoint.id)?;
    let mut changed_source_ids = Vec::new();
    let mut missing_source_ids = Vec::new();
    for (source_id, source_version) in sources {
        let current: Option<i64> = connection
            .query_row(
                "SELECT version FROM entities WHERE id=?1 AND project_id=?2",
                params![source_id, project_id],
                |row| row.get(0),
            )
            .optional()?;
        match current {
            None => missing_source_ids.push(source_id),
            Some(version) if version != source_version => changed_source_ids.push(source_id),
            Some(_) => {}
        }
    }
    let mut changed_artifact_ids = Vec::new();
    let mut artifact_statement = connection.prepare(
        "SELECT s.artifact_id,s.sha256,s.availability,s.classification,
                a.sha256,a.availability,a.classification
         FROM checkpoint_artifact_sources s
         LEFT JOIN artifacts a ON a.id=s.artifact_id AND a.project_id=?2
         WHERE s.checkpoint_id=?1 ORDER BY s.artifact_id",
    )?;
    let artifact_rows = artifact_statement
        .query_map(params![envelope.checkpoint.id, project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for row in artifact_rows {
        if row.4.as_deref() != Some(row.1.as_str())
            || row.5.as_deref() != Some(row.2.as_str())
            || row.6.as_deref().map(normalize_artifact_classification)
                != Some(normalize_artifact_classification(&row.3))
        {
            changed_artifact_ids.push(row.0);
        }
    }
    let scope = parse_checkpoint_scope(&envelope.checkpoint.scope)?;
    let (events, relevant_events_truncated) = relevant_event_delta(
        connection,
        project_id,
        envelope.checkpoint.ledger_sequence,
        current_sequence,
        scope,
        MAX_COMPARISON_EVENTS,
    )?;
    let mut repository_diverged_ids = Vec::new();
    let mut repository_unavailable_ids = Vec::new();
    evaluate_repository_freshness(
        connection,
        project_id,
        &envelope.repository_snapshot,
        &mut repository_diverged_ids,
        &mut repository_unavailable_ids,
    )?;
    let capture_changed = if matches!(
        scope,
        CheckpointScope::Research | CheckpointScope::Integrated | CheckpointScope::Core
    ) {
        hash_value(&capture_snapshot(connection, project_id, scope)?)?
            != hash_value(&envelope.capture_snapshot)?
    } else {
        false
    };
    let mut reasons = Vec::new();
    if !changed_source_ids.is_empty() {
        reasons.push("one or more checkpoint source entities changed version".into());
    }
    if !missing_source_ids.is_empty() {
        reasons.push("one or more checkpoint source entities are unavailable".into());
    }
    if !changed_artifact_ids.is_empty() {
        reasons
            .push("one or more checkpoint Artifacts changed availability or classification".into());
    }
    if !events.is_empty() {
        reasons.push("relevant canonical events occurred after the checkpoint".into());
    }
    if relevant_events_truncated {
        reasons.push("checkpoint freshness scan reached its bounded event limit".into());
    }
    if !repository_diverged_ids.is_empty() {
        reasons.push("live repository state diverged from the checkpoint snapshot".into());
    }
    if !repository_unavailable_ids.is_empty() {
        reasons.push("one or more checkpoint repositories cannot be inspected".into());
    }
    if capture_changed {
        reasons.push("capture state advanced or changed after the checkpoint boundary".into());
    }
    if envelope.privacy_policy_version != CONTEXT_PRIVACY_POLICY_VERSION
        && envelope.privacy_policy_version != "legacy-unspecified"
    {
        reasons.push("checkpoint privacy policy version differs from the current policy".into());
    }
    Ok(CheckpointFreshness {
        checkpoint_id: envelope.checkpoint.id.clone(),
        fresh: reasons.is_empty(),
        evaluated_at_ledger_sequence: current_sequence,
        events_since_checkpoint: events.len() as u64,
        relevant_events_truncated,
        changed_source_ids,
        missing_source_ids,
        changed_artifact_ids,
        repository_diverged_ids,
        repository_unavailable_ids,
        reasons,
    })
}

fn evaluate_repository_freshness(
    connection: &Connection,
    project_id: &str,
    snapshot: &Value,
    diverged: &mut Vec<String>,
    unavailable: &mut Vec<String>,
) -> Result<()> {
    for repository in snapshot
        .get("repositories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(repository_id) = repository.get("repository_id").and_then(Value::as_str) else {
            continue;
        };
        let root: Option<String> = connection
            .query_row(
                "SELECT root_path FROM repositories WHERE entity_id=?1 AND project_id=?2",
                params![repository_id, project_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(root) = root else {
            unavailable.push(repository_id.into());
            continue;
        };
        match inspect_live_state(Path::new(&root)) {
            Ok(live) => {
                let saved = repository.get("live").unwrap_or(&Value::Null);
                if saved.get("available").and_then(Value::as_bool) != Some(true)
                    || saved.get("head_oid").and_then(Value::as_str) != live.head_oid.as_deref()
                    || saved.get("head_ref").and_then(Value::as_str) != live.head_ref.as_deref()
                    || saved.get("worktree_fingerprint").and_then(Value::as_str)
                        != Some(live.worktree_fingerprint.as_str())
                {
                    diverged.push(repository_id.into());
                }
            }
            Err(_) => unavailable.push(repository_id.into()),
        }
    }
    Ok(())
}

fn event_delta(
    connection: &Connection,
    project_id: &str,
    after_sequence: i64,
    through_sequence: i64,
    limit: usize,
) -> Result<(Vec<CheckpointEventDelta>, bool)> {
    let mut statement = connection.prepare(
        "SELECT ledger_sequence,event_type,aggregate_id,occurred_at
         FROM audit_events WHERE project_id=?1 AND ledger_sequence>?2 AND ledger_sequence<=?3
         ORDER BY ledger_sequence,id LIMIT ?4",
    )?;
    let rows = statement
        .query_map(
            params![
                project_id,
                after_sequence,
                through_sequence,
                (limit + 1) as i64
            ],
            |row| {
                Ok(CheckpointEventDelta {
                    ledger_sequence: row.get(0)?,
                    event_type: row.get(1)?,
                    aggregate_id: row.get(2)?,
                    occurred_at: row.get(3)?,
                })
            },
        )?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let truncated = rows.len() > limit;
    Ok((rows.into_iter().take(limit).collect(), truncated))
}

fn relevant_event_delta(
    connection: &Connection,
    project_id: &str,
    after_sequence: i64,
    through_sequence: i64,
    scope: CheckpointScope,
    limit: usize,
) -> Result<(Vec<CheckpointEventDelta>, bool)> {
    let (events, raw_truncated) = event_delta(
        connection,
        project_id,
        after_sequence,
        through_sequence,
        limit.saturating_mul(4).min(5_000),
    )?;
    let mut relevant = Vec::new();
    for event in events {
        if event_relevant_to_scope(connection, project_id, &event, scope)? {
            relevant.push(event);
            if relevant.len() > limit {
                break;
            }
        }
    }
    let truncated = raw_truncated || relevant.len() > limit;
    relevant.truncate(limit);
    Ok((relevant, truncated))
}

fn event_relevant_to_scope(
    connection: &Connection,
    project_id: &str,
    event: &CheckpointEventDelta,
    scope: CheckpointScope,
) -> Result<bool> {
    let event_type = event.event_type.as_str();
    if matches!(
        event_type,
        "checkpoint.created"
            | "research.checkpoint.created"
            | "development.checkpoint.created"
            | "context.checkpoint.created"
            | "context.pack.saved"
            | "human_document.created"
            | "human_document.exported"
            | "mcp.grant.created"
            | "mcp.grant.revoked"
            | "mcp.proposal.reviewed"
    ) {
        return Ok(false);
    }
    if event_type.starts_with("capture.") || event_type.starts_with("research.") {
        return Ok(matches!(
            scope,
            CheckpointScope::Research | CheckpointScope::Integrated | CheckpointScope::Core
        ));
    }
    if event_type.starts_with("development.")
        || event_type.starts_with("repository.")
        || event_type.starts_with("code_intelligence.")
    {
        return Ok(matches!(
            scope,
            CheckpointScope::Development | CheckpointScope::Integrated | CheckpointScope::Core
        ));
    }
    if let Some(aggregate_id) = &event.aggregate_id {
        let entity: Option<(String, Option<String>)> = connection
            .query_row(
                "SELECT e.entity_type,req.created_in_space FROM entities e
                 LEFT JOIN requirements req ON req.entity_id=e.id
                 WHERE e.id=?1 AND e.project_id=?2",
                params![aggregate_id, project_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((entity_type, created_in_space)) = entity {
            return Ok(entity_type_matches_scope(
                &entity_type,
                created_in_space.as_deref(),
                scope,
            ));
        }
    }
    Ok(true)
}

fn parse_checkpoint_scope(value: &str) -> Result<CheckpointScope> {
    match value {
        "research" => Ok(CheckpointScope::Research),
        "development" => Ok(CheckpointScope::Development),
        "integrated" => Ok(CheckpointScope::Integrated),
        "core" => Ok(CheckpointScope::Core),
        _ => Err(CoreError::Validation(format!(
            "unknown checkpoint scope {value}"
        ))),
    }
}

fn normalize_artifact_classification(value: &str) -> &str {
    if value == "confidential" {
        "sensitive"
    } else {
        value
    }
}

fn json_string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn validate_context_request(request: &ContextPackRequest) -> Result<()> {
    validate_nonempty(&request.task, 4_000, "Context Pack task")?;
    validate_nonempty(
        &request.consumer_target,
        200,
        "Context Pack consumer target",
    )?;
    let budget = request.budget;
    if budget.soft_tokens == 0
        || budget.hard_tokens < budget.soft_tokens
        || budget.hard_tokens > MAX_CONTEXT_TOKENS
        || budget.max_bytes < 1_024
        || budget.max_bytes as usize > MAX_CONTEXT_PACK_BYTES
        || budget.max_items == 0
        || budget.max_items as usize > MAX_CONTEXT_ITEMS
        || budget.max_item_bytes < 512
        || budget.max_item_bytes as usize > MAX_CONTEXT_ITEM_BYTES
        || budget.max_item_bytes > budget.max_bytes
    {
        return Err(CoreError::Validation(
            "invalid Context Pack budget; require 0 < soft <= hard <= 128k tokens, pack 1 KiB..=4 MiB, 1..=500 items, and item 512 B..=64 KiB/pack"
                .into(),
        ));
    }
    let roots = request.root_entity_ids.iter().collect::<HashSet<_>>();
    if roots.len() != request.root_entity_ids.len() {
        return Err(CoreError::Validation(
            "Context Pack root IDs contain duplicates".into(),
        ));
    }
    let exclusions = request.exclude_source_ids.iter().collect::<HashSet<_>>();
    if exclusions.len() != request.exclude_source_ids.len() {
        return Err(CoreError::Validation(
            "Context Pack exclusions contain duplicates".into(),
        ));
    }
    for id in request
        .root_entity_ids
        .iter()
        .chain(request.exclude_source_ids.iter())
    {
        crate::store::validate_id(id, "Context Pack source ID")?;
    }
    if let Some(id) = &request.checkpoint_id {
        crate::store::validate_id(id, "Context Pack checkpoint_id")?;
    }
    let profile_matches = match request.retrieval_profile {
        RetrievalProfile::Research => request.scope == CheckpointScope::Research,
        RetrievalProfile::Development => request.scope == CheckpointScope::Development,
        RetrievalProfile::Integrated => request.scope == CheckpointScope::Integrated,
        RetrievalProfile::Resume | RetrievalProfile::Custom => true,
    };
    if !profile_matches {
        return Err(CoreError::Validation(
            "Context Pack retrieval profile conflicts with its scope".into(),
        ));
    }
    Ok(())
}

fn validate_page(page: PageRequest, label: &str) -> Result<()> {
    if page.limit == 0 || page.limit > 100 {
        return Err(CoreError::Validation(format!(
            "{label} page limit must be within 1..=100"
        )));
    }
    Ok(())
}

fn normalized_terms(task: &str) -> BTreeSet<String> {
    task.split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter_map(|term| {
            let term = term.trim().to_lowercase();
            (term.chars().count() >= 2).then_some(term)
        })
        .take(100)
        .collect()
}

fn relevance_matches(entity: &ScopedEntity, terms: &BTreeSet<String>) -> u32 {
    if terms.is_empty() {
        return 0;
    }
    let haystack = format!(
        "{} {} {} {}",
        entity.title,
        entity.entity_type,
        entity.status,
        serde_json::to_string(&entity.data).unwrap_or_default()
    )
    .to_lowercase();
    terms
        .iter()
        .filter(|term| haystack.contains(term.as_str()))
        .count() as u32
}

fn audience_allows(audience: ContextAudience, classification: DataClassification) -> bool {
    match audience {
        ContextAudience::LocalUser => true,
        ContextAudience::ExternalAi => classification.rank() <= DataClassification::Internal.rank(),
        ContextAudience::PublicPortable => classification == DataClassification::Public,
    }
}

fn external_secret_scan_blocks(audience: ContextAudience, entity: &ScopedEntity) -> bool {
    if audience == ContextAudience::LocalUser {
        return false;
    }
    has_secret_marker(&entity.title)
        || serde_json::to_string(&entity.metadata).is_ok_and(|value| has_secret_marker(&value))
        || serde_json::to_string(&entity.data).is_ok_and(|value| has_secret_marker(&value))
}

pub(crate) fn has_secret_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "-----begin private key-----",
        "authorization: bearer ",
        "api_key=",
        "apikey=",
        "secret_key=",
        "client_secret=",
        "password=",
        "ghp_",
        "sk-proj-",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn filtered_project_state_content(
    state: &CurrentProjectState,
    entities: &[ScopedEntity],
    audience: ContextAudience,
) -> Value {
    let permitted = entities
        .iter()
        .filter(|entity| {
            audience_allows(audience, entity.classification)
                && !external_secret_scan_blocks(audience, entity)
        })
        .collect::<Vec<_>>();
    let mut counts = BTreeMap::<String, usize>::new();
    for entity in &permitted {
        *counts.entry(entity.entity_type.clone()).or_default() += 1;
    }
    let active = permitted
        .iter()
        .filter(|entity| entity_is_active(entity))
        .take(25)
        .map(|entity| {
            json!({
                "id":entity.id,"type":entity.entity_type,"title":entity.title,
                "status":entity.status,"version":entity.version
            })
        })
        .collect::<Vec<_>>();
    json!({
        "project_id":state.project_id,
        "scope":state.scope,
        "as_of_ledger_sequence":state.as_of_ledger_sequence,
        "checkpoint_id":state.checkpoint.as_ref().map(|item|&item.checkpoint.id),
        "checkpoint_fresh":state.freshness.as_ref().map(|item|item.fresh),
        "counts":counts,
        "active_items":active,
        "next_actions":state.next_actions,
        "privacy_filtered":permitted.len()!=entities.len()
    })
}

fn public_project_state_content(state: &CurrentProjectState, entities: &[ScopedEntity]) -> Value {
    let permitted = entities
        .iter()
        .filter(|entity| {
            entity.classification == DataClassification::Public
                && !external_secret_scan_blocks(ContextAudience::PublicPortable, entity)
        })
        .collect::<Vec<_>>();
    let mut counts = BTreeMap::<String, usize>::new();
    for entity in &permitted {
        *counts.entry(entity.entity_type.clone()).or_default() += 1;
    }
    let active = permitted
        .iter()
        .filter(|entity| entity_is_active(entity))
        .take(25)
        .map(|entity| {
            json!({
                "id":entity.id,"type":entity.entity_type,"title":entity.title,
                "status":entity.status,"version":entity.version
            })
        })
        .collect::<Vec<_>>();
    json!({
        "scope":state.scope,
        "as_of_ledger_sequence":state.as_of_ledger_sequence,
        "counts":counts,
        "active_items":active,
        "privacy_filtered":permitted.len()!=entities.len(),
        "next_actions_withheld":true,
        "checkpoint_narrative_withheld":true
    })
}

fn highest_entity_classification_for_ids<'a>(
    entities: &[ScopedEntity],
    ids: impl Iterator<Item = &'a String>,
) -> DataClassification {
    let by_id = entities
        .iter()
        .map(|entity| (entity.id.as_str(), entity.classification))
        .collect::<HashMap<_, _>>();
    ids.filter_map(|id| by_id.get(id.as_str()).copied())
        .fold(DataClassification::Public, DataClassification::max)
}

fn safe_checkpoint_content(
    checkpoint: &CheckpointEnvelope,
    audience: ContextAudience,
    entities: &[ScopedEntity],
) -> Value {
    let all_allowed = entities
        .iter()
        .filter(|entity| {
            checkpoint
                .checkpoint
                .summary
                .to_string()
                .contains(&entity.id)
        })
        .all(|entity| {
            audience_allows(audience, entity.classification)
                && !external_secret_scan_blocks(audience, entity)
        });
    if audience == ContextAudience::LocalUser || all_allowed {
        json!({
            "checkpoint_id":checkpoint.checkpoint.id,
            "scope":checkpoint.checkpoint.scope,
            "ledger_sequence":checkpoint.checkpoint.ledger_sequence,
            "trigger":checkpoint.trigger,
            "note":checkpoint.deterministic.get("note"),
            "blockers":checkpoint.deterministic.get("blockers"),
            "risks":checkpoint.deterministic.get("risks"),
            "next_actions":checkpoint.deterministic.get("next_actions"),
            "state_counts":checkpoint.deterministic.pointer("/state/counts"),
            "repositories":checkpoint.repository_snapshot,
            "capture":checkpoint.capture_snapshot,
            "semantic_snapshot":checkpoint.semantic_snapshot,
            "privacy_policy_version":checkpoint.privacy_policy_version
        })
    } else {
        json!({
            "checkpoint_id":checkpoint.checkpoint.id,
            "scope":checkpoint.checkpoint.scope,
            "ledger_sequence":checkpoint.checkpoint.ledger_sequence,
            "trigger":checkpoint.trigger,
            "content_withheld":"one or more checkpoint sources exceed the requested audience"
        })
    }
}

fn entity_context_content(entity: &ScopedEntity, max_bytes: usize) -> Value {
    let full = json!({
        "id":entity.id,
        "type":entity.entity_type,
        "title":entity.title,
        "status":entity.status,
        "version":entity.version,
        "origin_space":entity.created_in_space,
        "updated_at":entity.updated_at,
        "metadata":entity.metadata,
        "data":entity.data
    });
    if serde_json::to_vec(&full).is_ok_and(|encoded| encoded.len() <= max_bytes) {
        return full;
    }
    let data = serde_json::to_string(&entity.data).unwrap_or_else(|_| "{}".into());
    let metadata = serde_json::to_string(&entity.metadata).unwrap_or_else(|_| "{}".into());
    let excerpt_budget = max_bytes.saturating_sub(1_024).max(256) / 2;
    json!({
        "id":entity.id,
        "type":entity.entity_type,
        "title":truncate_chars(&entity.title, 500),
        "status":entity.status,
        "version":entity.version,
        "origin_space":entity.created_in_space,
        "updated_at":entity.updated_at,
        "metadata_excerpt":truncate_utf8_bytes(&metadata,excerpt_budget),
        "data_excerpt":truncate_utf8_bytes(&data,excerpt_budget),
        "truncated":true
    })
}

fn relationship_candidates(
    connection: &Connection,
    project_id: &str,
    selected_ids: &HashSet<String>,
    excluded: &HashSet<String>,
) -> Result<Vec<CandidateItem>> {
    if selected_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut statement = connection.prepare(
        "SELECT id,relation_type,state_version,source_entity_id,target_entity_id,status,
                review_state,confidence,annotation
         FROM relationships WHERE project_id=?1 AND status='active'
         ORDER BY relation_type,source_entity_id,target_entity_id,id LIMIT 5001",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<f64>>(7)?,
                row.get::<_, String>(8)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    if rows.len() > 5_000 {
        return Err(CoreError::Conflict(
            "relationship candidate set exceeds 5000; narrow Context Pack roots".into(),
        ));
    }
    Ok(rows
        .into_iter()
        .filter(|row| {
            selected_ids.contains(&row.3)
                && selected_ids.contains(&row.4)
                && !excluded.contains(&row.0)
        })
        .map(|row| CandidateItem {
            tier: 3,
            score: 50,
            source_kind: "relationship".into(),
            source_id: row.0,
            source_version: row.2.to_string(),
            classification: DataClassification::Internal,
            relevance_basis: vec!["connects_selected_entities".into()],
            content: json!({
                "relation_type":row.1,"source_entity_id":row.3,"target_entity_id":row.4,
                "status":row.5,"review_state":row.6,"confidence":row.7,"annotation":row.8
            }),
        })
        .collect())
}

fn artifact_candidates(
    store: &ContinuityStore,
    connection: &Connection,
    entity_ids: &HashSet<String>,
    audience: ContextAudience,
    max_item_bytes: usize,
    excluded: &HashSet<String>,
    omissions: &mut Vec<ContextOmission>,
) -> Result<Vec<CandidateItem>> {
    if entity_ids.is_empty() {
        return Ok(Vec::new());
    }
    let mut records = BTreeMap::<String, (String, i64, String, String, String, Vec<String>)>::new();
    let mut statement = connection.prepare(
        "SELECT a.id,a.sha256,a.byte_size,a.media_type,a.classification,a.availability,l.role
         FROM entity_artifacts l JOIN artifacts a ON a.id=l.artifact_id
         WHERE l.entity_id=?1 AND a.project_id=?2 ORDER BY a.id,l.role",
    )?;
    for entity_id in entity_ids {
        let rows = statement
            .query_map(params![entity_id, store.manifest().project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for row in rows {
            records
                .entry(row.0.clone())
                .and_modify(|value| value.5.push(entity_id.clone()))
                .or_insert((row.1, row.2, row.3, row.4, row.5, vec![entity_id.clone()]));
        }
    }
    let mut candidates = Vec::new();
    for (artifact_id, (sha256, byte_size, media_type, classification, availability, owners)) in
        records
    {
        if excluded.contains(&artifact_id) {
            omissions.push(ContextOmission {
                source_id: Some(artifact_id),
                reason: "user_excluded".into(),
                detail: "Artifact was removed in the Context Pack preview request.".into(),
            });
            continue;
        }
        let classification = DataClassification::parse(&classification)?;
        if !audience_allows(audience, classification) {
            omissions.push(ContextOmission {
                source_id: Some(artifact_id),
                reason: "privacy_policy".into(),
                detail: format!(
                    "{} Artifact is not permitted for {}.",
                    classification.as_str(),
                    audience.as_str()
                ),
            });
            continue;
        }
        let mut content = json!({
            "artifact_id":artifact_id,
            "sha256":sha256,
            "byte_size":byte_size,
            "media_type":media_type,
            "classification":classification.as_str(),
            "availability":availability,
            "linked_entity_ids":owners,
            "payload_included":false
        });
        if availability == "available" && is_text_media_type(&media_type) {
            match store.read_artifact_bounded(&artifact_id, max_item_bytes as u64) {
                Ok(bytes) => match String::from_utf8(bytes) {
                    Ok(text)
                        if !has_secret_marker(&text) || audience == ContextAudience::LocalUser =>
                    {
                        content["text"] = Value::String(text);
                        content["payload_included"] = Value::Bool(true);
                    }
                    Ok(_) => omissions.push(ContextOmission {
                        source_id: Some(artifact_id.clone()),
                        reason: "secret_pattern_detected".into(),
                        detail: "Artifact metadata was retained but its text payload was excluded."
                            .into(),
                    }),
                    Err(_) => omissions.push(ContextOmission {
                        source_id: Some(artifact_id.clone()),
                        reason: "non_utf8_text".into(),
                        detail: "Artifact metadata was retained; payload is not valid UTF-8."
                            .into(),
                    }),
                },
                Err(error) => omissions.push(ContextOmission {
                    source_id: Some(artifact_id.clone()),
                    reason: "artifact_unavailable_or_too_large".into(),
                    detail: error.to_string(),
                }),
            }
        }
        candidates.push(CandidateItem {
            tier: 4,
            score: 10,
            source_kind: "artifact".into(),
            source_id: artifact_id,
            source_version: sha256,
            classification,
            relevance_basis: vec!["linked_to_selected_entity".into()],
            content,
        });
    }
    Ok(candidates)
}

fn is_text_media_type(media_type: &str) -> bool {
    media_type.starts_with("text/")
        || matches!(
            media_type,
            "application/json"
                | "application/xml"
                | "application/yaml"
                | "application/toml"
                | "application/javascript"
        )
}

fn conservative_token_estimate(bytes: u32) -> u32 {
    bytes.saturating_add(2) / 3
}

fn context_request_fingerprint(
    request: &ContextPackRequest,
    ledger_sequence: i64,
    checkpoint_fingerprint: Option<&str>,
) -> Result<String> {
    hash_value(&json!({
        "contract_version":CONTEXT_CONTRACT_VERSION,
        "request":request,
        "ledger_sequence":ledger_sequence,
        "checkpoint_fingerprint":checkpoint_fingerprint,
        "privacy_policy_version":CONTEXT_PRIVACY_POLICY_VERSION,
        "estimator_id":"utf8_bytes_div_3_ceiling_v1"
    }))
}

fn context_privacy_summary(
    items: &[ContextPackItem],
    omissions: &[ContextOmission],
    audience: ContextAudience,
) -> Value {
    let mut counts = BTreeMap::<String, usize>::new();
    for item in items {
        *counts
            .entry(item.classification.as_str().into())
            .or_default() += 1;
    }
    let privacy_omissions = omissions
        .iter()
        .filter(|item| {
            matches!(
                item.reason.as_str(),
                "privacy_policy" | "secret_pattern_detected"
            )
        })
        .count();
    json!({
        "policy_version":CONTEXT_PRIVACY_POLICY_VERSION,
        "audience":audience.as_str(),
        "included_classification_counts":counts,
        "privacy_omission_count":privacy_omissions,
        "transitive_rule":"relationships and checkpoint narrative are withheld unless their selected sources are permitted"
    })
}

fn validate_materialized_pack(pack: &ContextPack, expected_project_id: &str) -> Result<()> {
    crate::store::validate_id(&pack.id, "Context Pack id")?;
    if pack.project_id != expected_project_id {
        return Err(CoreError::Validation(
            "Context Pack belongs to a different project".into(),
        ));
    }
    if pack.schema_version != CONTEXT_CONTRACT_VERSION
        || !matches!(
            pack.scope.as_str(),
            "research" | "development" | "integrated" | "core"
        )
        || !matches!(
            pack.audience.as_str(),
            "local_user" | "external_ai" | "public_portable"
        )
        || !matches!(
            pack.freshness_requirement.as_str(),
            "current" | "allow_stale_with_warning"
        )
        || !matches!(
            pack.retrieval_profile.as_str(),
            "resume" | "research" | "development" | "integrated" | "custom"
        )
        || pack.privacy_policy_version != CONTEXT_PRIVACY_POLICY_VERSION
        || pack.estimator_id != "utf8_bytes_div_3_ceiling_v1"
        || pack.estimator_uncertainty != "medium"
    {
        return Err(CoreError::Validation(
            "Context Pack contract or policy version is unsupported".into(),
        ));
    }
    validate_nonempty(&pack.task, 4_000, "Context Pack task")?;
    validate_nonempty(&pack.consumer_target, 200, "Context Pack consumer target")?;
    if pack.items.is_empty() || pack.items.len() > pack.budget.max_items as usize {
        return Err(CoreError::Validation(
            "Context Pack item count violates its budget".into(),
        ));
    }
    let mut unique = HashSet::new();
    let mut bytes = 0_u32;
    let mut tokens = 0_u32;
    for (expected_ordinal, item) in pack.items.iter().enumerate() {
        if item.ordinal != expected_ordinal as u32
            || !(1..=4).contains(&item.tier)
            || !unique.insert((item.source_kind.as_str(), item.source_id.as_str()))
        {
            return Err(CoreError::Validation(
                "Context Pack item ordering or identity is invalid".into(),
            ));
        }
        let encoded = bounded_json(
            &item.content,
            pack.budget.max_item_bytes as usize,
            "Context Pack item",
        )?;
        let item_bytes = u32::try_from(encoded.len())
            .map_err(|_| CoreError::Validation("Context Pack item size overflow".into()))?;
        if item.content_bytes != item_bytes
            || item.estimated_tokens != conservative_token_estimate(item_bytes)
            || item.content_fingerprint != hash_bytes(encoded.as_bytes())
        {
            return Err(CoreError::Validation(
                "Context Pack item size, token estimate, or fingerprint mismatch".into(),
            ));
        }
        bytes = bytes.saturating_add(item_bytes);
        tokens = tokens.saturating_add(item.estimated_tokens);
    }
    if bytes != pack.included_bytes
        || tokens != pack.estimated_tokens
        || bytes > pack.budget.max_bytes
        || tokens > pack.budget.hard_tokens
    {
        return Err(CoreError::Validation(
            "Context Pack aggregate budget accounting mismatch".into(),
        ));
    }
    let expected_fingerprint = hash_value(&json!({
        "contract_version":CONTEXT_CONTRACT_VERSION,
        "request_fingerprint":pack.request_fingerprint,
        "items":pack.items.iter().map(|item| (&item.source_kind,&item.source_id,&item.source_version,&item.content_fingerprint)).collect::<Vec<_>>(),
        "omissions":pack.omissions,
        "ledger_sequence":pack.source_ledger_sequence
    }))?;
    if expected_fingerprint != pack.content_fingerprint {
        return Err(CoreError::Validation(
            "Context Pack content fingerprint mismatch".into(),
        ));
    }
    Ok(())
}

fn verify_pack_source_snapshot(
    connection: &Connection,
    project_id: &str,
    item: &ContextPackItem,
) -> Result<()> {
    let current = match item.source_kind.as_str() {
        "project_state" => connection
            .query_row(
                "SELECT CAST(ledger_sequence AS TEXT) FROM projects WHERE id=?1",
                [project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        "entity" => connection
            .query_row(
                "SELECT CAST(version AS TEXT) FROM entities WHERE id=?1 AND project_id=?2",
                params![item.source_id, project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        "relationship" => connection
            .query_row(
                "SELECT CAST(state_version AS TEXT) FROM relationships WHERE id=?1 AND project_id=?2",
                params![item.source_id, project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        "artifact" => connection
            .query_row(
                "SELECT sha256 FROM artifacts WHERE id=?1 AND project_id=?2",
                params![item.source_id, project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        "checkpoint" => {
            let raw = connection
                .query_row(
                    "SELECT c.id,c.project_id,c.scope,c.ledger_sequence,c.summary_json,c.created_at,
                            c.created_by,e.material_fingerprint
                     FROM checkpoints c LEFT JOIN checkpoint_envelopes e ON e.checkpoint_id=c.id
                     WHERE c.id=?1 AND c.project_id=?2",
                    params![item.source_id, project_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, i64>(3)?,
                            row.get::<_, String>(4)?,
                            row.get::<_, String>(5)?,
                            row.get::<_, String>(6)?,
                            row.get::<_, Option<String>>(7)?,
                        ))
                    },
                )
                .optional()?;
            raw.map(|raw| -> Result<String> {
                if let Some(fingerprint) = raw.7 {
                    Ok(fingerprint)
                } else {
                    Ok(legacy_checkpoint_envelope(Checkpoint {
                        id: raw.0,
                        project_id: raw.1,
                        scope: raw.2,
                        ledger_sequence: raw.3,
                        summary: serde_json::from_str(&raw.4)?,
                        created_at: raw.5,
                        created_by: raw.6,
                    })?
                    .material_fingerprint)
                }
            })
            .transpose()?
        }
        _ => Some(item.source_version.clone()),
    };
    if current.as_deref() != Some(item.source_version.as_str()) {
        return Err(CoreError::Conflict(format!(
            "Context Pack source {} changed after preview; rebuild before saving",
            item.source_id
        )));
    }
    Ok(())
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn truncate_utf8_bytes(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.into();
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value[..boundary].into()
}

fn fit_context_content(content: Value, max_bytes: usize) -> Result<Value> {
    let encoded = serde_json::to_string(&content)?;
    if encoded.len() <= max_bytes {
        return Ok(content);
    }
    let make_excerpt = |bytes| {
        json!({
            "truncated":true,
            "json_excerpt":truncate_utf8_bytes(&encoded,bytes),
            "original_bytes":encoded.len()
        })
    };
    let mut low = 0;
    let mut high = encoded.len().min(max_bytes);
    while low < high {
        let midpoint = low + (high - low).div_ceil(2);
        let candidate = make_excerpt(midpoint);
        if serde_json::to_vec(&candidate)?.len() <= max_bytes {
            low = midpoint;
        } else {
            high = midpoint - 1;
        }
    }
    let candidate = make_excerpt(low);
    if serde_json::to_vec(&candidate)?.len() > max_bytes {
        return Err(CoreError::Validation(
            "Context Pack item budget is too small for truncation metadata".into(),
        ));
    }
    Ok(candidate)
}

fn hash_value(value: &Value) -> Result<String> {
    Ok(hash_bytes(&serde_json::to_vec(value)?))
}

fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub(crate) fn append_context_integrity_issues(
    connection: &Connection,
    project_id: &str,
    report: &mut IntegrityReport,
) -> Result<()> {
    let checks = [
        (
            "checkpoint_envelope_scope_mismatch",
            "SELECT e.checkpoint_id FROM checkpoint_envelopes e
             JOIN checkpoints c ON c.id=e.checkpoint_id
             WHERE e.project_id=?1 AND (c.project_id<>e.project_id OR
                json_extract(e.deterministic_json,'$.state') IS NULL)",
            "Restore the immutable checkpoint envelope from its creation event and verified backup.",
        ),
        (
            "checkpoint_source_scope_mismatch",
            "SELECT s.checkpoint_id||':'||s.source_entity_id FROM checkpoint_sources s
             JOIN checkpoints c ON c.id=s.checkpoint_id
             JOIN entities e ON e.id=s.source_entity_id
             WHERE c.project_id=?1 AND e.project_id<>c.project_id",
            "Quarantine the cross-project checkpoint source and restore the original source set.",
        ),
        (
            "checkpoint_artifact_source_mismatch",
            "SELECT s.checkpoint_id||':'||s.artifact_id FROM checkpoint_artifact_sources s
             JOIN checkpoints c ON c.id=s.checkpoint_id
             JOIN artifacts a ON a.id=s.artifact_id
             WHERE c.project_id=?1 AND (a.project_id<>c.project_id OR a.sha256<>s.sha256)",
            "Restore the checkpoint Artifact snapshot from the immutable Artifact record or backup.",
        ),
        (
            "context_pack_record_mismatch",
            "SELECT r.context_pack_id FROM context_pack_records r
             JOIN context_packs p ON p.id=r.context_pack_id
             LEFT JOIN checkpoints c ON c.id=r.checkpoint_id
             WHERE r.project_id=?1 AND (p.project_id<>r.project_id OR p.scope NOT IN
                ('research','development','integrated','core') OR
                (r.checkpoint_id IS NOT NULL AND c.project_id<>r.project_id))",
            "Quarantine the saved Context Pack; rebuild it from its project-scoped sources.",
        ),
        (
            "context_pack_source_scope_mismatch",
            "SELECT s.context_pack_id||':'||s.source_id FROM context_pack_sources s
             JOIN context_packs p ON p.id=s.context_pack_id
             WHERE p.project_id=?1 AND (
                (s.source_kind='project_state' AND s.source_id<>p.project_id) OR
                (s.source_kind='entity' AND NOT EXISTS(SELECT 1 FROM entities e WHERE e.id=s.source_id AND e.project_id=p.project_id)) OR
                (s.source_kind='relationship' AND NOT EXISTS(SELECT 1 FROM relationships r WHERE r.id=s.source_id AND r.project_id=p.project_id)) OR
                (s.source_kind='artifact' AND NOT EXISTS(SELECT 1 FROM artifacts a WHERE a.id=s.source_id AND a.project_id=p.project_id)) OR
                (s.source_kind='checkpoint' AND NOT EXISTS(SELECT 1 FROM checkpoints c WHERE c.id=s.source_id AND c.project_id=p.project_id))
             )",
            "Restore the Context Pack source snapshot or rebuild the pack; never redirect source identity.",
        ),
        (
            "context_pack_generated_artifact_missing",
            "SELECT p.id FROM context_packs p
             LEFT JOIN context_pack_generated_artifacts l ON l.context_pack_id=p.id
             LEFT JOIN generated_artifacts g ON g.id=l.generated_artifact_id
             WHERE p.project_id=?1 AND (l.context_pack_id IS NULL OR g.id IS NULL OR
                g.project_id<>p.project_id OR g.kind<>'context_pack')",
            "Restore the GeneratedArtifact link from the saved Context Pack record and audit event.",
        ),
    ];
    for (code, sql, guidance) in checks {
        let mut statement = connection.prepare(sql)?;
        let ids = statement
            .query_map([project_id], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        report
            .issues
            .extend(ids.into_iter().map(|id| IntegrityIssue {
                code: code.into(),
                path_or_id: id,
                guidance: guidance.into(),
            }));
    }

    let mut statement = connection.prepare(
        "SELECT p.id,p.content_json,r.content_fingerprint,r.included_bytes,r.estimated_tokens
         FROM context_packs p JOIN context_pack_records r ON r.context_pack_id=p.id
         WHERE p.project_id=?1 ORDER BY p.id",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, u32>(3)?,
                row.get::<_, u32>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, content_json, fingerprint, bytes, tokens) in rows {
        let pack = match serde_json::from_str::<ContextPack>(&content_json) {
            Ok(pack)
                if validate_materialized_pack(&pack, project_id).is_ok()
                    && pack.content_fingerprint == fingerprint
                    && pack.included_bytes == bytes
                    && pack.estimated_tokens == tokens =>
            {
                pack
            }
            _ => {
                report.issues.push(IntegrityIssue {
                    code: "context_pack_content_invalid".into(),
                    path_or_id: id,
                    guidance:
                        "Quarantine the saved Context Pack and regenerate it from canonical sources."
                            .into(),
                });
                continue;
            }
        };
        let mut source_statement = connection.prepare(
            "SELECT ordinal,source_kind,source_id,source_version,classification,content_fingerprint
             FROM context_pack_sources WHERE context_pack_id=?1 ORDER BY ordinal",
        )?;
        let persisted_sources = source_statement
            .query_map([&id], |row| {
                Ok((
                    row.get::<_, u32>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let source_snapshot_matches = persisted_sources.len() == pack.items.len()
            && persisted_sources
                .iter()
                .zip(&pack.items)
                .all(|(persisted, item)| {
                    persisted.0 == item.ordinal
                        && persisted.1 == item.source_kind
                        && persisted.2 == item.source_id
                        && persisted.3 == item.source_version
                        && persisted.4 == item.classification.as_str()
                        && persisted.5 == item.content_fingerprint
                });
        if !source_snapshot_matches {
            report.issues.push(IntegrityIssue {
                code: "context_pack_sources_invalid".into(),
                path_or_id: id,
                guidance:
                    "Quarantine the saved Context Pack and restore its exact ordered source snapshot from a verified backup."
                        .into(),
            });
        }
    }
    Ok(())
}
