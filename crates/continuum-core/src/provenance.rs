use std::collections::{HashMap, HashSet, VecDeque};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::store::{
    append_event_with_context, bounded_json, legacy_origin, prior_result,
    record_command_with_context, validate_command_context, validate_nonempty,
};
use crate::{
    CommandContext, ContinuityStore, CoreError, Entity, IntegrityIssue, IntegrityReport,
    OriginKind, PageRequest, Relationship, Result, new_id,
};

const GRAPH_SCHEMA_VERSION: u32 = 1;
const MAX_GRAPH_DEPTH: u8 = 8;
const MAX_GRAPH_NODES: usize = 500;
const MAX_GRAPH_EDGES: usize = 1_500;
const MAX_GRAPH_ROOTS: usize = 50;
const MAX_FEEDBACK_LINKS: usize = 100;
const MAX_PROVENANCE_JSON_BYTES: usize = 1024 * 1024;
const MAX_PROVENANCE_TEXT: usize = 30_000;

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

string_enum!(GraphDirection {
    Inbound => "inbound",
    Outbound => "outbound",
    Both => "both",
});

string_enum!(FeedbackKind {
    ValidationFailure => "validation_failure",
    ValidationSuccess => "validation_success",
    Observation => "observation",
    Contradiction => "contradiction",
    RevisionRequest => "revision_request",
    Other => "other",
});

string_enum!(FeedbackSeverity {
    Info => "info",
    Warning => "warning",
    Error => "error",
});

string_enum!(FeedbackRelation {
    Supports => "supports",
    Challenges => "challenges",
    RequestsRevisionOf => "requests_revision_of",
});

string_enum!(RelationshipAction {
    Accept => "accepted",
    Reject => "rejected",
    Annotate => "annotated",
    Retire => "retired",
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedbackTarget {
    pub entity_id: String,
    pub relation: FeedbackRelation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewLearningFeedback {
    pub title: String,
    pub kind: FeedbackKind,
    pub summary: String,
    #[serde(default)]
    pub details: Value,
    pub severity: FeedbackSeverity,
    pub origin: OriginKind,
    #[serde(default)]
    pub source_entity_ids: Vec<String>,
    #[serde(default)]
    pub targets: Vec<FeedbackTarget>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LearningFeedbackRecord {
    pub entity: Entity,
    pub kind: String,
    pub summary: String,
    pub details: Value,
    pub severity: String,
    pub resolution_note: String,
    pub created_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LearningFeedbackTransition {
    pub target_status: String,
    pub note: String,
    pub expected_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationshipStateChange {
    pub action: RelationshipAction,
    pub annotation: String,
    pub expected_state_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationshipHistoryEntry {
    pub id: String,
    pub relationship_id: String,
    pub ledger_sequence: i64,
    pub action: String,
    pub state_version: i64,
    pub from_status: Option<String>,
    pub to_status: String,
    pub from_review_state: Option<String>,
    pub to_review_state: String,
    pub annotation: String,
    pub actor_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RelationshipHistoryPage {
    pub items: Vec<RelationshipHistoryEntry>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphQuery {
    #[serde(default)]
    pub root_entity_ids: Vec<String>,
    pub checkpoint_id: Option<String>,
    pub direction: GraphDirection,
    pub max_depth: u8,
    pub max_nodes: usize,
    pub max_edges: usize,
    #[serde(default)]
    pub entity_types: Vec<String>,
    #[serde(default)]
    pub entity_statuses: Vec<String>,
    #[serde(default)]
    pub relationship_types: Vec<String>,
    #[serde(default)]
    pub relationship_statuses: Vec<String>,
    #[serde(default)]
    pub review_states: Vec<String>,
    #[serde(default)]
    pub origins: Vec<String>,
    pub minimum_confidence: Option<f64>,
    pub created_from: Option<String>,
    pub created_through: Option<String>,
}

impl Default for GraphQuery {
    fn default() -> Self {
        Self {
            root_entity_ids: Vec::new(),
            checkpoint_id: None,
            direction: GraphDirection::Both,
            max_depth: 4,
            max_nodes: 200,
            max_edges: 500,
            entity_types: Vec::new(),
            entity_statuses: Vec::new(),
            relationship_types: Vec::new(),
            relationship_statuses: vec!["active".into()],
            review_states: Vec::new(),
            origins: Vec::new(),
            minimum_confidence: None,
            created_from: None,
            created_through: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphNode {
    pub entity: Entity,
    pub depth: u8,
    pub is_root: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProvenanceGraph {
    pub schema_version: u32,
    pub project_id: String,
    pub root_entity_ids: Vec<String>,
    pub checkpoint_id: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<Relationship>,
    pub truncated: bool,
    pub frontier_entity_ids: Vec<String>,
    pub omissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceIssue {
    pub code: String,
    pub severity: String,
    pub entity_ids: Vec<String>,
    pub relationship_ids: Vec<String>,
    pub message: String,
    pub guidance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProvenanceValidationReport {
    pub checked_entities: usize,
    pub checked_relationships: usize,
    pub issues: Vec<ProvenanceIssue>,
}

impl ProvenanceValidationReport {
    pub fn is_healthy(&self) -> bool {
        !self.issues.iter().any(|issue| issue.severity == "error")
    }
}

pub(crate) fn is_provenance_entity_type(value: &str) -> bool {
    value == "learning_feedback"
}

pub(crate) fn validate_provenance_relationship_pair(
    source: &str,
    relation: &str,
    target: &str,
) -> Result<()> {
    if source != "learning_feedback" && target != "learning_feedback" {
        return Ok(());
    }
    let valid = match relation {
        "produces" => {
            matches!(source, "test_run" | "result" | "evidence" | "change_set")
                && target == "learning_feedback"
        }
        "supports" | "challenges" => {
            source == "learning_feedback"
                && matches!(target, "evidence" | "finding" | "decision" | "requirement")
        }
        "requests_revision_of" => {
            source == "learning_feedback"
                && matches!(target, "finding" | "decision" | "requirement")
        }
        _ => false,
    };
    if valid {
        Ok(())
    } else {
        Err(CoreError::Validation(format!(
            "relationship {source} --{relation}--> {target} is not permitted by the CP6 bridge"
        )))
    }
}

pub(crate) fn relationship_would_cycle(
    connection: &Connection,
    project_id: &str,
    source_id: &str,
    target_id: &str,
    relation_type: &str,
) -> Result<bool> {
    if source_id == target_id {
        return Ok(true);
    }
    let mut pending = vec![target_id.to_owned()];
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }
        if current == source_id {
            return Ok(true);
        }
        if visited.len() > 100_000 {
            return Err(CoreError::Validation(
                "cycle check exceeded the bounded graph safety limit".into(),
            ));
        }
        let mut statement = connection.prepare(
            "SELECT target_entity_id FROM relationships
             WHERE project_id=?1 AND source_entity_id=?2 AND relation_type=?3
               AND status='active'",
        )?;
        let targets = statement
            .query_map(params![project_id, current, relation_type], |row| {
                row.get(0)
            })?
            .collect::<std::result::Result<Vec<String>, _>>()?;
        pending.extend(targets);
    }
    Ok(false)
}

impl ContinuityStore {
    pub fn create_learning_feedback(
        &self,
        command: &CommandContext,
        input: NewLearningFeedback,
    ) -> Result<LearningFeedbackRecord> {
        validate_command_context(command)?;
        validate_nonempty(&input.title, 500, "Learning Feedback title")?;
        validate_nonempty(
            &input.summary,
            MAX_PROVENANCE_TEXT,
            "Learning Feedback summary",
        )?;
        if input.origin == OriginKind::AiProposal {
            return Err(CoreError::Validation(
                "AI-proposed feedback belongs in CP7 candidate storage until accepted".into(),
            ));
        }
        if input.source_entity_ids.is_empty() {
            return Err(CoreError::Validation(
                "Learning Feedback must cite at least one validation source".into(),
            ));
        }
        if input.source_entity_ids.len() > MAX_FEEDBACK_LINKS
            || input.targets.len() > MAX_FEEDBACK_LINKS
        {
            return Err(CoreError::Validation(
                "Learning Feedback source and target lists are limited to 100 each".into(),
            ));
        }
        ensure_unique_ids(&input.source_entity_ids, "Learning Feedback source")?;
        let target_keys = input
            .targets
            .iter()
            .map(|target| format!("{}:{}", target.relation.as_str(), target.entity_id))
            .collect::<Vec<_>>();
        ensure_unique_ids(&target_keys, "Learning Feedback target")?;
        let details_json = bounded_json(
            &input.details,
            MAX_PROVENANCE_JSON_BYTES,
            "Learning Feedback details",
        )?;
        let metadata_json = bounded_json(
            &input.metadata,
            MAX_PROVENANCE_JSON_BYTES,
            "Learning Feedback metadata",
        )?;

        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_active_project(&tx, &self.manifest().project_id)?;
        if let Some(result_id) = prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "CreateLearningFeedback",
        )? {
            tx.commit()?;
            return self.get_learning_feedback(
                &result_id.ok_or_else(|| CoreError::Conflict("command has no result ID".into()))?,
            );
        }

        let id = new_id();
        let now = Utc::now().to_rfc3339();
        for source_id in &input.source_entity_ids {
            let source_type = entity_type(&tx, &self.manifest().project_id, source_id)?;
            validate_provenance_relationship_pair(&source_type, "produces", "learning_feedback")?;
        }
        for target in &input.targets {
            let target_type = entity_type(&tx, &self.manifest().project_id, &target.entity_id)?;
            validate_provenance_relationship_pair(
                "learning_feedback",
                target.relation.as_str(),
                &target_type,
            )?;
        }

        tx.execute(
            "INSERT INTO entities(
                id,project_id,entity_type,title,status,version,legacy_origin,data_json,
                created_at,updated_at,entity_schema_version,origin_type,metadata_json,
                created_by,updated_by
             ) VALUES(?1,?2,'learning_feedback',?3,'open',1,?4,'{}',?5,?5,1,?6,?7,?8,?8)",
            params![
                id,
                self.manifest().project_id,
                input.title,
                legacy_origin(input.origin),
                now,
                input.origin.as_str(),
                metadata_json,
                command.actor.id
            ],
        )?;
        tx.execute(
            "INSERT INTO learning_feedback(
                entity_id,feedback_kind,summary,details_json,severity,resolution_note,created_at
             ) VALUES(?1,?2,?3,?4,?5,'',?6)",
            params![
                id,
                input.kind.as_str(),
                input.summary,
                details_json,
                input.severity.as_str(),
                now
            ],
        )?;
        for source_id in &input.source_entity_ids {
            let source_type = entity_type(&tx, &self.manifest().project_id, source_id)?;
            insert_cp6_relationship(
                &tx,
                &self.manifest().project_id,
                command,
                source_id,
                &source_type,
                &id,
                "learning_feedback",
                "produces",
                input.origin,
                std::slice::from_ref(source_id),
            )?;
        }
        for target in &input.targets {
            let target_type = entity_type(&tx, &self.manifest().project_id, &target.entity_id)?;
            insert_cp6_relationship(
                &tx,
                &self.manifest().project_id,
                command,
                &id,
                "learning_feedback",
                &target.entity_id,
                &target_type,
                target.relation.as_str(),
                input.origin,
                &input.source_entity_ids,
            )?;
        }
        let payload = json!({
            "learning_feedback_id": id,
            "kind": input.kind.as_str(),
            "severity": input.severity.as_str(),
            "source_count": input.source_entity_ids.len(),
            "target_count": input.targets.len()
        });
        append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(&id),
            "provenance.learning_feedback.created",
            &payload,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "CreateLearningFeedback",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        self.get_learning_feedback(&id)
    }

    pub fn get_learning_feedback(&self, id: &str) -> Result<LearningFeedbackRecord> {
        let entity = self.get_entity(id)?;
        if entity.entity_type != "learning_feedback" {
            return Err(CoreError::Validation(format!(
                "entity {id} is not Learning Feedback"
            )));
        }
        let connection = self.connection()?;
        let raw = connection
            .query_row(
                "SELECT feedback_kind,summary,details_json,severity,resolution_note,created_at,resolved_at
                 FROM learning_feedback WHERE entity_id=?1",
                [id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        Ok(LearningFeedbackRecord {
            entity,
            kind: raw.0,
            summary: raw.1,
            details: serde_json::from_str(&raw.2)?,
            severity: raw.3,
            resolution_note: raw.4,
            created_at: raw.5,
            resolved_at: raw.6,
        })
    }

    pub fn transition_learning_feedback(
        &self,
        command: &CommandContext,
        feedback_id: &str,
        input: LearningFeedbackTransition,
    ) -> Result<LearningFeedbackRecord> {
        validate_command_context(command)?;
        validate_text(
            &input.note,
            MAX_PROVENANCE_TEXT,
            "Learning Feedback transition note",
        )?;
        if !matches!(input.target_status.as_str(), "acknowledged" | "resolved") {
            return Err(CoreError::Validation(
                "Learning Feedback target status must be acknowledged or resolved".into(),
            ));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_active_project(&tx, &self.manifest().project_id)?;
        if prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "TransitionLearningFeedback",
        )?
        .is_some()
        {
            tx.commit()?;
            return self.get_learning_feedback(feedback_id);
        }
        let current: String = tx
            .query_row(
                "SELECT status FROM entities WHERE id=?1 AND project_id=?2 AND entity_type='learning_feedback'",
                params![feedback_id, self.manifest().project_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(feedback_id.into()))?;
        let allowed = matches!(
            (current.as_str(), input.target_status.as_str()),
            ("open", "acknowledged") | ("open", "resolved") | ("acknowledged", "resolved")
        );
        if !allowed {
            return Err(CoreError::Validation(format!(
                "Learning Feedback cannot transition from {current} to {}",
                input.target_status
            )));
        }
        let now = Utc::now().to_rfc3339();
        let changed = tx.execute(
            "UPDATE entities SET status=?3,version=version+1,updated_at=?4,updated_by=?5
             WHERE id=?1 AND project_id=?2 AND version=?6",
            params![
                feedback_id,
                self.manifest().project_id,
                input.target_status,
                now,
                command.actor.id,
                input.expected_version
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(
                "Learning Feedback version is stale".into(),
            ));
        }
        tx.execute(
            "UPDATE learning_feedback SET resolution_note=?2,
                resolved_at=CASE WHEN ?3='resolved' THEN ?4 ELSE resolved_at END
             WHERE entity_id=?1",
            params![feedback_id, input.note, input.target_status, now],
        )?;
        let payload = json!({"learning_feedback_id":feedback_id,"from":current,
            "to":input.target_status,"note":input.note});
        append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(feedback_id),
            "provenance.learning_feedback.status_changed",
            &payload,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "TransitionLearningFeedback",
            Some(feedback_id),
            Some(input.expected_version),
            &payload,
        )?;
        tx.commit()?;
        self.get_learning_feedback(feedback_id)
    }

    pub fn change_relationship_state(
        &self,
        command: &CommandContext,
        relationship_id: &str,
        input: RelationshipStateChange,
    ) -> Result<Relationship> {
        validate_command_context(command)?;
        validate_text(&input.annotation, 10_000, "relationship annotation")?;
        if input.action == RelationshipAction::Annotate && input.annotation.trim().is_empty() {
            return Err(CoreError::Validation(
                "annotation action requires non-empty text".into(),
            ));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_active_project(&tx, &self.manifest().project_id)?;
        if prior_result(
            &tx,
            &self.manifest().project_id,
            command,
            "ChangeRelationshipState",
        )?
        .is_some()
        {
            tx.commit()?;
            return self.get_relationship(relationship_id);
        }
        let current = read_relationship(&tx, &self.manifest().project_id, relationship_id)?;
        if current.origin == OriginKind::Deterministic.as_str()
            && matches!(
                input.action,
                RelationshipAction::Reject | RelationshipAction::Retire
            )
        {
            return Err(CoreError::Validation(
                "deterministic observations may be accepted or annotated but not rejected or retired"
                    .into(),
            ));
        }
        if matches!(current.status.as_str(), "archived" | "superseded") {
            return Err(CoreError::Validation(
                "retired or superseded relationship state is terminal".into(),
            ));
        }
        let (next_status, next_review) = match input.action {
            RelationshipAction::Accept => ("active", "accepted"),
            RelationshipAction::Reject => ("rejected", "rejected"),
            RelationshipAction::Annotate => {
                (current.status.as_str(), current.review_state.as_str())
            }
            RelationshipAction::Retire => ("archived", current.review_state.as_str()),
        };
        let now = Utc::now().to_rfc3339();
        let annotation = if input.annotation.trim().is_empty() {
            current.annotation.clone()
        } else {
            input.annotation.trim().to_owned()
        };
        let changed = tx.execute(
            "UPDATE relationships SET status=?3,review_state=?4,state_version=state_version+1,
                annotation=?5,updated_at=?6,reviewed_at=CASE WHEN ?7 IN ('accepted','rejected') THEN ?6 ELSE reviewed_at END,
                reviewed_by=CASE WHEN ?7 IN ('accepted','rejected') THEN ?8 ELSE reviewed_by END,
                retired_at=CASE WHEN ?7='retired' THEN ?6 ELSE retired_at END
             WHERE id=?1 AND project_id=?2 AND state_version=?9",
            params![
                relationship_id,
                self.manifest().project_id,
                next_status,
                next_review,
                annotation,
                now,
                input.action.as_str(),
                command.actor.id,
                input.expected_state_version
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(
                "relationship state version is stale".into(),
            ));
        }
        let payload = json!({"relationship_id":relationship_id,
            "action":input.action.as_str(),"from_status":current.status,
            "to_status":next_status,"from_review_state":current.review_state,
            "to_review_state":next_review,"annotation":annotation});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest().project_id,
            command,
            Some(relationship_id),
            "provenance.relationship.state_changed",
            &payload,
        )?;
        tx.execute(
            "INSERT INTO relationship_history(
                id,project_id,relationship_id,ledger_sequence,action,state_version,
                from_status,to_status,from_review_state,to_review_state,annotation,actor_id,occurred_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                new_id(),
                self.manifest().project_id,
                relationship_id,
                sequence,
                input.action.as_str(),
                input.expected_state_version + 1,
                current.status,
                next_status,
                current.review_state,
                next_review,
                annotation,
                command.actor.id,
                now
            ],
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest().project_id,
            "ChangeRelationshipState",
            Some(relationship_id),
            Some(input.expected_state_version),
            &payload,
        )?;
        tx.commit()?;
        self.get_relationship(relationship_id)
    }

    pub fn list_relationship_history(
        &self,
        relationship_id: &str,
        page: PageRequest,
    ) -> Result<RelationshipHistoryPage> {
        if page.limit == 0 || page.limit > 100 {
            return Err(CoreError::Validation(
                "relationship history page limit must be within 1..=100".into(),
            ));
        }
        self.get_relationship(relationship_id)?;
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("history offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id,relationship_id,ledger_sequence,action,state_version,from_status,to_status,
                    from_review_state,to_review_state,annotation,actor_id,occurred_at
             FROM relationship_history WHERE project_id=?1 AND relationship_id=?2
             ORDER BY state_version,id LIMIT ?3 OFFSET ?4",
        )?;
        let rows = statement
            .query_map(
                params![
                    self.manifest().project_id,
                    relationship_id,
                    page.limit + 1,
                    offset
                ],
                |row| {
                    Ok(RelationshipHistoryEntry {
                        id: row.get(0)?,
                        relationship_id: row.get(1)?,
                        ledger_sequence: row.get(2)?,
                        action: row.get(3)?,
                        state_version: row.get(4)?,
                        from_status: row.get(5)?,
                        to_status: row.get(6)?,
                        from_review_state: row.get(7)?,
                        to_review_state: row.get(8)?,
                        annotation: row.get(9)?,
                        actor_id: row.get(10)?,
                        occurred_at: row.get(11)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = rows.len() > page.limit as usize;
        let items = rows
            .into_iter()
            .take(page.limit as usize)
            .collect::<Vec<_>>();
        Ok(RelationshipHistoryPage {
            next_offset: has_more.then_some(page.offset + items.len() as u64),
            items,
        })
    }

    pub fn traverse_provenance(&self, mut query: GraphQuery) -> Result<ProvenanceGraph> {
        validate_graph_query(&mut query)?;
        let connection = self.connection()?;
        let has_explicit_roots = !query.root_entity_ids.is_empty();
        let checkpoint_roots = if let Some(checkpoint_id) = &query.checkpoint_id {
            let exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM checkpoints WHERE id=?1 AND project_id=?2)",
                params![checkpoint_id, self.manifest().project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(CoreError::NotFound(checkpoint_id.clone()));
            }
            let mut statement = connection.prepare(
                "SELECT source_entity_id FROM checkpoint_sources WHERE checkpoint_id=?1
                 ORDER BY source_entity_id LIMIT ?2",
            )?;
            statement
                .query_map(params![checkpoint_id, query.max_nodes as i64], |row| {
                    row.get(0)
                })?
                .collect::<std::result::Result<Vec<String>, _>>()?
        } else {
            Vec::new()
        };
        if query.root_entity_ids.is_empty() {
            query.root_entity_ids = checkpoint_roots;
        }
        if query.root_entity_ids.is_empty() {
            return Err(CoreError::Validation(
                "graph traversal requires a root entity or a Checkpoint with sources".into(),
            ));
        }
        ensure_unique_ids(&query.root_entity_ids, "graph root")?;
        if has_explicit_roots && query.root_entity_ids.len() > MAX_GRAPH_ROOTS {
            return Err(CoreError::Validation(
                "graph traversal accepts at most 50 explicit roots".into(),
            ));
        }
        if query.root_entity_ids.len() > query.max_nodes {
            return Err(CoreError::Validation(
                "graph root count exceeds the declared node budget".into(),
            ));
        }

        let root_set = query
            .root_entity_ids
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let mut depths = HashMap::<String, u8>::new();
        let mut pending = VecDeque::new();
        for root in &query.root_entity_ids {
            read_entity(&connection, &self.manifest().project_id, root)?;
            depths.insert(root.clone(), 0);
            pending.push_back(root.clone());
        }
        let mut edge_ids = HashSet::new();
        let mut edges = Vec::new();
        let mut truncated = false;
        let mut frontier = HashSet::new();

        while let Some(current) = pending.pop_front() {
            let depth = depths[&current];
            if depth >= query.max_depth {
                continue;
            }
            let relationship_ids = adjacency_ids(
                &connection,
                &self.manifest().project_id,
                &current,
                query.direction,
            )?;
            for relationship_id in relationship_ids {
                if edge_ids.contains(&relationship_id) {
                    continue;
                }
                let relationship =
                    read_relationship(&connection, &self.manifest().project_id, &relationship_id)?;
                if !relationship_matches(&relationship, &query) {
                    continue;
                }
                let next_id = match query.direction {
                    GraphDirection::Inbound => relationship.source_entity_id.clone(),
                    GraphDirection::Outbound => relationship.target_entity_id.clone(),
                    GraphDirection::Both => {
                        if relationship.source_entity_id == current {
                            relationship.target_entity_id.clone()
                        } else {
                            relationship.source_entity_id.clone()
                        }
                    }
                };
                let next = read_entity(&connection, &self.manifest().project_id, &next_id)?;
                if !query.entity_types.is_empty()
                    && !query.entity_types.contains(&next.entity_type)
                    && !root_set.contains(&next_id)
                {
                    continue;
                }
                if !query.entity_statuses.is_empty()
                    && !query.entity_statuses.contains(&next.status)
                    && !root_set.contains(&next_id)
                {
                    continue;
                }
                if edges.len() >= query.max_edges {
                    truncated = true;
                    frontier.insert(current.clone());
                    break;
                }
                if !depths.contains_key(&next_id) {
                    if depths.len() >= query.max_nodes {
                        truncated = true;
                        frontier.insert(next_id);
                        continue;
                    }
                    depths.insert(next_id.clone(), depth + 1);
                    pending.push_back(next_id);
                }
                edge_ids.insert(relationship_id);
                edges.push(relationship);
            }
        }
        frontier.extend(pending);
        let mut nodes = depths
            .into_iter()
            .map(|(id, depth)| {
                Ok(GraphNode {
                    entity: read_entity(&connection, &self.manifest().project_id, &id)?,
                    depth,
                    is_root: root_set.contains(&id),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        nodes.sort_by(|left, right| {
            left.depth
                .cmp(&right.depth)
                .then_with(|| left.entity.created_at.cmp(&right.entity.created_at))
                .then_with(|| left.entity.id.cmp(&right.entity.id))
        });
        edges.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        let mut frontier_entity_ids = frontier.into_iter().collect::<Vec<_>>();
        frontier_entity_ids.sort();
        let mut omissions = Vec::new();
        if truncated {
            omissions.push("graph was truncated at the declared node/edge budget".into());
        }
        if query.checkpoint_id.is_some() {
            omissions.push(
                "Checkpoint selects traversal roots; returned relationship and entity states are current, not a reconstructed historical snapshot".into(),
            );
        }
        Ok(ProvenanceGraph {
            schema_version: GRAPH_SCHEMA_VERSION,
            project_id: self.manifest().project_id.clone(),
            root_entity_ids: query.root_entity_ids,
            checkpoint_id: query.checkpoint_id,
            nodes,
            edges,
            truncated,
            frontier_entity_ids,
            omissions,
        })
    }

    pub fn validate_provenance_graph(&self) -> Result<ProvenanceValidationReport> {
        let connection = self.connection()?;
        validate_graph(&connection, &self.manifest().project_id)
    }
}

fn validate_graph_query(query: &mut GraphQuery) -> Result<()> {
    if query.max_depth > MAX_GRAPH_DEPTH {
        return Err(CoreError::Validation(
            "graph depth must be within 0..=8".into(),
        ));
    }
    if query.max_nodes == 0 || query.max_nodes > MAX_GRAPH_NODES {
        return Err(CoreError::Validation(
            "graph node budget must be within 1..=500".into(),
        ));
    }
    if query.max_edges == 0 || query.max_edges > MAX_GRAPH_EDGES {
        return Err(CoreError::Validation(
            "graph edge budget must be within 1..=1500".into(),
        ));
    }
    if query
        .minimum_confidence
        .is_some_and(|value| !(0.0..=1.0).contains(&value))
    {
        return Err(CoreError::Validation(
            "minimum confidence must be within 0.0..=1.0".into(),
        ));
    }
    query.created_from = normalize_optional_time(query.created_from.take(), "created_from")?;
    query.created_through =
        normalize_optional_time(query.created_through.take(), "created_through")?;
    if query
        .created_from
        .as_ref()
        .zip(query.created_through.as_ref())
        .is_some_and(|(from, through)| from > through)
    {
        return Err(CoreError::Validation(
            "created_from must not be after created_through".into(),
        ));
    }
    Ok(())
}

fn normalize_optional_time(value: Option<String>, label: &str) -> Result<Option<String>> {
    value
        .map(|value| {
            chrono::DateTime::parse_from_rfc3339(&value)
                .map(|time| time.with_timezone(&Utc).to_rfc3339())
                .map_err(|_| CoreError::Validation(format!("{label} must be RFC3339")))
        })
        .transpose()
}

fn relationship_matches(relationship: &Relationship, query: &GraphQuery) -> bool {
    (query.relationship_types.is_empty()
        || query
            .relationship_types
            .contains(&relationship.relation_type))
        && (query.relationship_statuses.is_empty()
            || query.relationship_statuses.contains(&relationship.status))
        && (query.review_states.is_empty()
            || query.review_states.contains(&relationship.review_state))
        && (query.origins.is_empty() || query.origins.contains(&relationship.origin))
        && query.minimum_confidence.is_none_or(|minimum| {
            relationship
                .confidence
                .is_some_and(|value| value >= minimum)
        })
        && query
            .created_from
            .as_ref()
            .is_none_or(|from| &relationship.created_at >= from)
        && query
            .created_through
            .as_ref()
            .is_none_or(|through| &relationship.created_at <= through)
}

fn adjacency_ids(
    connection: &Connection,
    project_id: &str,
    entity_id: &str,
    direction: GraphDirection,
) -> Result<Vec<String>> {
    let condition = match direction {
        GraphDirection::Inbound => "target_entity_id=?2",
        GraphDirection::Outbound => "source_entity_id=?2",
        GraphDirection::Both => "(source_entity_id=?2 OR target_entity_id=?2)",
    };
    let sql = format!(
        "SELECT id FROM relationships WHERE project_id=?1 AND {condition} ORDER BY created_at,id"
    );
    let mut statement = connection.prepare(&sql)?;
    statement
        .query_map(params![project_id, entity_id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[allow(clippy::too_many_arguments)]
fn insert_cp6_relationship(
    tx: &Transaction<'_>,
    project_id: &str,
    command: &CommandContext,
    source_id: &str,
    source_type: &str,
    target_id: &str,
    target_type: &str,
    relation_type: &str,
    origin: OriginKind,
    direct_source_ids: &[String],
) -> Result<String> {
    validate_provenance_relationship_pair(source_type, relation_type, target_type)?;
    let duplicate: Option<String> = tx
        .query_row(
            "SELECT id FROM relationships WHERE project_id=?1 AND relation_type=?2
             AND source_entity_id=?3 AND target_entity_id=?4 AND status='active' LIMIT 1",
            params![project_id, relation_type, source_id, target_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = duplicate {
        return Ok(id);
    }
    let id = new_id();
    let now = Utc::now().to_rfc3339();
    let sources = bounded_json(
        &serde_json::to_value(direct_source_ids)?,
        MAX_PROVENANCE_JSON_BYTES,
        "relationship direct sources",
    )?;
    tx.execute(
        "INSERT INTO relationships(
            id,project_id,relation_type,relation_version,source_entity_id,source_entity_type,
            target_entity_id,target_entity_type,status,origin_type,actor_id,confidence,
            review_state,direct_source_ids_json,supersedes_id,created_at,updated_at
         ) VALUES(?1,?2,?3,1,?4,?5,?6,?7,'active',?8,?9,NULL,'accepted',?10,NULL,?11,?11)",
        params![
            id,
            project_id,
            relation_type,
            source_id,
            source_type,
            target_id,
            target_type,
            origin.as_str(),
            command.actor.id,
            sources,
            now
        ],
    )?;
    Ok(id)
}

fn validate_graph(connection: &Connection, project_id: &str) -> Result<ProvenanceValidationReport> {
    let checked_entities: i64 = connection.query_row(
        "SELECT count(*) FROM entities WHERE project_id=?1",
        [project_id],
        |row| row.get(0),
    )?;
    let relationship_ids = {
        let mut statement = connection
            .prepare("SELECT id FROM relationships WHERE project_id=?1 ORDER BY created_at,id")?;
        statement
            .query_map([project_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?
    };
    let mut issues = Vec::new();
    for relationship_id in &relationship_ids {
        let relationship = match read_relationship(connection, project_id, relationship_id) {
            Ok(relationship) => relationship,
            Err(_) => {
                issues.push(issue(
                    "malformed_relationship_record",
                    "error",
                    vec![],
                    vec![relationship_id.clone()],
                    "A relationship record or its direct-source payload cannot be decoded.",
                    "Preserve the ledger for diagnostics and recover the relationship from a verified backup.",
                ));
                continue;
            }
        };
        let source = match read_entity(connection, project_id, &relationship.source_entity_id) {
            Ok(source) => source,
            Err(_) => {
                issues.push(issue(
                    "missing_relationship_endpoint",
                    "error",
                    vec![relationship.source_entity_id.clone()],
                    vec![relationship.id.clone()],
                    "A relationship source is missing or belongs to another project.",
                    "Recover the endpoint from a verified backup or retire the invalid relationship.",
                ));
                continue;
            }
        };
        let target = match read_entity(connection, project_id, &relationship.target_entity_id) {
            Ok(target) => target,
            Err(_) => {
                issues.push(issue(
                    "missing_relationship_endpoint",
                    "error",
                    vec![relationship.target_entity_id.clone()],
                    vec![relationship.id.clone()],
                    "A relationship target is missing or belongs to another project.",
                    "Recover the endpoint from a verified backup or retire the invalid relationship.",
                ));
                continue;
            }
        };
        let pair_valid = crate::research::validate_registered_relationship_pair(
            &source.entity_type,
            &relationship.relation_type,
            &target.entity_type,
        )
        .and_then(|_| {
            crate::development::validate_development_relationship_pair(
                &source.entity_type,
                &relationship.relation_type,
                &target.entity_type,
            )
        })
        .and_then(|_| {
            validate_provenance_relationship_pair(
                &source.entity_type,
                &relationship.relation_type,
                &target.entity_type,
            )
        });
        if pair_valid.is_err()
            || relationship.source_entity_type != source.entity_type
            || relationship.target_entity_type != target.entity_type
        {
            issues.push(issue(
                "invalid_relationship_pair",
                "error",
                vec![source.id.clone(), target.id.clone()],
                vec![relationship.id.clone()],
                "A relationship no longer matches the registered source/type/target contract.",
                "Retire the invalid relationship and create a permitted typed replacement; do not rewrite its history.",
            ));
        }
        if relationship.status == "active"
            && (source.status == "archived" || target.status == "archived")
        {
            issues.push(issue(
                "stale_active_relationship",
                "warning",
                vec![source.id.clone(), target.id.clone()],
                vec![relationship.id.clone()],
                "An active relationship points to an archived entity.",
                "Review whether the relationship should be retired or a newer entity should supersede the archived endpoint.",
            ));
        }
        let mut direct_sources = HashSet::new();
        for direct_source_id in &relationship.direct_source_ids {
            if !direct_sources.insert(direct_source_id) {
                issues.push(issue(
                    "duplicate_direct_source",
                    "warning",
                    vec![direct_source_id.clone()],
                    vec![relationship.id.clone()],
                    "A relationship repeats the same direct source.",
                    "Retire and recreate the relationship with a unique source list.",
                ));
            }
            if read_entity(connection, project_id, direct_source_id).is_err() {
                issues.push(issue(
                    "missing_direct_source",
                    "error",
                    vec![direct_source_id.clone()],
                    vec![relationship.id.clone()],
                    "A relationship cites a missing or cross-project direct source.",
                    "Recover the source from a verified backup or retire the relationship.",
                ));
            }
        }
        let latest_history: Option<(i64, String, String)> = connection
            .query_row(
                "SELECT state_version,to_status,to_review_state FROM relationship_history
                 WHERE project_id=?1 AND relationship_id=?2 ORDER BY state_version DESC LIMIT 1",
                params![project_id, relationship.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if latest_history.as_ref().is_none_or(|history| {
            history.0 != relationship.state_version
                || history.1 != relationship.status
                || history.2 != relationship.review_state
        }) {
            issues.push(issue(
                "relationship_history_mismatch",
                "error",
                vec![],
                vec![relationship.id.clone()],
                "Relationship current state does not match its immutable history head.",
                "Preserve the ledger for diagnostics and recover from a verified backup.",
            ));
        }
    }

    append_cycle_issues(connection, project_id, &mut issues)?;
    append_ambiguity_issues(connection, project_id, &mut issues)?;
    append_gap_issues(connection, project_id, &mut issues)?;
    Ok(ProvenanceValidationReport {
        checked_entities: checked_entities as usize,
        checked_relationships: relationship_ids.len(),
        issues,
    })
}

fn append_cycle_issues(
    connection: &Connection,
    project_id: &str,
    issues: &mut Vec<ProvenanceIssue>,
) -> Result<()> {
    let ids = {
        let mut statement = connection.prepare(
            "SELECT DISTINCT source_entity_id FROM relationships
             WHERE project_id=?1 AND relation_type='supersedes' AND status='active'",
        )?;
        statement
            .query_map([project_id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<String>, _>>()?
    };
    for id in ids {
        let targets = adjacency_targets(connection, project_id, &id, "supersedes")?;
        let mut cyclic = false;
        for target in targets {
            if path_reaches(connection, project_id, &target, &id, "supersedes")? {
                cyclic = true;
                break;
            }
        }
        if cyclic {
            issues.push(issue(
                "supersedes_cycle",
                "error",
                vec![id],
                vec![],
                "The supersession graph contains a cycle.",
                "Retire the newest invalid supersession edge while preserving all entity history.",
            ));
        }
    }
    Ok(())
}

fn append_ambiguity_issues(
    connection: &Connection,
    project_id: &str,
    issues: &mut Vec<ProvenanceIssue>,
) -> Result<()> {
    let duplicates = {
        let mut statement = connection.prepare(
            "SELECT source_entity_id,relation_type,target_entity_id,group_concat(id),count(*)
             FROM relationships WHERE project_id=?1 AND status='active'
             GROUP BY source_entity_id,relation_type,target_entity_id HAVING count(*)>1",
        )?;
        statement
            .query_map([project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    for (source, relation, target, ids) in duplicates {
        issues.push(issue(
            "ambiguous_duplicate_relationship",
            "warning",
            vec![source, target],
            ids.split(',').map(str::to_owned).collect(),
            &format!("Multiple active canonical relationships assert the same {relation} edge."),
            "Review the duplicates and retire or supersede redundant assertions without deleting history.",
        ));
    }
    let superseders = {
        let mut statement = connection.prepare(
            "SELECT target_entity_id,group_concat(id),count(*) FROM relationships
             WHERE project_id=?1 AND relation_type='supersedes' AND status='active'
             GROUP BY target_entity_id HAVING count(*)>1",
        )?;
        statement
            .query_map([project_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?
    };
    for (target, ids) in superseders {
        issues.push(issue(
            "ambiguous_supersession_head",
            "warning",
            vec![target],
            ids.split(',').map(str::to_owned).collect(),
            "One record has multiple active superseding successors.",
            "Review which successor is authoritative and retire the competing supersession edge.",
        ));
    }
    Ok(())
}

fn append_gap_issues(
    connection: &Connection,
    project_id: &str,
    issues: &mut Vec<ProvenanceIssue>,
) -> Result<()> {
    let research_requirements = query_entity_ids(
        connection,
        "SELECT e.id FROM entities e JOIN requirements r ON r.entity_id=e.id
         WHERE e.project_id=?1 AND e.status<>'archived' AND r.rationale_origin='research'",
        project_id,
    )?;
    for id in research_requirements {
        if !has_incoming(
            connection,
            project_id,
            &id,
            &["creates", "modifies", "retires"],
        )? {
            issues.push(issue(
                "research_requirement_missing_decision",
                "error",
                vec![id],
                vec![],
                "A research-origin Requirement has no Decision provenance.",
                "Link the accepted Decision that created, modified, or retired the Requirement.",
            ));
        }
    }
    let accepted_findings = query_entity_ids(
        connection,
        "SELECT id FROM entities WHERE project_id=?1 AND entity_type='finding' AND status='accepted'",
        project_id,
    )?;
    for id in accepted_findings {
        if !has_incoming(
            connection,
            project_id,
            &id,
            &["supports", "challenges", "inconclusive_for"],
        )? {
            issues.push(issue(
                "accepted_finding_missing_source",
                "warning",
                vec![id],
                vec![],
                "An accepted Finding has no supporting, challenging, or inconclusive source link.",
                "Attach the Evidence or Result used to assess the Finding, or document the external/legacy gap.",
            ));
        }
    }
    let research_change_sets = query_entity_ids(
        connection,
        "SELECT e.id FROM entities e JOIN change_sets c ON c.entity_id=e.id
         WHERE e.project_id=?1 AND e.status<>'archived' AND c.intent_origin='research'",
        project_id,
    )?;
    for id in research_change_sets {
        if !has_outgoing(
            connection,
            project_id,
            &id,
            &["implements", "partially_implements", "reverts"],
        )? {
            issues.push(issue(
                "research_change_set_missing_requirement",
                "error",
                vec![id],
                vec![],
                "A ChangeSet marked as research-origin has no Requirement link.",
                "Link the Requirement that authorized this change or correct the declared intent origin.",
            ));
        }
    }
    let feedback_ids = query_entity_ids(
        connection,
        "SELECT id FROM entities WHERE project_id=?1 AND entity_type='learning_feedback' AND status<>'archived'",
        project_id,
    )?;
    for id in feedback_ids {
        if !has_incoming(connection, project_id, &id, &["produces"])? {
            issues.push(issue(
                "learning_feedback_missing_source",
                "error",
                vec![id.clone()],
                vec![],
                "Learning Feedback has no validation source.",
                "Link the Result, TestRun, ChangeSet, or Evidence that produced the feedback.",
            ));
        }
        if !has_outgoing(
            connection,
            project_id,
            &id,
            &["supports", "challenges", "requests_revision_of"],
        )? {
            issues.push(issue(
                "learning_feedback_unresolved_target",
                "info",
                vec![id],
                vec![],
                "Learning Feedback is retained as an explicitly unresolved item with no target.",
                "Select a target later, or resolve the feedback with a note explaining why no revision is needed.",
            ));
        }
    }
    Ok(())
}

pub(crate) fn append_provenance_integrity_issues(
    connection: &Connection,
    project_id: &str,
    report: &mut IntegrityReport,
) -> Result<()> {
    let validation = validate_graph(connection, project_id)?;
    for issue in validation
        .issues
        .into_iter()
        .filter(|issue| issue.severity == "error")
    {
        report.issues.push(IntegrityIssue {
            code: issue.code,
            path_or_id: issue
                .relationship_ids
                .first()
                .or_else(|| issue.entity_ids.first())
                .cloned()
                .unwrap_or_else(|| project_id.to_owned()),
            guidance: issue.guidance,
        });
    }
    Ok(())
}

fn issue(
    code: &str,
    severity: &str,
    entity_ids: Vec<String>,
    relationship_ids: Vec<String>,
    message: &str,
    guidance: &str,
) -> ProvenanceIssue {
    ProvenanceIssue {
        code: code.into(),
        severity: severity.into(),
        entity_ids,
        relationship_ids,
        message: message.into(),
        guidance: guidance.into(),
    }
}

fn has_incoming(
    connection: &Connection,
    project_id: &str,
    target_id: &str,
    relations: &[&str],
) -> Result<bool> {
    let mut statement = connection.prepare(
        "SELECT relation_type FROM relationships
         WHERE project_id=?1 AND target_entity_id=?2 AND status='active'",
    )?;
    let found = statement
        .query_map(params![project_id, target_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .any(|relation| relations.contains(&relation.as_str()));
    Ok(found)
}

fn has_outgoing(
    connection: &Connection,
    project_id: &str,
    source_id: &str,
    relations: &[&str],
) -> Result<bool> {
    let mut statement = connection.prepare(
        "SELECT relation_type FROM relationships
         WHERE project_id=?1 AND source_entity_id=?2 AND status='active'",
    )?;
    let found = statement
        .query_map(params![project_id, source_id], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
        .into_iter()
        .any(|relation| relations.contains(&relation.as_str()));
    Ok(found)
}

fn query_entity_ids(connection: &Connection, sql: &str, project_id: &str) -> Result<Vec<String>> {
    let mut statement = connection.prepare(sql)?;
    statement
        .query_map([project_id], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn adjacency_targets(
    connection: &Connection,
    project_id: &str,
    source_id: &str,
    relation: &str,
) -> Result<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT target_entity_id FROM relationships
         WHERE project_id=?1 AND source_entity_id=?2 AND relation_type=?3 AND status='active'",
    )?;
    statement
        .query_map(params![project_id, source_id, relation], |row| row.get(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn path_reaches(
    connection: &Connection,
    project_id: &str,
    start: &str,
    target: &str,
    relation: &str,
) -> Result<bool> {
    let mut pending = vec![start.to_owned()];
    let mut visited = HashSet::new();
    while let Some(current) = pending.pop() {
        if current == target {
            return Ok(true);
        }
        if !visited.insert(current.clone()) {
            continue;
        }
        if visited.len() > 100_000 {
            return Err(CoreError::Validation(
                "graph reachability exceeded its safety limit".into(),
            ));
        }
        pending.extend(adjacency_targets(
            connection, project_id, &current, relation,
        )?);
    }
    Ok(false)
}

fn ensure_unique_ids(ids: &[String], label: &str) -> Result<()> {
    let mut unique = HashSet::new();
    if ids.iter().any(|id| !unique.insert(id)) {
        return Err(CoreError::Validation(format!("duplicate {label} ID")));
    }
    Ok(())
}

fn validate_text(value: &str, max_chars: usize, label: &str) -> Result<()> {
    if value.chars().count() > max_chars {
        return Err(CoreError::Validation(format!(
            "{label} exceeds {max_chars} characters"
        )));
    }
    Ok(())
}

pub(crate) fn require_active_project(connection: &Connection, project_id: &str) -> Result<()> {
    let status: Option<String> = connection
        .query_row(
            "SELECT status FROM projects WHERE id=?1",
            [project_id],
            |row| row.get(0),
        )
        .optional()?;
    match status.as_deref() {
        Some("active") => Ok(()),
        Some(_) => Err(CoreError::Conflict("project is archived".into())),
        None => Err(CoreError::NotFound(project_id.into())),
    }
}

fn entity_type(connection: &Connection, project_id: &str, id: &str) -> Result<String> {
    connection
        .query_row(
            "SELECT entity_type FROM entities WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| CoreError::Validation(format!("entity {id} is absent from this project")))
}

fn read_entity(connection: &Connection, project_id: &str, id: &str) -> Result<Entity> {
    let raw = connection
        .query_row(
            "SELECT id,project_id,entity_type,entity_schema_version,title,status,version,origin_type,
                    metadata_json,data_json,created_at,created_by,updated_at,updated_by,archived_at
             FROM entities WHERE id=?1 AND project_id=?2",
            params![id, project_id],
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

fn read_relationship(connection: &Connection, project_id: &str, id: &str) -> Result<Relationship> {
    let raw = connection
        .query_row(
            "SELECT id,project_id,relation_type,relation_version,source_entity_id,source_entity_type,
                    target_entity_id,target_entity_type,status,origin_type,actor_id,confidence,
                    review_state,direct_source_ids_json,supersedes_id,created_at,updated_at,
                    state_version,annotation,reviewed_at,reviewed_by,retired_at
             FROM relationships WHERE id=?1 AND project_id=?2",
            params![id, project_id],
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
        .ok_or_else(|| CoreError::NotFound(id.into()))?;
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
        direct_source_ids: serde_json::from_str(&raw.13)?,
        supersedes_id: raw.14,
        created_at: raw.15,
        updated_at: raw.16,
        state_version: raw.17,
        annotation: raw.18,
        reviewed_at: raw.19,
        reviewed_by: raw.20,
        retired_at: raw.21,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_matrix_is_closed() {
        for (source, relation, target) in [
            ("test_run", "produces", "learning_feedback"),
            ("result", "produces", "learning_feedback"),
            ("learning_feedback", "challenges", "finding"),
            ("learning_feedback", "requests_revision_of", "requirement"),
        ] {
            assert!(validate_provenance_relationship_pair(source, relation, target).is_ok());
        }
        for (source, relation, target) in [
            ("test", "produces", "learning_feedback"),
            ("learning_feedback", "implements", "requirement"),
            ("learning_feedback", "requests_revision_of", "evidence"),
        ] {
            assert!(validate_provenance_relationship_pair(source, relation, target).is_err());
        }
    }
}
