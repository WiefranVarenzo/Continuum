use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::store::{
    append_event_with_context, bounded_json, legacy_origin, prior_result,
    record_command_with_context, validate_command_context, validate_nonempty,
};
use crate::{
    Checkpoint, CommandContext, ContinuityStore, CoreError, Entity, IntegrityIssue,
    IntegrityReport, OriginKind, PageRequest, RequirementRationaleOrigin, Result, new_id,
};

const GIT_ADAPTER_VERSION: &str = "git-cli-v1";
const MAX_GIT_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_GIT_ERROR_BYTES: usize = 64 * 1024;
const MAX_GIT_OPERATION: Duration = Duration::from_secs(60);
const MAX_COMMITS_PER_INGEST: usize = 10_000;
const MAX_REACHABLE_COMMITS: usize = 100_000;
const MAX_FILE_CHANGES: usize = 100_000;
const MAX_WORKTREE_CONTENT_BYTES: u64 = 512 * 1024 * 1024;
const MAX_DEVELOPMENT_JSON_BYTES: usize = 1024 * 1024;
const MAX_DEVELOPMENT_TEXT: usize = 30_000;
const MAX_CHECKPOINT_SOURCES: usize = 2_000;

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

string_enum!(BaselineRelation {
    Initial => "initial",
    Unchanged => "unchanged",
    WorktreeChanged => "worktree_changed",
    FastForward => "fast_forward",
    BranchSwitch => "branch_switch",
    HistoryRewrite => "history_rewrite",
    DetachedHead => "detached_head",
    Unborn => "unborn",
});

string_enum!(ChangeSetKind {
    WorkingTree => "working_tree",
    Committed => "committed",
});

string_enum!(DevelopmentIntentOrigin {
    User => "user",
    Import => "import",
    External => "external",
    Legacy => "legacy",
    Research => "research",
    Unknown => "unknown",
});

string_enum!(RequirementImplementation {
    Implements => "implements",
    PartiallyImplements => "partially_implements",
    Reverts => "reverts",
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkingTreeEntry {
    pub index_status: String,
    pub worktree_status: String,
    pub path: String,
    pub original_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChange {
    pub change_kind: String,
    pub old_path: Option<String>,
    pub new_path: String,
    pub similarity: Option<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryRecord {
    pub entity: Entity,
    pub root_path: String,
    pub root_fingerprint: String,
    pub git_common_dir_fingerprint: String,
    pub object_format: String,
    pub adapter_version: String,
    pub attached_at: String,
    pub last_observed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryBaselineRecord {
    pub entity: Entity,
    pub repository_id: String,
    pub head_oid: Option<String>,
    pub head_ref: Option<String>,
    pub branch_name: Option<String>,
    pub worktree_fingerprint: String,
    pub worktree_status: Vec<WorkingTreeEntry>,
    pub observed_at: String,
    pub previous_baseline_id: Option<String>,
    pub relation_to_previous: String,
    pub adapter_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryReconciliation {
    pub id: String,
    pub repository_id: String,
    pub previous_baseline_id: Option<String>,
    pub current_baseline_id: String,
    pub relation_kind: String,
    pub details: Value,
    pub observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CommitObservation {
    pub entity: Entity,
    pub repository_id: String,
    pub first_observed_baseline_id: String,
    pub commit_oid: String,
    pub tree_oid: String,
    pub parent_oids: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub authored_at: String,
    pub committed_at: String,
    pub subject: String,
    pub observed_at: String,
    pub adapter_version: String,
    pub file_changes: Vec<FileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChangeSetRecord {
    pub entity: Entity,
    pub repository_id: String,
    pub baseline_id: String,
    pub kind: String,
    pub summary: String,
    pub intent_origin: String,
    pub worktree_fingerprint: Option<String>,
    pub supersedes_change_set_id: Option<String>,
    pub commit_entity_ids: Vec<String>,
    pub file_changes: Vec<FileChange>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RepositoryAttachInput {
    pub path: PathBuf,
    pub title: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitIngestionInput {
    pub repository_id: String,
    pub baseline_id: String,
    pub max_commits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommitIngestionResult {
    pub reachable_commits: usize,
    pub previously_known_commits: usize,
    pub ingested_commit_ids: Vec<String>,
    pub remaining_commits: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequirementLinkInput {
    pub requirement_id: String,
    pub relationship: RequirementImplementation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewChangeSet {
    pub title: String,
    pub summary: String,
    pub repository_id: String,
    pub baseline_id: String,
    pub kind: ChangeSetKind,
    #[serde(default)]
    pub commit_entity_ids: Vec<String>,
    #[serde(default)]
    pub requirement_links: Vec<RequirementLinkInput>,
    pub intent_origin: DevelopmentIntentOrigin,
    pub supersedes_change_set_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewDevelopmentRequirement {
    pub title: String,
    pub statement: String,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    pub priority: u8,
    pub rationale_origin: RequirementRationaleOrigin,
    pub verification_method: String,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevelopmentCheckpointInput {
    pub note: String,
    #[serde(default)]
    pub blockers: Vec<String>,
    #[serde(default)]
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevelopmentResumeState {
    pub checkpoint: Option<Checkpoint>,
    pub checkpoint_is_stale: bool,
    pub events_since_checkpoint: u64,
    pub active_items: Vec<Value>,
    pub active_item_count: u64,
    pub active_items_truncated: bool,
    pub blockers: Vec<String>,
    pub next_actions: Vec<String>,
    pub repository_diverged: bool,
    pub repository_state_unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevelopmentReport {
    pub project_id: String,
    pub source_ledger_sequence: i64,
    pub cited_entity_ids: Vec<String>,
    pub markdown: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevelopmentSearchQuery {
    pub text: String,
    pub entity_type: Option<String>,
    pub status: Option<String>,
    pub page: PageRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevelopmentSearchHit {
    pub entity_id: String,
    pub entity_type: String,
    pub title: String,
    pub status: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevelopmentSearchPage {
    pub items: Vec<DevelopmentSearchHit>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DevelopmentTimelineFilter {
    pub repository_id: Option<String>,
    pub entity_id: Option<String>,
    pub event_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevelopmentTimelineEntry {
    pub id: String,
    pub ledger_sequence: i64,
    pub repository_id: Option<String>,
    pub entity_id: Option<String>,
    pub event_type: String,
    pub payload: Value,
    pub actor_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevelopmentTimelinePage {
    pub items: Vec<DevelopmentTimelineEntry>,
    pub next_offset: Option<u64>,
}

pub(crate) fn is_development_entity_type(value: &str) -> bool {
    matches!(
        value,
        "repository" | "repository_baseline" | "commit_observation" | "change_set"
    ) || crate::code_intelligence::is_code_intelligence_entity_type(value)
}

pub(crate) fn validate_development_relationship_pair(
    source: &str,
    relation: &str,
    target: &str,
) -> Result<()> {
    let has_development_endpoint = is_development_entity_type(source)
        || is_development_entity_type(target)
        || (source == "change_set" && target == "requirement");
    if !has_development_endpoint {
        return Ok(());
    }
    let valid = matches!(
        (source, relation, target),
        (
            "change_set",
            "implements" | "partially_implements" | "reverts",
            "requirement"
        ) | ("change_set", "supersedes", "change_set")
            | ("repository", "defines", "code_entity" | "test")
            | ("change_set", "modifies", "code_entity")
            | ("test", "verifies", "requirement" | "code_entity")
            | ("test_run", "executes", "test")
            | ("test_run", "observed_at", "repository_baseline")
    );
    if valid {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "relationship {source} --{relation}--> {target} is not permitted by Development Space"
        )))
    }
}

#[derive(Debug)]
struct RepositoryInspection {
    root_path: String,
    root_fingerprint: String,
    common_dir_fingerprint: String,
    object_format: String,
}

#[derive(Debug)]
struct LiveRepositoryState {
    head_oid: Option<String>,
    head_ref: Option<String>,
    branch_name: Option<String>,
    worktree_fingerprint: String,
    status: Vec<WorkingTreeEntry>,
}

#[derive(Debug, PartialEq, Eq)]
struct WorktreeIdentity {
    status_bytes: Vec<u8>,
    content_digest: String,
}

#[derive(Debug)]
struct RawCommit {
    oid: String,
    tree_oid: String,
    parents: Vec<String>,
    author_name: String,
    author_email: String,
    authored_at: String,
    committed_at: String,
    subject: String,
    file_changes: Vec<FileChange>,
}

#[derive(Debug)]
pub(crate) struct GitTreeFile {
    pub path: String,
    pub mode: String,
    pub object_type: String,
    pub object_id: String,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug)]
struct GitOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    stderr_truncated: bool,
}

impl ContinuityStore {
    pub fn attach_repository(
        &self,
        command: &CommandContext,
        input: RepositoryAttachInput,
    ) -> Result<RepositoryRecord> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        let inspection = inspect_repository(&input.path)?;
        let title = input
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| {
                Path::new(&inspection.root_path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Git repository")
                    .to_owned()
            });
        validate_nonempty(&title, 500, "repository title")?;
        let metadata_json = bounded_json(
            &input.metadata,
            MAX_DEVELOPMENT_JSON_BYTES,
            "repository metadata",
        )?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "AttachRepository",
        )? {
            tx.commit()?;
            return self.get_repository(&id);
        }
        if let Some(id) = tx
            .query_row(
                "SELECT entity_id FROM repositories WHERE project_id=?1 AND root_path=?2",
                params![self.manifest().project_id, inspection.root_path],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            record_command_with_context(
                &tx,
                command,
                &self.manifest().project_id,
                "AttachRepository",
                Some(&id),
                None,
                &json!({"repository_id":id,"already_attached":true}),
            )?;
            tx.commit()?;
            return self.get_repository(&id);
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        insert_common_entity(
            &tx,
            &self.manifest().project_id,
            command,
            &id,
            "repository",
            &title,
            "attached",
            OriginKind::User,
            &metadata_json,
            &json!({"root_fingerprint":inspection.root_fingerprint,"object_format":inspection.object_format}),
            &now,
        )?;
        tx.execute(
            "INSERT INTO repositories(entity_id,project_id,root_path,root_fingerprint,
                git_common_dir_fingerprint,object_format,adapter_version,attached_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                id,
                self.manifest().project_id,
                inspection.root_path,
                inspection.root_fingerprint,
                inspection.common_dir_fingerprint,
                inspection.object_format,
                GIT_ADAPTER_VERSION,
                now
            ],
        )?;
        upsert_development_search(
            &tx,
            &self.manifest().project_id,
            &id,
            "repository",
            &title,
            &inspection.root_path,
            &now,
        )?;
        let payload = json!({"repository_id":id,"root_fingerprint":inspection.root_fingerprint,
            "object_format":inspection.object_format,"adapter_version":GIT_ADAPTER_VERSION});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&id),
            "development.repository.attached",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            Some(&id),
            Some(&id),
            "development.repository.attached",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "AttachRepository",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        self.get_repository(&id)
    }

    pub fn get_repository(&self, id: &str) -> Result<RepositoryRecord> {
        let entity = self.get_entity(id)?;
        if entity.entity_type != "repository" {
            return Err(CoreError::Validation("entity is not a Repository".into()));
        }
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT root_path,root_fingerprint,git_common_dir_fingerprint,object_format,
                        adapter_version,attached_at,last_observed_at
                 FROM repositories WHERE entity_id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok(RepositoryRecord {
                        entity: entity.clone(),
                        root_path: row.get(0)?,
                        root_fingerprint: row.get(1)?,
                        git_common_dir_fingerprint: row.get(2)?,
                        object_format: row.get(3)?,
                        adapter_version: row.get(4)?,
                        attached_at: row.get(5)?,
                        last_observed_at: row.get(6)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))
    }

    pub fn relocate_repository(
        &self,
        command: &CommandContext,
        repository_id: &str,
        expected_version: i64,
        new_path: &Path,
    ) -> Result<RepositoryRecord> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        let current = self.get_repository(repository_id)?;
        if current.entity.version != expected_version {
            return Err(CoreError::Conflict(format!(
                "Repository {repository_id} version {expected_version} is stale"
            )));
        }
        let inspection = inspect_repository(new_path)?;
        if inspection.object_format != current.object_format {
            return Err(CoreError::Validation(
                "relocated Repository uses a different Git object format".into(),
            ));
        }
        let connection = self.connection()?;
        let proof_oid: Option<String> = connection
            .query_row(
                "SELECT commit_oid FROM git_commit_observations
                 WHERE project_id=?1 AND repository_id=?2 ORDER BY observed_at DESC,entity_id DESC LIMIT 1",
                params![self.manifest().project_id, repository_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(oid) = &proof_oid {
            ensure_commit_exists(Path::new(&inspection.root_path), oid).map_err(|_| {
                CoreError::Validation(
                    "new path cannot prove continuity with a previously observed commit".into(),
                )
            })?;
        }
        drop(connection);
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "RelocateRepository",
        )?
        .is_some()
        {
            tx.commit()?;
            return self.get_repository(repository_id);
        }
        let conflict: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM repositories WHERE project_id=?1 AND root_path=?2 AND entity_id<>?3)",
            params![self.manifest().project_id,inspection.root_path,repository_id],
            |row|row.get(0))?;
        if conflict {
            return Err(CoreError::Conflict(
                "the new path is already attached as another Repository".into(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let changed = tx.execute(
            "UPDATE entities SET version=version+1,updated_at=?3,updated_by=?4
             WHERE id=?1 AND project_id=?2 AND version=?5 AND entity_type='repository'",
            params![
                repository_id,
                self.manifest().project_id,
                now,
                command.actor.id,
                expected_version
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(format!(
                "Repository {repository_id} version {expected_version} became stale"
            )));
        }
        tx.execute(
            "UPDATE repositories SET root_path=?3,root_fingerprint=?4,
                git_common_dir_fingerprint=?5,last_observed_at=?6
             WHERE entity_id=?1 AND project_id=?2",
            params![
                repository_id,
                self.manifest().project_id,
                inspection.root_path,
                inspection.root_fingerprint,
                inspection.common_dir_fingerprint,
                now
            ],
        )?;
        let payload = json!({"repository_id":repository_id,
            "previous_root_fingerprint":current.root_fingerprint,
            "current_root_fingerprint":inspection.root_fingerprint,
            "continuity_proof_commit":proof_oid});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(repository_id),
            "development.repository.relocated",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            Some(repository_id),
            Some(repository_id),
            "development.repository.relocated",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "RelocateRepository",
            Some(repository_id),
            Some(expected_version),
            &payload,
        )?;
        tx.commit()?;
        self.get_repository(repository_id)
    }

    pub fn observe_repository_baseline(
        &self,
        command: &CommandContext,
        repository_id: &str,
    ) -> Result<RepositoryBaselineRecord> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        let repository = self.get_repository(repository_id)?;
        let inspection = inspect_repository(Path::new(&repository.root_path))?;
        if inspection.common_dir_fingerprint != repository.git_common_dir_fingerprint
            || inspection.object_format != repository.object_format
        {
            return Err(CoreError::Conflict(
                "attached path now resolves to a different Git object store; explicitly attach it as a new Repository"
                    .into(),
            ));
        }
        let live = inspect_live_state(Path::new(&repository.root_path))?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "ObserveRepositoryBaseline",
        )? {
            tx.commit()?;
            return self.get_repository_baseline(&id);
        }
        let head_key = live.head_oid.clone().unwrap_or_default();
        let ref_key = live.head_ref.clone().unwrap_or_default();
        let previous = latest_baseline_raw(&tx, &self.manifest().project_id, repository_id)?;
        if let Some(id) = tx
            .query_row(
                "SELECT entity_id FROM repository_baselines
                 WHERE repository_id=?1 AND head_oid=?2 AND head_ref=?3 AND worktree_fingerprint=?4",
                params![repository_id, head_key, ref_key, live.worktree_fingerprint],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            let repeated_latest = previous.as_ref().is_some_and(|value| value.id == id);
            let relation = classify_baseline_relation(
                Path::new(&repository.root_path),
                previous.as_ref(),
                &live,
            )?;
            let now = Utc::now().to_rfc3339();
            if !repeated_latest {
                let details = json!({"previous_head":previous.as_ref().and_then(|value|value.head_oid.clone()),
                    "current_head":live.head_oid,"previous_ref":previous.as_ref().and_then(|value|value.head_ref.clone()),
                    "current_ref":live.head_ref,"reused_baseline":true});
                tx.execute(
                    "INSERT INTO repository_reconciliations(id,project_id,repository_id,previous_baseline_id,
                        current_baseline_id,relation_kind,details_json,observed_at)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![
                        new_id(), self.manifest().project_id, repository_id,
                        previous.as_ref().map(|value| value.id.as_str()), id, relation.as_str(),
                        bounded_json(&details,MAX_DEVELOPMENT_JSON_BYTES,"repository reconciliation")?, now
                    ],
                )?;
                tx.execute(
                    "UPDATE repositories SET last_observed_at=?3 WHERE entity_id=?1 AND project_id=?2",
                    params![repository_id, self.manifest().project_id, now],
                )?;
                let payload = json!({"repository_id":repository_id,"baseline_id":id,
                    "head_oid":live.head_oid,"head_ref":live.head_ref,"relation_to_previous":relation.as_str(),
                    "working_tree_entry_count":live.status.len(),"reused_baseline":true});
                let sequence = append_event_with_context(
                    &tx,&self.manifest().project_id,command,Some(&id),
                    "development.repository.baseline_observed",&payload,
                )?;
                insert_development_timeline(
                    &tx,&self.manifest().project_id,sequence,Some(repository_id),Some(&id),
                    "development.repository.baseline_observed",&payload,&command.actor.id,&now,
                )?;
            }
            record_command_with_context(
                &tx,
                command,
                &self.manifest().project_id,
                "ObserveRepositoryBaseline",
                Some(&id),
                None,
                &json!({"repository_id":repository_id,"baseline_id":id,
                    "unchanged":repeated_latest,"relation_to_previous":relation.as_str()}),
            )?;
            tx.commit()?;
            return self.get_repository_baseline(&id);
        }
        let relation =
            classify_baseline_relation(Path::new(&repository.root_path), previous.as_ref(), &live)?;
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        let short_head = live
            .head_oid
            .as_deref()
            .map(|oid| oid.chars().take(12).collect::<String>())
            .unwrap_or_else(|| "unborn".into());
        let dirty = if live.status.is_empty() {
            "clean"
        } else {
            "dirty"
        };
        let title = format!(
            "{} @ {short_head} ({dirty})",
            live.branch_name.as_deref().unwrap_or("detached")
        );
        let status_json = bounded_json(
            &serde_json::to_value(&live.status)?,
            MAX_DEVELOPMENT_JSON_BYTES,
            "working-tree status",
        )?;
        insert_common_entity(
            &tx,
            &self.manifest().project_id,
            command,
            &id,
            "repository_baseline",
            &title,
            "observed",
            OriginKind::Deterministic,
            "{}",
            &json!({"repository_id":repository_id,"head_oid":live.head_oid,
                "worktree_fingerprint":live.worktree_fingerprint}),
            &now,
        )?;
        tx.execute(
            "INSERT INTO repository_baselines(entity_id,project_id,repository_id,head_oid,head_ref,
                branch_name,worktree_fingerprint,worktree_status_json,observed_at,previous_baseline_id,
                relation_to_previous,adapter_version)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
            params![
                id,
                self.manifest().project_id,
                repository_id,
                head_key,
                ref_key,
                live.branch_name.clone().unwrap_or_default(),
                live.worktree_fingerprint,
                status_json,
                now,
                previous.as_ref().map(|value| value.id.as_str()),
                relation.as_str(),
                GIT_ADAPTER_VERSION
            ],
        )?;
        tx.execute(
            "INSERT INTO repository_reconciliations(id,project_id,repository_id,previous_baseline_id,
                current_baseline_id,relation_kind,details_json,observed_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                new_id(),
                self.manifest().project_id,
                repository_id,
                previous.as_ref().map(|value| value.id.as_str()),
                id,
                relation.as_str(),
                bounded_json(
                    &json!({"previous_head":previous.as_ref().and_then(|value|value.head_oid.clone()),
                        "current_head":live.head_oid,"previous_ref":previous.as_ref().and_then(|value|value.head_ref.clone()),
                        "current_ref":live.head_ref}),
                    MAX_DEVELOPMENT_JSON_BYTES,
                    "repository reconciliation",
                )?,
                now
            ],
        )?;
        tx.execute(
            "UPDATE repositories SET last_observed_at=?3 WHERE entity_id=?1 AND project_id=?2",
            params![repository_id, self.manifest().project_id, now],
        )?;
        let payload = json!({"repository_id":repository_id,"baseline_id":id,
            "head_oid":live.head_oid,"head_ref":live.head_ref,"relation_to_previous":relation.as_str(),
            "working_tree_entry_count":live.status.len()});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&id),
            "development.repository.baseline_observed",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            Some(repository_id),
            Some(&id),
            "development.repository.baseline_observed",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "ObserveRepositoryBaseline",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        self.get_repository_baseline(&id)
    }

    pub fn get_repository_baseline(&self, id: &str) -> Result<RepositoryBaselineRecord> {
        let entity = self.get_entity(id)?;
        if entity.entity_type != "repository_baseline" {
            return Err(CoreError::Validation(
                "entity is not a RepositoryBaseline".into(),
            ));
        }
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT repository_id,head_oid,head_ref,branch_name,worktree_fingerprint,
                        worktree_status_json,observed_at,previous_baseline_id,relation_to_previous,adapter_version
                 FROM repository_baselines WHERE entity_id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        Ok(RepositoryBaselineRecord {
            entity,
            repository_id: raw.0,
            head_oid: nonempty_option(raw.1),
            head_ref: nonempty_option(raw.2),
            branch_name: nonempty_option(raw.3),
            worktree_fingerprint: raw.4,
            worktree_status: serde_json::from_str(&raw.5)?,
            observed_at: raw.6,
            previous_baseline_id: raw.7,
            relation_to_previous: raw.8,
            adapter_version: raw.9,
        })
    }

    pub fn latest_repository_reconciliation(
        &self,
        repository_id: &str,
    ) -> Result<RepositoryReconciliation> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT id,repository_id,previous_baseline_id,current_baseline_id,relation_kind,
                        details_json,observed_at FROM repository_reconciliations
                 WHERE project_id=?1 AND repository_id=?2 ORDER BY observed_at DESC,id DESC LIMIT 1",
                params![self.manifest().project_id, repository_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(repository_id.into()))
            .and_then(|raw| {
                Ok(RepositoryReconciliation {
                    id: raw.0,
                    repository_id: raw.1,
                    previous_baseline_id: raw.2,
                    current_baseline_id: raw.3,
                    relation_kind: raw.4,
                    details: serde_json::from_str(&raw.5)?,
                    observed_at: raw.6,
                })
            })
    }

    pub fn ingest_git_commits(
        &self,
        command: &CommandContext,
        input: CommitIngestionInput,
    ) -> Result<CommitIngestionResult> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        if input.max_commits == 0 || input.max_commits > MAX_COMMITS_PER_INGEST {
            return Err(CoreError::Validation(format!(
                "max_commits must be within 1..={MAX_COMMITS_PER_INGEST}"
            )));
        }
        let repository = self.get_repository(&input.repository_id)?;
        let baseline = self.get_repository_baseline(&input.baseline_id)?;
        if baseline.repository_id != input.repository_id {
            return Err(CoreError::Validation(
                "baseline does not belong to the requested Repository".into(),
            ));
        }
        let Some(head_oid) = baseline.head_oid.as_deref() else {
            let mut connection = self.connection()?;
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            require_development_enabled(&tx, &self.manifest().project_id)?;
            if prior_result(
                &tx,
                &self.manifest().project_id,
                command,
                "IngestGitCommits",
            )?
            .is_none()
            {
                record_command_with_context(
                    &tx,
                    command,
                    &self.manifest().project_id,
                    "IngestGitCommits",
                    Some(&input.baseline_id),
                    None,
                    &json!({"repository_id":input.repository_id,"baseline_id":input.baseline_id,
                        "reachable_commit_count":0,"ingested_commit_count":0,"remaining_commit_count":0,
                        "unborn":true}),
                )?;
            }
            tx.commit()?;
            return Ok(CommitIngestionResult {
                reachable_commits: 0,
                previously_known_commits: 0,
                ingested_commit_ids: Vec::new(),
                remaining_commits: 0,
            });
        };
        let root = Path::new(&repository.root_path);
        ensure_commit_exists(root, head_oid)?;
        let mut commits = read_reachable_commits(root, head_oid)?;
        if commits.len() > MAX_REACHABLE_COMMITS {
            return Err(CoreError::Conflict(format!(
                "repository exposes more than {MAX_REACHABLE_COMMITS} reachable commits; ingest a narrower supported history"
            )));
        }
        let connection = self.connection()?;
        let known = known_commit_oids(
            &connection,
            &self.manifest().project_id,
            &input.repository_id,
        )?;
        let reachable_count = commits.len();
        let previously_known = commits
            .iter()
            .filter(|commit| known.contains(&commit.oid))
            .count();
        commits.retain(|commit| !known.contains(&commit.oid));
        let remaining = commits.len().saturating_sub(input.max_commits);
        commits.truncate(input.max_commits);
        for commit in &mut commits {
            commit.file_changes = read_commit_file_changes(root, &commit.oid)?;
        }
        drop(connection);

        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "IngestGitCommits",
        )?
        .is_some()
        {
            tx.commit()?;
            return Ok(CommitIngestionResult {
                reachable_commits: reachable_count,
                previously_known_commits: reachable_count,
                ingested_commit_ids: Vec::new(),
                remaining_commits: 0,
            });
        }
        let now = Utc::now().to_rfc3339();
        let mut ids = Vec::with_capacity(commits.len());
        for commit in &commits {
            let already_exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM git_commit_observations
                 WHERE project_id=?1 AND repository_id=?2 AND commit_oid=?3)",
                params![self.manifest().project_id, input.repository_id, commit.oid],
                |row| row.get(0),
            )?;
            if already_exists {
                continue;
            }
            let id = new_id();
            let title = if commit.subject.trim().is_empty() {
                format!("Commit {}", &commit.oid[..12.min(commit.oid.len())])
            } else {
                excerpt(&commit.subject, 500)
            };
            insert_common_entity(
                &tx,
                &self.manifest().project_id,
                command,
                &id,
                "commit_observation",
                &title,
                "observed",
                OriginKind::Deterministic,
                "{}",
                &json!({"repository_id":input.repository_id,"commit_oid":commit.oid,
                    "first_observed_baseline_id":input.baseline_id}),
                &now,
            )?;
            tx.execute(
                "INSERT INTO git_commit_observations(entity_id,project_id,repository_id,
                    first_observed_baseline_id,commit_oid,tree_oid,parent_oids_json,author_name,
                    author_email,authored_at,committed_at,subject,observed_at,adapter_version)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
                params![
                    id,
                    self.manifest().project_id,
                    input.repository_id,
                    input.baseline_id,
                    commit.oid,
                    commit.tree_oid,
                    bounded_json(
                        &json!(commit.parents),
                        MAX_DEVELOPMENT_JSON_BYTES,
                        "commit parents"
                    )?,
                    commit.author_name,
                    commit.author_email,
                    commit.authored_at,
                    commit.committed_at,
                    commit.subject,
                    now,
                    GIT_ADAPTER_VERSION
                ],
            )?;
            for (ordinal, change) in commit.file_changes.iter().enumerate() {
                tx.execute(
                    "INSERT INTO git_commit_file_changes(commit_entity_id,ordinal,change_kind,
                        old_path,new_path,similarity) VALUES(?1,?2,?3,?4,?5,?6)",
                    params![
                        id,
                        ordinal as i64,
                        change.change_kind,
                        change.old_path,
                        change.new_path,
                        change.similarity.map(i64::from)
                    ],
                )?;
            }
            upsert_development_search(
                &tx,
                &self.manifest().project_id,
                &id,
                "commit_observation",
                &title,
                &format!("{} {}", commit.oid, commit.subject),
                &now,
            )?;
            ids.push(id);
        }
        let payload = json!({"repository_id":input.repository_id,"baseline_id":input.baseline_id,
            "reachable_commit_count":reachable_count,"ingested_commit_count":ids.len(),
            "remaining_commit_count":remaining});
        if !ids.is_empty() {
            let sequence = append_event_with_context(
                &tx,
                &self.manifest().project_id,
                command,
                Some(&input.repository_id),
                "development.commits.ingested",
                &payload,
            )?;
            insert_development_timeline(
                &tx,
                &self.manifest().project_id,
                sequence,
                Some(&input.repository_id),
                None,
                "development.commits.ingested",
                &payload,
                &command.actor.id,
                &now,
            )?;
        }
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "IngestGitCommits",
            Some(&input.baseline_id),
            None,
            &payload,
        )?;
        tx.commit()?;
        Ok(CommitIngestionResult {
            reachable_commits: reachable_count,
            previously_known_commits: previously_known,
            ingested_commit_ids: ids,
            remaining_commits: remaining,
        })
    }

    pub fn get_commit_observation(&self, id: &str) -> Result<CommitObservation> {
        let entity = self.get_entity(id)?;
        if entity.entity_type != "commit_observation" {
            return Err(CoreError::Validation(
                "entity is not a CommitObservation".into(),
            ));
        }
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT repository_id,first_observed_baseline_id,commit_oid,tree_oid,parent_oids_json,
                        author_name,author_email,authored_at,committed_at,subject,observed_at,adapter_version
                 FROM git_commit_observations WHERE entity_id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?, row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?, row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?, row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?, row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?, row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?, row.get::<_, String>(11)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        Ok(CommitObservation {
            entity,
            repository_id: raw.0,
            first_observed_baseline_id: raw.1,
            commit_oid: raw.2,
            tree_oid: raw.3,
            parent_oids: serde_json::from_str(&raw.4)?,
            author_name: raw.5,
            author_email: raw.6,
            authored_at: raw.7,
            committed_at: raw.8,
            subject: raw.9,
            observed_at: raw.10,
            adapter_version: raw.11,
            file_changes: read_file_changes(
                &connection,
                "git_commit_file_changes",
                "commit_entity_id",
                id,
            )?,
        })
    }
}

impl ContinuityStore {
    pub fn create_development_requirement(
        &self,
        command: &CommandContext,
        input: NewDevelopmentRequirement,
    ) -> Result<Entity> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        validate_nonempty(&input.title, 500, "requirement title")?;
        validate_nonempty(&input.statement, 20_000, "requirement statement")?;
        validate_text(
            &input.verification_method,
            10_000,
            "requirement verification method",
        )?;
        validate_string_list(&input.acceptance_criteria, "acceptance criteria")?;
        if input.priority > 4 {
            return Err(CoreError::Validation(
                "requirement priority must be within 0..=4".into(),
            ));
        }
        if input.rationale_origin == RequirementRationaleOrigin::Research {
            return Err(CoreError::Validation(
                "a research-origin Requirement must be created from its Decision in Research Space"
                    .into(),
            ));
        }
        let origin = match input.rationale_origin {
            RequirementRationaleOrigin::User => OriginKind::User,
            RequirementRationaleOrigin::Import => OriginKind::Import,
            RequirementRationaleOrigin::External => OriginKind::External,
            RequirementRationaleOrigin::Legacy => OriginKind::Legacy,
            RequirementRationaleOrigin::Unknown => OriginKind::Unknown,
            RequirementRationaleOrigin::Research => unreachable!(),
        };
        let metadata_json = bounded_json(
            &input.metadata,
            MAX_DEVELOPMENT_JSON_BYTES,
            "requirement metadata",
        )?;
        let criteria_json = bounded_json(
            &json!(input.acceptance_criteria),
            MAX_DEVELOPMENT_JSON_BYTES,
            "acceptance criteria",
        )?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "CreateDevelopmentRequirement",
        )? {
            tx.commit()?;
            return self.get_entity(&id);
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        insert_common_entity(
            &tx,
            &self.manifest().project_id,
            command,
            &id,
            "requirement",
            &input.title,
            "draft",
            origin,
            &metadata_json,
            &json!({"rationale_origin":input.rationale_origin.as_str(),"created_in_space":"development"}),
            &now,
        )?;
        tx.execute(
            "INSERT INTO requirements(entity_id,statement,acceptance_criteria_json,priority,
                rationale_origin,verification_method,created_in_space)
             VALUES(?1,?2,?3,?4,?5,?6,'development')",
            params![
                id,
                input.statement,
                criteria_json,
                input.priority,
                input.rationale_origin.as_str(),
                input.verification_method
            ],
        )?;
        upsert_development_search(
            &tx,
            &self.manifest().project_id,
            &id,
            "requirement",
            &input.title,
            &format!(
                "{}\n{}\n{}",
                input.statement,
                input.acceptance_criteria.join("\n"),
                input.verification_method
            ),
            &now,
        )?;
        let payload = json!({"requirement_id":id,"rationale_origin":input.rationale_origin.as_str(),
            "created_in_space":"development"});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&id),
            "development.requirement.created",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            None,
            Some(&id),
            "development.requirement.created",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "CreateDevelopmentRequirement",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        self.get_entity(&id)
    }

    pub fn transition_development_requirement(
        &self,
        command: &CommandContext,
        requirement_id: &str,
        expected_version: i64,
        target_status: &str,
    ) -> Result<Entity> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        validate_nonempty(target_status, 50, "target status")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "TransitionDevelopmentRequirement",
        )?
        .is_some()
        {
            tx.commit()?;
            return self.get_entity(requirement_id);
        }
        let raw: Option<(String, i64, String)> = tx
            .query_row(
                "SELECT entity_type,version,status FROM entities WHERE id=?1 AND project_id=?2",
                params![requirement_id, self.manifest().project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((kind, version, current)) = raw else {
            return Err(CoreError::NotFound(requirement_id.into()));
        };
        if kind != "requirement" {
            return Err(CoreError::Validation("entity is not a Requirement".into()));
        }
        if version != expected_version {
            return Err(CoreError::Conflict(format!(
                "requirement {requirement_id} version {expected_version} is stale"
            )));
        }
        validate_requirement_transition(&current, target_status)?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE entities SET status=?3,version=version+1,updated_at=?4,updated_by=?5,
                archived_at=CASE WHEN ?3='archived' THEN ?4 ELSE archived_at END
             WHERE id=?1 AND project_id=?2 AND version=?6",
            params![
                requirement_id,
                self.manifest().project_id,
                target_status,
                now,
                command.actor.id,
                expected_version
            ],
        )?;
        let payload = json!({"requirement_id":requirement_id,"previous_status":current,
            "target_status":target_status,"previous_version":expected_version});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(requirement_id),
            "development.requirement.transitioned",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            None,
            Some(requirement_id),
            "development.requirement.transitioned",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "TransitionDevelopmentRequirement",
            Some(requirement_id),
            Some(expected_version),
            &payload,
        )?;
        tx.commit()?;
        self.get_entity(requirement_id)
    }

    pub fn create_change_set(
        &self,
        command: &CommandContext,
        input: NewChangeSet,
    ) -> Result<ChangeSetRecord> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        validate_nonempty(&input.title, 500, "ChangeSet title")?;
        validate_nonempty(&input.summary, MAX_DEVELOPMENT_TEXT, "ChangeSet summary")?;
        if input.commit_entity_ids.len() > 500 {
            return Err(CoreError::Validation(
                "a ChangeSet may contain at most 500 commits".into(),
            ));
        }
        if input.requirement_links.len() > 100 {
            return Err(CoreError::Validation(
                "a ChangeSet may link at most 100 Requirements".into(),
            ));
        }
        ensure_unique(
            input.commit_entity_ids.iter().map(String::as_str),
            "duplicate commit in ChangeSet",
        )?;
        ensure_unique(
            input
                .requirement_links
                .iter()
                .map(|link| link.requirement_id.as_str()),
            "duplicate Requirement in ChangeSet",
        )?;
        let repository = self.get_repository(&input.repository_id)?;
        let baseline = self.get_repository_baseline(&input.baseline_id)?;
        if baseline.repository_id != input.repository_id {
            return Err(CoreError::Validation(
                "ChangeSet baseline belongs to another Repository".into(),
            ));
        }
        let mut commit_records = Vec::new();
        match input.kind {
            ChangeSetKind::WorkingTree => {
                if !input.commit_entity_ids.is_empty() {
                    return Err(CoreError::Validation(
                        "a working-tree ChangeSet cannot contain committed observations".into(),
                    ));
                }
                if baseline.worktree_status.is_empty() {
                    return Err(CoreError::Validation(
                        "cannot create an empty working-tree ChangeSet".into(),
                    ));
                }
                let live = inspect_live_state(Path::new(&repository.root_path))?;
                if live.head_oid != baseline.head_oid
                    || live.head_ref != baseline.head_ref
                    || live.worktree_fingerprint != baseline.worktree_fingerprint
                {
                    return Err(CoreError::Conflict(
                        "working tree no longer matches the selected baseline; observe a new baseline before creating the draft ChangeSet"
                            .into(),
                    ));
                }
            }
            ChangeSetKind::Committed => {
                if input.commit_entity_ids.is_empty() {
                    return Err(CoreError::Validation(
                        "a committed ChangeSet requires at least one CommitObservation".into(),
                    ));
                }
                let head = baseline.head_oid.as_deref().ok_or_else(|| {
                    CoreError::Validation(
                        "an unborn baseline cannot anchor a committed ChangeSet".into(),
                    )
                })?;
                for id in &input.commit_entity_ids {
                    let commit = self.get_commit_observation(id)?;
                    if commit.repository_id != input.repository_id {
                        return Err(CoreError::Validation(
                            "ChangeSet commit belongs to another Repository".into(),
                        ));
                    }
                    if !is_ancestor(Path::new(&repository.root_path), &commit.commit_oid, head)? {
                        return Err(CoreError::Validation(format!(
                            "commit {} is not reachable from the selected baseline",
                            commit.commit_oid
                        )));
                    }
                    commit_records.push(commit);
                }
            }
        }
        if let Some(previous_id) = &input.supersedes_change_set_id {
            let previous = self.get_change_set(previous_id)?;
            if previous.repository_id != input.repository_id {
                return Err(CoreError::Validation(
                    "a ChangeSet can only supersede one from the same Repository".into(),
                ));
            }
        }
        let metadata_json = bounded_json(
            &input.metadata,
            MAX_DEVELOPMENT_JSON_BYTES,
            "ChangeSet metadata",
        )?;
        let origin = match input.intent_origin {
            DevelopmentIntentOrigin::User | DevelopmentIntentOrigin::Research => OriginKind::User,
            DevelopmentIntentOrigin::Import => OriginKind::Import,
            DevelopmentIntentOrigin::External => OriginKind::External,
            DevelopmentIntentOrigin::Legacy => OriginKind::Legacy,
            DevelopmentIntentOrigin::Unknown => OriginKind::Unknown,
        };
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if let Some(Some(id)) =
            prior_result(&tx, &self.manifest().project_id, command, "CreateChangeSet")?
        {
            tx.commit()?;
            return self.get_change_set(&id);
        }
        let mut research_rationale_found = false;
        for link in &input.requirement_links {
            let rationale: Option<String> = tx
                .query_row(
                    "SELECT req.rationale_origin FROM requirements req JOIN entities e ON e.id=req.entity_id
                     WHERE req.entity_id=?1 AND e.project_id=?2",
                    params![link.requirement_id, self.manifest().project_id],
                    |row| row.get(0),
                )
                .optional()?;
            let Some(rationale) = rationale else {
                return Err(CoreError::Validation(format!(
                    "Requirement {} does not exist in this project",
                    link.requirement_id
                )));
            };
            research_rationale_found |= rationale == "research";
        }
        if input.intent_origin == DevelopmentIntentOrigin::Research && !research_rationale_found {
            return Err(CoreError::Validation(
                "research-origin ChangeSet requires a linked research-origin Requirement".into(),
            ));
        }
        let mut file_changes = if input.kind == ChangeSetKind::WorkingTree {
            baseline
                .worktree_status
                .iter()
                .map(working_entry_to_change)
                .collect::<Vec<_>>()
        } else {
            commit_records
                .iter()
                .flat_map(|commit| commit.file_changes.iter().cloned())
                .collect::<Vec<_>>()
        };
        if file_changes.len() > MAX_FILE_CHANGES {
            return Err(CoreError::Conflict(format!(
                "ChangeSet exceeds {MAX_FILE_CHANGES} file-change observations"
            )));
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        let status = if input.kind == ChangeSetKind::WorkingTree {
            "draft"
        } else {
            "observed"
        };
        insert_common_entity(
            &tx,
            &self.manifest().project_id,
            command,
            &id,
            "change_set",
            &input.title,
            status,
            origin,
            &metadata_json,
            &json!({"repository_id":input.repository_id,"baseline_id":input.baseline_id,
                "intent_origin":input.intent_origin.as_str()}),
            &now,
        )?;
        tx.execute(
            "INSERT INTO change_sets(entity_id,project_id,repository_id,baseline_id,change_set_kind,
                summary,intent_origin,worktree_fingerprint,supersedes_change_set_id,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                id,
                self.manifest().project_id,
                input.repository_id,
                input.baseline_id,
                input.kind.as_str(),
                input.summary,
                input.intent_origin.as_str(),
                (input.kind == ChangeSetKind::WorkingTree)
                    .then_some(baseline.worktree_fingerprint.as_str()),
                input.supersedes_change_set_id,
                now
            ],
        )?;
        for (ordinal, commit) in commit_records.iter().enumerate() {
            tx.execute(
                "INSERT INTO change_set_commits(change_set_id,commit_entity_id,ordinal)
                 VALUES(?1,?2,?3)",
                params![id, commit.entity.id, ordinal as i64],
            )?;
        }
        let mut source_commit_ids = Vec::new();
        if input.kind == ChangeSetKind::Committed {
            for commit in &commit_records {
                source_commit_ids.extend(std::iter::repeat_n(
                    Some(commit.entity.id.clone()),
                    commit.file_changes.len(),
                ));
            }
        } else {
            source_commit_ids.resize(file_changes.len(), None);
        }
        for (ordinal, (change, source_commit_id)) in file_changes
            .iter_mut()
            .zip(source_commit_ids.iter())
            .enumerate()
        {
            tx.execute(
                "INSERT INTO change_set_file_changes(change_set_id,ordinal,source_commit_entity_id,
                    change_kind,old_path,new_path,similarity) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    id,
                    ordinal as i64,
                    source_commit_id,
                    change.change_kind,
                    change.old_path,
                    change.new_path,
                    change.similarity.map(i64::from)
                ],
            )?;
        }
        for link in &input.requirement_links {
            insert_development_relationship(
                &tx,
                &self.manifest().project_id,
                command,
                &id,
                "change_set",
                &link.requirement_id,
                "requirement",
                link.relationship.as_str(),
            )?;
        }
        crate::code_intelligence::link_change_set_to_known_code_entities(
            &tx,
            &self.manifest().project_id,
            command,
            &id,
            &input.repository_id,
            &input.baseline_id,
        )?;
        if let Some(previous_id) = &input.supersedes_change_set_id {
            insert_development_relationship(
                &tx,
                &self.manifest().project_id,
                command,
                &id,
                "change_set",
                previous_id,
                "change_set",
                "supersedes",
            )?;
            tx.execute(
                "UPDATE entities SET status='superseded',version=version+1,updated_at=?3,updated_by=?4
                 WHERE id=?1 AND project_id=?2 AND entity_type='change_set'",
                params![previous_id, self.manifest().project_id, now, command.actor.id],
            )?;
        }
        upsert_development_search(
            &tx,
            &self.manifest().project_id,
            &id,
            "change_set",
            &input.title,
            &format!("{}\n{}", input.summary, input.intent_origin.as_str()),
            &now,
        )?;
        let payload = json!({"change_set_id":id,"repository_id":input.repository_id,
            "baseline_id":input.baseline_id,"kind":input.kind.as_str(),
            "commit_count":input.commit_entity_ids.len(),"file_change_count":file_changes.len(),
            "requirement_count":input.requirement_links.len(),"intent_origin":input.intent_origin.as_str(),
            "supersedes_change_set_id":input.supersedes_change_set_id});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&id),
            "development.change_set.created",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            Some(&input.repository_id),
            Some(&id),
            "development.change_set.created",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "CreateChangeSet",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        self.get_change_set(&id)
    }

    pub fn get_change_set(&self, id: &str) -> Result<ChangeSetRecord> {
        let entity = self.get_entity(id)?;
        if entity.entity_type != "change_set" {
            return Err(CoreError::Validation("entity is not a ChangeSet".into()));
        }
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT repository_id,baseline_id,change_set_kind,summary,intent_origin,
                        worktree_fingerprint,supersedes_change_set_id
                 FROM change_sets WHERE entity_id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        let mut statement = connection.prepare(
            "SELECT commit_entity_id FROM change_set_commits WHERE change_set_id=?1 ORDER BY ordinal",
        )?;
        let commits = statement
            .query_map([id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(ChangeSetRecord {
            entity,
            repository_id: raw.0,
            baseline_id: raw.1,
            kind: raw.2,
            summary: raw.3,
            intent_origin: raw.4,
            worktree_fingerprint: raw.5,
            supersedes_change_set_id: raw.6,
            commit_entity_ids: commits,
            file_changes: read_file_changes(
                &connection,
                "change_set_file_changes",
                "change_set_id",
                id,
            )?,
        })
    }

    pub fn link_change_set_requirement(
        &self,
        command: &CommandContext,
        change_set_id: &str,
        requirement_id: &str,
        relationship: RequirementImplementation,
    ) -> Result<String> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "LinkChangeSetRequirement",
        )? {
            tx.commit()?;
            return Ok(id);
        }
        require_entity_kind(
            &tx,
            &self.manifest().project_id,
            change_set_id,
            "change_set",
        )?;
        require_entity_kind(
            &tx,
            &self.manifest().project_id,
            requirement_id,
            "requirement",
        )?;
        if let Some(id) = tx
            .query_row(
                "SELECT id FROM relationships WHERE project_id=?1 AND source_entity_id=?2
                 AND relation_type=?3 AND target_entity_id=?4 AND status='active'",
                params![
                    self.manifest().project_id,
                    change_set_id,
                    relationship.as_str(),
                    requirement_id
                ],
                |row| row.get::<_, String>(0),
            )
            .optional()?
        {
            record_command_with_context(
                &tx,
                command,
                &self.manifest().project_id,
                "LinkChangeSetRequirement",
                Some(&id),
                None,
                &json!({"relationship_id":id,"already_linked":true}),
            )?;
            tx.commit()?;
            return Ok(id);
        }
        let id = insert_development_relationship(
            &tx,
            &self.manifest().project_id,
            command,
            change_set_id,
            "change_set",
            requirement_id,
            "requirement",
            relationship.as_str(),
        )?;
        let now = Utc::now().to_rfc3339();
        let payload = json!({"relationship_id":id,"change_set_id":change_set_id,
            "requirement_id":requirement_id,"relation_type":relationship.as_str()});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&id),
            "development.requirement.linked",
            &payload,
        )?;
        let repository_id: String = tx.query_row(
            "SELECT repository_id FROM change_sets WHERE entity_id=?1",
            [change_set_id],
            |row| row.get(0),
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            Some(&repository_id),
            Some(change_set_id),
            "development.requirement.linked",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "LinkChangeSetRequirement",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn transition_change_set(
        &self,
        command: &CommandContext,
        change_set_id: &str,
        expected_version: i64,
        target_status: &str,
    ) -> Result<ChangeSetRecord> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "TransitionChangeSet",
        )?
        .is_some()
        {
            tx.commit()?;
            return self.get_change_set(change_set_id);
        }
        let raw: Option<(i64, String, String)> = tx
            .query_row(
                "SELECT e.version,e.status,cs.repository_id FROM entities e JOIN change_sets cs ON cs.entity_id=e.id
                 WHERE e.id=?1 AND e.project_id=?2",
                params![change_set_id, self.manifest().project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((version, current, repository_id)) = raw else {
            return Err(CoreError::NotFound(change_set_id.into()));
        };
        if version != expected_version {
            return Err(CoreError::Conflict(format!(
                "ChangeSet {change_set_id} version {expected_version} is stale"
            )));
        }
        validate_change_set_transition(&current, target_status)?;
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE entities SET status=?3,version=version+1,updated_at=?4,updated_by=?5,
                archived_at=CASE WHEN ?3='archived' THEN ?4 ELSE archived_at END
             WHERE id=?1 AND project_id=?2 AND version=?6",
            params![
                change_set_id,
                self.manifest().project_id,
                target_status,
                now,
                command.actor.id,
                expected_version
            ],
        )?;
        let payload = json!({"change_set_id":change_set_id,"previous_status":current,
            "target_status":target_status,"previous_version":expected_version});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(change_set_id),
            "development.change_set.transitioned",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            sequence,
            Some(&repository_id),
            Some(change_set_id),
            "development.change_set.transitioned",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "TransitionChangeSet",
            Some(change_set_id),
            Some(expected_version),
            &payload,
        )?;
        tx.commit()?;
        self.get_change_set(change_set_id)
    }

    pub fn search_development(
        &self,
        query: DevelopmentSearchQuery,
    ) -> Result<DevelopmentSearchPage> {
        validate_page(query.page)?;
        validate_text(&query.text, 1_000, "Development search text")?;
        if let Some(kind) = &query.entity_type {
            validate_nonempty(kind, 100, "Development search entity type")?;
        }
        if let Some(status) = &query.status {
            validate_nonempty(status, 50, "Development search status")?;
        }
        let offset = i64::try_from(query.page.offset)
            .map_err(|_| CoreError::Validation("search page offset is too large".into()))?;
        let connection = self.connection()?;
        require_development_enabled(&connection, &self.manifest().project_id)?;
        let mut statement = connection.prepare(
            "SELECT d.entity_id,d.entity_type,e.title,e.status,substr(d.body,1,300)
             FROM development_search_documents d JOIN entities e ON e.id=d.entity_id
             WHERE d.project_id=?1
               AND (?2='' OR instr(lower(d.title || ' ' || d.body),lower(?2))>0)
               AND (?3 IS NULL OR d.entity_type=?3)
               AND (?4 IS NULL OR e.status=?4)
             ORDER BY e.updated_at DESC,e.id LIMIT ?5 OFFSET ?6",
        )?;
        let rows = statement
            .query_map(
                params![
                    self.manifest().project_id,
                    query.text,
                    query.entity_type,
                    query.status,
                    query.page.limit + 1,
                    offset
                ],
                |row| {
                    Ok(DevelopmentSearchHit {
                        entity_id: row.get(0)?,
                        entity_type: row.get(1)?,
                        title: row.get(2)?,
                        status: row.get(3)?,
                        snippet: row.get(4)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = rows.len() > query.page.limit as usize;
        let items = rows
            .into_iter()
            .take(query.page.limit as usize)
            .collect::<Vec<_>>();
        Ok(DevelopmentSearchPage {
            next_offset: has_more.then_some(query.page.offset + items.len() as u64),
            items,
        })
    }

    pub fn development_timeline(
        &self,
        filter: DevelopmentTimelineFilter,
        page: PageRequest,
    ) -> Result<DevelopmentTimelinePage> {
        validate_page(page)?;
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("timeline page offset is too large".into()))?;
        let connection = self.connection()?;
        require_development_enabled(&connection, &self.manifest().project_id)?;
        let mut statement = connection.prepare(
            "SELECT id,ledger_sequence,repository_id,entity_id,event_type,payload_json,actor_id,occurred_at
             FROM development_timeline WHERE project_id=?1
               AND (?2 IS NULL OR repository_id=?2)
               AND (?3 IS NULL OR entity_id=?3)
               AND (?4 IS NULL OR event_type=?4)
             ORDER BY ledger_sequence,id LIMIT ?5 OFFSET ?6",
        )?;
        let rows = statement
            .query_map(
                params![
                    self.manifest().project_id,
                    filter.repository_id,
                    filter.entity_id,
                    filter.event_type,
                    page.limit + 1,
                    offset
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = rows.len() > page.limit as usize;
        let items = rows
            .into_iter()
            .take(page.limit as usize)
            .map(|raw| {
                Ok(DevelopmentTimelineEntry {
                    id: raw.0,
                    ledger_sequence: raw.1,
                    repository_id: raw.2,
                    entity_id: raw.3,
                    event_type: raw.4,
                    payload: serde_json::from_str(&raw.5)?,
                    actor_id: raw.6,
                    occurred_at: raw.7,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(DevelopmentTimelinePage {
            next_offset: has_more.then_some(page.offset + items.len() as u64),
            items,
        })
    }

    pub fn create_development_checkpoint(
        &self,
        command: &CommandContext,
        input: DevelopmentCheckpointInput,
    ) -> Result<Checkpoint> {
        validate_command_context(command)?;
        reject_ai_actor(command)?;
        validate_text(&input.note, 20_000, "checkpoint note")?;
        validate_string_list(&input.blockers, "checkpoint blockers")?;
        validate_string_list(&input.next_actions, "checkpoint next actions")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_development_enabled(&tx, &self.manifest().project_id)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "CreateDevelopmentCheckpoint",
        )? {
            tx.commit()?;
            return self.get_checkpoint(&id);
        }
        let repositories = latest_repository_states(&tx, &self.manifest().project_id)?;
        let attached_repository_count: i64 = tx.query_row(
            "SELECT count(*) FROM repositories WHERE project_id=?1",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        if attached_repository_count as usize != repositories.len() {
            return Err(CoreError::Validation(
                "every attached Repository requires an observed baseline before creating a Development Checkpoint"
                    .into(),
            ));
        }
        let active_items = active_development_items(&tx, &self.manifest().project_id, 2_001)?;
        let intelligence_items = latest_intelligence_items(&tx, &self.manifest().project_id)?;
        let source_count = repositories.len() * 2 + active_items.len() + intelligence_items.len();
        if source_count > MAX_CHECKPOINT_SOURCES {
            return Err(CoreError::Conflict(format!(
                "development checkpoint has {source_count} sources; narrow or archive state before exceeding {MAX_CHECKPOINT_SOURCES}"
            )));
        }
        let sequence: i64 = tx.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        let summary = json!({"schema_version":2,"scope":"development",
            "source_ledger_sequence":sequence,"note":input.note,"repositories":repositories,
            "active_items":active_items.iter().map(|item|json!({"id":item.id,"type":item.kind,"status":item.status})).collect::<Vec<_>>(),
            "code_intelligence":intelligence_items.iter().map(|item|json!({"id":item.id,"type":item.kind,"status":item.status})).collect::<Vec<_>>(),
            "blockers":input.blockers,"next_actions":input.next_actions,"research_required":false,"complete":false});
        let summary_json =
            bounded_json(&summary, 2 * 1024 * 1024, "development checkpoint summary")?;
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO checkpoints(id,project_id,scope,ledger_sequence,summary_json,created_at,created_by)
             VALUES(?1,?2,'development',?3,?4,?5,?6)",
            params![id, self.manifest().project_id, sequence, summary_json, now, command.actor.id],
        )?;
        let mut sources = HashMap::new();
        for repository in &repositories {
            sources.insert(
                repository.repository_id.clone(),
                repository.repository_version,
            );
            sources.insert(repository.baseline_id.clone(), repository.baseline_version);
        }
        for item in &active_items {
            sources.insert(item.id.clone(), item.version);
        }
        for item in &intelligence_items {
            sources.insert(item.id.clone(), item.version);
        }
        for (source_id, version) in sources {
            tx.execute(
                "INSERT INTO checkpoint_sources(checkpoint_id,source_entity_id,source_version)
                 VALUES(?1,?2,?3)",
                params![id, source_id, version],
            )?;
        }
        let payload = json!({"checkpoint_id":id,"source_ledger_sequence":sequence,
            "repository_count":repositories.len(),"active_item_count":active_items.len()});
        let event_sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&id),
            "development.checkpoint.created",
            &payload,
        )?;
        insert_development_timeline(
            &tx,
            &self.manifest().project_id,
            event_sequence,
            None,
            None,
            "development.checkpoint.created",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "CreateDevelopmentCheckpoint",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        self.get_checkpoint(&id)
    }

    pub fn resume_development(&self) -> Result<DevelopmentResumeState> {
        let connection = self.connection()?;
        let raw: Option<(String, i64, String)> = connection
            .query_row(
                "SELECT id,ledger_sequence,summary_json FROM checkpoints
                 WHERE project_id=?1 AND scope='development' ORDER BY created_at DESC,id DESC LIMIT 1",
                [&self.manifest().project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let mut repository_diverged = false;
        let mut repository_state_unavailable = false;
        let (checkpoint, checkpoint_sequence, blockers, next_actions) =
            if let Some((id, sequence, summary_json)) = raw {
                let summary: Value = serde_json::from_str(&summary_json)?;
                for state in summary
                    .get("repositories")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(repository_id) = state.get("repository_id").and_then(Value::as_str)
                    else {
                        continue;
                    };
                    let saved_head = state.get("head_oid").and_then(Value::as_str);
                    let saved_fingerprint = state
                        .get("worktree_fingerprint")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    match self
                        .get_repository(repository_id)
                        .and_then(|repository| inspect_live_state(Path::new(&repository.root_path)))
                    {
                        Ok(live) => {
                            if live.head_oid.as_deref() != saved_head
                                || live.worktree_fingerprint != saved_fingerprint
                            {
                                repository_diverged = true;
                            }
                        }
                        Err(_) => repository_state_unavailable = true,
                    }
                }
                (
                    Some(self.get_checkpoint(&id)?),
                    sequence,
                    json_string_list(&summary, "blockers"),
                    json_string_list(&summary, "next_actions"),
                )
            } else {
                (None, 0, Vec::new(), Vec::new())
            };
        let events: i64 = connection.query_row(
            "SELECT count(*) FROM development_timeline WHERE project_id=?1 AND ledger_sequence>?2
             AND event_type<>'development.checkpoint.created'",
            params![self.manifest().project_id, checkpoint_sequence],
            |row| row.get(0),
        )?;
        let items = active_development_items(&connection, &self.manifest().project_id, 101)?;
        let total: i64 = connection.query_row(
            "SELECT count(*) FROM entities WHERE project_id=?1
             AND ((entity_type='change_set' AND status IN ('draft','observed','linked','validated'))
               OR (entity_type='requirement' AND status IN ('draft','accepted','in_progress','blocked','implemented')))",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        let active_items = items
            .into_iter()
            .take(100)
            .map(|item| json!({"id":item.id,"type":item.kind,"title":item.title,"status":item.status}))
            .collect::<Vec<_>>();
        Ok(DevelopmentResumeState {
            checkpoint_is_stale: checkpoint.is_some()
                && (events > 0 || repository_diverged || repository_state_unavailable),
            checkpoint,
            events_since_checkpoint: events as u64,
            active_item_count: total as u64,
            active_items_truncated: total as usize > active_items.len(),
            active_items,
            blockers,
            next_actions,
            repository_diverged,
            repository_state_unavailable,
        })
    }

    pub fn generate_development_report(&self) -> Result<DevelopmentReport> {
        let connection = self.connection()?;
        let sequence: i64 = connection.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        let row_count: i64 = connection.query_row(
            "SELECT count(*) FROM entities WHERE project_id=?1 AND entity_type IN
                ('repository','repository_baseline','commit_observation','change_set','requirement',
                 'analysis_run','code_entity','test','test_run')",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        if row_count > 5_000 {
            return Err(CoreError::Conflict(
                "development report exceeds 5000 records; use a scoped report in CP8".into(),
            ));
        }
        let mut markdown = format!(
            "# Development Report — {}\n\nProject ID: `{}`  \nSource ledger sequence: `{sequence}`\n\n",
            report_text(&self.manifest().name, 300),
            self.manifest().project_id
        );
        let mut cited = Vec::new();
        markdown.push_str("## Repositories\n\n");
        let mut statement = connection.prepare(
            "SELECT e.id,e.title,r.root_fingerprint,
                (SELECT rr.current_baseline_id FROM repository_reconciliations rr
                 WHERE rr.repository_id=r.entity_id ORDER BY rr.observed_at DESC,rr.id DESC LIMIT 1)
             FROM repositories r JOIN entities e ON e.id=r.entity_id
             WHERE r.project_id=?1 ORDER BY e.created_at,e.id",
        )?;
        let repositories = statement
            .query_map([&self.manifest().project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, title, fingerprint, baseline_id) in repositories {
            cited.push(id.clone());
            if let Some(baseline_id) = baseline_id {
                let baseline = self.get_repository_baseline(&baseline_id)?;
                cited.push(baseline_id.clone());
                writeln!(
                    markdown,
                    "- **{}** (`{id}`): root `{}`, baseline `{}` at `{}`, {}, relation `{}`",
                    report_text(&title, 300),
                    &fingerprint[..12],
                    baseline_id,
                    baseline.head_oid.as_deref().unwrap_or("unborn"),
                    if baseline.worktree_status.is_empty() {
                        "clean"
                    } else {
                        "dirty"
                    },
                    baseline.relation_to_previous
                )
                .expect("writing to String cannot fail");
            } else {
                writeln!(
                    markdown,
                    "- **{}** (`{id}`): attached; no baseline yet",
                    report_text(&title, 300)
                )
                .expect("writing to String cannot fail");
            }
        }
        markdown.push_str("\n## Requirements\n\n");
        let mut statement = connection.prepare(
            "SELECT e.id,e.title,e.status,req.statement,req.rationale_origin,req.created_in_space
             FROM requirements req JOIN entities e ON e.id=req.entity_id
             WHERE e.project_id=?1 AND e.status<>'archived' ORDER BY e.created_at,e.id",
        )?;
        let requirements = statement
            .query_map([&self.manifest().project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, title, status, requirement, rationale, created_in_space) in requirements {
            cited.push(id.clone());
            writeln!(
                markdown,
                "- **{}** (`{id}`, `{status}`): {} — rationale `{rationale}`, created in `{created_in_space}`",
                report_text(&title, 300),
                report_text(&requirement, 500)
            )
            .expect("writing to String cannot fail");
        }
        markdown.push_str("\n## ChangeSets\n\n");
        let mut statement = connection.prepare(
            "SELECT e.id,e.title,e.status,cs.summary,cs.intent_origin,cs.change_set_kind,
                    (SELECT count(*) FROM change_set_commits csc WHERE csc.change_set_id=cs.entity_id),
                    (SELECT count(*) FROM change_set_file_changes f WHERE f.change_set_id=cs.entity_id)
             FROM change_sets cs JOIN entities e ON e.id=cs.entity_id
             WHERE e.project_id=?1 AND e.status<>'archived' ORDER BY e.created_at,e.id",
        )?;
        let changes = statement
            .query_map([&self.manifest().project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, title, status, summary, origin, kind, commit_count, file_count) in changes {
            cited.push(id.clone());
            writeln!(
                markdown,
                "- **{}** (`{id}`, `{status}`): {} — `{kind}`, intent `{origin}`, {commit_count} commits, {file_count} file observations",
                report_text(&title, 300),
                report_text(&summary, 600)
            )
            .expect("writing to String cannot fail");
        }
        markdown.push_str("\n## Code Intelligence\n\n");
        let mut statement = connection.prepare(
            "SELECT e.id,ar.completeness,ar.file_count,ar.analyzed_file_count,
                    ar.code_entity_count,ar.test_count,ar.limitation_count,
                    ar.analyzer_bundle_version,ar.baseline_id
             FROM analysis_runs ar JOIN entities e ON e.id=ar.entity_id
             WHERE ar.project_id=?1 ORDER BY ar.completed_at,e.id",
        )?;
        let runs = statement
            .query_map([&self.manifest().project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, String>(8)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, completeness, files, analyzed, entities, tests, limitations, version, baseline) in
            runs
        {
            cited.push(id.clone());
            writeln!(markdown,
                "- Analysis `{id}` at baseline `{baseline}`: `{completeness}`, {analyzed}/{files} structurally analyzed files, {entities} CodeEntities, {tests} Tests, {limitations} explicit limitations; bundle `{version}`."
            ).expect("writing to String cannot fail");
        }
        let code_count: i64 = connection.query_row(
            "SELECT count(*) FROM code_entities WHERE project_id=?1",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        let test_count: i64 = connection.query_row(
            "SELECT count(*) FROM tests WHERE project_id=?1",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        writeln!(
            markdown,
            "\n- Current addressable inventory: {code_count} CodeEntities and {test_count} Tests."
        )
        .expect("writing to String cannot fail");
        markdown.push_str("\n## Test Runs\n\n");
        let mut statement = connection.prepare(
            "SELECT e.id,e.title,tr.outcome,tr.command_label,tr.baseline_id,
                    (SELECT count(*) FROM test_run_results rr WHERE rr.test_run_id=tr.entity_id)
             FROM test_runs tr JOIN entities e ON e.id=tr.entity_id
             WHERE tr.project_id=?1 ORDER BY tr.observed_at,e.id",
        )?;
        let test_runs = statement
            .query_map([&self.manifest().project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (id, title, outcome, command_label, baseline, count) in test_runs {
            cited.push(id.clone());
            writeln!(markdown,"- **{}** (`{id}`): `{outcome}`, command `{}`, baseline `{baseline}`, {count} Test results.",
                report_text(&title,300),report_text(&command_label,500))
                .expect("writing to String cannot fail");
        }
        let commit_count: i64 = connection.query_row(
            "SELECT count(*) FROM git_commit_observations WHERE project_id=?1",
            [&self.manifest().project_id],
            |row| row.get(0),
        )?;
        writeln!(
            markdown,
            "\n## Repository History\n\n- {commit_count} immutable CommitObservations ingested.\n- Git remains authoritative; this report contains local observations, not a replacement repository."
        )
        .expect("writing to String cannot fail");
        cited.sort();
        cited.dedup();
        Ok(DevelopmentReport {
            project_id: self.manifest().project_id.clone(),
            source_ledger_sequence: sequence,
            cited_entity_ids: cited,
            markdown,
        })
    }
}

#[derive(Debug)]
struct ActiveDevelopmentItem {
    id: String,
    kind: String,
    title: String,
    status: String,
    version: i64,
}

#[derive(Debug, Serialize)]
struct LatestRepositoryState {
    repository_id: String,
    repository_version: i64,
    baseline_id: String,
    baseline_version: i64,
    head_oid: Option<String>,
    head_ref: Option<String>,
    worktree_fingerprint: String,
}

fn latest_repository_states(
    connection: &Connection,
    project_id: &str,
) -> Result<Vec<LatestRepositoryState>> {
    let mut statement = connection.prepare(
        "SELECT r.entity_id,re.version,rb.entity_id,be.version,rb.head_oid,rb.head_ref,rb.worktree_fingerprint
         FROM repositories r JOIN entities re ON re.id=r.entity_id
         JOIN repository_reconciliations rr ON rr.id=(
             SELECT rr2.id FROM repository_reconciliations rr2 WHERE rr2.repository_id=r.entity_id
             ORDER BY rr2.observed_at DESC,rr2.id DESC LIMIT 1)
         JOIN repository_baselines rb ON rb.entity_id=rr.current_baseline_id
         JOIN entities be ON be.id=rb.entity_id
         WHERE r.project_id=?1 ORDER BY r.entity_id",
    )?;
    statement
        .query_map([project_id], |row| {
            Ok(LatestRepositoryState {
                repository_id: row.get(0)?,
                repository_version: row.get(1)?,
                baseline_id: row.get(2)?,
                baseline_version: row.get(3)?,
                head_oid: nonempty_option(row.get(4)?),
                head_ref: nonempty_option(row.get(5)?),
                worktree_fingerprint: row.get(6)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn active_development_items(
    connection: &Connection,
    project_id: &str,
    limit: usize,
) -> Result<Vec<ActiveDevelopmentItem>> {
    let mut statement = connection.prepare(
        "SELECT id,entity_type,title,status,version FROM entities WHERE project_id=?1
         AND ((entity_type='change_set' AND status IN ('draft','observed','linked','validated'))
           OR (entity_type='requirement' AND status IN ('draft','accepted','in_progress','blocked','implemented')))
         ORDER BY entity_type,created_at,id LIMIT ?2",
    )?;
    statement
        .query_map(params![project_id, limit as i64], |row| {
            Ok(ActiveDevelopmentItem {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                version: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn latest_intelligence_items(
    connection: &Connection,
    project_id: &str,
) -> Result<Vec<ActiveDevelopmentItem>> {
    let mut statement = connection.prepare(
        "SELECT e.id,e.entity_type,e.title,e.status,e.version FROM entities e
         WHERE e.project_id=?1 AND (
           e.id IN (SELECT ar.entity_id FROM analysis_runs ar WHERE ar.project_id=?1
             AND ar.entity_id=(SELECT ar2.entity_id FROM analysis_runs ar2
               WHERE ar2.repository_id=ar.repository_id ORDER BY ar2.completed_at DESC,ar2.entity_id DESC LIMIT 1))
           OR e.id IN (SELECT tr.entity_id FROM test_runs tr WHERE tr.project_id=?1
             AND tr.entity_id=(SELECT tr2.entity_id FROM test_runs tr2
               WHERE tr2.repository_id=tr.repository_id ORDER BY tr2.observed_at DESC,tr2.entity_id DESC LIMIT 1)))
         ORDER BY e.entity_type,e.id",
    )?;
    statement
        .query_map([project_id], |row| {
            Ok(ActiveDevelopmentItem {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                version: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn working_entry_to_change(entry: &WorkingTreeEntry) -> FileChange {
    let code = if entry.index_status != " " {
        entry.index_status.chars().next().unwrap_or('X')
    } else {
        entry.worktree_status.chars().next().unwrap_or('X')
    };
    FileChange {
        change_kind: match code {
            '?' | 'A' => "added",
            'C' => "copied",
            'D' => "deleted",
            'M' => "modified",
            'R' => "renamed",
            'T' => "type_changed",
            'U' => "unmerged",
            _ => "unknown",
        }
        .into(),
        old_path: entry
            .original_path
            .clone()
            .or_else(|| (code == 'D').then_some(entry.path.clone())),
        new_path: entry.path.clone(),
        similarity: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn insert_development_relationship(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    source_id: &str,
    source_type: &str,
    target_id: &str,
    target_type: &str,
    relation_type: &str,
) -> Result<String> {
    validate_development_relationship_pair(source_type, relation_type, target_type)?;
    require_entity_kind(tx, project_id, source_id, source_type)?;
    require_entity_kind(tx, project_id, target_id, target_type)?;
    let id = new_id();
    let now = Utc::now().to_rfc3339();
    tx.execute(
        "INSERT INTO relationships(id,project_id,relation_type,relation_version,source_entity_id,
            source_entity_type,target_entity_id,target_entity_type,status,origin_type,actor_id,
            confidence,review_state,direct_source_ids_json,supersedes_id,created_at,updated_at)
         VALUES(?1,?2,?3,1,?4,?5,?6,?7,'active','user',?8,NULL,'accepted','[]',NULL,?9,?9)",
        params![
            id,
            project_id,
            relation_type,
            source_id,
            source_type,
            target_id,
            target_type,
            command.actor.id,
            now
        ],
    )?;
    Ok(id)
}

fn require_entity_kind(
    connection: &Connection,
    project_id: &str,
    id: &str,
    expected: &str,
) -> Result<()> {
    let kind: Option<String> = connection
        .query_row(
            "SELECT entity_type FROM entities WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| row.get(0),
        )
        .optional()?;
    match kind.as_deref() {
        Some(value) if value == expected => Ok(()),
        Some(value) => Err(CoreError::Validation(format!(
            "entity {id} is {value}, expected {expected}"
        ))),
        None => Err(CoreError::NotFound(id.into())),
    }
}

fn validate_requirement_transition(current: &str, target: &str) -> Result<()> {
    let allowed = matches!(
        (current, target),
        ("draft", "accepted" | "retired" | "archived")
            | (
                "accepted",
                "in_progress" | "blocked" | "retired" | "superseded"
            )
            | ("in_progress", "implemented" | "blocked" | "retired")
            | ("blocked", "in_progress" | "retired" | "superseded")
            | ("implemented", "verified" | "blocked" | "retired")
            | ("verified", "retired" | "superseded")
            | ("superseded" | "retired", "archived")
    );
    if allowed {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "invalid requirement lifecycle transition {current} -> {target}"
        )))
    }
}

fn validate_change_set_transition(current: &str, target: &str) -> Result<()> {
    let allowed = matches!(
        (current, target),
        ("draft", "abandoned" | "superseded" | "archived")
            | (
                "observed",
                "linked" | "abandoned" | "superseded" | "archived"
            )
            | (
                "linked",
                "validated" | "abandoned" | "superseded" | "archived"
            )
            | (
                "validated",
                "closed" | "reverted" | "superseded" | "archived"
            )
            | ("closed", "reverted" | "superseded" | "archived")
            | ("reverted" | "abandoned" | "superseded", "archived")
    );
    if allowed {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "invalid ChangeSet lifecycle transition {current} -> {target}"
        )))
    }
}

fn validate_string_list(items: &[String], label: &str) -> Result<()> {
    if items.len() > 100 {
        return Err(CoreError::Validation(format!(
            "{label} may contain at most 100 items"
        )));
    }
    for item in items {
        validate_nonempty(item, 10_000, label)?;
    }
    Ok(())
}

fn validate_page(page: PageRequest) -> Result<()> {
    if page.limit == 0 || page.limit > 100 {
        return Err(CoreError::Validation(
            "Development page limit must be within 1..=100".into(),
        ));
    }
    Ok(())
}

fn ensure_unique<'a>(items: impl Iterator<Item = &'a str>, message: &str) -> Result<()> {
    let mut seen = HashSet::new();
    if items.into_iter().all(|value| seen.insert(value)) {
        Ok(())
    } else {
        Err(CoreError::Validation(message.into()))
    }
}

fn json_string_list(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

fn report_text(value: &str, max_chars: usize) -> String {
    let flattened = value.replace(['\r', '\n'], " ");
    let escaped = flattened
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let mut result = escaped.chars().take(max_chars).collect::<String>();
    if escaped.chars().count() > max_chars {
        result.push('…');
    }
    result
}

pub(crate) fn append_development_integrity_issues(
    connection: &Connection,
    project_id: &str,
    report: &mut IntegrityReport,
) -> Result<()> {
    for (kind, table) in [
        ("repository", "repositories"),
        ("repository_baseline", "repository_baselines"),
        ("commit_observation", "git_commit_observations"),
        ("change_set", "change_sets"),
    ] {
        let sql = format!(
            "SELECT e.id FROM entities e LEFT JOIN {table} d ON d.entity_id=e.id
             WHERE e.project_id=?1 AND e.entity_type=?2 AND d.entity_id IS NULL"
        );
        let mut statement = connection.prepare(&sql)?;
        let ids = statement
            .query_map(params![project_id, kind], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for id in ids {
            report.issues.push(IntegrityIssue {
                code: "missing_development_detail".into(),
                path_or_id: id,
                guidance: format!(
                    "Restore the normalized {kind} row from a verified backup; do not infer missing repository state."
                ),
            });
        }
    }
    let mut statement = connection.prepare(
        "SELECT e.id FROM entities e LEFT JOIN development_search_documents d ON d.entity_id=e.id
         LEFT JOIN requirements req ON req.entity_id=e.id
         WHERE e.project_id=?1
         AND (e.entity_type IN ('repository','commit_observation','change_set')
              OR (e.entity_type='requirement' AND req.created_in_space='development'))
         AND d.entity_id IS NULL",
    )?;
    let ids = statement
        .query_map([project_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for id in ids {
        report.issues.push(IntegrityIssue {
            code: "missing_development_search_projection".into(),
            path_or_id: id,
            guidance:
                "Rebuild the deterministic Development search projection from canonical rows."
                    .into(),
        });
    }
    let mut statement = connection.prepare(
         "SELECT id,source_entity_type,relation_type,target_entity_type FROM relationships
         WHERE project_id=?1 AND (source_entity_type IN
            ('repository','repository_baseline','commit_observation','change_set','analysis_run','code_entity','test','test_run')
            OR target_entity_type IN
            ('repository','repository_baseline','commit_observation','change_set','analysis_run','code_entity','test','test_run'))",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, source, relation, target) in rows {
        if validate_development_relationship_pair(&source, &relation, &target).is_err() {
            report.issues.push(IntegrityIssue {
                code: "invalid_development_relationship".into(),
                path_or_id: id,
                guidance: "Archive or repair the relationship through a reviewed migration; preserve its audit history."
                    .into(),
            });
        }
    }
    let mut statement = connection.prepare(
        "SELECT cs.entity_id,cs.change_set_kind,
            (SELECT count(*) FROM change_set_commits csc WHERE csc.change_set_id=cs.entity_id)
         FROM change_sets cs WHERE cs.project_id=?1",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, kind, commits) in rows {
        if (kind == "working_tree" && commits != 0) || (kind == "committed" && commits == 0) {
            report.issues.push(IntegrityIssue {
                code: "invalid_change_set_material".into(),
                path_or_id: id,
                guidance: "Restore the ChangeSet membership from a verified backup; do not guess commit membership."
                    .into(),
            });
        }
    }
    Ok(())
}

#[derive(Debug)]
struct PriorBaseline {
    id: String,
    head_oid: Option<String>,
    head_ref: Option<String>,
    worktree_fingerprint: String,
}

fn latest_baseline_raw(
    connection: &Connection,
    project_id: &str,
    repository_id: &str,
) -> Result<Option<PriorBaseline>> {
    connection
        .query_row(
            "SELECT rb.entity_id,rb.head_oid,rb.head_ref,rb.worktree_fingerprint
             FROM repository_reconciliations rr
             JOIN repository_baselines rb ON rb.entity_id=rr.current_baseline_id
             WHERE rr.project_id=?1 AND rr.repository_id=?2
             ORDER BY rr.observed_at DESC,rr.id DESC LIMIT 1",
            params![project_id, repository_id],
            |row| {
                Ok(PriorBaseline {
                    id: row.get(0)?,
                    head_oid: nonempty_option(row.get(1)?),
                    head_ref: nonempty_option(row.get(2)?),
                    worktree_fingerprint: row.get(3)?,
                })
            },
        )
        .optional()
        .map_err(Into::into)
}

fn classify_baseline_relation(
    root: &Path,
    previous: Option<&PriorBaseline>,
    current: &LiveRepositoryState,
) -> Result<BaselineRelation> {
    let Some(previous) = previous else {
        return Ok(if current.head_oid.is_none() {
            BaselineRelation::Unborn
        } else if current.head_ref.is_none() {
            BaselineRelation::DetachedHead
        } else {
            BaselineRelation::Initial
        });
    };
    if current.head_oid.is_none() {
        return Ok(BaselineRelation::Unborn);
    }
    if current.head_ref.is_none() {
        return Ok(BaselineRelation::DetachedHead);
    }
    if previous.head_oid == current.head_oid && previous.head_ref == current.head_ref {
        return Ok(
            if previous.worktree_fingerprint == current.worktree_fingerprint {
                BaselineRelation::Unchanged
            } else {
                BaselineRelation::WorktreeChanged
            },
        );
    }
    if previous.head_ref != current.head_ref {
        return Ok(BaselineRelation::BranchSwitch);
    }
    if let (Some(old), Some(new)) = (previous.head_oid.as_deref(), current.head_oid.as_deref())
        && is_ancestor(root, old, new)?
    {
        return Ok(BaselineRelation::FastForward);
    }
    Ok(BaselineRelation::HistoryRewrite)
}

fn inspect_repository(path: &Path) -> Result<RepositoryInspection> {
    let requested = fs::canonicalize(path).map_err(|error| {
        CoreError::Validation(format!("repository path cannot be resolved: {error}"))
    })?;
    let top = git_text(&requested, &["rev-parse", "--show-toplevel"])?;
    let top = fs::canonicalize(top.trim()).map_err(|error| {
        CoreError::Validation(format!("Git top-level path cannot be resolved: {error}"))
    })?;
    if requested != top {
        return Err(CoreError::Validation(format!(
            "repository attachment requires its exact top-level directory: {}",
            top.display()
        )));
    }
    let bare = git_text(&top, &["rev-parse", "--is-bare-repository"])?;
    if bare.trim() != "false" {
        return Err(CoreError::Validation(
            "bare repositories are outside the CP4 Development Space contract".into(),
        ));
    }
    validate_repository_local_config(&top)?;
    let common_raw = git_text(&top, &["rev-parse", "--git-common-dir"])?;
    let common = Path::new(common_raw.trim());
    let common = if common.is_absolute() {
        common.to_path_buf()
    } else {
        top.join(common)
    };
    let common = fs::canonicalize(common).map_err(|error| {
        CoreError::Validation(format!("Git common directory cannot be resolved: {error}"))
    })?;
    let object_format = git_text(&top, &["rev-parse", "--show-object-format"])?
        .trim()
        .to_owned();
    if !matches!(object_format.as_str(), "sha1" | "sha256") {
        return Err(CoreError::Validation(format!(
            "unsupported Git object format: {object_format}"
        )));
    }
    let root_path = top
        .to_str()
        .ok_or_else(|| CoreError::Validation("repository path must be valid UTF-8".into()))?
        .to_owned();
    let common_path = common
        .to_str()
        .ok_or_else(|| CoreError::Validation("Git common path must be valid UTF-8".into()))?;
    Ok(RepositoryInspection {
        root_fingerprint: sha256_hex(format!("root\0{root_path}").as_bytes()),
        common_dir_fingerprint: sha256_hex(
            format!("git-common\0{object_format}\0{common_path}").as_bytes(),
        ),
        root_path,
        object_format,
    })
}

pub(crate) fn read_repository_tree_files(
    root: &Path,
    head_oid: &str,
    max_files: usize,
    max_file_bytes: usize,
    max_total_bytes: usize,
) -> Result<Vec<GitTreeFile>> {
    validate_oid(head_oid)?;
    validate_repository_local_config(root)?;
    ensure_commit_exists(root, head_oid)?;
    let output = run_git(
        root,
        &["ls-tree", "-r", "-z", "-l", "--full-tree", head_oid],
        MAX_GIT_OUTPUT_BYTES,
    )?;
    require_git_success("list baseline tree", &output)?;
    let mut files = Vec::new();
    let mut requested_blobs = Vec::new();
    let mut requested_oids = HashSet::new();
    let mut total_bytes = 0_usize;
    for record in output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|row| !row.is_empty())
    {
        if files.len() >= max_files {
            return Err(CoreError::Conflict(format!(
                "repository tree exceeds the configured {max_files}-file CP5 analysis limit"
            )));
        }
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| {
                CoreError::Validation("malformed NUL-delimited git ls-tree output".into())
            })?;
        let header = std::str::from_utf8(&record[..tab])
            .map_err(|_| CoreError::Validation("Git tree header must be UTF-8".into()))?;
        let path = std::str::from_utf8(&record[tab + 1..])
            .map_err(|_| {
                CoreError::Validation(
                    "non-UTF-8 Git paths are outside the CP5 analyzer contract".into(),
                )
            })?
            .to_owned();
        validate_git_relative_path(&path)?;
        let mut fields = header.split_ascii_whitespace();
        let mode = fields.next().unwrap_or_default().to_owned();
        let object_type = fields.next().unwrap_or_default().to_owned();
        let object_id = fields.next().unwrap_or_default().to_owned();
        let size = fields.next().unwrap_or_default();
        if fields.next().is_some()
            || mode.is_empty()
            || object_type.is_empty()
            || object_id.is_empty()
            || size.is_empty()
        {
            return Err(CoreError::Validation("malformed git ls-tree record".into()));
        }
        validate_oid(&object_id)?;
        if object_type == "blob" {
            let declared_size = size.parse::<usize>().map_err(|_| {
                CoreError::Validation("Git blob size is malformed or unsupported".into())
            })?;
            if declared_size <= max_file_bytes {
                total_bytes = total_bytes.checked_add(declared_size).ok_or_else(|| {
                    CoreError::Conflict("CP5 analysis byte accounting overflow".into())
                })?;
                if total_bytes > max_total_bytes {
                    return Err(CoreError::Conflict(format!(
                        "repository tree exceeds the configured {max_total_bytes}-byte CP5 analysis limit"
                    )));
                }
                if requested_oids.insert(object_id.clone()) {
                    requested_blobs.push((object_id.clone(), declared_size));
                }
            }
        }
        files.push(GitTreeFile {
            path,
            mode,
            object_type,
            object_id,
            bytes: None,
        });
    }
    let blobs = read_git_blobs_batch(root, &requested_blobs, max_total_bytes)?;
    for file in &mut files {
        if let Some(bytes) = blobs.get(&file.object_id) {
            file.bytes = Some(bytes.clone());
        }
    }
    Ok(files)
}

fn read_git_blobs_batch(
    root: &Path,
    requested: &[(String, usize)],
    max_total_bytes: usize,
) -> Result<HashMap<String, Vec<u8>>> {
    if requested.is_empty() {
        return Ok(HashMap::new());
    }
    let mut input = Vec::new();
    for (oid, _) in requested {
        input.extend_from_slice(oid.as_bytes());
        input.push(b'\n');
    }
    let framing_allowance = requested
        .len()
        .checked_mul(160)
        .ok_or_else(|| CoreError::Conflict("Git batch framing limit overflow".into()))?;
    let output_limit = max_total_bytes
        .checked_add(framing_allowance)
        .ok_or_else(|| CoreError::Conflict("Git batch output limit overflow".into()))?;
    let output = run_git_with_input(root, &["cat-file", "--batch"], input, output_limit)?;
    require_git_success("read baseline blobs", &output)?;

    let mut cursor = 0_usize;
    let mut blobs = HashMap::with_capacity(requested.len());
    for (expected_oid, expected_size) in requested {
        let header_end = output.stdout[cursor..]
            .iter()
            .position(|byte| *byte == b'\n')
            .map(|offset| cursor + offset)
            .ok_or_else(|| CoreError::Validation("truncated git cat-file batch header".into()))?;
        let header = std::str::from_utf8(&output.stdout[cursor..header_end])
            .map_err(|_| CoreError::Validation("Git batch header must be UTF-8".into()))?;
        let mut fields = header.split_ascii_whitespace();
        let oid = fields.next().unwrap_or_default();
        let object_type = fields.next().unwrap_or_default();
        let size = fields
            .next()
            .and_then(|value| value.parse::<usize>().ok())
            .ok_or_else(|| CoreError::Validation("malformed git cat-file batch size".into()))?;
        if fields.next().is_some()
            || oid != expected_oid
            || object_type != "blob"
            || size != *expected_size
        {
            return Err(CoreError::Conflict(format!(
                "Git batch response did not match requested immutable blob {expected_oid}"
            )));
        }
        let content_start = header_end + 1;
        let content_end = content_start
            .checked_add(size)
            .ok_or_else(|| CoreError::Conflict("Git batch content offset overflow".into()))?;
        if content_end >= output.stdout.len() || output.stdout[content_end] != b'\n' {
            return Err(CoreError::Validation(
                "truncated or malformed git cat-file batch payload".into(),
            ));
        }
        blobs.insert(
            expected_oid.clone(),
            output.stdout[content_start..content_end].to_vec(),
        );
        cursor = content_end + 1;
    }
    if cursor != output.stdout.len() {
        return Err(CoreError::Validation(
            "unexpected trailing bytes in git cat-file batch output".into(),
        ));
    }
    Ok(blobs)
}

fn validate_repository_local_config(root: &Path) -> Result<()> {
    let output = run_git(
        root,
        &[
            "config",
            "--local",
            "--includes",
            "--null",
            "--get-regexp",
            r"^(filter\..*\.(clean|smudge|process|required)|diff\..*\.(command|textconv))$",
        ],
        MAX_GIT_ERROR_BYTES,
    )?;
    match output.status.code() {
        Some(1) if output.stdout.is_empty() => Ok(()),
        Some(0) if output.stdout.is_empty() => Ok(()),
        Some(0) => Err(CoreError::Validation(
            "repository-local executable filter/diff configuration is not permitted by the CP4 read-only adapter"
                .into(),
        )),
        _ => Err(git_failure("validate repository-local Git configuration", &output)),
    }
}

fn inspect_live_state(root: &Path) -> Result<LiveRepositoryState> {
    validate_repository_local_config(root)?;
    let head_output = run_git(
        root,
        &["rev-parse", "--verify", "HEAD"],
        MAX_GIT_ERROR_BYTES,
    )?;
    let head_oid = if head_output.status.success() {
        Some(
            output_text(&head_output.stdout, "Git HEAD")?
                .trim()
                .to_owned(),
        )
    } else {
        let all = git_text(root, &["rev-list", "--all", "--max-count=1"])?;
        if all.trim().is_empty() {
            None
        } else {
            return Err(git_failure("resolve HEAD", &head_output));
        }
    };
    if let Some(oid) = &head_oid {
        validate_oid(oid)?;
    }
    let symbolic = run_git(
        root,
        &["symbolic-ref", "--quiet", "HEAD"],
        MAX_GIT_ERROR_BYTES,
    )?;
    let head_ref = if symbolic.status.success() {
        Some(
            output_text(&symbolic.stdout, "Git symbolic HEAD")?
                .trim()
                .to_owned(),
        )
    } else if symbolic.status.code() == Some(1) {
        None
    } else {
        return Err(git_failure("resolve symbolic HEAD", &symbolic));
    };
    let branch_name = head_ref
        .as_deref()
        .and_then(|value| value.strip_prefix("refs/heads/"))
        .map(str::to_owned);
    let first = capture_worktree_identity(root)?;
    let second = capture_worktree_identity(root)?;
    if first != second {
        return Err(CoreError::Conflict(
            "working tree changed while its baseline was being observed; retry after writes settle"
                .into(),
        ));
    }
    let status = parse_porcelain_status(&first.status_bytes)?;
    let mut hasher = Sha256::new();
    hasher.update(b"worktree-v1\0");
    hasher.update(head_oid.as_deref().unwrap_or("unborn").as_bytes());
    hasher.update(b"\0");
    hasher.update(head_ref.as_deref().unwrap_or("detached").as_bytes());
    hasher.update(b"\0");
    hasher.update(&first.status_bytes);
    hasher.update(b"\0");
    hasher.update(first.content_digest.as_bytes());
    Ok(LiveRepositoryState {
        head_oid,
        head_ref,
        branch_name,
        worktree_fingerprint: hex::encode(hasher.finalize()),
        status,
    })
}

fn capture_worktree_identity(root: &Path) -> Result<WorktreeIdentity> {
    let status_output = git_ok(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules=none",
        ],
        MAX_GIT_OUTPUT_BYTES,
        "read working-tree status",
    )?;
    let status = parse_porcelain_status(&status_output.stdout)?;
    let mut hasher = Sha256::new();
    hasher.update(b"worktree-content-v1\0");
    let mut observed_bytes = 0_u64;
    for args in [
        [
            "diff",
            "--cached",
            "--binary",
            "--full-index",
            "--no-ext-diff",
            "--no-textconv",
        ]
        .as_slice(),
        [
            "diff",
            "--binary",
            "--full-index",
            "--no-ext-diff",
            "--no-textconv",
        ]
        .as_slice(),
    ] {
        let (digest, bytes) = run_git_digest(root, args, MAX_WORKTREE_CONTENT_BYTES)?;
        observed_bytes = observed_bytes.saturating_add(bytes);
        if observed_bytes > MAX_WORKTREE_CONTENT_BYTES {
            return Err(CoreError::Conflict(format!(
                "working-tree diff exceeds the {MAX_WORKTREE_CONTENT_BYTES}-byte observation limit"
            )));
        }
        hasher.update(digest);
        hasher.update(bytes.to_le_bytes());
    }
    let started = Instant::now();
    for entry in status
        .iter()
        .filter(|entry| entry.index_status == "?" && entry.worktree_status == "?")
    {
        if started.elapsed() >= MAX_GIT_OPERATION {
            return Err(CoreError::Conflict(
                "untracked-content hashing exceeded the Git observation deadline".into(),
            ));
        }
        let relative = Path::new(&entry.path);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(CoreError::Validation(
                "Git returned an unsafe untracked path".into(),
            ));
        }
        let path = root.join(relative);
        let metadata = fs::symlink_metadata(&path)?;
        hasher.update(entry.path.as_bytes());
        hasher.update(b"\0");
        if metadata.file_type().is_symlink() {
            let target = fs::read_link(&path)?;
            let target = target.to_str().ok_or_else(|| {
                CoreError::Validation("untracked symlink target must be valid UTF-8".into())
            })?;
            observed_bytes = observed_bytes.saturating_add(target.len() as u64);
            hasher.update(b"symlink\0");
            hasher.update(target.as_bytes());
        } else if metadata.is_file() {
            observed_bytes = observed_bytes.saturating_add(metadata.len());
            if observed_bytes > MAX_WORKTREE_CONTENT_BYTES {
                return Err(CoreError::Conflict(format!(
                    "working-tree content exceeds the {MAX_WORKTREE_CONTENT_BYTES}-byte observation limit"
                )));
            }
            hasher.update(b"file\0");
            let mut file = File::open(path)?;
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let read = file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                hasher.update(&buffer[..read]);
            }
        } else {
            return Err(CoreError::Validation(
                "untracked repository entry is neither a regular file nor a symlink".into(),
            ));
        }
        hasher.update(b"\0");
    }
    Ok(WorktreeIdentity {
        status_bytes: status_output.stdout,
        content_digest: hex::encode(hasher.finalize()),
    })
}

fn parse_porcelain_status(bytes: &[u8]) -> Result<Vec<WorkingTreeEntry>> {
    let fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut index = 0;
    let mut entries = Vec::new();
    while index < fields.len() && !fields[index].is_empty() {
        let field = fields[index];
        if field.len() < 4 || field[2] != b' ' {
            return Err(CoreError::Validation(
                "Git returned malformed porcelain status".into(),
            ));
        }
        let index_status = char::from(field[0]).to_string();
        let worktree_status = char::from(field[1]).to_string();
        let path = utf8_git_path(&field[3..])?;
        index += 1;
        let original_path = if matches!(field[0], b'R' | b'C') {
            let value = fields.get(index).ok_or_else(|| {
                CoreError::Validation("Git rename status omitted its original path".into())
            })?;
            index += 1;
            Some(utf8_git_path(value)?)
        } else {
            None
        };
        entries.push(WorkingTreeEntry {
            index_status,
            worktree_status,
            path,
            original_path,
        });
        if entries.len() > MAX_FILE_CHANGES {
            return Err(CoreError::Conflict(format!(
                "working tree exceeds {MAX_FILE_CHANGES} changed paths"
            )));
        }
    }
    Ok(entries)
}

fn read_reachable_commits(root: &Path, head_oid: &str) -> Result<Vec<RawCommit>> {
    let output = git_ok(
        root,
        &[
            "log",
            "--reverse",
            "--topo-order",
            "--no-show-signature",
            "--format=format:%H%x00%T%x00%P%x00%an%x00%ae%x00%aI%x00%cI%x00%s%x00",
            head_oid,
        ],
        MAX_GIT_OUTPUT_BYTES,
        "read commit metadata",
    )?;
    let fields = output.stdout.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut commits = Vec::new();
    let mut index = 0;
    while index + 7 < fields.len() {
        if fields[index..index + 8]
            .iter()
            .all(|field| field.is_empty())
        {
            break;
        }
        let oid = output_text(fields[index], "commit OID")?
            .trim_start_matches(['\r', '\n'])
            .to_owned();
        if oid.is_empty() {
            break;
        }
        validate_oid(&oid)?;
        let tree_oid = output_text(fields[index + 1], "tree OID")?.to_owned();
        validate_oid(&tree_oid)?;
        let parents = output_text(fields[index + 2], "commit parents")?
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for parent in &parents {
            validate_oid(parent)?;
        }
        let subject = output_text(fields[index + 7], "commit subject")?.to_owned();
        validate_text(&subject, MAX_DEVELOPMENT_TEXT, "commit subject")?;
        commits.push(RawCommit {
            oid,
            tree_oid,
            parents,
            author_name: output_text(fields[index + 3], "commit author")?.to_owned(),
            author_email: output_text(fields[index + 4], "commit author email")?.to_owned(),
            authored_at: output_text(fields[index + 5], "commit authored time")?.to_owned(),
            committed_at: output_text(fields[index + 6], "commit committed time")?.to_owned(),
            subject,
            file_changes: Vec::new(),
        });
        index += 8;
    }
    Ok(commits)
}

fn read_commit_file_changes(root: &Path, oid: &str) -> Result<Vec<FileChange>> {
    let output = git_ok(
        root,
        &[
            "diff-tree",
            "--root",
            "--no-commit-id",
            "--name-status",
            "-r",
            "-z",
            "-M",
            "--diff-merges=first-parent",
            "--no-ext-diff",
            "--no-textconv",
            oid,
        ],
        MAX_GIT_OUTPUT_BYTES,
        "read commit file changes",
    )?;
    parse_name_status(&output.stdout)
}

fn parse_name_status(bytes: &[u8]) -> Result<Vec<FileChange>> {
    let fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    let mut index = 0;
    let mut changes = Vec::new();
    while index < fields.len() && !fields[index].is_empty() {
        let status = output_text(fields[index], "Git file status")?;
        index += 1;
        let first = fields
            .get(index)
            .ok_or_else(|| CoreError::Validation("Git file status omitted a path".into()))?;
        index += 1;
        let first = utf8_git_path(first)?;
        let code = status.chars().next().unwrap_or('X');
        let similarity = status
            .get(1..)
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<u8>().ok());
        let (old_path, new_path) = if matches!(code, 'R' | 'C') {
            let second = fields.get(index).ok_or_else(|| {
                CoreError::Validation("Git rename/copy status omitted its destination".into())
            })?;
            index += 1;
            (Some(first), utf8_git_path(second)?)
        } else if code == 'D' {
            (Some(first.clone()), first)
        } else {
            (None, first)
        };
        changes.push(FileChange {
            change_kind: match code {
                'A' => "added",
                'C' => "copied",
                'D' => "deleted",
                'M' => "modified",
                'R' => "renamed",
                'T' => "type_changed",
                'U' => "unmerged",
                _ => "unknown",
            }
            .into(),
            old_path,
            new_path,
            similarity,
        });
        if changes.len() > MAX_FILE_CHANGES {
            return Err(CoreError::Conflict(format!(
                "commit exceeds {MAX_FILE_CHANGES} changed paths"
            )));
        }
    }
    Ok(changes)
}

fn ensure_commit_exists(root: &Path, oid: &str) -> Result<()> {
    git_ok(
        root,
        &["cat-file", "-e", &format!("{oid}^{{commit}}")],
        MAX_GIT_ERROR_BYTES,
        "verify baseline commit",
    )?;
    Ok(())
}

fn is_ancestor(root: &Path, old: &str, new: &str) -> Result<bool> {
    let output = run_git(
        root,
        &["merge-base", "--is-ancestor", old, new],
        MAX_GIT_ERROR_BYTES,
    )?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(git_failure("compare commit ancestry", &output)),
    }
}

fn known_commit_oids(
    connection: &Connection,
    project_id: &str,
    repository_id: &str,
) -> Result<HashSet<String>> {
    let mut statement = connection.prepare(
        "SELECT commit_oid FROM git_commit_observations WHERE project_id=?1 AND repository_id=?2",
    )?;
    statement
        .query_map(params![project_id, repository_id], |row| row.get(0))?
        .collect::<std::result::Result<HashSet<_>, _>>()
        .map_err(Into::into)
}

fn git_text(root: &Path, args: &[&str]) -> Result<String> {
    let output = git_ok(root, args, MAX_GIT_OUTPUT_BYTES, "inspect repository")?;
    output_text(&output.stdout, "Git output").map(str::to_owned)
}

fn git_ok(root: &Path, args: &[&str], limit: usize, action: &str) -> Result<GitOutput> {
    let output = run_git(root, args, limit)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(git_failure(action, &output))
    }
}

fn run_git(root: &Path, args: &[&str], stdout_limit: usize) -> Result<GitOutput> {
    let mut child = Command::new("git")
        .current_dir(root)
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg("core.untrackedCache=false")
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_EXTERNAL_DIFF")
        .env_remove("GIT_DIFF_OPTS")
        .env_remove("GIT_CONFIG_COUNT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| CoreError::Validation(format!("cannot start Git adapter: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::Validation("Git stdout was unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CoreError::Validation("Git stderr was unavailable".into()))?;
    let stdout_reader = thread::spawn(move || read_limited(stdout, stdout_limit));
    let stderr_reader = thread::spawn(move || read_limited(stderr, MAX_GIT_ERROR_BYTES));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= MAX_GIT_OPERATION {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CoreError::Conflict(format!(
                "Git operation exceeded {} seconds",
                MAX_GIT_OPERATION.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let (stdout, stdout_truncated) = stdout_reader
        .join()
        .map_err(|_| CoreError::Validation("Git stdout reader failed".into()))??;
    let (stderr, stderr_truncated) = stderr_reader
        .join()
        .map_err(|_| CoreError::Validation("Git stderr reader failed".into()))??;
    if stdout_truncated {
        return Err(CoreError::Conflict(format!(
            "Git output exceeded the {stdout_limit}-byte safety limit"
        )));
    }
    Ok(GitOutput {
        status,
        stdout,
        stderr,
        stderr_truncated,
    })
}

fn run_git_with_input(
    root: &Path,
    args: &[&str],
    input: Vec<u8>,
    stdout_limit: usize,
) -> Result<GitOutput> {
    let mut child = Command::new("git")
        .current_dir(root)
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg("core.untrackedCache=false")
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_EXTERNAL_DIFF")
        .env_remove("GIT_DIFF_OPTS")
        .env_remove("GIT_CONFIG_COUNT")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| CoreError::Validation(format!("cannot start Git adapter: {error}")))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| CoreError::Validation("Git stdin was unavailable".into()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::Validation("Git stdout was unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CoreError::Validation("Git stderr was unavailable".into()))?;
    let stdin_writer = thread::spawn(move || -> std::io::Result<()> {
        stdin.write_all(&input)?;
        drop(stdin);
        Ok(())
    });
    let stdout_reader = thread::spawn(move || read_limited(stdout, stdout_limit));
    let stderr_reader = thread::spawn(move || read_limited(stderr, MAX_GIT_ERROR_BYTES));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= MAX_GIT_OPERATION {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CoreError::Conflict(format!(
                "Git operation exceeded {} seconds",
                MAX_GIT_OPERATION.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(10));
    };
    stdin_writer
        .join()
        .map_err(|_| CoreError::Validation("Git stdin writer failed".into()))??;
    let (stdout, stdout_truncated) = stdout_reader
        .join()
        .map_err(|_| CoreError::Validation("Git stdout reader failed".into()))??;
    let (stderr, stderr_truncated) = stderr_reader
        .join()
        .map_err(|_| CoreError::Validation("Git stderr reader failed".into()))??;
    if stdout_truncated {
        return Err(CoreError::Conflict(format!(
            "Git output exceeded the {stdout_limit}-byte safety limit"
        )));
    }
    Ok(GitOutput {
        status,
        stdout,
        stderr,
        stderr_truncated,
    })
}

fn run_git_digest(root: &Path, args: &[&str], byte_limit: u64) -> Result<(Vec<u8>, u64)> {
    let mut child = Command::new("git")
        .current_dir(root)
        .arg("-c")
        .arg("core.fsmonitor=false")
        .arg("-c")
        .arg("core.untrackedCache=false")
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_EXTERNAL_DIFF")
        .env_remove("GIT_DIFF_OPTS")
        .env_remove("GIT_CONFIG_COUNT")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| CoreError::Validation(format!("cannot start Git adapter: {error}")))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| CoreError::Validation("Git stdout was unavailable".into()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| CoreError::Validation("Git stderr was unavailable".into()))?;
    let stdout_reader = thread::spawn(move || hash_limited(stdout, byte_limit));
    let stderr_reader = thread::spawn(move || read_limited(stderr, MAX_GIT_ERROR_BYTES));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= MAX_GIT_OPERATION {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CoreError::Conflict(format!(
                "Git operation exceeded {} seconds",
                MAX_GIT_OPERATION.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(10));
    };
    let (digest, byte_count, truncated) = stdout_reader
        .join()
        .map_err(|_| CoreError::Validation("Git digest reader failed".into()))??;
    let (stderr, stderr_truncated) = stderr_reader
        .join()
        .map_err(|_| CoreError::Validation("Git stderr reader failed".into()))??;
    if truncated {
        return Err(CoreError::Conflict(format!(
            "Git diff exceeds the {byte_limit}-byte observation limit"
        )));
    }
    if !status.success() {
        return Err(git_failure(
            "hash working-tree content",
            &GitOutput {
                status,
                stdout: Vec::new(),
                stderr,
                stderr_truncated,
            },
        ));
    }
    Ok((digest, byte_count))
}

fn read_limited(mut reader: impl Read, limit: usize) -> std::io::Result<(Vec<u8>, bool)> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    let truncated = bytes.len() > limit;
    bytes.truncate(limit);
    Ok((bytes, truncated))
}

fn hash_limited(mut reader: impl Read, limit: u64) -> std::io::Result<(Vec<u8>, u64, bool)> {
    let mut hasher = Sha256::new();
    let mut count = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        count = count.saturating_add(read as u64);
        if count > limit {
            return Ok((hasher.finalize().to_vec(), count, true));
        }
        hasher.update(&buffer[..read]);
    }
    Ok((hasher.finalize().to_vec(), count, false))
}

fn git_failure(action: &str, output: &GitOutput) -> CoreError {
    let mut message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if output.stderr_truncated {
        message.push_str(" …[truncated]");
    }
    if message.is_empty() {
        message = format!("Git exited with status {}", output.status);
    }
    CoreError::Validation(format!("cannot {action}: {message}"))
}

fn require_git_success(action: &str, output: &GitOutput) -> Result<()> {
    if output.status.success() {
        Ok(())
    } else {
        Err(git_failure(action, output))
    }
}

fn output_text<'a>(bytes: &'a [u8], label: &str) -> Result<&'a str> {
    std::str::from_utf8(bytes)
        .map_err(|_| CoreError::Validation(format!("{label} must be valid UTF-8")))
}

fn utf8_git_path(bytes: &[u8]) -> Result<String> {
    let path = output_text(bytes, "Git path")?;
    validate_nonempty(path, 4096, "Git path")?;
    Ok(path.to_owned())
}

fn validate_git_relative_path(path: &str) -> Result<()> {
    validate_nonempty(path, 4096, "Git path")?;
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(CoreError::Validation(
            "Git tree path must remain repository-relative".into(),
        ));
    }
    Ok(())
}

fn validate_oid(oid: &str) -> Result<()> {
    if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CoreError::Validation(format!(
            "Git returned an invalid object ID: {oid}"
        )));
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn nonempty_option(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

fn validate_text(value: &str, max_chars: usize, label: &str) -> Result<()> {
    if value.chars().count() > max_chars {
        return Err(CoreError::Validation(format!(
            "{label} exceeds {max_chars} characters"
        )));
    }
    Ok(())
}

fn excerpt(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

pub(crate) fn reject_ai_actor(command: &CommandContext) -> Result<()> {
    if command.actor.kind == crate::ActorKind::AiProposal {
        return Err(CoreError::Validation(
            "AI proposals cannot execute canonical Development commands".into(),
        ));
    }
    Ok(())
}

pub(crate) fn require_development_enabled(connection: &Connection, project_id: &str) -> Result<()> {
    let enabled: bool = connection
        .query_row(
            "SELECT enabled FROM space_capabilities WHERE project_id=?1 AND space='development'",
            [project_id],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(false);
    if !enabled {
        return Err(CoreError::Validation(
            "Development Space capability is disabled".into(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_common_entity(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    id: &str,
    entity_type: &str,
    title: &str,
    status: &str,
    origin: OriginKind,
    metadata_json: &str,
    data: &Value,
    now: &str,
) -> Result<()> {
    let data_json = bounded_json(data, MAX_DEVELOPMENT_JSON_BYTES, "Development entity data")?;
    tx.execute(
        "INSERT INTO entities(id,project_id,entity_type,title,status,version,legacy_origin,data_json,
            created_at,updated_at,entity_schema_version,origin_type,metadata_json,created_by,updated_by)
         VALUES(?1,?2,?3,?4,?5,1,?6,?7,?8,?8,1,?9,?10,?11,?11)",
        params![
            id,
            project_id,
            entity_type,
            title,
            status,
            legacy_origin(origin),
            data_json,
            now,
            origin.as_str(),
            metadata_json,
            command.actor.id
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn insert_development_timeline(
    tx: &Transaction<'_>,
    project_id: &str,
    sequence: i64,
    repository_id: Option<&str>,
    entity_id: Option<&str>,
    event_type: &str,
    payload: &Value,
    actor_id: &str,
    occurred_at: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO development_timeline(id,project_id,ledger_sequence,repository_id,entity_id,
            event_type,payload_json,actor_id,occurred_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![
            new_id(),
            project_id,
            sequence,
            repository_id,
            entity_id,
            event_type,
            bounded_json(
                payload,
                MAX_DEVELOPMENT_JSON_BYTES,
                "Development timeline payload"
            )?,
            actor_id,
            occurred_at
        ],
    )?;
    Ok(())
}

pub(crate) fn upsert_development_search(
    tx: &Transaction<'_>,
    project_id: &str,
    entity_id: &str,
    entity_type: &str,
    title: &str,
    body: &str,
    now: &str,
) -> Result<()> {
    tx.execute(
        "INSERT INTO development_search_documents(entity_id,project_id,entity_type,title,body,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6)
         ON CONFLICT(entity_id) DO UPDATE SET title=excluded.title,body=excluded.body,updated_at=excluded.updated_at",
        params![entity_id, project_id, entity_type, title, body, now],
    )?;
    Ok(())
}

fn read_file_changes(
    connection: &Connection,
    table: &str,
    id_column: &str,
    id: &str,
) -> Result<Vec<FileChange>> {
    let sql = format!(
        "SELECT change_kind,old_path,new_path,similarity FROM {table} WHERE {id_column}=?1 ORDER BY ordinal"
    );
    let mut statement = connection.prepare(&sql)?;
    statement
        .query_map([id], |row| {
            Ok(FileChange {
                change_kind: row.get(0)?,
                old_path: row.get(1)?,
                new_path: row.get(2)?,
                similarity: row.get::<_, Option<i64>>(3)?.map(|value| value as u8),
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_relationship_matrix_is_closed() {
        for (source, relation, target) in [
            ("change_set", "implements", "requirement"),
            ("change_set", "partially_implements", "requirement"),
            ("change_set", "reverts", "requirement"),
            ("change_set", "supersedes", "change_set"),
        ] {
            assert!(validate_development_relationship_pair(source, relation, target).is_ok());
        }
        for (source, relation, target) in [
            ("repository", "implements", "requirement"),
            ("change_set", "modifies", "repository"),
            ("requirement", "implements", "change_set"),
            ("change_set", "supersedes", "repository_baseline"),
        ] {
            assert!(validate_development_relationship_pair(source, relation, target).is_err());
        }
    }

    #[test]
    fn change_set_and_requirement_lifecycles_reject_skipped_states() {
        assert!(validate_change_set_transition("observed", "linked").is_ok());
        assert!(validate_change_set_transition("linked", "validated").is_ok());
        assert!(validate_change_set_transition("observed", "closed").is_err());
        assert!(validate_change_set_transition("closed", "observed").is_err());
        assert!(validate_requirement_transition("draft", "accepted").is_ok());
        assert!(validate_requirement_transition("accepted", "in_progress").is_ok());
        assert!(validate_requirement_transition("draft", "verified").is_err());
    }

    #[test]
    fn nul_delimited_git_status_and_rename_metadata_are_lossless() {
        let status =
            parse_porcelain_status(b" M README.md\0R  new name.rs\0old name.rs\0?? fresh.txt\0")
                .unwrap();
        assert_eq!(status.len(), 3);
        assert_eq!(status[1].path, "new name.rs");
        assert_eq!(status[1].original_path.as_deref(), Some("old name.rs"));
        let changes = parse_name_status(b"M\0README.md\0R100\0old name.rs\0new name.rs\0").unwrap();
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[1].change_kind, "renamed");
        assert_eq!(changes[1].similarity, Some(100));
        assert_eq!(changes[1].old_path.as_deref(), Some("old name.rs"));
        assert_eq!(changes[1].new_path, "new name.rs");
    }
}
