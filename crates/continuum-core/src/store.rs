use std::collections::HashSet;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::artifact::{hash_file, relative_content_path};
use crate::manifest::{LEDGER_FILE, MANIFEST_FILE, ProjectManifest};
use crate::{
    ActorKind, ActorRef, ArtifactAvailability, ArtifactClassification, ArtifactRecord, AuditEvent,
    CORE_SCHEMA_VERSION, Checkpoint, CheckpointScope, CommandContext, CoreError,
    CoreRelationshipPolicy, Entity, EntityPage, EntityUpdate, IntegrityIssue, IntegrityReport, Job,
    NewEntity, NewRelationship, OriginKind, OutboxMessage, PageRequest, ProjectSummary,
    Relationship, RelationshipPage, RelationshipPolicy, RelationshipReviewState, Result, Space,
    new_id,
};

const MIGRATION_1: &str = include_str!("../migrations/0001_core.sql");
const MIGRATION_2: &str = include_str!("../migrations/0002_contract_alignment.sql");
const MIGRATION_3: &str = include_str!("../migrations/0003_research_core.sql");
const MIGRATION_4: &str = include_str!("../migrations/0004_development_core.sql");
const MIGRATION_5: &str = include_str!("../migrations/0005_code_intelligence.sql");
const MIGRATION_6: &str = include_str!("../migrations/0006_cp5_1_hardening.sql");
const MIGRATION_7: &str = include_str!("../migrations/0007_provenance_graph.sql");
const MIGRATION_8: &str = include_str!("../migrations/0008_semantic_intelligence.sql");
const MIGRATION_9: &str = include_str!("../migrations/0009_visual_intelligence.sql");
const MIGRATION_10: &str = include_str!("../migrations/0010_research_capture.sql");
const MIGRATION_11: &str = include_str!("../migrations/0011_checkpoint_context_engine.sql");
const MIGRATION_12: &str = include_str!("../migrations/0012_ai_continuity_interface.sql");
const MIGRATION_13: &str = include_str!("../migrations/0013_research_presentation_proposals.sql");
const MIGRATION_14: &str = include_str!("../migrations/0014_reviewed_report_sources.sql");
const MIGRATION_15: &str = include_str!("../migrations/0015_workspace_documents.sql");
const MAX_ACTIVE_JOBS: i64 = 1_000;
const MAX_ENTITY_JSON_BYTES: usize = 1024 * 1024;
const MAX_CHECKPOINT_JSON_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ContinuityStore {
    pub(crate) root: PathBuf,
    pub(crate) manifest: ProjectManifest,
}

struct PendingContentFile {
    path: PathBuf,
    armed: bool,
}

impl Drop for PendingContentFile {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

struct PendingDirectory {
    path: PathBuf,
    armed: bool,
}

struct PriorCommandReceipt {
    operation: String,
    result_id: Option<String>,
    project_id: String,
    actor_kind: String,
    actor_id: String,
    command_type_version: i64,
    payload_schema_version: i64,
}

impl Drop for PendingDirectory {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

impl ContinuityStore {
    pub fn create(root: impl AsRef<Path>, name: impl Into<String>) -> Result<Self> {
        Self::create_with_actor(root, name, ActorRef::system("continuum-core"))
    }

    pub fn create_with_actor(
        root: impl AsRef<Path>,
        name: impl Into<String>,
        actor: ActorRef,
    ) -> Result<Self> {
        let root = root.as_ref();
        let name = name.into().trim().to_owned();
        if name.is_empty() || name.chars().count() > 200 {
            return Err(CoreError::Validation(
                "project name must contain 1..=200 characters".into(),
            ));
        }
        if root.exists() {
            return Err(CoreError::Conflict(format!(
                "project destination already exists: {}",
                root.display()
            )));
        }
        if let Some(parent) = root.parent() {
            fs::create_dir_all(parent)?;
        }
        let staging_root = root.with_extension(format!("creating-{}", new_id()));
        let mut pending = PendingDirectory {
            path: staging_root.clone(),
            armed: true,
        };
        fs::create_dir(&staging_root)?;
        for directory in [
            "artifacts/sha256",
            "staging",
            "quarantine",
            "backups",
            "derived",
        ] {
            fs::create_dir_all(staging_root.join(directory))?;
        }

        let project_id = new_id();
        let manifest = ProjectManifest::new(project_id.clone(), name.clone());
        manifest.write_atomic(&staging_root)?;

        let mut connection = open_connection(&manifest.ledger_path(&staging_root))?;
        apply_migrations(&mut connection)?;
        let now = Utc::now().to_rfc3339();
        let command = CommandContext::new(actor);
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute(
            "INSERT INTO projects(id,name,status,lifecycle_version,ledger_sequence,created_at,updated_at)
             VALUES(?1,?2,'active',1,0,?3,?3)",
            params![project_id, name, now],
        )?;
        tx.execute(
            "INSERT INTO space_capabilities(project_id,space,enabled,updated_at)
             VALUES(?1,'research',0,?2), (?1,'development',0,?2)",
            params![manifest.project_id, now],
        )?;
        append_event_with_context(
            &tx,
            &manifest.project_id,
            &command,
            Some(&manifest.project_id),
            "project.created",
            &json!({"name": manifest.name}),
        )?;
        record_command_with_context(
            &tx,
            &command,
            &manifest.project_id,
            "CreateProject",
            Some(&manifest.project_id),
            None,
            &json!({"name": manifest.name}),
        )?;
        tx.commit()?;
        drop(connection);
        fs::rename(&staging_root, root)?;
        pending.armed = false;

        Ok(Self {
            root: root.to_path_buf(),
            manifest,
        })
    }

    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        let manifest = ProjectManifest::load(root)?;
        let ledger_path = manifest.ledger_path(root);
        if !ledger_path.is_file() {
            return Err(CoreError::NotFound(format!(
                "project ledger is missing: {}",
                ledger_path.display()
            )));
        }
        let mut connection = open_connection(&ledger_path)?;
        let current_version = current_schema_version(&connection)?;
        if current_version < CORE_SCHEMA_VERSION {
            let backup_path = root.join("backups").join(format!(
                "pre-migration-v{current_version}-to-v{CORE_SCHEMA_VERSION}-{}.sqlite3",
                Utc::now().timestamp_millis()
            ));
            backup_connection(&connection, &backup_path)?;
        }
        apply_migrations(&mut connection)?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1)",
            [&manifest.project_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(CoreError::Validation(
                "manifest project_id does not exist in ledger".into(),
            ));
        }
        Ok(Self {
            root: root.to_path_buf(),
            manifest,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn manifest(&self) -> &ProjectManifest {
        &self.manifest
    }

    pub fn summary(&self) -> Result<ProjectSummary> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT id,name,status,ledger_sequence FROM projects WHERE id=?1",
                [&self.manifest.project_id],
                |row| {
                    Ok(ProjectSummary {
                        project_id: row.get(0)?,
                        name: row.get(1)?,
                        status: row.get(2)?,
                        ledger_sequence: row.get(3)?,
                    })
                },
            )
            .map_err(Into::into)
    }

    pub fn set_project_archived(&self, command_id: &str, archived: bool) -> Result<()> {
        self.set_project_archived_with_context(
            &CommandContext::system_with_id(command_id),
            archived,
        )
    }

    pub fn set_project_archived_with_context(
        &self,
        command: &CommandContext,
        archived: bool,
    ) -> Result<()> {
        validate_command_context(command)?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "SetProjectArchived",
        )?
        .is_some()
        {
            return tx.commit().map_err(Into::into);
        }
        let status = if archived { "archived" } else { "active" };
        tx.execute(
            "UPDATE projects SET status=?2,lifecycle_version=lifecycle_version+1,updated_at=?3 WHERE id=?1",
            params![self.manifest.project_id, status, Utc::now().to_rfc3339()],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&self.manifest.project_id),
            "project.lifecycle_changed",
            &json!({"status": status}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "SetProjectArchived",
            None,
            None,
            &json!({"archived": archived}),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn capability_enabled(&self, space: Space) -> Result<bool> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT enabled FROM space_capabilities WHERE project_id=?1 AND space=?2",
                params![self.manifest.project_id, space.as_str()],
                |row| row.get(0),
            )
            .map_err(Into::into)
    }

    pub fn set_space_capability(
        &self,
        command_id: &str,
        space: Space,
        enabled: bool,
    ) -> Result<()> {
        self.set_space_capability_with_context(
            &CommandContext::system_with_id(command_id),
            space,
            enabled,
        )
    }

    pub fn set_space_capability_with_context(
        &self,
        command: &CommandContext,
        space: Space,
        enabled: bool,
    ) -> Result<()> {
        validate_command_context(command)?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "SetSpaceCapability",
        )?
        .is_some()
        {
            return tx.commit().map_err(Into::into);
        }
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE space_capabilities SET enabled=?3,updated_at=?4
             WHERE project_id=?1 AND space=?2",
            params![self.manifest.project_id, space.as_str(), enabled, now],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&self.manifest.project_id),
            "space.capability_changed",
            &json!({"space": space.as_str(), "enabled": enabled}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "SetSpaceCapability",
            None,
            None,
            &json!({"space": space.as_str(), "enabled": enabled}),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn create_entity(&self, command_id: &str, input: NewEntity) -> Result<String> {
        self.create_entity_with_context(&CommandContext::system_with_id(command_id), input)
    }

    pub fn create_entity_with_context(
        &self,
        command: &CommandContext,
        input: NewEntity,
    ) -> Result<String> {
        validate_command_context(command)?;
        validate_nonempty(&input.entity_type, 100, "entity_type")?;
        if crate::research::is_reserved_domain_entity_type(&input.entity_type)
            || crate::development::is_development_entity_type(&input.entity_type)
            || crate::provenance::is_provenance_entity_type(&input.entity_type)
        {
            return Err(CoreError::Validation(format!(
                "{} is owned by a typed domain module; use its typed command API",
                input.entity_type
            )));
        }
        validate_nonempty(&input.title, 500, "title")?;
        if input.schema_version == 0 {
            return Err(CoreError::Validation(
                "entity schema_version must be positive".into(),
            ));
        }
        if input.origin == OriginKind::AiProposal {
            return Err(CoreError::Validation(
                "AI proposal origin belongs in candidate storage, not canonical entities".into(),
            ));
        }
        let metadata_json =
            bounded_json(&input.metadata, MAX_ENTITY_JSON_BYTES, "entity metadata")?;
        let data_json = bounded_json(&input.data, MAX_ENTITY_JSON_BYTES, "entity data")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(result) = prior_result(&tx, &self.manifest.project_id, command, "CreateEntity")?
        {
            return result.ok_or_else(|| CoreError::Conflict("command has no result ID".into()));
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO entities(
                id,project_id,entity_type,title,status,version,legacy_origin,data_json,created_at,updated_at,
                entity_schema_version,origin_type,metadata_json,created_by,updated_by
             ) VALUES(?1,?2,?3,?4,'active',1,?5,?6,?7,?7,?8,?9,?10,?11,?11)",
            params![
                id,
                self.manifest.project_id,
                input.entity_type,
                input.title,
                legacy_origin(input.origin),
                data_json,
                now,
                input.schema_version,
                input.origin.as_str(),
                metadata_json,
                command.actor.id
            ],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&id),
            "entity.created",
            &json!({"entity_id": id, "entity_type": input.entity_type}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "CreateEntity",
            Some(&id),
            None,
            &json!({"entity_type": input.entity_type, "schema_version": input.schema_version}),
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn get_entity(&self, id: &str) -> Result<Entity> {
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT id,project_id,entity_type,entity_schema_version,title,status,version,origin_type,
                        metadata_json,data_json,created_at,created_by,updated_at,updated_by,archived_at
                 FROM entities WHERE id=?1 AND project_id=?2",
                params![id, self.manifest.project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, Option<String>>(14)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        Ok(Entity {
            id: raw.0,
            project_id: raw.1,
            entity_type: raw.2,
            schema_version: raw.3,
            title: raw.4,
            status: raw.5,
            version: raw.6,
            origin: raw.7,
            metadata: serde_json::from_str(&raw.8)?,
            data: serde_json::from_str(&raw.9)?,
            created_at: raw.10,
            created_by: raw.11,
            updated_at: raw.12,
            updated_by: raw.13,
            archived_at: raw.14,
        })
    }

    pub fn list_entities(&self, page: PageRequest) -> Result<EntityPage> {
        if page.limit == 0 || page.limit > 100 {
            return Err(CoreError::Validation(
                "entity page limit must be within 1..=100".into(),
            ));
        }
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("entity page offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id FROM entities WHERE project_id=?1 ORDER BY created_at,id LIMIT ?2 OFFSET ?3",
        )?;
        let ids = statement
            .query_map(
                params![self.manifest.project_id, page.limit + 1, offset],
                |row| row.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = ids.len() > page.limit as usize;
        let items = ids
            .into_iter()
            .take(page.limit as usize)
            .map(|id| self.get_entity(&id))
            .collect::<Result<Vec<_>>>()?;
        Ok(EntityPage {
            next_offset: has_more.then_some(page.offset + items.len() as u64),
            items,
        })
    }

    pub fn update_entity(
        &self,
        command_id: &str,
        entity_id: &str,
        update: EntityUpdate,
    ) -> Result<Entity> {
        self.update_entity_with_context(
            &CommandContext::system_with_id(command_id),
            entity_id,
            update,
        )
    }

    pub fn update_entity_with_context(
        &self,
        command: &CommandContext,
        entity_id: &str,
        update: EntityUpdate,
    ) -> Result<Entity> {
        validate_command_context(command)?;
        let existing_type = self.get_entity(entity_id)?.entity_type;
        if crate::research::is_reserved_domain_entity_type(&existing_type)
            || crate::development::is_development_entity_type(&existing_type)
            || crate::provenance::is_provenance_entity_type(&existing_type)
        {
            return Err(CoreError::Validation(format!(
                "{existing_type} is owned by a typed domain module; use its typed command API"
            )));
        }
        validate_nonempty(&update.title, 500, "title")?;
        validate_nonempty(&update.status, 50, "status")?;
        let metadata_json =
            bounded_json(&update.metadata, MAX_ENTITY_JSON_BYTES, "entity metadata")?;
        let data_json = bounded_json(&update.data, MAX_ENTITY_JSON_BYTES, "entity data")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(&tx, &self.manifest.project_id, command, "UpdateEntity")?.is_some() {
            tx.commit()?;
            return self.get_entity(entity_id);
        }
        let now = Utc::now().to_rfc3339();
        let changed = tx.execute(
            "UPDATE entities SET title=?3,status=?4,metadata_json=?5,data_json=?6,
                    version=version+1,updated_at=?7,updated_by=?8,
                    archived_at=CASE WHEN ?4='archived' THEN ?7 ELSE NULL END
             WHERE id=?1 AND project_id=?2 AND version=?9",
            params![
                entity_id,
                self.manifest.project_id,
                update.title,
                update.status,
                metadata_json,
                data_json,
                now,
                command.actor.id,
                update.expected_version
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(format!(
                "entity {entity_id} was missing or version {} is stale",
                update.expected_version
            )));
        }
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(entity_id),
            "entity.updated",
            &json!({"entity_id": entity_id, "previous_version": update.expected_version}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "UpdateEntity",
            Some(entity_id),
            Some(update.expected_version),
            &json!({"entity_id": entity_id, "status": update.status}),
        )?;
        tx.commit()?;
        self.get_entity(entity_id)
    }

    pub fn archive_entity(
        &self,
        command_id: &str,
        entity_id: &str,
        expected_version: i64,
    ) -> Result<Entity> {
        self.archive_entity_with_context(
            &CommandContext::system_with_id(command_id),
            entity_id,
            expected_version,
        )
    }

    pub fn archive_entity_with_context(
        &self,
        command: &CommandContext,
        entity_id: &str,
        expected_version: i64,
    ) -> Result<Entity> {
        let current = self.get_entity(entity_id)?;
        self.update_entity_with_context(
            command,
            entity_id,
            EntityUpdate {
                title: current.title,
                status: "archived".into(),
                metadata: current.metadata,
                data: current.data,
                expected_version,
            },
        )
    }

    pub fn create_relationship(
        &self,
        command_id: &str,
        relation_type: &str,
        source_entity_id: &str,
        target_entity_id: &str,
        origin: OriginKind,
    ) -> Result<String> {
        self.create_relationship_with_context(
            &CommandContext::system_with_id(command_id),
            NewRelationship {
                relation_type: relation_type.into(),
                relation_version: 1,
                source_entity_id: source_entity_id.into(),
                target_entity_id: target_entity_id.into(),
                origin,
                confidence: None,
                review_state: RelationshipReviewState::Unreviewed,
                direct_source_ids: Vec::new(),
                supersedes_id: None,
            },
        )
    }

    pub fn create_relationship_with_context(
        &self,
        command: &CommandContext,
        input: NewRelationship,
    ) -> Result<String> {
        self.create_relationship_with_policy(command, input, &CoreRelationshipPolicy)
    }

    pub fn create_relationship_with_policy(
        &self,
        command: &CommandContext,
        input: NewRelationship,
        policy: &dyn RelationshipPolicy,
    ) -> Result<String> {
        validate_command_context(command)?;
        validate_nonempty(&input.relation_type, 100, "relation_type")?;
        if input.relation_version == 0 {
            return Err(CoreError::Validation(
                "relation_version must be positive".into(),
            ));
        }
        if input.origin == OriginKind::AiProposal {
            return Err(CoreError::Validation(
                "AI proposal origin belongs in candidate storage, not canonical relationships"
                    .into(),
            ));
        }
        if input
            .confidence
            .is_some_and(|value| !(0.0..=1.0).contains(&value))
        {
            return Err(CoreError::Validation(
                "relationship confidence must be within 0.0..=1.0".into(),
            ));
        }
        if input.source_entity_id == input.target_entity_id
            && input.relation_type != "contextualizes"
        {
            return Err(CoreError::Validation(
                "self-relationship is not allowed for this relation family".into(),
            ));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        crate::provenance::require_active_project(&tx, &self.manifest.project_id)?;
        if let Some(result) = prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "CreateRelationship",
        )? {
            return result.ok_or_else(|| CoreError::Conflict("command has no result ID".into()));
        }
        let source_type =
            entity_type_in_project(&tx, &self.manifest.project_id, &input.source_entity_id)?;
        let target_type =
            entity_type_in_project(&tx, &self.manifest.project_id, &input.target_entity_id)?;
        policy
            .validate_pair(&source_type, &input.relation_type, &target_type)
            .map_err(CoreError::Validation)?;
        crate::research::validate_registered_relationship_pair(
            &source_type,
            &input.relation_type,
            &target_type,
        )?;
        crate::development::validate_development_relationship_pair(
            &source_type,
            &input.relation_type,
            &target_type,
        )?;
        crate::provenance::validate_provenance_relationship_pair(
            &source_type,
            &input.relation_type,
            &target_type,
        )?;
        if input.relation_type == "supersedes"
            && crate::provenance::relationship_would_cycle(
                &tx,
                &self.manifest.project_id,
                &input.source_entity_id,
                &input.target_entity_id,
                "supersedes",
            )?
        {
            return Err(CoreError::Validation(
                "supersedes relationship would create a cycle".into(),
            ));
        }
        for source_id in &input.direct_source_ids {
            entity_type_in_project(&tx, &self.manifest.project_id, source_id)?;
        }
        let superseded_state = if let Some(supersedes_id) = &input.supersedes_id {
            let prior: Option<(String, String, String, i64)> = tx
                .query_row(
                    "SELECT relation_type,status,review_state,state_version FROM relationships
                     WHERE id=?1 AND project_id=?2",
                    params![supersedes_id, self.manifest.project_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
                )
                .optional()?;
            if prior.as_ref().map(|value| value.0.as_str()) != Some(input.relation_type.as_str()) {
                return Err(CoreError::Validation(
                    "superseded relationship must exist in this project and use the same type"
                        .into(),
                ));
            }
            if prior
                .as_ref()
                .is_some_and(|value| matches!(value.1.as_str(), "archived" | "superseded"))
            {
                return Err(CoreError::Validation(
                    "retired or superseded relationship cannot be superseded again".into(),
                ));
            }
            prior.map(|value| (supersedes_id.clone(), value.1, value.2, value.3))
        } else {
            None
        };
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        let direct_source_ids_json = bounded_json(
            &serde_json::to_value(&input.direct_source_ids)?,
            MAX_ENTITY_JSON_BYTES,
            "relationship direct sources",
        )?;
        tx.execute(
            "INSERT INTO relationships(
                id,project_id,relation_type,relation_version,source_entity_id,source_entity_type,
                target_entity_id,target_entity_type,status,origin_type,actor_id,confidence,
                review_state,direct_source_ids_json,supersedes_id,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'active',?9,?10,?11,?12,?13,?14,?15,?15)",
            params![
                id,
                self.manifest.project_id,
                input.relation_type,
                input.relation_version,
                input.source_entity_id,
                source_type,
                input.target_entity_id,
                target_type,
                input.origin.as_str(),
                command.actor.id,
                input.confidence,
                input.review_state.as_str(),
                direct_source_ids_json,
                input.supersedes_id,
                now
            ],
        )?;
        if let Some(supersedes_id) = &input.supersedes_id {
            tx.execute(
                "UPDATE relationships SET status='superseded',state_version=state_version+1,
                    updated_at=?3
                 WHERE id=?1 AND project_id=?2",
                params![supersedes_id, self.manifest.project_id, now],
            )?;
        }
        let sequence = append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&id),
            "relationship.created",
            &json!({"relationship_id": id, "type": input.relation_type, "source": input.source_entity_id, "target": input.target_entity_id}),
        )?;
        if let Some((supersedes_id, prior_status, prior_review, prior_version)) = superseded_state {
            tx.execute(
                "INSERT INTO relationship_history(
                    id,project_id,relationship_id,ledger_sequence,action,state_version,
                    from_status,to_status,from_review_state,to_review_state,annotation,actor_id,occurred_at
                 ) VALUES(?1,?2,?3,?4,'superseded',?5,?6,'superseded',?7,?7,'',?8,?9)",
                params![
                    new_id(),
                    self.manifest.project_id,
                    supersedes_id,
                    sequence,
                    prior_version + 1,
                    prior_status,
                    prior_review,
                    command.actor.id,
                    now
                ],
            )?;
        }
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "CreateRelationship",
            Some(&id),
            None,
            &json!({"relation_type": input.relation_type, "relation_version": input.relation_version}),
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn get_relationship(&self, id: &str) -> Result<Relationship> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT id,project_id,relation_type,relation_version,source_entity_id,source_entity_type,
                        target_entity_id,target_entity_type,status,origin_type,actor_id,confidence,
                        review_state,direct_source_ids_json,supersedes_id,created_at,updated_at,
                        state_version,annotation,reviewed_at,reviewed_by,retired_at
                 FROM relationships WHERE id=?1 AND project_id=?2",
                params![id, self.manifest.project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                        row.get::<_, Option<f64>>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, String>(13)?,
                        row.get::<_, Option<String>>(14)?,
                        row.get::<_, String>(15)?,
                        row.get::<_, String>(16)?,
                        row.get::<_, i64>(17)?,
                        row.get::<_, String>(18)?,
                        row.get::<_, Option<String>>(19)?,
                        row.get::<_, Option<String>>(20)?,
                        row.get::<_, Option<String>>(21)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))
            .and_then(|raw| {
                Ok(Relationship {
                    id: raw.0,
                    project_id: raw.1,
                    relation_type: raw.2,
                    relation_version: raw.3,
                    source_entity_id: raw.4,
                    source_entity_type: raw.5,
                    target_entity_id: raw.6,
                    target_entity_type: raw.7,
                    status: raw.8,
                    origin: raw.9,
                    actor_id: raw.10,
                    confidence: raw.11,
                    review_state: raw.12,
                    state_version: raw.17,
                    annotation: raw.18,
                    direct_source_ids: serde_json::from_str(&raw.13)?,
                    supersedes_id: raw.14,
                    created_at: raw.15,
                    updated_at: raw.16,
                    reviewed_at: raw.19,
                    reviewed_by: raw.20,
                    retired_at: raw.21,
                })
            })
    }

    pub fn list_relationships_for_entity(
        &self,
        entity_id: &str,
        page: PageRequest,
    ) -> Result<RelationshipPage> {
        if page.limit == 0 || page.limit > 100 {
            return Err(CoreError::Validation(
                "relationship page limit must be within 1..=100".into(),
            ));
        }
        self.get_entity(entity_id)?;
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("relationship page offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id FROM relationships
             WHERE project_id=?1 AND (source_entity_id=?2 OR target_entity_id=?2)
             ORDER BY created_at,id LIMIT ?3 OFFSET ?4",
        )?;
        let ids = statement
            .query_map(
                params![self.manifest.project_id, entity_id, page.limit + 1, offset],
                |row| row.get::<_, String>(0),
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = ids.len() > page.limit as usize;
        let items = ids
            .into_iter()
            .take(page.limit as usize)
            .map(|id| self.get_relationship(&id))
            .collect::<Result<Vec<_>>>()?;
        Ok(RelationshipPage {
            next_offset: has_more.then_some(page.offset + items.len() as u64),
            items,
        })
    }

    pub fn ingest_artifact(
        &self,
        command_id: &str,
        bytes: &[u8],
        media_type: &str,
    ) -> Result<ArtifactRecord> {
        self.ingest_artifact_reader_with_context(
            &CommandContext::system_with_id(command_id),
            Cursor::new(bytes),
            media_type,
            ArtifactClassification::Internal,
            OriginKind::Deterministic,
            &json!({}),
        )
    }

    pub fn ingest_artifact_file(
        &self,
        command_id: &str,
        source: impl AsRef<Path>,
        media_type: &str,
    ) -> Result<ArtifactRecord> {
        let source = source.as_ref();
        if fs::symlink_metadata(source)?.file_type().is_symlink() {
            return Err(CoreError::Validation(
                "artifact source must not be a symbolic link".into(),
            ));
        }
        self.ingest_artifact_reader_with_context(
            &CommandContext::system_with_id(command_id),
            open_regular_file_nofollow(source)?,
            media_type,
            ArtifactClassification::Internal,
            OriginKind::Import,
            &json!({"source_kind": "file"}),
        )
    }

    pub fn ingest_artifact_reader(
        &self,
        command_id: &str,
        reader: impl Read,
        media_type: &str,
    ) -> Result<ArtifactRecord> {
        self.ingest_artifact_reader_with_context(
            &CommandContext::system_with_id(command_id),
            reader,
            media_type,
            ArtifactClassification::Internal,
            OriginKind::Deterministic,
            &json!({}),
        )
    }

    pub fn ingest_artifact_reader_with_context(
        &self,
        command: &CommandContext,
        mut reader: impl Read,
        media_type: &str,
        classification: ArtifactClassification,
        origin: OriginKind,
        metadata: &Value,
    ) -> Result<ArtifactRecord> {
        validate_command_context(command)?;
        validate_nonempty(media_type, 200, "media_type")?;
        if origin == OriginKind::AiProposal {
            return Err(CoreError::Validation(
                "AI proposal payloads require the generated-artifact candidate workflow".into(),
            ));
        }
        let metadata_json = bounded_json(metadata, MAX_ENTITY_JSON_BYTES, "artifact metadata")?;
        let staging_path = self.root.join("staging").join(format!("{}.part", new_id()));
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut staging = options.open(&staging_path)?;
        let mut digest = Sha256::new();
        let mut staged_size = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = reader.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
            staging.write_all(&buffer[..read])?;
            staged_size = staged_size
                .checked_add(read as u64)
                .ok_or_else(|| CoreError::Validation("artifact size overflow".into()))?;
        }
        staging.sync_all()?;
        drop(staging);
        let hash = hex::encode(digest.finalize());
        let (verified_hash, verified_size) = hash_file(&staging_path)?;
        if verified_hash != hash || verified_size != staged_size {
            let quarantine = self.root.join("quarantine").join(
                staging_path
                    .file_name()
                    .expect("staging filename is present"),
            );
            fs::rename(&staging_path, &quarantine)?;
            return Err(CoreError::ArtifactIntegrity {
                path: quarantine,
                reason: "staged bytes changed before finalization".into(),
            });
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(Some(result_id)) =
            prior_result(&tx, &self.manifest.project_id, command, "FinalizeArtifact")?
        {
            let prior: Option<(String, String)> = tx
                .query_row(
                    "SELECT a.sha256,c.payload_json FROM commands c
                     JOIN artifacts a ON a.id=c.result_id AND a.project_id=c.project_id
                     WHERE c.project_id=?1 AND c.operation='FinalizeArtifact'
                       AND (c.id=?2 OR c.idempotency_key=?3)
                     ORDER BY CASE WHEN c.id=?2 THEN 0 ELSE 1 END LIMIT 1",
                    params![
                        self.manifest.project_id,
                        command.command_id,
                        command.idempotency_key
                    ],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            let Some((stored_artifact_hash, prior_payload_json)) = prior else {
                let _ = fs::remove_file(&staging_path);
                return Err(CoreError::Conflict(
                    "idempotent artifact command points to a missing result".into(),
                ));
            };
            let prior_payload: Value = serde_json::from_str(&prior_payload_json)?;
            let prior_hash = prior_payload
                .get("sha256")
                .and_then(Value::as_str)
                .unwrap_or(&stored_artifact_hash);
            let prior_media_type = prior_payload.get("media_type").and_then(Value::as_str);
            let prior_classification = prior_payload.get("classification").and_then(Value::as_str);
            let prior_origin = prior_payload.get("origin").and_then(Value::as_str);
            let prior_metadata = prior_payload.get("metadata");
            if prior_hash != hash
                || prior_media_type != Some(media_type)
                || prior_classification != Some(classification.as_str())
                || prior_origin.is_some_and(|value| value != origin.as_str())
                || prior_metadata.is_some_and(|value| value != metadata)
            {
                let _ = fs::remove_file(&staging_path);
                return Err(CoreError::Conflict(
                    "idempotency key was reused with different artifact bytes or attributes".into(),
                ));
            }
            tx.commit()?;
            fs::remove_file(&staging_path)?;
            return self.get_artifact(&result_id);
        }
        let relative = relative_content_path(&hash)?;
        let final_path = self.root.join(&relative);
        fs::create_dir_all(final_path.parent().expect("content path has parent"))?;
        let mut pending_content = PendingContentFile {
            path: final_path.clone(),
            armed: false,
        };
        if final_path.exists() {
            let (existing_hash, existing_size) = hash_file(&final_path)?;
            if existing_hash != hash || existing_size != staged_size {
                return Err(CoreError::ArtifactIntegrity {
                    path: final_path,
                    reason: "content-addressed destination has unexpected bytes".into(),
                });
            }
            fs::remove_file(&staging_path)?;
        } else {
            fs::rename(&staging_path, &final_path)?;
            pending_content.armed = true;
        }
        let existing: Option<(String, String)> = tx
            .query_row(
                "SELECT id,classification FROM artifacts WHERE project_id=?1 AND sha256=?2",
                params![self.manifest.project_id, hash],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let id = existing
            .as_ref()
            .map(|value| value.0.clone())
            .unwrap_or_else(new_id);
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT OR IGNORE INTO artifacts(
                id,project_id,sha256,byte_size,media_type,relative_path,created_at,
                classification,availability,origin_type,metadata_json,created_by,updated_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'available',?9,?10,?11,?7)",
            params![
                id,
                self.manifest.project_id,
                hash,
                staged_size as i64,
                media_type,
                path_to_slashes(&relative),
                now,
                classification.as_str(),
                origin.as_str(),
                metadata_json,
                command.actor.id
            ],
        )?;
        if let Some((_, existing_classification)) = existing {
            let effective =
                more_restrictive_classification(&existing_classification, classification.as_str());
            tx.execute(
                "UPDATE artifacts SET classification=?2,availability='available',unavailable_reason=NULL,updated_at=?3
                 WHERE id=?1",
                params![id, effective, now],
            )?;
        }
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&id),
            "artifact.finalized",
            &json!({"artifact_id": id, "sha256": hash, "byte_size": staged_size, "classification": classification.as_str()}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "FinalizeArtifact",
            Some(&id),
            None,
            &json!({"sha256":hash,"byte_size":staged_size,"media_type":media_type,
                "classification":classification.as_str(),"origin":origin.as_str(),
                "metadata":metadata}),
        )?;
        tx.commit()?;
        pending_content.armed = false;
        self.get_artifact(&id)
    }

    pub fn get_artifact(&self, id: &str) -> Result<ArtifactRecord> {
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT id,project_id,sha256,byte_size,media_type,relative_path,classification,
                        availability,origin_type,metadata_json,unavailable_reason,created_at,created_by,updated_at
                 FROM artifacts WHERE id=?1 AND project_id=?2",
                params![id, self.manifest.project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, String>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, String>(13)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        Ok(ArtifactRecord {
            id: raw.0,
            project_id: raw.1,
            sha256: raw.2,
            byte_size: raw.3,
            media_type: raw.4,
            relative_path: raw.5,
            classification: raw.6,
            availability: raw.7,
            origin: raw.8,
            metadata: serde_json::from_str(&raw.9)?,
            unavailable_reason: raw.10,
            created_at: raw.11,
            created_by: raw.12,
            updated_at: raw.13,
        })
    }

    pub fn read_artifact_bounded(&self, id: &str, max_bytes: u64) -> Result<Vec<u8>> {
        const ABSOLUTE_PREVIEW_MAX: u64 = 32 * 1024 * 1024;
        if max_bytes == 0 || max_bytes > ABSOLUTE_PREVIEW_MAX {
            return Err(CoreError::Validation(
                "artifact read bound must be within 1..=32 MiB".into(),
            ));
        }
        let artifact = self.get_artifact(id)?;
        if artifact.availability != ArtifactAvailability::Available.as_str() {
            return Err(CoreError::Conflict(
                "artifact payload is unavailable".into(),
            ));
        }
        let byte_size = u64::try_from(artifact.byte_size)
            .map_err(|_| CoreError::Validation("artifact size is invalid".into()))?;
        if byte_size > max_bytes {
            return Err(CoreError::Validation(format!(
                "artifact exceeds the {max_bytes}-byte read bound"
            )));
        }
        let expected_relative = relative_content_path(&artifact.sha256)?;
        if path_to_slashes(&expected_relative) != artifact.relative_path {
            return Err(CoreError::ArtifactIntegrity {
                path: self.root.join(&artifact.relative_path),
                reason: "artifact metadata path does not match its hash".into(),
            });
        }
        let path = self.root.join(expected_relative);
        let mut reader = open_regular_file_nofollow(&path)?.take(max_bytes.saturating_add(1));
        let mut bytes = Vec::with_capacity(usize::try_from(byte_size).unwrap_or_default());
        reader.read_to_end(&mut bytes)?;
        if bytes.len() as u64 != byte_size || hex::encode(Sha256::digest(&bytes)) != artifact.sha256
        {
            return Err(CoreError::ArtifactIntegrity {
                path,
                reason: "artifact preview failed size or SHA-256 verification".into(),
            });
        }
        Ok(bytes)
    }

    pub fn set_artifact_classification(
        &self,
        command: &CommandContext,
        artifact_id: &str,
        classification: ArtifactClassification,
        reason: &str,
    ) -> Result<ArtifactRecord> {
        validate_command_context(command)?;
        validate_nonempty(reason, 1_000, "classification change reason")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "SetArtifactClassification",
        )?
        .is_some()
        {
            tx.commit()?;
            return self.get_artifact(artifact_id);
        }
        let changed = tx.execute(
            "UPDATE artifacts SET classification=?3,updated_at=?4 WHERE id=?1 AND project_id=?2",
            params![
                artifact_id,
                self.manifest.project_id,
                classification.as_str(),
                Utc::now().to_rfc3339()
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::NotFound(artifact_id.into()));
        }
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(artifact_id),
            "artifact.classification_changed",
            &json!({"artifact_id": artifact_id, "classification": classification.as_str(), "reason": truncate(reason, 200)}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "SetArtifactClassification",
            Some(artifact_id),
            None,
            &json!({"classification": classification.as_str(), "reason": truncate(reason, 200)}),
        )?;
        tx.commit()?;
        self.get_artifact(artifact_id)
    }

    pub fn mark_artifact_unavailable(
        &self,
        command: &CommandContext,
        artifact_id: &str,
        reason: &str,
    ) -> Result<ArtifactRecord> {
        validate_command_context(command)?;
        validate_nonempty(reason, 2_000, "unavailable reason")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "MarkArtifactUnavailable",
        )?
        .is_some()
        {
            tx.commit()?;
            return self.get_artifact(artifact_id);
        }
        let changed = tx.execute(
            "UPDATE artifacts SET availability='unavailable',unavailable_reason=?3,updated_at=?4
             WHERE id=?1 AND project_id=?2",
            params![
                artifact_id,
                self.manifest.project_id,
                truncate(reason, 2_000),
                Utc::now().to_rfc3339()
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::NotFound(artifact_id.into()));
        }
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(artifact_id),
            "artifact.unavailable",
            &json!({"artifact_id": artifact_id, "reason": truncate(reason, 200)}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "MarkArtifactUnavailable",
            Some(artifact_id),
            None,
            &json!({"reason_code": "declared_unavailable"}),
        )?;
        tx.commit()?;
        self.get_artifact(artifact_id)
    }

    pub fn purge_artifact_payload(
        &self,
        command: &CommandContext,
        artifact_id: &str,
    ) -> Result<ArtifactRecord> {
        validate_command_context(command)?;
        let artifact = self.get_artifact(artifact_id)?;
        let expected_relative = relative_content_path(&artifact.sha256)?;
        if path_to_slashes(&expected_relative) != artifact.relative_path {
            return Err(CoreError::ArtifactIntegrity {
                path: self.root.join(&artifact.relative_path),
                reason: "artifact metadata path does not match its hash".into(),
            });
        }
        let payload = self.root.join(&expected_relative);
        let quarantined =
            self.root
                .join("quarantine")
                .join(format!("purge-{}-{}", artifact.sha256, new_id()));

        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "PurgeArtifactPayload",
        )?
        .is_some()
        {
            tx.commit()?;
            if payload.exists() {
                fs::remove_file(&payload)?;
            }
            return self.get_artifact(artifact_id);
        }
        if payload.exists() {
            fs::rename(&payload, &quarantined)?;
        }
        let transaction_result = (|| -> Result<()> {
            tx.execute(
                "UPDATE artifacts SET availability='purged_payload',unavailable_reason='explicit purge',updated_at=?3
                 WHERE id=?1 AND project_id=?2",
                params![artifact_id, self.manifest.project_id, Utc::now().to_rfc3339()],
            )?;
            append_event_with_context(
                &tx,
                &self.manifest.project_id,
                command,
                Some(artifact_id),
                "artifact.payload_purged",
                &json!({"artifact_id": artifact_id, "sha256": artifact.sha256}),
            )?;
            record_command_with_context(
                &tx,
                command,
                &self.manifest.project_id,
                "PurgeArtifactPayload",
                Some(artifact_id),
                None,
                &json!({"artifact_id": artifact_id}),
            )?;
            tx.commit()?;
            Ok(())
        })();
        if let Err(error) = transaction_result {
            if quarantined.exists() && !payload.exists() {
                let _ = fs::rename(&quarantined, &payload);
            }
            return Err(error);
        }
        if quarantined.exists() {
            fs::remove_file(quarantined)?;
        }
        self.get_artifact(artifact_id)
    }

    pub fn link_artifact_to_entity(
        &self,
        command_id: &str,
        entity_id: &str,
        artifact_id: &str,
        role: &str,
    ) -> Result<()> {
        self.link_artifact_to_entity_with_context(
            &CommandContext::system_with_id(command_id),
            entity_id,
            artifact_id,
            role,
        )
    }

    pub fn link_artifact_to_entity_with_context(
        &self,
        command: &CommandContext,
        entity_id: &str,
        artifact_id: &str,
        role: &str,
    ) -> Result<()> {
        validate_command_context(command)?;
        validate_nonempty(role, 100, "artifact role")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "LinkArtifactToEntity",
        )?
        .is_some()
        {
            return tx.commit().map_err(Into::into);
        }
        let valid_pair: bool = tx.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM entities e JOIN artifacts a ON a.project_id=e.project_id
                WHERE e.id=?1 AND a.id=?2 AND e.project_id=?3
             )",
            params![entity_id, artifact_id, self.manifest.project_id],
            |row| row.get(0),
        )?;
        if !valid_pair {
            return Err(CoreError::Validation(
                "entity and artifact must exist in the same project".into(),
            ));
        }
        tx.execute(
            "INSERT INTO entity_artifacts(entity_id,artifact_id,role,created_at) VALUES(?1,?2,?3,?4)",
            params![entity_id, artifact_id, role, Utc::now().to_rfc3339()],
        )?;
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(entity_id),
            "artifact.linked",
            &json!({"entity_id": entity_id, "artifact_id": artifact_id, "role": role}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "LinkArtifactToEntity",
            None,
            None,
            &json!({"entity_id": entity_id, "artifact_id": artifact_id, "role": role}),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn create_checkpoint(
        &self,
        command_id: &str,
        scope: CheckpointScope,
        summary: &Value,
        source_entity_ids: &[String],
    ) -> Result<Checkpoint> {
        self.create_checkpoint_with_context(
            &CommandContext::system_with_id(command_id),
            scope,
            summary,
            source_entity_ids,
        )
    }

    pub fn create_checkpoint_with_context(
        &self,
        command: &CommandContext,
        scope: CheckpointScope,
        summary: &Value,
        source_entity_ids: &[String],
    ) -> Result<Checkpoint> {
        validate_command_context(command)?;
        let summary_json = bounded_json(summary, MAX_CHECKPOINT_JSON_BYTES, "checkpoint summary")?;
        let mut unique_sources = HashSet::new();
        if !source_entity_ids.iter().all(|id| unique_sources.insert(id)) {
            return Err(CoreError::Validation("duplicate checkpoint source".into()));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(Some(result_id)) =
            prior_result(&tx, &self.manifest.project_id, command, "CreateCheckpoint")?
        {
            tx.commit()?;
            return self.get_checkpoint(&result_id);
        }
        let sequence: i64 = tx.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [&self.manifest.project_id],
            |row| row.get(0),
        )?;
        let mut source_versions = Vec::with_capacity(source_entity_ids.len());
        for source in source_entity_ids {
            let version: i64 = tx
                .query_row(
                    "SELECT version FROM entities WHERE id=?1 AND project_id=?2",
                    params![source, self.manifest.project_id],
                    |row| row.get(0),
                )
                .optional()?
                .ok_or_else(|| {
                    CoreError::Validation(format!("checkpoint source {source} is invalid"))
                })?;
            source_versions.push((source, version));
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO checkpoints(id,project_id,scope,ledger_sequence,summary_json,created_at,created_by)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![
                id,
                self.manifest.project_id,
                scope.as_str(),
                sequence,
                summary_json,
                now,
                command.actor.id
            ],
        )?;
        for (source, version) in source_versions {
            tx.execute(
                "INSERT INTO checkpoint_sources(checkpoint_id,source_entity_id,source_version)
                 VALUES(?1,?2,?3)",
                params![id, source, version],
            )?;
        }
        append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&id),
            "checkpoint.created",
            &json!({"checkpoint_id": id, "scope": scope.as_str(), "source_ledger_sequence": sequence}),
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "CreateCheckpoint",
            Some(&id),
            None,
            &json!({"scope": scope.as_str(), "source_count": source_entity_ids.len()}),
        )?;
        tx.commit()?;
        self.get_checkpoint(&id)
    }

    pub fn get_checkpoint(&self, id: &str) -> Result<Checkpoint> {
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT id,project_id,scope,ledger_sequence,summary_json,created_at,created_by
                 FROM checkpoints WHERE id=?1 AND project_id=?2",
                params![id, self.manifest.project_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        Ok(Checkpoint {
            id: raw.0,
            project_id: raw.1,
            scope: raw.2,
            ledger_sequence: raw.3,
            summary: serde_json::from_str(&raw.4)?,
            created_at: raw.5,
            created_by: raw.6,
        })
    }

    pub fn enqueue_job(
        &self,
        job_type: &str,
        idempotency_key: &str,
        payload: &Value,
        max_attempts: i64,
    ) -> Result<Job> {
        self.enqueue_job_with_context(
            &CommandContext::system(),
            job_type,
            idempotency_key,
            payload,
            max_attempts,
        )
    }

    pub fn enqueue_job_with_context(
        &self,
        command: &CommandContext,
        job_type: &str,
        idempotency_key: &str,
        payload: &Value,
        max_attempts: i64,
    ) -> Result<Job> {
        validate_command_context(command)?;
        validate_nonempty(job_type, 100, "job_type")?;
        validate_nonempty(idempotency_key, 200, "idempotency_key")?;
        if !(1..=20).contains(&max_attempts) {
            return Err(CoreError::Validation(
                "max_attempts must be within 1..=20".into(),
            ));
        }
        let payload_json = bounded_json(payload, MAX_ENTITY_JSON_BYTES, "job payload")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: Option<String> = tx
            .query_row(
                "SELECT id FROM jobs WHERE project_id=?1 AND job_type=?2 AND idempotency_key=?3",
                params![self.manifest.project_id, job_type, idempotency_key],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(id) = existing {
            tx.commit()?;
            return self.get_job(&id);
        }
        let active: i64 = tx.query_row(
            "SELECT count(*) FROM jobs WHERE project_id=?1 AND state IN ('queued','running')",
            [&self.manifest.project_id],
            |row| row.get(0),
        )?;
        if active >= MAX_ACTIVE_JOBS {
            return Err(CoreError::Conflict(format!(
                "active job limit {MAX_ACTIVE_JOBS} reached"
            )));
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO jobs(
                id,project_id,job_type,state,attempts,max_attempts,payload_json,idempotency_key,
                available_at,created_at,updated_at,created_by,updated_by
             ) VALUES(?1,?2,?3,'queued',0,?4,?5,?6,?7,?7,?7,?8,?8)",
            params![
                id,
                self.manifest.project_id,
                job_type,
                max_attempts,
                payload_json,
                idempotency_key,
                now,
                command.actor.id
            ],
        )?;
        tx.commit()?;
        self.get_job(&id)
    }

    pub fn claim_next_job(&self, lease: Duration) -> Result<Option<Job>> {
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = Utc::now();
        let now_text = now.to_rfc3339();
        let id: Option<String> = tx
            .query_row(
                "SELECT id FROM jobs
                 WHERE project_id=?1 AND (
                    (state='queued' AND available_at<=?2 AND cancellation_requested=0) OR
                    (state='running' AND lease_until IS NOT NULL AND lease_until<?2 AND cancellation_requested=0)
                 ) AND attempts < max_attempts
                 ORDER BY created_at LIMIT 1",
                params![self.manifest.project_id, now_text],
                |row| row.get(0),
            )
            .optional()?;
        let Some(id) = id else {
            tx.commit()?;
            return Ok(None);
        };
        let lease_until = (now
            + ChronoDuration::from_std(lease).map_err(|_| {
                CoreError::Validation("job lease duration is outside supported range".into())
            })?)
        .to_rfc3339();
        tx.execute(
            "UPDATE jobs SET state='running',attempts=attempts+1,lease_until=?2,updated_at=?3,
                    updated_by='continuum-worker' WHERE id=?1",
            params![id, lease_until, now_text],
        )?;
        tx.commit()?;
        Ok(Some(self.get_job(&id)?))
    }

    pub fn finish_job(&self, id: &str, succeeded: bool, error: Option<&str>) -> Result<Job> {
        let connection = self.connection()?;
        let state = if succeeded { "succeeded" } else { "failed" };
        let changed = connection.execute(
            "UPDATE jobs SET state=CASE WHEN cancellation_requested=1 THEN 'cancelled' ELSE ?3 END,
                    lease_until=NULL,last_error=?4,updated_at=?5,updated_by='continuum-worker'
             WHERE id=?1 AND project_id=?2 AND state='running'",
            params![
                id,
                self.manifest.project_id,
                state,
                error.map(|value| truncate(value, 2_000)),
                Utc::now().to_rfc3339()
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(format!("job {id} is not running")));
        }
        self.get_job(id)
    }

    pub fn update_job_progress(
        &self,
        id: &str,
        current: i64,
        total: Option<i64>,
        message: Option<&str>,
    ) -> Result<Job> {
        if current < 0 || total.is_some_and(|value| value < current || value < 0) {
            return Err(CoreError::Validation(
                "job progress must be non-negative and current cannot exceed total".into(),
            ));
        }
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE jobs SET progress_current=?3,progress_total=?4,progress_message=?5,updated_at=?6,
                    updated_by='continuum-worker'
             WHERE id=?1 AND project_id=?2 AND state='running'",
            params![
                id,
                self.manifest.project_id,
                current,
                total,
                message.map(|value| truncate(value, 500)),
                Utc::now().to_rfc3339()
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(format!("job {id} is not running")));
        }
        self.get_job(id)
    }

    pub fn request_job_cancellation(&self, id: &str) -> Result<Job> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE jobs SET cancellation_requested=1,
                    state=CASE WHEN state='queued' THEN 'cancelled' ELSE state END,
                    updated_at=?3,updated_by='continuum-core'
             WHERE id=?1 AND project_id=?2 AND state IN ('queued','running')",
            params![id, self.manifest.project_id, Utc::now().to_rfc3339()],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(format!(
                "job {id} cannot be cancelled from its current state"
            )));
        }
        self.get_job(id)
    }

    pub fn retry_failed_job(&self, id: &str) -> Result<Job> {
        let connection = self.connection()?;
        let now = Utc::now().to_rfc3339();
        let changed = connection.execute(
            "UPDATE jobs SET state='queued',cancellation_requested=0,available_at=?3,
                    lease_until=NULL,last_error=NULL,updated_at=?3,updated_by='continuum-core'
             WHERE id=?1 AND project_id=?2 AND state='failed' AND attempts<max_attempts",
            params![id, self.manifest.project_id, now],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(format!("job {id} is not retryable")));
        }
        self.get_job(id)
    }

    pub fn get_job(&self, id: &str) -> Result<Job> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT id,project_id,job_type,state,attempts,max_attempts,payload_json,
                        progress_current,progress_total,progress_message,cancellation_requested,
                        created_by,updated_by
                 FROM jobs WHERE id=?1 AND project_id=?2",
                params![id, self.manifest.project_id],
                |row| {
                    Ok(Job {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        job_type: row.get(2)?,
                        state: row.get(3)?,
                        attempts: row.get(4)?,
                        max_attempts: row.get(5)?,
                        payload_json: row.get(6)?,
                        progress_current: row.get(7)?,
                        progress_total: row.get(8)?,
                        progress_message: row.get(9)?,
                        cancellation_requested: row.get(10)?,
                        created_by: row.get(11)?,
                        updated_by: row.get(12)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))
    }

    pub fn claim_next_outbox(&self, lease: Duration) -> Result<Option<OutboxMessage>> {
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = Utc::now();
        let now_text = now.to_rfc3339();
        let id: Option<String> = tx
            .query_row(
                "SELECT id FROM outbox
                 WHERE project_id=?1 AND processed_at IS NULL
                   AND (lease_until IS NULL OR lease_until<?2)
                 ORDER BY created_at,id LIMIT 1",
                params![self.manifest.project_id, now_text],
                |row| row.get(0),
            )
            .optional()?;
        let Some(id) = id else {
            tx.commit()?;
            return Ok(None);
        };
        let lease_until = (now
            + ChronoDuration::from_std(lease).map_err(|_| {
                CoreError::Validation("outbox lease duration is outside supported range".into())
            })?)
        .to_rfc3339();
        tx.execute(
            "UPDATE outbox SET attempts=attempts+1,lease_until=?2,last_error=NULL WHERE id=?1",
            params![id, lease_until],
        )?;
        let message = read_outbox(&tx, &self.manifest.project_id, &id)?;
        tx.commit()?;
        Ok(Some(message))
    }

    pub fn complete_outbox(&self, id: &str) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE outbox SET processed_at=?3,lease_until=NULL,last_error=NULL
             WHERE id=?1 AND project_id=?2 AND processed_at IS NULL AND lease_until IS NOT NULL",
            params![id, self.manifest.project_id, Utc::now().to_rfc3339()],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(format!(
                "outbox message {id} is not actively leased"
            )));
        }
        Ok(())
    }

    pub fn fail_outbox(&self, id: &str, error: &str) -> Result<()> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE outbox SET lease_until=NULL,last_error=?3
             WHERE id=?1 AND project_id=?2 AND processed_at IS NULL AND lease_until IS NOT NULL",
            params![id, self.manifest.project_id, truncate(error, 2_000)],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(format!(
                "outbox message {id} is not actively leased"
            )));
        }
        Ok(())
    }

    pub fn get_audit_event(&self, id: &str) -> Result<AuditEvent> {
        let connection = self.connection()?;
        connection
            .query_row(
                "SELECT id,project_id,ledger_sequence,command_id,aggregate_id,event_type,event_version,
                        actor_kind,actor_id,correlation_id,causation_id,payload_json,occurred_at
                 FROM audit_events WHERE id=?1 AND project_id=?2",
                params![id, self.manifest.project_id],
                |row| {
                    Ok(AuditEvent {
                        id: row.get(0)?,
                        project_id: row.get(1)?,
                        ledger_sequence: row.get(2)?,
                        command_id: row.get(3)?,
                        aggregate_id: row.get(4)?,
                        event_type: row.get(5)?,
                        event_version: row.get(6)?,
                        actor_kind: row.get(7)?,
                        actor_id: row.get(8)?,
                        correlation_id: row.get(9)?,
                        causation_id: row.get(10)?,
                        payload_json: row.get(11)?,
                        occurred_at: row.get(12)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))
    }

    pub fn verify_integrity(&self) -> Result<IntegrityReport> {
        let connection = self.connection()?;
        let sqlite_status: String =
            connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        let mut report = IntegrityReport::default();
        if sqlite_status != "ok" {
            report.issues.push(IntegrityIssue {
                code: "database_integrity".into(),
                path_or_id: LEDGER_FILE.into(),
                guidance:
                    "Restore the latest verified backup; preserve this project for diagnostics."
                        .into(),
            });
        }

        let mut foreign_key_check = connection.prepare("PRAGMA foreign_key_check")?;
        let foreign_key_issues = foreign_key_check
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for (table, row_id, parent) in foreign_key_issues {
            report.issues.push(IntegrityIssue {
                code: "dangling_foreign_key".into(),
                path_or_id: format!("{table}:{}", row_id.unwrap_or_default()),
                guidance: format!(
                    "Recover the missing {parent} row from backup or quarantine the referencing record."
                ),
            });
        }

        let mut statement = connection.prepare(
            "SELECT id,sha256,byte_size,relative_path,availability FROM artifacts WHERE project_id=?1",
        )?;
        let records = statement
            .query_map([&self.manifest.project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let mut expected_paths = HashSet::new();
        for (id, expected_hash, expected_size, relative, availability) in records {
            report.checked_artifacts += 1;
            let expected_relative = relative_content_path(&expected_hash)?;
            if path_to_slashes(&expected_relative) != relative || !safe_relative_path(&relative) {
                report.issues.push(IntegrityIssue {
                    code: "invalid_artifact_path".into(),
                    path_or_id: id,
                    guidance: "Quarantine the metadata row and recover from a verified export."
                        .into(),
                });
                continue;
            }
            let path = self.root.join(&relative);
            if availability == ArtifactAvailability::PurgedPayload.as_str() {
                if path.exists() {
                    report.issues.push(IntegrityIssue {
                        code: "purged_artifact_payload_present".into(),
                        path_or_id: relative,
                        guidance: "Move the payload to quarantine and complete the approved purge procedure.".into(),
                    });
                }
                continue;
            }
            expected_paths.insert(relative.clone());
            if !path.exists() {
                if availability == ArtifactAvailability::Unavailable.as_str() {
                    continue;
                }
                report.issues.push(IntegrityIssue {
                    code: "missing_artifact_payload".into(),
                    path_or_id: relative,
                    guidance: "Restore the payload from a verified backup/export or remove its references after review.".into(),
                });
                continue;
            }
            let (actual_hash, actual_size) = hash_file(&path)?;
            if actual_hash != expected_hash || actual_size as i64 != expected_size {
                report.issues.push(IntegrityIssue {
                    code: "artifact_hash_or_size_mismatch".into(),
                    path_or_id: relative,
                    guidance:
                        "Move the payload to quarantine and restore the expected hash from backup."
                            .into(),
                });
            }
        }
        let artifact_root = self.root.join("artifacts/sha256");
        for file in walk_files(&artifact_root)? {
            let relative = path_to_slashes(
                file.strip_prefix(&self.root)
                    .map_err(|_| CoreError::Validation("artifact escaped project root".into()))?,
            );
            if !expected_paths.contains(&relative) {
                report.issues.push(IntegrityIssue {
                    code: "orphan_artifact_payload".into(),
                    path_or_id: relative,
                    guidance: "Keep in recovery quarantine until provenance is established; then relink or remove explicitly.".into(),
                });
            }
        }
        crate::research::append_research_integrity_issues(
            &connection,
            &self.manifest.project_id,
            &mut report,
        )?;
        crate::development::append_development_integrity_issues(
            &connection,
            &self.manifest.project_id,
            &mut report,
        )?;
        crate::code_intelligence::append_code_intelligence_integrity_issues(
            &connection,
            &self.manifest.project_id,
            &mut report,
        )?;
        crate::provenance::append_provenance_integrity_issues(
            &connection,
            &self.manifest.project_id,
            &mut report,
        )?;
        crate::semantic::append_semantic_integrity_issues(
            &connection,
            &self.manifest.project_id,
            &mut report,
        )?;
        crate::human_document::append_human_document_integrity_issues(
            &connection,
            &self.manifest.project_id,
            &mut report,
        )?;
        crate::capture::append_capture_integrity_issues(
            &connection,
            &self.manifest.project_id,
            &mut report,
        )?;
        crate::context::append_context_integrity_issues(
            &connection,
            &self.manifest.project_id,
            &mut report,
        )?;
        crate::mcp::append_mcp_integrity_issues(
            &connection,
            &self.manifest.project_id,
            &mut report,
        )?;
        Ok(report)
    }

    pub fn backup_database(&self, destination: impl AsRef<Path>) -> Result<PathBuf> {
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(CoreError::Conflict(format!(
                "backup destination already exists: {}",
                destination.display()
            )));
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        backup_connection(&self.connection()?, destination)?;
        Ok(destination.to_path_buf())
    }

    pub fn export_project(&self, destination: impl AsRef<Path>) -> Result<()> {
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(CoreError::Conflict(format!(
                "export destination already exists: {}",
                destination.display()
            )));
        }
        let destination = resolve_new_destination(destination)?;
        let project_root = fs::canonicalize(&self.root)?;
        if destination.starts_with(&project_root) {
            return Err(CoreError::Validation(
                "project export destination must be outside the source project".into(),
            ));
        }
        let integrity = self.verify_integrity()?;
        if !integrity.is_healthy() {
            return Err(CoreError::Conflict(
                "project export blocked because integrity verification failed".into(),
            ));
        }
        let staging = destination.with_extension(format!("exporting-{}", new_id()));
        let mut pending = PendingDirectory {
            path: staging.clone(),
            armed: true,
        };
        fs::create_dir(&staging)?;
        fs::copy(self.root.join(MANIFEST_FILE), staging.join(MANIFEST_FILE))?;
        self.backup_database(staging.join(LEDGER_FILE))?;
        copy_tree(&self.root.join("artifacts"), &staging.join("artifacts"))?;
        fs::write(
            staging.join("continuum.export.json"),
            serde_json::to_vec_pretty(&json!({
                "export_version": 1,
                "project_id": self.manifest.project_id,
                "created_at": Utc::now().to_rfc3339(),
                "schema_version": CORE_SCHEMA_VERSION,
                "integrity": "verified"
            }))?,
        )?;
        fs::rename(&staging, &destination)?;
        pending.armed = false;
        Ok(())
    }

    pub fn import_export(source: impl AsRef<Path>, destination: impl AsRef<Path>) -> Result<Self> {
        let source = fs::canonicalize(source.as_ref())?;
        let destination = destination.as_ref();
        if destination.exists() {
            return Err(CoreError::Conflict(format!(
                "import destination already exists: {}",
                destination.display()
            )));
        }
        let destination = resolve_new_destination(destination)?;
        if destination.starts_with(&source) {
            return Err(CoreError::Validation(
                "project import destination must be outside the source export".into(),
            ));
        }
        ProjectManifest::load(&source)?;
        let staging = destination.with_extension(format!("importing-{}", new_id()));
        copy_tree(&source, &staging)?;
        for directory in ["staging", "quarantine", "backups", "derived"] {
            fs::create_dir_all(staging.join(directory))?;
        }
        let imported = Self::open(&staging)?;
        let report = imported.verify_integrity()?;
        if !report.is_healthy() {
            return Err(CoreError::Conflict(
                "import verification failed; staged copy retained for diagnostics".into(),
            ));
        }
        fs::rename(&staging, &destination)?;
        Self::open(&destination)
    }

    #[doc(hidden)]
    pub fn debug_connection(&self) -> Result<Connection> {
        self.connection()
    }

    pub(crate) fn connection(&self) -> Result<Connection> {
        open_connection(&self.manifest.ledger_path(&self.root))
    }
}

fn open_connection(path: &Path) -> Result<Connection> {
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch(
        "PRAGMA foreign_keys=ON;
         PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA trusted_schema=OFF;",
    )?;
    Ok(connection)
}

fn apply_migrations(connection: &mut Connection) -> Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations(
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            checksum TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );",
    )?;
    let migrations = [
        (1_u32, "core", MIGRATION_1),
        (2_u32, "contract_alignment", MIGRATION_2),
        (3_u32, "research_core", MIGRATION_3),
        (4_u32, "development_core", MIGRATION_4),
        (5_u32, "code_intelligence", MIGRATION_5),
        (6_u32, "cp5_1_hardening", MIGRATION_6),
        (7_u32, "provenance_graph", MIGRATION_7),
        (8_u32, "semantic_intelligence", MIGRATION_8),
        (9_u32, "visual_intelligence", MIGRATION_9),
        (10_u32, "research_capture", MIGRATION_10),
        (11_u32, "checkpoint_context_engine", MIGRATION_11),
        (12_u32, "ai_continuity_interface", MIGRATION_12),
        (13_u32, "research_presentation_proposals", MIGRATION_13),
        (14_u32, "reviewed_report_sources", MIGRATION_14),
        (15_u32, "workspace_documents", MIGRATION_15),
    ];
    for (version, name, sql) in migrations {
        let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
        let existing: Option<String> = connection
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE version=?1",
                [version],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing != checksum {
                return Err(CoreError::MigrationChecksum { version });
            }
            continue;
        }
        apply_single_migration(connection, version, name, sql, &checksum)?;
    }
    let version: u32 = connection.query_row(
        "SELECT COALESCE(max(version),0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if version > CORE_SCHEMA_VERSION {
        return Err(CoreError::UnsupportedSchema {
            found: version,
            supported: CORE_SCHEMA_VERSION,
        });
    }
    Ok(())
}

fn current_schema_version(connection: &Connection) -> Result<u32> {
    let migrations_exist: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='schema_migrations')",
        [],
        |row| row.get(0),
    )?;
    if !migrations_exist {
        return Ok(0);
    }
    connection
        .query_row(
            "SELECT COALESCE(max(version),0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(Into::into)
}

fn apply_single_migration(
    connection: &mut Connection,
    version: u32,
    name: &str,
    sql: &str,
    checksum: &str,
) -> Result<()> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(sql)?;
    tx.execute(
        "INSERT INTO schema_migrations(version,name,checksum,applied_at) VALUES(?1,?2,?3,?4)",
        params![version, name, checksum, Utc::now().to_rfc3339()],
    )?;
    tx.commit()?;
    Ok(())
}

fn backup_connection(source: &Connection, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut target = Connection::open(destination)?;
    let backup = rusqlite::backup::Backup::new(source, &mut target)?;
    backup.run_to_completion(64, Duration::from_millis(5), None)?;
    drop(backup);
    let status: String = target.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
    if status != "ok" {
        return Err(CoreError::ArtifactIntegrity {
            path: destination.to_path_buf(),
            reason: format!("backup database integrity check returned {status}"),
        });
    }
    Ok(())
}

fn resolve_new_destination(destination: &Path) -> Result<PathBuf> {
    let name = destination
        .file_name()
        .ok_or_else(|| CoreError::Validation("destination must name a project directory".into()))?;
    let parent = destination
        .parent()
        .ok_or_else(|| CoreError::Validation("destination must have a parent directory".into()))?;
    fs::create_dir_all(parent)?;
    Ok(fs::canonicalize(parent)?.join(name))
}

fn open_regular_file_nofollow(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x00200000); // FILE_FLAG_OPEN_REPARSE_POINT
    }
    let file = options.open(path)?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if file.metadata()?.file_attributes() & 0x00000400 != 0 {
            return Err(CoreError::Validation("artifact source must not be a Windows reparse point".into()));
        }
    }
    if !file.metadata()?.is_file() {
        return Err(CoreError::Validation(
            "artifact source must be a regular file".into(),
        ));
    }
    Ok(file)
}

pub(crate) fn prior_result(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    operation: &str,
) -> Result<Option<Option<String>>> {
    let existing: Option<PriorCommandReceipt> = tx
        .query_row(
            "SELECT operation,result_id,project_id,actor_kind,actor_id,
                    command_type_version,payload_schema_version FROM commands
             WHERE id=?1 OR (project_id=?2 AND operation=?3 AND idempotency_key=?4)
             ORDER BY CASE WHEN id=?1 THEN 0 ELSE 1 END LIMIT 1",
            params![
                command.command_id,
                project_id,
                operation,
                command.idempotency_key
            ],
            |row| {
                Ok(PriorCommandReceipt {
                    operation: row.get(0)?,
                    result_id: row.get(1)?,
                    project_id: row.get(2)?,
                    actor_kind: row.get(3)?,
                    actor_id: row.get(4)?,
                    command_type_version: row.get(5)?,
                    payload_schema_version: row.get(6)?,
                })
            },
        )
        .optional()?;
    match existing {
        Some(existing) if existing.operation != operation => Err(CoreError::Conflict(format!(
            "command {} was already used for {}",
            command.command_id, existing.operation
        ))),
        Some(existing) if existing.project_id != project_id => Err(CoreError::Conflict(format!(
            "command {} belongs to a different project",
            command.command_id
        ))),
        Some(existing)
            if existing.actor_kind != command.actor.kind.as_str()
                || existing.actor_id != command.actor.id
                || existing.command_type_version != i64::from(command.command_type_version)
                || existing.payload_schema_version != i64::from(command.payload_schema_version) =>
        {
            Err(CoreError::Conflict(
                "idempotent retry changed the actor or command schema envelope".into(),
            ))
        }
        Some(existing) => Ok(Some(existing.result_id)),
        None => Ok(None),
    }
}

pub(crate) fn record_command_with_context(
    tx: &Transaction<'_>,
    command: &CommandContext,
    project_id: &str,
    operation: &str,
    result_id: Option<&str>,
    expected_version: Option<i64>,
    payload: &Value,
) -> Result<()> {
    validate_command_context(command)?;
    let payload_json = bounded_json(payload, MAX_ENTITY_JSON_BYTES, "command payload")?;
    tx.execute(
        "INSERT INTO commands(
            id,project_id,operation,result_id,created_at,command_type_version,actor_kind,actor_id,
            expected_version,idempotency_key,payload_schema_version,payload_json,issued_at,
            correlation_id,causation_id
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
        params![
            command.command_id,
            project_id,
            operation,
            result_id,
            Utc::now().to_rfc3339(),
            command.command_type_version,
            command.actor.kind.as_str(),
            command.actor.id,
            expected_version,
            command.idempotency_key,
            command.payload_schema_version,
            payload_json,
            command.issued_at,
            command.correlation_id,
            command.causation_id
        ],
    )?;
    Ok(())
}

pub(crate) fn append_event_with_context(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    aggregate_id: Option<&str>,
    event_type: &str,
    payload: &Value,
) -> Result<i64> {
    validate_command_context(command)?;
    let payload_json = bounded_json(payload, MAX_ENTITY_JSON_BYTES, "event payload")?;
    let current: i64 = tx.query_row(
        "SELECT ledger_sequence FROM projects WHERE id=?1",
        [project_id],
        |row| row.get(0),
    )?;
    let sequence = current + 1;
    let now = Utc::now().to_rfc3339();
    let event_id = new_id();
    tx.execute(
        "UPDATE projects SET ledger_sequence=?2,updated_at=?3 WHERE id=?1",
        params![project_id, sequence, now],
    )?;
    tx.execute(
        "INSERT INTO audit_events(
            id,project_id,ledger_sequence,command_id,event_type,payload_json,occurred_at,
            aggregate_id,event_version,actor_kind,actor_id,correlation_id,causation_id
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,1,?9,?10,?11,?12)",
        params![
            event_id,
            project_id,
            sequence,
            command.command_id,
            event_type,
            payload_json,
            now,
            aggregate_id,
            command.actor.kind.as_str(),
            command.actor.id,
            command.correlation_id,
            command.causation_id
        ],
    )?;
    tx.execute(
        "INSERT INTO outbox(id,project_id,audit_event_id,topic,payload_json,created_at)
         VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            new_id(),
            project_id,
            event_id,
            event_type,
            payload_json,
            now
        ],
    )?;
    Ok(sequence)
}

pub(crate) fn validate_command_context(command: &CommandContext) -> Result<()> {
    validate_id(&command.command_id, "command_id")?;
    validate_id(&command.correlation_id, "correlation_id")?;
    if let Some(causation_id) = &command.causation_id {
        validate_id(causation_id, "causation_id")?;
    }
    validate_nonempty(&command.actor.id, 200, "actor id")?;
    validate_nonempty(&command.idempotency_key, 200, "idempotency key")?;
    if command.actor.kind == ActorKind::AiProposal {
        return Err(CoreError::Validation(
            "AI proposals cannot execute canonical Continuity Core commands".into(),
        ));
    }
    if command.command_type_version == 0 || command.payload_schema_version == 0 {
        return Err(CoreError::Validation(
            "command and payload schema versions must be positive".into(),
        ));
    }
    chrono::DateTime::parse_from_rfc3339(&command.issued_at)
        .map_err(|_| CoreError::Validation("issued_at must be RFC3339".into()))?;
    Ok(())
}

pub(crate) fn legacy_origin(origin: OriginKind) -> &'static str {
    match origin {
        OriginKind::User => "manual",
        OriginKind::Deterministic => "system",
        OriginKind::Import => "imported",
        OriginKind::External => "external",
        OriginKind::Legacy => "legacy",
        OriginKind::AiProposal | OriginKind::Unknown => "unknown",
    }
}

fn entity_type_in_project(
    tx: &Transaction<'_>,
    project_id: &str,
    entity_id: &str,
) -> Result<String> {
    tx.query_row(
        "SELECT entity_type FROM entities WHERE id=?1 AND project_id=?2",
        params![entity_id, project_id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(|| {
        CoreError::Validation(format!(
            "entity {entity_id} is absent or belongs to another project"
        ))
    })
}

fn more_restrictive_classification<'a>(existing: &'a str, requested: &'a str) -> &'a str {
    fn rank(value: &str) -> u8 {
        match value {
            "public" => 0,
            "internal" => 1,
            "confidential" => 2,
            "secret" => 3,
            "never_send" => 4,
            _ => 4,
        }
    }
    if rank(existing) >= rank(requested) {
        existing
    } else {
        requested
    }
}

fn read_outbox(connection: &Connection, project_id: &str, id: &str) -> Result<OutboxMessage> {
    connection
        .query_row(
            "SELECT id,project_id,audit_event_id,topic,payload_json,attempts
             FROM outbox WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| {
                Ok(OutboxMessage {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    audit_event_id: row.get(2)?,
                    topic: row.get(3)?,
                    payload_json: row.get(4)?,
                    attempts: row.get(5)?,
                })
            },
        )
        .map_err(Into::into)
}

pub(crate) fn bounded_json(value: &Value, max_bytes: usize, label: &str) -> Result<String> {
    let encoded = serde_json::to_string(value)?;
    if encoded.len() > max_bytes {
        return Err(CoreError::Validation(format!(
            "{label} exceeds {max_bytes} bytes"
        )));
    }
    Ok(encoded)
}

pub(crate) fn validate_id(value: &str, label: &str) -> Result<()> {
    let parsed = uuid::Uuid::parse_str(value)
        .map_err(|_| CoreError::Validation(format!("{label} must be a UUID")))?;
    if parsed.get_version_num() != 7 {
        return Err(CoreError::Validation(format!("{label} must be UUIDv7")));
    }
    Ok(())
}

pub(crate) fn validate_nonempty(value: &str, max_chars: usize, label: &str) -> Result<()> {
    let length = value.trim().chars().count();
    if length == 0 || length > max_chars {
        return Err(CoreError::Validation(format!(
            "{label} must contain 1..={max_chars} characters"
        )));
    }
    Ok(())
}

fn truncate(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn path_to_slashes(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn safe_relative_path(path: &str) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn walk_files(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                return Err(CoreError::Validation(format!(
                    "symlink is not allowed in project payload tree: {}",
                    entry.path().display()
                )));
            }
            if kind.is_dir() {
                pending.push(entry.path());
            } else if kind.is_file() {
                files.push(entry.path());
            }
        }
    }
    Ok(files)
}

fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    if !source.exists() {
        fs::create_dir_all(destination)?;
        return Ok(());
    }
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(CoreError::Validation(format!(
                "symlink is not allowed during export/import: {}",
                entry.path().display()
            )));
        }
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentation_migration_preserves_old_proposals_and_immutability() {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch("PRAGMA foreign_keys=OFF; CREATE TABLE projects(id TEXT PRIMARY KEY); CREATE TABLE mcp_client_grants(id TEXT PRIMARY KEY); CREATE TABLE mcp_sessions(id TEXT PRIMARY KEY);").unwrap();
        let legacy = MIGRATION_12.split_once("CREATE TABLE external_proposals").unwrap().1.split("CREATE TRIGGER mcp_grant_scope_immutable").next().unwrap();
        connection.execute_batch(&format!("CREATE TABLE external_proposals{legacy}")).unwrap();
        let insert = "INSERT INTO external_proposals(id,project_id,grant_id,idempotency_key,proposal_kind,scope,title,rationale,payload_json,source_refs_json,payload_fingerprint,status,created_at,expires_at) VALUES(?1,'p','g',?1,?2,'research','Preserved','Reason','{}','[]',?3,'pending','2026-01-01','2027-01-01')";
        connection.execute(insert, params!["old", "research_note", "a".repeat(64)]).unwrap();
        connection.execute_batch("BEGIN IMMEDIATE").unwrap();
        connection.execute_batch(MIGRATION_13).unwrap();
        connection.execute_batch("COMMIT").unwrap();
        assert_eq!(connection.query_row("SELECT title FROM external_proposals WHERE id='old'", [], |r| r.get::<_,String>(0)).unwrap(), "Preserved");
        for kind in ["research_synthesis", "diagram_plan"] { connection.execute(insert, params![kind, kind, "b".repeat(64)]).unwrap(); }
        assert!(connection.execute("UPDATE external_proposals SET payload_json='[]' WHERE id='old'", []).is_err());
        assert!(connection.execute(insert, params!["invalid", "unrecognized", "c".repeat(64)]).is_err());
    }

    #[test]
    fn failed_migration_rolls_back_every_statement() {
        let directory = tempfile::tempdir().unwrap();
        let mut connection = Connection::open(directory.path().join("migration.sqlite3")).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations(
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    checksum TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                 );
                 CREATE TABLE stable(value TEXT NOT NULL);
                 INSERT INTO stable(value) VALUES('before');",
            )
            .unwrap();

        let result = apply_single_migration(
            &mut connection,
            2,
            "deliberate_failure",
            "INSERT INTO stable(value) VALUES('must_rollback'); THIS IS INVALID SQL;",
            "test-checksum",
        );
        assert!(result.is_err());
        let stable_rows: i64 = connection
            .query_row("SELECT count(*) FROM stable", [], |row| row.get(0))
            .unwrap();
        let migration_rows: i64 = connection
            .query_row("SELECT count(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stable_rows, 1);
        assert_eq!(migration_rows, 0);
    }

    #[test]
    fn v1_project_is_backed_up_and_migrated_losslessly_to_current_schema() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("legacy-project");
        fs::create_dir_all(&root).unwrap();
        let project_id = new_id();
        let entity_id = new_id();
        let target_id = new_id();
        let relationship_id = new_id();
        let manifest = ProjectManifest::new(project_id.clone(), "Legacy fixture".into());
        manifest.write_atomic(&root).unwrap();
        let mut connection = open_connection(&manifest.ledger_path(&root)).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations(
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    checksum TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap();
        let checksum = hex::encode(Sha256::digest(MIGRATION_1.as_bytes()));
        apply_single_migration(&mut connection, 1, "core", MIGRATION_1, &checksum).unwrap();
        let now = Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT INTO projects(id,name,status,lifecycle_version,ledger_sequence,created_at,updated_at)
                 VALUES(?1,'Legacy fixture','active',1,0,?2,?2)",
                params![project_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO entities(id,project_id,entity_type,title,status,version,origin,data_json,created_at,updated_at)
                 VALUES(?1,?2,'legacy.note','Legacy entity','active',1,'manual','{}',?3,?3)",
                params![entity_id, project_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO entities(id,project_id,entity_type,title,status,version,origin,data_json,created_at,updated_at)
                 VALUES(?1,?2,'legacy.note','Legacy target','active',1,'imported','{}',?3,?3)",
                params![target_id, project_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO relationships(id,project_id,relation_type,source_entity_id,target_entity_id,origin,created_at)
                 VALUES(?1,?2,'supports',?3,?4,'manual',?5)",
                params![relationship_id, project_id, entity_id, target_id, now],
            )
            .unwrap();
        drop(connection);

        let store = ContinuityStore::open(&root).unwrap();
        let migrated = store.get_entity(&entity_id).unwrap();
        assert_eq!(migrated.origin, "user");
        assert_eq!(migrated.created_by, "continuum-core");
        let migrated_relationship = store.get_relationship(&relationship_id).unwrap();
        assert_eq!(migrated_relationship.origin, "user");
        assert_eq!(migrated_relationship.source_entity_type, "legacy.note");
        assert_eq!(migrated_relationship.target_entity_type, "legacy.note");
        let version: u32 = store
            .debug_connection()
            .unwrap()
            .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, CORE_SCHEMA_VERSION);
        let backups = fs::read_dir(root.join("backups")).unwrap().count();
        assert_eq!(backups, 1);
    }

    #[test]
    fn v2_project_is_backed_up_and_gains_empty_research_schema_without_core_mutation() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("cp2-project");
        fs::create_dir_all(&root).unwrap();
        let project_id = new_id();
        let entity_id = new_id();
        let manifest = ProjectManifest::new(project_id.clone(), "CP2 fixture".into());
        manifest.write_atomic(&root).unwrap();
        let mut connection = open_connection(&manifest.ledger_path(&root)).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations(
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    checksum TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap();
        for (version, name, sql) in [
            (1_u32, "core", MIGRATION_1),
            (2_u32, "contract_alignment", MIGRATION_2),
        ] {
            let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
            apply_single_migration(&mut connection, version, name, sql, &checksum).unwrap();
        }
        let now = Utc::now().to_rfc3339();
        connection.execute("INSERT INTO projects(id,name,status,lifecycle_version,ledger_sequence,created_at,updated_at)
            VALUES(?1,'CP2 fixture','active',1,0,?2,?2)",params![project_id,now]).unwrap();
        connection
            .execute(
                "INSERT INTO space_capabilities(project_id,space,enabled,updated_at)
            VALUES(?1,'research',0,?2),(?1,'development',0,?2)",
                params![project_id, now],
            )
            .unwrap();
        connection.execute("INSERT INTO entities(id,project_id,entity_type,title,status,version,legacy_origin,data_json,
            created_at,updated_at,entity_schema_version,origin_type,metadata_json,created_by,updated_by)
            VALUES(?1,?2,'core.fixture','Preserved core state','active',1,'manual','{}',?3,?3,1,'user','{}','owner','owner')",
            params![entity_id,project_id,now]).unwrap();
        drop(connection);

        let store = ContinuityStore::open(&root).unwrap();
        assert_eq!(
            store.get_entity(&entity_id).unwrap().title,
            "Preserved core state"
        );
        let connection = store.debug_connection().unwrap();
        let version: u32 = connection
            .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, CORE_SCHEMA_VERSION);
        let research_rows: i64 = connection
            .query_row("SELECT count(*) FROM research_questions", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(research_rows, 0);
        assert_eq!(fs::read_dir(root.join("backups")).unwrap().count(), 1);
    }

    #[test]
    fn v3_project_is_backed_up_and_gains_development_schema_without_rewriting_research() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("cp3-project");
        fs::create_dir_all(&root).unwrap();
        let project_id = new_id();
        let requirement_id = new_id();
        let manifest = ProjectManifest::new(project_id.clone(), "CP3 fixture".into());
        manifest.write_atomic(&root).unwrap();
        let mut connection = open_connection(&manifest.ledger_path(&root)).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations(
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    checksum TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap();
        for (version, name, sql) in [
            (1_u32, "core", MIGRATION_1),
            (2_u32, "contract_alignment", MIGRATION_2),
            (3_u32, "research_core", MIGRATION_3),
        ] {
            let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
            apply_single_migration(&mut connection, version, name, sql, &checksum).unwrap();
        }
        let now = Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT INTO projects(id,name,status,lifecycle_version,ledger_sequence,created_at,updated_at)
                 VALUES(?1,'CP3 fixture','active',1,0,?2,?2)",
                params![project_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO space_capabilities(project_id,space,enabled,updated_at)
            VALUES(?1,'research',1,?2),(?1,'development',0,?2)",
                params![project_id, now],
            )
            .unwrap();
        connection.execute("INSERT INTO entities(id,project_id,entity_type,title,status,version,legacy_origin,data_json,
            created_at,updated_at,entity_schema_version,origin_type,metadata_json,created_by,updated_by)
            VALUES(?1,?2,'requirement','Preserved requirement','draft',1,'manual','{}',?3,?3,1,'user','{}','owner','owner')",
            params![requirement_id,project_id,now]).unwrap();
        connection.execute("INSERT INTO requirements(entity_id,statement,acceptance_criteria_json,priority,rationale_origin,verification_method)
            VALUES(?1,'Preserve CP3 identity','[]',2,'user','inspect')",[&requirement_id]).unwrap();
        connection.execute("INSERT INTO research_search_documents(entity_id,project_id,entity_type,title,body,updated_at)
            VALUES(?1,?2,'requirement','Preserved requirement','Preserve CP3 identity',?3)",
            params![requirement_id,project_id,now]).unwrap();
        drop(connection);

        let store = ContinuityStore::open(&root).unwrap();
        assert_eq!(
            store.get_entity(&requirement_id).unwrap().title,
            "Preserved requirement"
        );
        let connection = store.debug_connection().unwrap();
        let version: u32 = connection
            .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, CORE_SCHEMA_VERSION);
        let created_in_space: String = connection
            .query_row(
                "SELECT created_in_space FROM requirements WHERE entity_id=?1",
                [&requirement_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(created_in_space, "research");
        let repository_rows: i64 = connection
            .query_row("SELECT count(*) FROM repositories", [], |row| row.get(0))
            .unwrap();
        assert_eq!(repository_rows, 0);
        assert_eq!(fs::read_dir(root.join("backups")).unwrap().count(), 1);
    }

    #[test]
    fn v4_project_is_backed_up_and_gains_empty_code_intelligence_without_rewriting_git_state() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("cp4-project");
        fs::create_dir_all(&root).unwrap();
        let project_id = new_id();
        let repository_id = new_id();
        let baseline_id = new_id();
        let manifest = ProjectManifest::new(project_id.clone(), "CP4 fixture".into());
        manifest.write_atomic(&root).unwrap();
        let mut connection = open_connection(&manifest.ledger_path(&root)).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations(
                    version INTEGER PRIMARY KEY,
                    name TEXT NOT NULL,
                    checksum TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap();
        for (version, name, sql) in [
            (1_u32, "core", MIGRATION_1),
            (2_u32, "contract_alignment", MIGRATION_2),
            (3_u32, "research_core", MIGRATION_3),
            (4_u32, "development_core", MIGRATION_4),
        ] {
            let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
            apply_single_migration(&mut connection, version, name, sql, &checksum).unwrap();
        }
        let now = Utc::now().to_rfc3339();
        connection
            .execute(
                "INSERT INTO projects(id,name,status,lifecycle_version,ledger_sequence,created_at,updated_at)
                 VALUES(?1,'CP4 fixture','active',1,0,?2,?2)",
                params![project_id, now],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO space_capabilities(project_id,space,enabled,updated_at)
                 VALUES(?1,'research',0,?2),(?1,'development',1,?2)",
                params![project_id, now],
            )
            .unwrap();
        for (id, entity_type, title) in [
            (&repository_id, "repository", "Preserved repository"),
            (&baseline_id, "repository_baseline", "Preserved baseline"),
        ] {
            connection.execute("INSERT INTO entities(id,project_id,entity_type,title,status,version,legacy_origin,data_json,
                created_at,updated_at,entity_schema_version,origin_type,metadata_json,created_by,updated_by)
                VALUES(?1,?2,?3,?4,'active',1,'system','{}',?5,?5,1,'deterministic','{}','continuum-core','continuum-core')",
                params![id,project_id,entity_type,title,now]).unwrap();
        }
        let fingerprint = "a".repeat(64);
        connection
            .execute(
                "INSERT INTO repositories(entity_id,project_id,root_path,root_fingerprint,
            git_common_dir_fingerprint,object_format,adapter_version,attached_at,last_observed_at)
            VALUES(?1,?2,'/fixture/repository',?3,?3,'sha1','continuum-git-v1',?4,?4)",
                params![repository_id, project_id, fingerprint, now],
            )
            .unwrap();
        connection.execute("INSERT INTO repository_baselines(entity_id,project_id,repository_id,head_oid,
            head_ref,branch_name,worktree_fingerprint,worktree_status_json,observed_at,previous_baseline_id,
            relation_to_previous,adapter_version)
            VALUES(?1,?2,?3,?4,'refs/heads/main','main',?5,'[]',?6,NULL,'initial','continuum-git-v1')",
            params![baseline_id,project_id,repository_id,"b".repeat(40),fingerprint,now]).unwrap();
        drop(connection);

        let store = ContinuityStore::open(&root).unwrap();
        assert_eq!(
            store.get_entity(&repository_id).unwrap().title,
            "Preserved repository"
        );
        assert_eq!(
            store.get_entity(&baseline_id).unwrap().title,
            "Preserved baseline"
        );
        let connection = store.debug_connection().unwrap();
        let version: u32 = connection
            .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, CORE_SCHEMA_VERSION);
        let analysis_rows: i64 = connection
            .query_row("SELECT count(*) FROM analysis_runs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(analysis_rows, 0);
        let preserved_baseline: i64 = connection
            .query_row(
                "SELECT count(*) FROM repository_baselines WHERE entity_id=?1 AND repository_id=?2",
                params![baseline_id, repository_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(preserved_baseline, 1);
        assert_eq!(fs::read_dir(root.join("backups")).unwrap().count(), 1);
    }

    #[test]
    fn v5_project_migrates_alias_and_cache_state_losslessly_to_cp5_1() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("cp5-project");
        fs::create_dir_all(&root).unwrap();
        let project_id = new_id();
        let repository_id = new_id();
        let baseline_id = new_id();
        let file_id = new_id();
        let manifest = ProjectManifest::new(project_id.clone(), "CP5 fixture".into());
        manifest.write_atomic(&root).unwrap();
        let mut connection = open_connection(&manifest.ledger_path(&root)).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations(
                    version INTEGER PRIMARY KEY,name TEXT NOT NULL,checksum TEXT NOT NULL,
                    applied_at TEXT NOT NULL
                );",
            )
            .unwrap();
        for (version, name, sql) in [
            (1_u32, "core", MIGRATION_1),
            (2_u32, "contract_alignment", MIGRATION_2),
            (3_u32, "research_core", MIGRATION_3),
            (4_u32, "development_core", MIGRATION_4),
            (5_u32, "code_intelligence", MIGRATION_5),
        ] {
            let checksum = hex::encode(Sha256::digest(sql.as_bytes()));
            apply_single_migration(&mut connection, version, name, sql, &checksum).unwrap();
        }
        let now = Utc::now().to_rfc3339();
        connection.execute("INSERT INTO projects(id,name,status,lifecycle_version,ledger_sequence,created_at,updated_at)
            VALUES(?1,'CP5 fixture','active',1,0,?2,?2)",params![project_id,now]).unwrap();
        connection
            .execute(
                "INSERT INTO space_capabilities(project_id,space,enabled,updated_at)
            VALUES(?1,'research',0,?2),(?1,'development',1,?2)",
                params![project_id, now],
            )
            .unwrap();
        for (id, entity_type, title) in [
            (&repository_id, "repository", "Repository"),
            (&baseline_id, "repository_baseline", "Baseline"),
            (&file_id, "code_entity", "src/lib.rs"),
        ] {
            connection.execute("INSERT INTO entities(id,project_id,entity_type,title,status,version,legacy_origin,data_json,
                created_at,updated_at,entity_schema_version,origin_type,metadata_json,created_by,updated_by)
                VALUES(?1,?2,?3,?4,'active',1,'system','{}',?5,?5,1,'deterministic','{}','continuum-core','continuum-core')",
                params![id,project_id,entity_type,title,now]).unwrap();
        }
        let fingerprint = "a".repeat(64);
        connection
            .execute(
                "INSERT INTO repositories(entity_id,project_id,root_path,root_fingerprint,
            git_common_dir_fingerprint,object_format,adapter_version,attached_at,last_observed_at)
            VALUES(?1,?2,'/fixture/repository',?3,?3,'sha1','continuum-git-v1',?4,?4)",
                params![repository_id, project_id, fingerprint, now],
            )
            .unwrap();
        connection.execute("INSERT INTO repository_baselines(entity_id,project_id,repository_id,head_oid,
            head_ref,branch_name,worktree_fingerprint,worktree_status_json,observed_at,previous_baseline_id,
            relation_to_previous,adapter_version)
            VALUES(?1,?2,?3,?4,'refs/heads/main','main',?5,'[]',?6,NULL,'initial','continuum-git-v1')",
            params![baseline_id,project_id,repository_id,"b".repeat(40),fingerprint,now]).unwrap();
        connection.execute("INSERT INTO code_entities(entity_id,project_id,repository_id,entity_kind,stable_key,
            language,first_seen_baseline_id) VALUES(?1,?2,?3,'file','file:src/lib.rs','rust',?4)",
            params![file_id,project_id,repository_id,baseline_id]).unwrap();
        connection.execute("INSERT INTO code_entity_aliases(repository_id,alias_kind,alias_value,code_entity_id,
            first_seen_baseline_id) VALUES(?1,'path','src/lib.rs',?2,?3)",
            params![repository_id,file_id,baseline_id]).unwrap();
        connection.execute("INSERT INTO analyzer_cache(content_sha256,language,analyzer_id,analyzer_version,
            output_schema_version,output_json,created_at) VALUES(?1,'rust','code-intelligence-file','v1',1,'{}',?2)",
            params!["c".repeat(64),now]).unwrap();
        drop(connection);

        let store = ContinuityStore::open(&root).unwrap();
        let connection = store.debug_connection().unwrap();
        let alias: (String, String, Option<String>) = connection
            .query_row(
                "SELECT first_seen_baseline_id,last_seen_baseline_id,retired_at_baseline_id
             FROM code_entity_aliases WHERE code_entity_id=?1",
                [&file_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(alias, (baseline_id.clone(), baseline_id, None));
        let cache: (i64, String) = connection
            .query_row(
                "SELECT byte_size,last_accessed_at FROM analyzer_cache",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(cache.0, 2);
        assert_eq!(cache.1, now);
        let version: u32 = connection
            .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version, CORE_SCHEMA_VERSION);
        assert_eq!(fs::read_dir(root.join("backups")).unwrap().count(), 1);
    }
}
