use std::collections::HashSet;
use std::fmt::Write as _;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::store::{
    append_event_with_context, bounded_json, legacy_origin, prior_result,
    record_command_with_context, validate_command_context, validate_nonempty,
};
use crate::{
    ArtifactAvailability, Checkpoint, CommandContext, ContinuityStore, CoreError, Entity,
    IntegrityIssue, IntegrityReport, OriginKind, PageRequest, RelationshipPolicy, Result, new_id,
};

const MAX_RESEARCH_TEXT: usize = 30_000;
const MAX_RESEARCH_JSON_BYTES: usize = 1024 * 1024;
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

string_enum!(ResearchEntityKind {
    ResearchSession => "research_session",
    ResearchQuestion => "research_question",
    Evidence => "evidence",
    Experiment => "experiment",
    Result => "result",
    Finding => "finding",
    Decision => "decision",
    Requirement => "requirement",
});

string_enum!(QuestionKind {
    Question => "question",
    Hypothesis => "hypothesis",
    Uncertainty => "uncertainty",
});

string_enum!(EvidenceKind {
    Note => "note",
    Web => "web",
    File => "file",
    Screenshot => "screenshot",
    RecordingSegment => "recording_segment",
    CaptureMarker => "capture_marker",
    Observation => "observation",
    Other => "other",
});

string_enum!(ResearchResultOutcome {
    Positive => "positive",
    Negative => "negative",
    Mixed => "mixed",
    Inconclusive => "inconclusive",
    Error => "error",
    Observed => "observed",
});

string_enum!(FindingSourceAssessment {
    Supports => "supports",
    Challenges => "challenges",
    Contextualizes => "contextualizes",
    InconclusiveFor => "inconclusive_for",
});

string_enum!(RequirementRationaleOrigin {
    Research => "research",
    User => "user",
    Import => "import",
    External => "external",
    Legacy => "legacy",
    Unknown => "unknown",
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchItem {
    pub entity: Entity,
    pub details: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewResearchSession {
    pub title: String,
    pub objective: String,
    pub started_at: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchSessionUpdate {
    pub title: String,
    pub objective: String,
    #[serde(default)]
    pub metadata: Value,
    pub expected_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewResearchQuestion {
    pub title: String,
    pub kind: QuestionKind,
    pub question: String,
    pub context: String,
    pub desired_outcome: String,
    pub priority: u8,
    pub due_at: Option<String>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchQuestionUpdate {
    pub title: String,
    pub kind: QuestionKind,
    pub question: String,
    pub context: String,
    pub desired_outcome: String,
    pub priority: u8,
    pub due_at: Option<String>,
    #[serde(default)]
    pub metadata: Value,
    pub expected_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewEvidence {
    pub title: String,
    pub kind: EvidenceKind,
    pub origin: OriginKind,
    pub source_uri: Option<String>,
    pub source_title: Option<String>,
    pub source_author: Option<String>,
    pub captured_at: Option<String>,
    pub capture_method: String,
    pub stable_reference: Option<String>,
    pub source_content: Option<String>,
    pub annotation: String,
    pub summary: String,
    pub relevance: String,
    pub original_artifact_id: Option<String>,
    pub question_id: Option<String>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceAnnotationUpdate {
    pub annotation: String,
    pub summary: String,
    pub relevance: String,
    pub expected_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvidenceDetailsUpdate {
    pub title: String,
    pub annotation: String,
    pub summary: String,
    pub relevance: String,
    pub expected_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewExperiment {
    pub title: String,
    pub hypothesis: String,
    pub method: String,
    #[serde(default)]
    pub inputs: Value,
    pub expected_observations: String,
    pub question_id: Option<String>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewResearchResult {
    pub title: String,
    pub experiment_id: String,
    pub observation: String,
    pub outcome: ResearchResultOutcome,
    #[serde(default)]
    pub measurements: Value,
    pub observed_at: Option<String>,
    pub artifact_id: Option<String>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FindingSource {
    pub entity_id: String,
    pub assessment: FindingSourceAssessment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewFinding {
    pub title: String,
    pub claim: String,
    pub interpretation: String,
    pub uncertainty: String,
    pub confidence: Option<f64>,
    #[serde(default)]
    pub sources: Vec<FindingSource>,
    pub answers_question_id: Option<String>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewDecision {
    pub title: String,
    pub selected_option: String,
    pub rationale: String,
    #[serde(default)]
    pub alternatives: Vec<String>,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub finding_ids: Vec<String>,
    pub supersedes_decision_id: Option<String>,
    pub supersedes_expected_version: Option<i64>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NewRequirement {
    pub title: String,
    pub statement: String,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    pub priority: u8,
    pub rationale_origin: RequirementRationaleOrigin,
    pub verification_method: String,
    pub decision_id: Option<String>,
    pub session_id: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchSearchQuery {
    pub text: String,
    pub entity_type: Option<ResearchEntityKind>,
    pub status: Option<String>,
    pub session_id: Option<String>,
    pub page: PageRequest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchSearchHit {
    pub entity_id: String,
    pub entity_type: String,
    pub title: String,
    pub status: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchSearchPage {
    pub items: Vec<ResearchSearchHit>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchTimelineFilter {
    pub session_id: Option<String>,
    pub entity_id: Option<String>,
    pub event_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchTimelineEntry {
    pub id: String,
    pub ledger_sequence: i64,
    pub session_id: Option<String>,
    pub entity_id: Option<String>,
    pub event_type: String,
    pub payload: Value,
    pub actor_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchTimelinePage {
    pub items: Vec<ResearchTimelineEntry>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchCheckpointInput {
    pub note: String,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResearchResumeState {
    pub checkpoint: Option<Checkpoint>,
    pub checkpoint_is_stale: bool,
    pub events_since_checkpoint: u64,
    pub active_items: Vec<ResearchSearchHit>,
    pub active_item_count: u64,
    pub active_items_truncated: bool,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchReport {
    pub project_id: String,
    pub source_ledger_sequence: i64,
    pub generated_at: String,
    pub markdown: String,
    pub cited_entity_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ResearchRelationshipPolicy;

impl RelationshipPolicy for ResearchRelationshipPolicy {
    fn validate_pair(
        &self,
        source_entity_type: &str,
        relation_type: &str,
        target_entity_type: &str,
    ) -> std::result::Result<(), String> {
        validate_research_pair(source_entity_type, relation_type, target_entity_type)
            .map_err(|error| error.to_string())
    }
}

pub(crate) fn is_reserved_domain_entity_type(value: &str) -> bool {
    matches!(
        value,
        "research_session"
            | "research_question"
            | "evidence"
            | "experiment"
            | "result"
            | "finding"
            | "decision"
            | "requirement"
    )
}

pub(crate) fn validate_registered_relationship_pair(
    source: &str,
    relation: &str,
    target: &str,
) -> Result<()> {
    let declared_cross_space_pair = matches!(
        (source, relation, target),
        (
            "change_set",
            "implements" | "partially_implements" | "reverts",
            "requirement"
        ) | ("test", "verifies", "requirement")
            | ("evidence" | "result", "produces", "learning_feedback")
            | (
                "learning_feedback",
                "supports" | "challenges" | "requests_revision_of",
                "evidence" | "finding" | "decision" | "requirement"
            )
    );
    if declared_cross_space_pair {
        return Ok(());
    }
    let strict_research_endpoint = |value: &str| {
        matches!(
            value,
            "research_session"
                | "research_question"
                | "evidence"
                | "experiment"
                | "result"
                | "finding"
                | "decision"
        )
    };
    if strict_research_endpoint(source) || strict_research_endpoint(target) {
        validate_research_pair(source, relation, target)?;
    } else if source == "requirement" || target == "requirement" {
        return Err(CoreError::Validation(format!(
            "relationship {source} --{relation}--> {target} is not declared for Requirement"
        )));
    }
    Ok(())
}

fn validate_research_pair(source: &str, relation: &str, target: &str) -> Result<()> {
    let valid = match relation {
        "addresses" => source == "experiment" && target == "research_question",
        "produces" => source == "experiment" && target == "result",
        "supports" | "challenges" | "inconclusive_for" => {
            matches!(source, "evidence" | "result") && target == "finding"
        }
        "contextualizes" => {
            source == "evidence" && matches!(target, "research_question" | "experiment" | "finding")
        }
        "answers" => source == "finding" && target == "research_question",
        "informs" => source == "finding" && target == "decision",
        "creates" | "modifies" | "retires" => source == "decision" && target == "requirement",
        "supersedes" => {
            source == target
                && matches!(
                    source,
                    "research_question" | "finding" | "decision" | "requirement"
                )
        }
        _ => false,
    };
    if !valid {
        return Err(CoreError::Validation(format!(
            "relationship {source} --{relation}--> {target} is not permitted by Research Space"
        )));
    }
    Ok(())
}

struct CreateSpec<'a> {
    operation: &'static str,
    event_type: &'static str,
    entity_type: &'static str,
    title: &'a str,
    status: &'static str,
    origin: OriginKind,
    metadata: &'a Value,
    session_id: Option<&'a str>,
    search_body: String,
    event_details: Value,
}

impl ContinuityStore {
    pub fn create_research_session(
        &self,
        command: &CommandContext,
        input: NewResearchSession,
    ) -> Result<ResearchItem> {
        validate_nonempty(&input.objective, 10_000, "research objective")?;
        let started_at = validated_time_or_now(input.started_at.as_deref(), "started_at")?;
        self.create_research_record(
            command,
            CreateSpec {
                operation: "CreateResearchSession",
                event_type: "research.session.started",
                entity_type: "research_session",
                title: &input.title,
                status: "active",
                origin: OriginKind::User,
                metadata: &input.metadata,
                session_id: None,
                search_body: input.objective.clone(),
                event_details: json!({}),
            },
            |tx, id| {
                tx.execute(
                    "INSERT INTO research_sessions(entity_id,objective,started_at) VALUES(?1,?2,?3)",
                    params![id, input.objective, started_at],
                )?;
                Ok(())
            },
        )
    }

    pub fn update_research_session(
        &self,
        command: &CommandContext,
        entity_id: &str,
        update: ResearchSessionUpdate,
    ) -> Result<ResearchItem> {
        validate_command_context(command)?;
        validate_nonempty(&update.title, 500, "title")?;
        validate_nonempty(&update.objective, 10_000, "research objective")?;
        let metadata = bounded_json(
            &update.metadata,
            MAX_RESEARCH_JSON_BYTES,
            "research metadata",
        )?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_research_enabled(&tx, &self.manifest.project_id)?;
        if let Some(result) = prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "UpdateResearchSession",
        )? {
            tx.commit()?;
            return self.get_research_item(&result.unwrap_or_else(|| entity_id.to_owned()));
        }
        let status = research_entity_status(
            &tx,
            &self.manifest.project_id,
            entity_id,
            "research_session",
        )?;
        if status != "active" {
            return Err(CoreError::Conflict(
                "only an active research session can be edited".into(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let changed = tx.execute(
            "UPDATE entities SET title=?3,metadata_json=?4,version=version+1,updated_at=?5,updated_by=?6
             WHERE id=?1 AND project_id=?2 AND version=?7",
            params![entity_id,self.manifest.project_id,update.title,metadata,now,command.actor.id,update.expected_version],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(
                "research session version is stale or missing".into(),
            ));
        }
        tx.execute(
            "UPDATE research_sessions SET objective=?2 WHERE entity_id=?1",
            params![entity_id, update.objective],
        )?;
        upsert_search_document(
            &tx,
            &self.manifest.project_id,
            entity_id,
            "research_session",
            &update.title,
            &update.objective,
            &now,
        )?;
        let payload = json!({"entity_id":entity_id,"previous_version":update.expected_version});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(entity_id),
            "research.session.updated",
            &payload,
        )?;
        insert_timeline(
            &tx,
            &self.manifest.project_id,
            sequence,
            Some(entity_id),
            Some(entity_id),
            "research.session.updated",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "UpdateResearchSession",
            Some(entity_id),
            Some(update.expected_version),
            &payload,
        )?;
        tx.commit()?;
        self.get_research_item(entity_id)
    }

    pub fn create_research_question(
        &self,
        command: &CommandContext,
        input: NewResearchQuestion,
    ) -> Result<ResearchItem> {
        validate_nonempty(&input.question, 20_000, "research question")?;
        validate_text(&input.context, 20_000, "question context")?;
        validate_text(&input.desired_outcome, 20_000, "desired outcome")?;
        validate_priority(input.priority)?;
        validate_optional_time(input.due_at.as_deref(), "due_at")?;
        let body = format!(
            "{}\n{}\n{}",
            input.question, input.context, input.desired_outcome
        );
        self.create_research_record(
            command,
            CreateSpec {
                operation: "CreateResearchQuestion",
                event_type: "research.question.created",
                entity_type: "research_question",
                title: &input.title,
                status: "active",
                origin: OriginKind::User,
                metadata: &input.metadata,
                session_id: input.session_id.as_deref(),
                search_body: body,
                event_details: json!({}),
            },
            |tx, id| {
                tx.execute(
                    "INSERT INTO research_questions(
                        entity_id,question_kind,question_text,context,desired_outcome,priority,due_at
                     ) VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    params![id, input.kind.as_str(), input.question, input.context,
                        input.desired_outcome, input.priority, input.due_at],
                )?;
                Ok(())
            },
        )
    }

    pub fn update_research_question(
        &self,
        command: &CommandContext,
        entity_id: &str,
        update: ResearchQuestionUpdate,
    ) -> Result<ResearchItem> {
        validate_command_context(command)?;
        validate_nonempty(&update.title, 500, "title")?;
        validate_nonempty(&update.question, 20_000, "research question")?;
        validate_text(&update.context, 20_000, "question context")?;
        validate_text(&update.desired_outcome, 20_000, "desired outcome")?;
        validate_priority(update.priority)?;
        validate_optional_time(update.due_at.as_deref(), "due_at")?;
        let metadata = bounded_json(
            &update.metadata,
            MAX_RESEARCH_JSON_BYTES,
            "research metadata",
        )?;
        let body = format!(
            "{}\n{}\n{}",
            update.question, update.context, update.desired_outcome
        );
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_research_enabled(&tx, &self.manifest.project_id)?;
        if let Some(result) = prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "UpdateResearchQuestion",
        )? {
            tx.commit()?;
            return self.get_research_item(&result.unwrap_or_else(|| entity_id.to_owned()));
        }
        let status = research_entity_status(
            &tx,
            &self.manifest.project_id,
            entity_id,
            "research_question",
        )?;
        if matches!(status.as_str(), "answered" | "superseded" | "archived") {
            return Err(CoreError::Conflict(
                "closed research question cannot be edited; supersede it instead".into(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let changed = tx.execute(
            "UPDATE entities SET title=?3,metadata_json=?4,version=version+1,updated_at=?5,updated_by=?6
             WHERE id=?1 AND project_id=?2 AND version=?7",
            params![entity_id,self.manifest.project_id,update.title,metadata,now,command.actor.id,update.expected_version],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(
                "research question version is stale or missing".into(),
            ));
        }
        tx.execute(
            "UPDATE research_questions SET question_kind=?2,question_text=?3,context=?4,
                    desired_outcome=?5,priority=?6,due_at=?7 WHERE entity_id=?1",
            params![
                entity_id,
                update.kind.as_str(),
                update.question,
                update.context,
                update.desired_outcome,
                update.priority,
                update.due_at
            ],
        )?;
        upsert_search_document(
            &tx,
            &self.manifest.project_id,
            entity_id,
            "research_question",
            &update.title,
            &body,
            &now,
        )?;
        let sequence = append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(entity_id),
            "research.question.updated",
            &json!({"entity_id":entity_id,"previous_version":update.expected_version}),
        )?;
        insert_timeline(
            &tx,
            &self.manifest.project_id,
            sequence,
            None,
            Some(entity_id),
            "research.question.updated",
            &json!({"entity_id":entity_id}),
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "UpdateResearchQuestion",
            Some(entity_id),
            Some(update.expected_version),
            &json!({"entity_id":entity_id}),
        )?;
        tx.commit()?;
        self.get_research_item(entity_id)
    }

    pub fn create_evidence(
        &self,
        command: &CommandContext,
        input: NewEvidence,
    ) -> Result<ResearchItem> {
        if input.origin == OriginKind::AiProposal {
            return Err(CoreError::Validation(
                "AI proposals cannot become canonical Evidence without acceptance".into(),
            ));
        }
        validate_nonempty(&input.capture_method, 200, "capture method")?;
        validate_optional_text(&input.source_uri, 4_096, "source URI")?;
        validate_optional_text(&input.source_title, 1_000, "source title")?;
        validate_optional_text(&input.source_author, 500, "source author")?;
        validate_optional_text(&input.stable_reference, 4_096, "stable reference")?;
        validate_optional_text(&input.source_content, 1_000_000, "source content")?;
        for (value, label) in [
            (&input.annotation, "evidence annotation"),
            (&input.summary, "evidence summary"),
            (&input.relevance, "evidence relevance"),
        ] {
            if value.chars().count() > MAX_RESEARCH_TEXT {
                return Err(CoreError::Validation(format!(
                    "{label} exceeds {MAX_RESEARCH_TEXT} characters"
                )));
            }
        }
        if input.original_artifact_id.is_none()
            && optional_blank(&input.source_uri)
            && optional_blank(&input.stable_reference)
            && optional_blank(&input.source_content)
        {
            return Err(CoreError::Validation(
                "Evidence requires original content, an artifact, source URI, or stable reference"
                    .into(),
            ));
        }
        let captured_at = validated_time_or_now(input.captured_at.as_deref(), "captured_at")?;
        let body = format!(
            "{}\n{}\n{}\n{}\n{}",
            input.source_title.as_deref().unwrap_or(""),
            input.source_content.as_deref().unwrap_or(""),
            input.annotation,
            input.summary,
            input.relevance
        );
        self.create_research_record(
            command,
            CreateSpec {
                operation: "CreateEvidence",
                event_type: "research.evidence.created",
                entity_type: "evidence",
                title: &input.title,
                status: "available",
                origin: input.origin,
                metadata: &input.metadata,
                session_id: input.session_id.as_deref(),
                search_body: body,
                event_details: json!({
                    "question_id": input.question_id,
                    "artifact_id": input.original_artifact_id,
                    "origin": input.origin.as_str()
                }),
            },
            |tx, id| {
                if let Some(artifact_id) = &input.original_artifact_id {
                    require_available_artifact(tx,&self.manifest.project_id,artifact_id)?;
                }
                if let Some(question_id) = &input.question_id {
                    require_entity_type(tx,&self.manifest.project_id,question_id,"research_question")?;
                }
                tx.execute(
                    "INSERT INTO evidence(entity_id,evidence_kind,source_uri,source_title,source_author,
                        captured_at,capture_method,stable_reference,source_content,annotation_text,
                        summary_text,relevance,original_artifact_id)
                     VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
                    params![id,input.kind.as_str(),input.source_uri,input.source_title,input.source_author,
                        captured_at,input.capture_method,input.stable_reference,input.source_content,
                        input.annotation,input.summary,input.relevance,input.original_artifact_id],
                )?;
                if let Some(artifact_id) = &input.original_artifact_id {
                    tx.execute(
                        "INSERT INTO entity_artifacts(entity_id,artifact_id,role,created_at) VALUES(?1,?2,'original',?3)",
                        params![id,artifact_id,Utc::now().to_rfc3339()],
                    )?;
                }
                if let Some(question_id) = &input.question_id {
                    insert_relationship(tx,&self.manifest.project_id,command,id,"evidence",question_id,
                        "research_question","contextualizes",input.origin,&[])?;
                }
                Ok(())
            },
        )
    }

    pub fn update_evidence_annotation(
        &self,
        command: &CommandContext,
        entity_id: &str,
        update: EvidenceAnnotationUpdate,
    ) -> Result<ResearchItem> {
        let title = self.get_entity(entity_id)?.title;
        self.update_evidence_fields(
            command,
            entity_id,
            EvidenceDetailsUpdate {
                title,
                annotation: update.annotation,
                summary: update.summary,
                relevance: update.relevance,
                expected_version: update.expected_version,
            },
            "UpdateEvidenceAnnotation",
            "research.evidence.annotation_updated",
        )
    }

    pub fn update_evidence_details(
        &self,
        command: &CommandContext,
        entity_id: &str,
        update: EvidenceDetailsUpdate,
    ) -> Result<ResearchItem> {
        self.update_evidence_fields(
            command,
            entity_id,
            update,
            "UpdateEvidenceDetails",
            "research.evidence.details_updated",
        )
    }

    fn update_evidence_fields(
        &self,
        command: &CommandContext,
        entity_id: &str,
        update: EvidenceDetailsUpdate,
        operation: &str,
        event_type: &str,
    ) -> Result<ResearchItem> {
        validate_command_context(command)?;
        validate_nonempty(&update.title, 500, "evidence title")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_research_enabled(&tx, &self.manifest.project_id)?;
        if let Some(result) = prior_result(&tx, &self.manifest.project_id, command, operation)? {
            tx.commit()?;
            return self.get_research_item(&result.unwrap_or_else(|| entity_id.to_owned()));
        }
        let status = research_entity_status(&tx, &self.manifest.project_id, entity_id, "evidence")?;
        if matches!(status.as_str(), "archived" | "superseded") {
            return Err(CoreError::Conflict(
                "archived or superseded Evidence cannot be annotated".into(),
            ));
        }
        let current = tx.query_row(
            "SELECT e.title,ev.source_content,ev.source_title FROM entities e JOIN evidence ev ON ev.entity_id=e.id WHERE e.id=?1",
            [entity_id],|row| Ok((row.get::<_,String>(0)?,row.get::<_,Option<String>>(1)?,row.get::<_,Option<String>>(2)?)))?;
        let now = Utc::now().to_rfc3339();
        let changed=tx.execute(
            "UPDATE entities SET title=?3,version=version+1,updated_at=?4,updated_by=?5 WHERE id=?1 AND project_id=?2 AND version=?6",
            params![entity_id,self.manifest.project_id,update.title,now,command.actor.id,update.expected_version])?;
        if changed != 1 {
            return Err(CoreError::Conflict(
                "Evidence version is stale or missing".into(),
            ));
        }
        tx.execute("UPDATE evidence SET annotation_text=?2,summary_text=?3,relevance=?4 WHERE entity_id=?1",
            params![entity_id,update.annotation,update.summary,update.relevance])?;
        let body = format!(
            "{}\n{}\n{}\n{}\n{}",
            current.2.unwrap_or_default(),
            current.1.unwrap_or_default(),
            update.annotation,
            update.summary,
            update.relevance
        );
        upsert_search_document(
            &tx,
            &self.manifest.project_id,
            entity_id,
            "evidence",
            &update.title,
            &body,
            &now,
        )?;
        let sequence = append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(entity_id),
            event_type,
            &json!({"entity_id":entity_id,"title":update.title,"original_source_unchanged":true}),
        )?;
        insert_timeline(
            &tx,
            &self.manifest.project_id,
            sequence,
            None,
            Some(entity_id),
            event_type,
            &json!({"entity_id":entity_id}),
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            operation,
            Some(entity_id),
            Some(update.expected_version),
            &json!({"entity_id":entity_id}),
        )?;
        tx.commit()?;
        self.get_research_item(entity_id)
    }

    pub fn create_experiment(
        &self,
        command: &CommandContext,
        input: NewExperiment,
    ) -> Result<ResearchItem> {
        validate_nonempty(&input.method, MAX_RESEARCH_TEXT, "experiment method")?;
        validate_text(&input.hypothesis, 20_000, "experiment hypothesis")?;
        validate_nonempty(
            &input.expected_observations,
            MAX_RESEARCH_TEXT,
            "expected observations",
        )?;
        if input.question_id.is_none() && input.hypothesis.trim().is_empty() {
            return Err(CoreError::Validation(
                "Experiment requires a Research Question or hypothesis".into(),
            ));
        }
        bounded_json(&input.inputs, MAX_RESEARCH_JSON_BYTES, "experiment inputs")?;
        let body = format!(
            "{}\n{}\n{}",
            input.hypothesis, input.method, input.expected_observations
        );
        self.create_research_record(command,CreateSpec{
            operation:"CreateExperiment",event_type:"research.experiment.created",entity_type:"experiment",
            title:&input.title,status:"planned",origin:OriginKind::User,metadata:&input.metadata,
            session_id:input.session_id.as_deref(),search_body:body,
            event_details:json!({"question_id":input.question_id}),
        },|tx,id|{
            let inputs=bounded_json(&input.inputs,MAX_RESEARCH_JSON_BYTES,"experiment inputs")?;
            tx.execute("INSERT INTO experiments(entity_id,hypothesis,method,inputs_json,expected_observations)
                        VALUES(?1,?2,?3,?4,?5)",params![id,input.hypothesis,input.method,inputs,input.expected_observations])?;
            if let Some(question_id)=&input.question_id {
                require_entity_type(tx,&self.manifest.project_id,question_id,"research_question")?;
                insert_relationship(tx,&self.manifest.project_id,command,id,"experiment",question_id,
                    "research_question","addresses",OriginKind::User,&[])?;
            }
            Ok(())
        })
    }

    pub fn create_research_result(
        &self,
        command: &CommandContext,
        input: NewResearchResult,
    ) -> Result<ResearchItem> {
        validate_nonempty(&input.observation, MAX_RESEARCH_TEXT, "result observation")?;
        bounded_json(
            &input.measurements,
            MAX_RESEARCH_JSON_BYTES,
            "result measurements",
        )?;
        let observed_at = validated_time_or_now(input.observed_at.as_deref(), "observed_at")?;
        let body = format!("{}\n{}", input.observation, input.outcome.as_str());
        self.create_research_record(command,CreateSpec{
            operation:"CreateResearchResult",event_type:"research.result.recorded",entity_type:"result",
            title:&input.title,status:"recorded",origin:OriginKind::User,metadata:&input.metadata,
            session_id:input.session_id.as_deref(),search_body:body,
            event_details:json!({"experiment_id":input.experiment_id,"artifact_id":input.artifact_id}),
        },|tx,id|{
            let experiment_status=research_entity_status(tx,&self.manifest.project_id,&input.experiment_id,"experiment")?;
            if !matches!(experiment_status.as_str(),"running"|"completed"|"failed") {
                return Err(CoreError::Conflict("Result requires a running, completed, or failed Experiment".into()));
            }
            if let Some(artifact_id)=&input.artifact_id { require_available_artifact(tx,&self.manifest.project_id,artifact_id)?; }
            let measurements=bounded_json(&input.measurements,MAX_RESEARCH_JSON_BYTES,"result measurements")?;
            tx.execute("INSERT INTO results(entity_id,experiment_id,observation_text,outcome,measurements_json,observed_at,artifact_id)
                        VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,input.experiment_id,input.observation,
                        input.outcome.as_str(),measurements,observed_at,input.artifact_id])?;
            if let Some(artifact_id)=&input.artifact_id {
                tx.execute("INSERT INTO entity_artifacts(entity_id,artifact_id,role,created_at) VALUES(?1,?2,'result_output',?3)",
                    params![id,artifact_id,Utc::now().to_rfc3339()])?;
            }
            insert_relationship(tx,&self.manifest.project_id,command,&input.experiment_id,"experiment",id,"result",
                "produces",OriginKind::User,&[])?;
            Ok(())
        })
    }

    pub fn create_finding(
        &self,
        command: &CommandContext,
        input: NewFinding,
    ) -> Result<ResearchItem> {
        validate_nonempty(&input.claim, 20_000, "finding claim")?;
        validate_nonempty(
            &input.interpretation,
            MAX_RESEARCH_TEXT,
            "finding interpretation",
        )?;
        validate_text(&input.uncertainty, 20_000, "finding uncertainty")?;
        validate_confidence(input.confidence)?;
        if input.sources.len() > 100 {
            return Err(CoreError::Validation(
                "a Finding may reference at most 100 direct sources".into(),
            ));
        }
        let mut unique = HashSet::new();
        if !input
            .sources
            .iter()
            .all(|source| unique.insert((source.entity_id.clone(), source.assessment.as_str())))
        {
            return Err(CoreError::Validation(
                "duplicate Finding source assessment".into(),
            ));
        }
        let body = format!(
            "{}\n{}\n{}",
            input.claim, input.interpretation, input.uncertainty
        );
        self.create_research_record(command,CreateSpec{
            operation:"CreateFinding",event_type:"research.finding.created",entity_type:"finding",title:&input.title,
            status:"candidate",origin:OriginKind::User,metadata:&input.metadata,session_id:input.session_id.as_deref(),search_body:body,
            event_details:json!({"sources":input.sources,"answers_question_id":input.answers_question_id}),
        },|tx,id|{
            tx.execute("INSERT INTO findings(entity_id,claim,interpretation,uncertainty,confidence) VALUES(?1,?2,?3,?4,?5)",
                params![id,input.claim,input.interpretation,input.uncertainty,input.confidence])?;
            for source in &input.sources {
                let source_type=entity_type(tx,&self.manifest.project_id,&source.entity_id)?;
                insert_relationship(tx,&self.manifest.project_id,command,&source.entity_id,&source_type,id,"finding",
                    source.assessment.as_str(),OriginKind::User,std::slice::from_ref(&source.entity_id))?;
            }
            if let Some(question_id)=&input.answers_question_id {
                require_entity_type(tx,&self.manifest.project_id,question_id,"research_question")?;
                insert_relationship(tx,&self.manifest.project_id,command,id,"finding",question_id,"research_question",
                    "answers",OriginKind::User,&input.sources.iter().map(|s|s.entity_id.clone()).collect::<Vec<_>>())?;
            }
            Ok(())
        })
    }

    pub fn create_decision(
        &self,
        command: &CommandContext,
        input: NewDecision,
    ) -> Result<ResearchItem> {
        validate_nonempty(&input.selected_option, 10_000, "selected option")?;
        validate_nonempty(&input.rationale, MAX_RESEARCH_TEXT, "decision rationale")?;
        validate_string_list(&input.alternatives, "decision alternatives")?;
        validate_string_list(&input.constraints, "decision constraints")?;
        if input.finding_ids.len() > 100 {
            return Err(CoreError::Validation(
                "a Decision may reference at most 100 Findings".into(),
            ));
        }
        let mut findings = HashSet::new();
        if !input.finding_ids.iter().all(|id| findings.insert(id)) {
            return Err(CoreError::Validation("duplicate Decision Finding".into()));
        }
        if input.supersedes_decision_id.is_some() != input.supersedes_expected_version.is_some() {
            return Err(CoreError::Validation(
                "superseded Decision ID and expected version must be provided together".into(),
            ));
        }
        let body = format!(
            "{}\n{}\n{}\n{}",
            input.selected_option,
            input.rationale,
            input.alternatives.join("\n"),
            input.constraints.join("\n")
        );
        self.create_research_record(command,CreateSpec{
            operation:"CreateDecision",event_type:"research.decision.created",entity_type:"decision",title:&input.title,
            status:"proposed",origin:OriginKind::User,metadata:&input.metadata,session_id:input.session_id.as_deref(),search_body:body,
            event_details:json!({"finding_ids":input.finding_ids,"supersedes_decision_id":input.supersedes_decision_id,
                "supersedes_expected_version":input.supersedes_expected_version}),
        },|tx,id|{
            let alternatives=bounded_json(&json!(input.alternatives),MAX_RESEARCH_JSON_BYTES,"decision alternatives")?;
            let constraints=bounded_json(&json!(input.constraints),MAX_RESEARCH_JSON_BYTES,"decision constraints")?;
            tx.execute("INSERT INTO decisions(entity_id,selected_option,rationale,alternatives_json,constraints_json)
                        VALUES(?1,?2,?3,?4,?5)",params![id,input.selected_option,input.rationale,alternatives,constraints])?;
            for finding_id in &input.finding_ids {
                require_entity_type(tx,&self.manifest.project_id,finding_id,"finding")?;
                insert_relationship(tx,&self.manifest.project_id,command,finding_id,"finding",id,"decision","informs",
                    OriginKind::User,std::slice::from_ref(finding_id))?;
            }
            if let Some(old_id)=&input.supersedes_decision_id {
                let (old_status,old_version):(String,i64)=tx.query_row("SELECT status,version FROM entities
                    WHERE id=?1 AND project_id=?2 AND entity_type='decision'",params![old_id,self.manifest.project_id],
                    |row|Ok((row.get(0)?,row.get(1)?))).optional()?.ok_or_else(||CoreError::NotFound(format!("decision {old_id}")))?;
                if !matches!(old_status.as_str(),"proposed"|"accepted"|"rejected") {
                    return Err(CoreError::Conflict("only an open or decided Decision can be superseded".into()));
                }
                if Some(old_version)!=input.supersedes_expected_version {
                    return Err(CoreError::Conflict("superseded Decision version is stale".into()));
                }
                insert_relationship(tx,&self.manifest.project_id,command,id,"decision",old_id,"decision","supersedes",
                    OriginKind::User,&input.finding_ids)?;
                let changed=tx.execute("UPDATE entities SET status='superseded',version=version+1,updated_at=?3,updated_by=?4
                            WHERE id=?1 AND project_id=?2 AND version=?5",params![old_id,self.manifest.project_id,
                            Utc::now().to_rfc3339(),command.actor.id,old_version])?;
                if changed!=1 { return Err(CoreError::Conflict("superseded Decision changed concurrently".into())); }
            }
            Ok(())
        })
    }

    pub fn create_requirement(
        &self,
        command: &CommandContext,
        input: NewRequirement,
    ) -> Result<ResearchItem> {
        validate_nonempty(&input.statement, 20_000, "requirement statement")?;
        validate_priority(input.priority)?;
        validate_string_list(&input.acceptance_criteria, "acceptance criteria")?;
        validate_text(
            &input.verification_method,
            10_000,
            "requirement verification method",
        )?;
        if input.rationale_origin == RequirementRationaleOrigin::Research
            && input.decision_id.is_none()
        {
            return Err(CoreError::Validation(
                "research-origin Requirement requires a Decision".into(),
            ));
        }
        let origin = match input.rationale_origin {
            RequirementRationaleOrigin::Research | RequirementRationaleOrigin::User => {
                OriginKind::User
            }
            RequirementRationaleOrigin::Import => OriginKind::Import,
            RequirementRationaleOrigin::External => OriginKind::External,
            RequirementRationaleOrigin::Legacy => OriginKind::Legacy,
            RequirementRationaleOrigin::Unknown => OriginKind::Unknown,
        };
        let body = format!(
            "{}\n{}\n{}",
            input.statement,
            input.acceptance_criteria.join("\n"),
            input.verification_method
        );
        self.create_research_record(command,CreateSpec{
            operation:"CreateRequirement",event_type:"research.requirement.created",entity_type:"requirement",title:&input.title,
            status:"draft",origin,metadata:&input.metadata,session_id:input.session_id.as_deref(),search_body:body,
            event_details:json!({"decision_id":input.decision_id,"rationale_origin":input.rationale_origin.as_str()}),
        },|tx,id|{
            let criteria=bounded_json(&json!(input.acceptance_criteria),MAX_RESEARCH_JSON_BYTES,"acceptance criteria")?;
            tx.execute("INSERT INTO requirements(entity_id,statement,acceptance_criteria_json,priority,rationale_origin,verification_method,created_in_space)
                        VALUES(?1,?2,?3,?4,?5,?6,'research')",params![id,input.statement,criteria,input.priority,
                        input.rationale_origin.as_str(),input.verification_method])?;
            if let Some(decision_id)=&input.decision_id {
                require_entity_type(tx,&self.manifest.project_id,decision_id,"decision")?;
                if input.rationale_origin == RequirementRationaleOrigin::Research {
                    let status = research_entity_status(
                        tx,
                        &self.manifest.project_id,
                        decision_id,
                        "decision",
                    )?;
                    if status != "accepted" {
                        return Err(CoreError::Validation(
                            "research-origin Requirement requires an accepted Decision".into(),
                        ));
                    }
                }
                insert_relationship(tx,&self.manifest.project_id,command,decision_id,"decision",id,"requirement","creates",origin,
                    std::slice::from_ref(decision_id))?;
            }
            Ok(())
        })
    }

    pub fn transition_research_item(
        &self,
        command: &CommandContext,
        entity_id: &str,
        expected_version: i64,
        target_status: &str,
        note: Option<&str>,
    ) -> Result<ResearchItem> {
        validate_command_context(command)?;
        validate_nonempty(target_status, 50, "target status")?;
        if let Some(note) = note
            && note.chars().count() > 10_000
        {
            return Err(CoreError::Validation(
                "transition note exceeds 10000 characters".into(),
            ));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_research_enabled(&tx, &self.manifest.project_id)?;
        if let Some(result) = prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "TransitionResearchItem",
        )? {
            tx.commit()?;
            return self.get_research_item(&result.unwrap_or_else(|| entity_id.to_owned()));
        }
        let (kind, current) = tx
            .query_row(
                "SELECT entity_type,status FROM entities WHERE id=?1 AND project_id=?2",
                params![entity_id, self.manifest.project_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(entity_id.into()))?;
        if !is_reserved_domain_entity_type(&kind) {
            return Err(CoreError::Validation(
                "entity is not owned by Research Space".into(),
            ));
        }
        validate_lifecycle_transition(&kind, &current, target_status)?;
        if kind == "evidence" && matches!(target_status, "available" | "purged_payload") {
            let artifact_state: Option<Option<String>> = tx
                .query_row(
                    "SELECT a.availability FROM evidence ev LEFT JOIN artifacts a
                     ON a.id=ev.original_artifact_id WHERE ev.entity_id=?1",
                    [entity_id],
                    |row| row.get(0),
                )
                .optional()?;
            match (target_status, artifact_state.flatten().as_deref()) {
                ("available", Some("available")) | ("available", None) => {}
                ("purged_payload", Some("purged_payload")) => {}
                ("purged_payload", _) => {
                    return Err(CoreError::Conflict(
                        "Evidence can enter purged_payload only after its original artifact payload is purged"
                            .into(),
                    ));
                }
                _ => {
                    return Err(CoreError::Conflict(
                        "Evidence cannot be available while its original artifact is unavailable"
                            .into(),
                    ));
                }
            }
        }
        if kind == "finding" && target_status == "accepted" {
            let supports: i64 = tx.query_row(
                "SELECT count(*) FROM relationships WHERE project_id=?1 AND target_entity_id=?2
                AND target_entity_type='finding' AND relation_type='supports' AND status='active'",
                params![self.manifest.project_id, entity_id],
                |row| row.get(0),
            )?;
            if supports == 0 {
                return Err(CoreError::Validation(
                    "accepted Finding requires at least one supporting Evidence or Result".into(),
                ));
            }
        }
        let now = Utc::now().to_rfc3339();
        let changed = tx.execute(
            "UPDATE entities SET status=?3,version=version+1,updated_at=?4,updated_by=?5,
                    archived_at=CASE WHEN ?3='archived' THEN ?4 ELSE archived_at END
                    WHERE id=?1 AND project_id=?2 AND version=?6",
            params![
                entity_id,
                self.manifest.project_id,
                target_status,
                now,
                command.actor.id,
                expected_version
            ],
        )?;
        if changed != 1 {
            return Err(CoreError::Conflict(format!(
                "{kind} version is stale or missing"
            )));
        }
        match kind.as_str() {
            "research_session" if matches!(target_status, "completed" | "cancelled") => {
                tx.execute("UPDATE research_sessions SET ended_at=?2,completion_note=?3 WHERE entity_id=?1",
                    params![entity_id,now,note])?;
            }
            "experiment" if target_status == "running" => {
                tx.execute(
                    "UPDATE experiments SET started_at=?2 WHERE entity_id=?1",
                    params![entity_id, now],
                )?;
            }
            "experiment" if matches!(target_status, "completed" | "failed" | "cancelled") => {
                tx.execute(
                    "UPDATE experiments SET ended_at=?2 WHERE entity_id=?1",
                    params![entity_id, now],
                )?;
            }
            "finding" if target_status == "accepted" => {
                tx.execute(
                    "UPDATE findings SET accepted_at=?2 WHERE entity_id=?1",
                    params![entity_id, now],
                )?;
            }
            "decision" if matches!(target_status, "accepted" | "rejected") => {
                tx.execute(
                    "UPDATE decisions SET decided_at=?2 WHERE entity_id=?1",
                    params![entity_id, now],
                )?;
            }
            _ => {}
        }
        let event_type = format!("research.{kind}.status_changed");
        let payload = json!({"entity_id":entity_id,"from":current,"to":target_status,"note":note});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(entity_id),
            &event_type,
            &payload,
        )?;
        insert_timeline(
            &tx,
            &self.manifest.project_id,
            sequence,
            None,
            Some(entity_id),
            &event_type,
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "TransitionResearchItem",
            Some(entity_id),
            Some(expected_version),
            &json!({"entity_id":entity_id,"target_status":target_status}),
        )?;
        tx.commit()?;
        self.get_research_item(entity_id)
    }

    pub fn link_research_entities(
        &self,
        command: &CommandContext,
        source_id: &str,
        relation_type: &str,
        target_id: &str,
        direct_source_ids: &[String],
    ) -> Result<String> {
        validate_command_context(command)?;
        if direct_source_ids.len() > 100 {
            return Err(CoreError::Validation(
                "a Research relationship may cite at most 100 direct sources".into(),
            ));
        }
        let mut unique_sources = HashSet::new();
        if !direct_source_ids.iter().all(|id| unique_sources.insert(id)) {
            return Err(CoreError::Validation(
                "duplicate direct source on Research relationship".into(),
            ));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_research_enabled(&tx, &self.manifest.project_id)?;
        if let Some(result) = prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "LinkResearchEntities",
        )? {
            return result
                .ok_or_else(|| CoreError::Conflict("command has no relationship result".into()));
        }
        let source_type = entity_type(&tx, &self.manifest.project_id, source_id)?;
        let target_type = entity_type(&tx, &self.manifest.project_id, target_id)?;
        if relation_type == "supersedes"
            && supersedes_would_cycle(&tx, &self.manifest.project_id, source_id, target_id)?
        {
            return Err(CoreError::Validation(
                "supersedes relationship would create a cycle".into(),
            ));
        }
        let id = insert_relationship(
            &tx,
            &self.manifest.project_id,
            command,
            source_id,
            &source_type,
            target_id,
            &target_type,
            relation_type,
            OriginKind::User,
            direct_source_ids,
        )?;
        let payload = json!({"relationship_id":id,"source":source_id,"relation_type":relation_type,"target":target_id});
        let now = Utc::now().to_rfc3339();
        let sequence = append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&id),
            "research.relationship.created",
            &payload,
        )?;
        insert_timeline(
            &tx,
            &self.manifest.project_id,
            sequence,
            None,
            Some(source_id),
            "research.relationship.created",
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "LinkResearchEntities",
            Some(&id),
            None,
            &payload,
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn associate_research_item_with_session(
        &self,
        command: &CommandContext,
        session_id: &str,
        entity_id: &str,
    ) -> Result<()> {
        validate_command_context(command)?;
        if session_id == entity_id {
            return Err(CoreError::Validation(
                "a Research Session cannot contain itself".into(),
            ));
        }
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_research_enabled(&tx, &self.manifest.project_id)?;
        if prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "AssociateResearchItemWithSession",
        )?
        .is_some()
        {
            return tx.commit().map_err(Into::into);
        }
        require_active_session(&tx, &self.manifest.project_id, session_id)?;
        let item_type = entity_type(&tx, &self.manifest.project_id, entity_id)?;
        if !is_reserved_domain_entity_type(&item_type) || item_type == "research_session" {
            return Err(CoreError::Validation(
                "only a non-session Research item can be associated with a Research Session".into(),
            ));
        }
        let now = Utc::now().to_rfc3339();
        let changed = tx.execute(
            "INSERT OR IGNORE INTO research_session_items(session_entity_id,entity_id,added_at,added_by)
             VALUES(?1,?2,?3,?4)",
            params![session_id, entity_id, now, command.actor.id],
        )?;
        if changed == 1 {
            let payload =
                json!({"session_id":session_id,"entity_id":entity_id,"entity_type":item_type});
            let sequence = append_event_with_context(
                &tx,
                &self.manifest.project_id,
                command,
                Some(entity_id),
                "research.session.item_associated",
                &payload,
            )?;
            insert_timeline(
                &tx,
                &self.manifest.project_id,
                sequence,
                Some(session_id),
                Some(entity_id),
                "research.session.item_associated",
                &payload,
                &command.actor.id,
                &now,
            )?;
        }
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "AssociateResearchItemWithSession",
            Some(entity_id),
            None,
            &json!({"session_id":session_id,"entity_id":entity_id,"already_associated":changed==0}),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn get_research_item(&self, entity_id: &str) -> Result<ResearchItem> {
        let entity = self.get_entity(entity_id)?;
        if !is_reserved_domain_entity_type(&entity.entity_type) {
            return Err(CoreError::Validation(
                "entity is not a Research Space record".into(),
            ));
        }
        let connection = self.connection()?;
        let details = research_details(&connection, &entity)?;
        Ok(ResearchItem { entity, details })
    }

    pub fn search_research(&self, query: ResearchSearchQuery) -> Result<ResearchSearchPage> {
        validate_page(query.page)?;
        if query.text.chars().count() > 500 {
            return Err(CoreError::Validation(
                "research search text exceeds 500 characters".into(),
            ));
        }
        let type_filter = query.entity_type.map(|value| value.as_str());
        let limit = query.page.limit + 1;
        let offset = i64::try_from(query.page.offset)
            .map_err(|_| CoreError::Validation("research search offset is too large".into()))?;
        let connection = self.connection()?;
        if let Some(session) = &query.session_id {
            require_entity_type(
                &connection,
                &self.manifest.project_id,
                session,
                "research_session",
            )?;
        }
        let mut statement = connection.prepare(
            "SELECT d.entity_id,d.entity_type,e.title,e.status,substr(d.body,1,300)
             FROM research_search_documents d JOIN entities e ON e.id=d.entity_id
             WHERE d.project_id=?1
               AND (?2='' OR instr(lower(d.title||' '||d.body),lower(?2))>0)
               AND (?3 IS NULL OR d.entity_type=?3)
               AND (?4 IS NULL OR e.status=?4)
               AND (?5 IS NULL OR EXISTS(SELECT 1 FROM research_session_items si
                   WHERE si.entity_id=d.entity_id AND si.session_entity_id=?5))
             ORDER BY e.updated_at DESC,e.id LIMIT ?6 OFFSET ?7",
        )?;
        let mut items = statement
            .query_map(
                params![
                    self.manifest.project_id,
                    query.text,
                    type_filter,
                    query.status,
                    query.session_id,
                    limit,
                    offset
                ],
                |row| {
                    Ok(ResearchSearchHit {
                        entity_id: row.get(0)?,
                        entity_type: row.get(1)?,
                        title: row.get(2)?,
                        status: row.get(3)?,
                        snippet: row.get(4)?,
                    })
                },
            )?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let has_more = items.len() > query.page.limit as usize;
        items.truncate(query.page.limit as usize);
        Ok(ResearchSearchPage {
            next_offset: has_more.then_some(query.page.offset + items.len() as u64),
            items,
        })
    }

    pub fn research_timeline(
        &self,
        filter: ResearchTimelineFilter,
        page: PageRequest,
    ) -> Result<ResearchTimelinePage> {
        validate_page(page)?;
        let limit = page.limit + 1;
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("timeline offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement=connection.prepare(
            "SELECT id,ledger_sequence,session_entity_id,entity_id,event_type,payload_json,actor_id,occurred_at
             FROM research_timeline t WHERE project_id=?1
               AND (?2 IS NULL OR session_entity_id=?2 OR EXISTS(
                   SELECT 1 FROM research_session_items si
                   WHERE si.entity_id=t.entity_id AND si.session_entity_id=?2))
               AND (?3 IS NULL OR entity_id=?3)
               AND (?4 IS NULL OR event_type=?4)
             ORDER BY ledger_sequence,id LIMIT ?5 OFFSET ?6")?;
        let raw = statement
            .query_map(
                params![
                    self.manifest.project_id,
                    filter.session_id,
                    filter.entity_id,
                    filter.event_type,
                    limit,
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
        let has_more = raw.len() > page.limit as usize;
        let items = raw
            .into_iter()
            .take(page.limit as usize)
            .map(|row| {
                Ok(ResearchTimelineEntry {
                    id: row.0,
                    ledger_sequence: row.1,
                    session_id: row.2,
                    entity_id: row.3,
                    event_type: row.4,
                    payload: serde_json::from_str(&row.5)?,
                    actor_id: row.6,
                    occurred_at: row.7,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ResearchTimelinePage {
            next_offset: has_more.then_some(page.offset + items.len() as u64),
            items,
        })
    }

    pub fn create_research_checkpoint(
        &self,
        command: &CommandContext,
        input: ResearchCheckpointInput,
    ) -> Result<Checkpoint> {
        validate_command_context(command)?;
        if input.note.chars().count() > 20_000 {
            return Err(CoreError::Validation(
                "checkpoint note exceeds 20000 characters".into(),
            ));
        }
        validate_string_list(&input.next_actions, "checkpoint next actions")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_research_enabled(&tx, &self.manifest.project_id)?;
        if let Some(Some(id)) = prior_result(
            &tx,
            &self.manifest.project_id,
            command,
            "CreateResearchCheckpoint",
        )? {
            tx.commit()?;
            return self.get_checkpoint(&id);
        }
        let mut statement=tx.prepare("SELECT e.id,e.entity_type,e.status,e.version FROM entities e
            LEFT JOIN requirements req ON req.entity_id=e.id WHERE e.project_id=?1
            AND (e.entity_type IN ('research_session','research_question','evidence','experiment','result','finding','decision')
                 OR (e.entity_type='requirement' AND req.created_in_space='research'))
            AND e.status<>'archived' ORDER BY e.entity_type,e.created_at,e.id")?;
        let records = statement
            .query_map([&self.manifest.project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        drop(statement);
        if records.len() > MAX_CHECKPOINT_SOURCES {
            return Err(CoreError::Conflict(format!(
                "research checkpoint has {} sources; narrow or archive state before exceeding {MAX_CHECKPOINT_SOURCES}",
                records.len()
            )));
        }
        let sequence: i64 = tx.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [&self.manifest.project_id],
            |row| row.get(0),
        )?;
        let mut counts = serde_json::Map::new();
        let mut unresolved = Vec::new();
        for (id, kind, status, _) in &records {
            *counts.entry(kind.clone()).or_insert(json!(0)) =
                json!(counts.get(kind).and_then(Value::as_u64).unwrap_or(0) + 1);
            if is_unresolved(kind, status) {
                unresolved.push(json!({"id":id,"type":kind,"status":status}));
            }
        }
        let summary = json!({"schema_version":1,"scope":"research","source_ledger_sequence":sequence,
            "note":input.note,"counts":counts,"unresolved":unresolved,"next_actions":input.next_actions,
            "development_required":false,"complete":false});
        let summary_json = bounded_json(&summary, 2 * 1024 * 1024, "research checkpoint summary")?;
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        tx.execute("INSERT INTO checkpoints(id,project_id,scope,ledger_sequence,summary_json,created_at,created_by)
                    VALUES(?1,?2,'research',?3,?4,?5,?6)",params![id,self.manifest.project_id,sequence,summary_json,now,command.actor.id])?;
        for (source, _, _, version) in &records {
            tx.execute("INSERT INTO checkpoint_sources(checkpoint_id,source_entity_id,source_version) VALUES(?1,?2,?3)",params![id,source,version])?;
        }
        let event_sequence = append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&id),
            "research.checkpoint.created",
            &json!({"checkpoint_id":id,"source_ledger_sequence":sequence,"source_count":records.len()}),
        )?;
        insert_timeline(
            &tx,
            &self.manifest.project_id,
            event_sequence,
            None,
            None,
            "research.checkpoint.created",
            &json!({"checkpoint_id":id}),
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            "CreateResearchCheckpoint",
            Some(&id),
            None,
            &json!({"source_count":records.len(),"next_action_count":input.next_actions.len()}),
        )?;
        tx.commit()?;
        self.get_checkpoint(&id)
    }

    pub fn resume_research(&self) -> Result<ResearchResumeState> {
        let connection = self.connection()?;
        let raw: Option<(String, i64, String)> = connection
            .query_row(
                "SELECT id,ledger_sequence,summary_json FROM checkpoints
            WHERE project_id=?1 AND scope='research' ORDER BY created_at DESC,id DESC LIMIT 1",
                [&self.manifest.project_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let (checkpoint, checkpoint_sequence, next_actions) =
            if let Some((id, sequence, summary_json)) = raw {
                let summary: Value = serde_json::from_str(&summary_json)?;
                let next = summary
                    .get("next_actions")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .filter_map(Value::as_str)
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default();
                (Some(self.get_checkpoint(&id)?), sequence, next)
            } else {
                (None, 0, Vec::new())
            };
        let events: i64 = connection.query_row(
            "SELECT count(*) FROM research_timeline WHERE project_id=?1 AND ledger_sequence>?2
             AND event_type<>'research.checkpoint.created'",
            params![self.manifest.project_id, checkpoint_sequence],
            |row| row.get(0),
        )?;
        let (active_items, active_item_count) =
            unresolved_research_hits(&connection, &self.manifest.project_id, 100)?;
        let checkpoint_is_stale = checkpoint.is_some() && events > 0;
        Ok(ResearchResumeState {
            checkpoint,
            checkpoint_is_stale,
            events_since_checkpoint: events as u64,
            active_items_truncated: active_item_count > active_items.len() as u64,
            active_item_count,
            active_items,
            next_actions,
        })
    }

    pub fn generate_research_report(&self) -> Result<ResearchReport> {
        let connection = self.connection()?;
        let sequence: i64 = connection.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [&self.manifest.project_id],
            |row| row.get(0),
        )?;
        let mut statement=connection.prepare("SELECT e.id,e.entity_type,e.title,e.status FROM entities e
            LEFT JOIN requirements req ON req.entity_id=e.id WHERE e.project_id=?1
            AND (e.entity_type IN ('research_session','research_question','evidence','experiment','result','finding','decision')
                 OR (e.entity_type='requirement' AND req.created_in_space='research'))
            AND e.status<>'archived' ORDER BY e.entity_type,e.created_at,e.id LIMIT 5001")?;
        let rows = statement
            .query_map([&self.manifest.project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        if rows.len() > 5000 {
            return Err(CoreError::Conflict(
                "research report exceeds 5000 records; use a scoped report in CP8".into(),
            ));
        }
        let mut markdown = String::from("# Research Report\n\n");
        writeln!(markdown, "Project: `{}`  ", self.manifest.project_id).unwrap();
        writeln!(markdown, "Ledger position: `{sequence}`\n").unwrap();
        let sections = [
            ("research_question", "Research Questions"),
            ("evidence", "Evidence"),
            ("experiment", "Experiments"),
            ("result", "Results"),
            ("finding", "Findings"),
            ("decision", "Decisions"),
            ("requirement", "Requirements / Optional Handoff"),
        ];
        for (kind, label) in sections {
            writeln!(markdown, "## {label}\n").unwrap();
            let matching: Vec<_> = rows.iter().filter(|row| row.1 == kind).collect();
            if matching.is_empty() {
                markdown.push_str("_No records._\n\n");
                continue;
            }
            for row in matching {
                writeln!(
                    markdown,
                    "- {} — **{}** (`{}`)",
                    report_excerpt(&row.2, 500),
                    row.3,
                    row.0
                )
                .unwrap();
                let detail =
                    research_report_detail(&connection, &self.manifest.project_id, &row.0, &row.1)?;
                if !detail.is_empty() {
                    writeln!(markdown, "  {}", report_excerpt(&detail, 3_000)).unwrap();
                }
            }
            markdown.push('\n');
        }
        markdown.push_str("## Provenance Note\n\nEvery identifier above resolves to canonical local state. Findings must be inspected with their typed source relationships; absence of a link is not treated as evidence.\n");
        let cited_entity_ids = rows
            .iter()
            .filter(|row| {
                matches!(
                    row.1.as_str(),
                    "evidence" | "result" | "finding" | "decision"
                )
            })
            .map(|row| row.0.clone())
            .collect();
        Ok(ResearchReport {
            project_id: self.manifest.project_id.clone(),
            source_ledger_sequence: sequence,
            generated_at: Utc::now().to_rfc3339(),
            markdown,
            cited_entity_ids,
        })
    }

    fn create_research_record<F>(
        &self,
        command: &CommandContext,
        spec: CreateSpec<'_>,
        insert_details: F,
    ) -> Result<ResearchItem>
    where
        F: FnOnce(&Transaction<'_>, &str) -> Result<()>,
    {
        validate_command_context(command)?;
        validate_nonempty(spec.title, 500, "title")?;
        if spec.origin == OriginKind::AiProposal {
            return Err(CoreError::Validation(
                "AI proposal origin cannot write canonical Research state".into(),
            ));
        }
        let metadata = bounded_json(spec.metadata, MAX_RESEARCH_JSON_BYTES, "research metadata")?;
        let mut connection = self.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_research_enabled(&tx, &self.manifest.project_id)?;
        if let Some(result) = prior_result(&tx, &self.manifest.project_id, command, spec.operation)?
        {
            tx.commit()?;
            return self.get_research_item(
                &result.ok_or_else(|| {
                    CoreError::Conflict("research command has no result ID".into())
                })?,
            );
        }
        if let Some(session) = spec.session_id {
            require_active_session(&tx, &self.manifest.project_id, session)?;
        }
        let id = new_id();
        let now = Utc::now().to_rfc3339();
        tx.execute("INSERT INTO entities(id,project_id,entity_type,title,status,version,legacy_origin,data_json,created_at,updated_at,
                    entity_schema_version,origin_type,metadata_json,created_by,updated_by)
                    VALUES(?1,?2,?3,?4,?5,1,?6,'{}',?7,?7,1,?8,?9,?10,?10)",
            params![id,self.manifest.project_id,spec.entity_type,spec.title,spec.status,legacy_origin(spec.origin),now,
                spec.origin.as_str(),metadata,command.actor.id])?;
        insert_details(&tx, &id)?;
        if let Some(session) = spec.session_id {
            tx.execute("INSERT INTO research_session_items(session_entity_id,entity_id,added_at,added_by) VALUES(?1,?2,?3,?4)",
                params![session,id,now,command.actor.id])?;
        }
        upsert_search_document(
            &tx,
            &self.manifest.project_id,
            &id,
            spec.entity_type,
            spec.title,
            &spec.search_body,
            &now,
        )?;
        let payload = json!({"entity_id":id,"entity_type":spec.entity_type,
            "session_id":spec.session_id,"details":spec.event_details});
        let sequence = append_event_with_context(
            &tx,
            &self.manifest.project_id,
            command,
            Some(&id),
            spec.event_type,
            &payload,
        )?;
        insert_timeline(
            &tx,
            &self.manifest.project_id,
            sequence,
            spec.session_id,
            Some(&id),
            spec.event_type,
            &payload,
            &command.actor.id,
            &now,
        )?;
        record_command_with_context(
            &tx,
            command,
            &self.manifest.project_id,
            spec.operation,
            Some(&id),
            None,
            &json!({"entity_type":spec.entity_type,"schema_version":1,"details":spec.event_details}),
        )?;
        tx.commit()?;
        self.get_research_item(&id)
    }
}

fn validated_time_or_now(value: Option<&str>, label: &str) -> Result<String> {
    match value {
        Some(value) => {
            validate_optional_time(Some(value), label)?;
            Ok(value.to_owned())
        }
        None => Ok(Utc::now().to_rfc3339()),
    }
}

fn validate_optional_time(value: Option<&str>, label: &str) -> Result<()> {
    if let Some(value) = value {
        chrono::DateTime::parse_from_rfc3339(value)
            .map_err(|_| CoreError::Validation(format!("{label} must be RFC3339")))?;
    }
    Ok(())
}

fn validate_priority(priority: u8) -> Result<()> {
    if priority > 4 {
        Err(CoreError::Validation(
            "priority must be within 0..=4".into(),
        ))
    } else {
        Ok(())
    }
}

fn validate_confidence(confidence: Option<f64>) -> Result<()> {
    if confidence.is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value)) {
        Err(CoreError::Validation(
            "confidence must be finite and within 0.0..=1.0".into(),
        ))
    } else {
        Ok(())
    }
}

fn validate_string_list(items: &[String], label: &str) -> Result<()> {
    if items.len() > 100 {
        return Err(CoreError::Validation(format!("{label} exceeds 100 items")));
    }
    if items
        .iter()
        .any(|item| item.trim().is_empty() || item.chars().count() > 2_000)
    {
        return Err(CoreError::Validation(format!(
            "{label} items must contain 1..=2000 characters"
        )));
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

fn validate_optional_text(value: &Option<String>, max_chars: usize, label: &str) -> Result<()> {
    if let Some(value) = value {
        validate_text(value, max_chars, label)?;
    }
    Ok(())
}

fn optional_blank(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(|value| value.trim().is_empty())
}

fn validate_page(page: PageRequest) -> Result<()> {
    if page.limit == 0 || page.limit > 100 {
        Err(CoreError::Validation(
            "page limit must be within 1..=100".into(),
        ))
    } else {
        Ok(())
    }
}

fn require_research_enabled(connection: &Connection, project_id: &str) -> Result<()> {
    let state: (String, bool) = connection.query_row(
        "SELECT p.status,c.enabled FROM projects p JOIN space_capabilities c ON c.project_id=p.id
        WHERE p.id=?1 AND c.space='research'",
        [project_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    if state.0 != "active" {
        return Err(CoreError::Conflict("project is archived".into()));
    }
    if !state.1 {
        return Err(CoreError::Conflict(
            "Research Space is disabled for this project".into(),
        ));
    }
    Ok(())
}

fn require_active_session(
    connection: &Connection,
    project_id: &str,
    session_id: &str,
) -> Result<()> {
    let status = research_entity_status(connection, project_id, session_id, "research_session")?;
    if status != "active" {
        return Err(CoreError::Conflict("research session is not active".into()));
    }
    Ok(())
}

fn entity_type(connection: &Connection, project_id: &str, id: &str) -> Result<String> {
    connection
        .query_row(
            "SELECT entity_type FROM entities WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| CoreError::NotFound(id.into()))
}

fn require_entity_type(
    connection: &Connection,
    project_id: &str,
    id: &str,
    expected: &str,
) -> Result<()> {
    let actual = entity_type(connection, project_id, id)?;
    if actual != expected {
        return Err(CoreError::Validation(format!(
            "entity {id} must be {expected}, found {actual}"
        )));
    }
    Ok(())
}

fn research_entity_status(
    connection: &Connection,
    project_id: &str,
    id: &str,
    expected: &str,
) -> Result<String> {
    connection
        .query_row(
            "SELECT status FROM entities WHERE id=?1 AND project_id=?2 AND entity_type=?3",
            params![id, project_id, expected],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| CoreError::NotFound(format!("{expected} {id}")))
}

fn require_available_artifact(connection: &Connection, project_id: &str, id: &str) -> Result<()> {
    let availability: Option<String> = connection
        .query_row(
            "SELECT availability FROM artifacts WHERE id=?1 AND project_id=?2",
            params![id, project_id],
            |row| row.get(0),
        )
        .optional()?;
    match availability.as_deref() {
        Some(value) if value == ArtifactAvailability::Available.as_str() => Ok(()),
        Some(_) => Err(CoreError::Conflict(format!(
            "artifact {id} is not available"
        ))),
        None => Err(CoreError::NotFound(format!("artifact {id}"))),
    }
}

fn upsert_search_document(
    connection: &Connection,
    project_id: &str,
    entity_id: &str,
    entity_type: &str,
    title: &str,
    body: &str,
    now: &str,
) -> Result<()> {
    connection.execute("INSERT INTO research_search_documents(entity_id,project_id,entity_type,title,body,updated_at)
        VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(entity_id) DO UPDATE SET title=excluded.title,body=excluded.body,updated_at=excluded.updated_at",
        params![entity_id,project_id,entity_type,title,body,now])?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_timeline(
    connection: &Connection,
    project_id: &str,
    sequence: i64,
    session_id: Option<&str>,
    entity_id: Option<&str>,
    event_type: &str,
    payload: &Value,
    actor_id: &str,
    now: &str,
) -> Result<()> {
    let payload = bounded_json(
        payload,
        MAX_RESEARCH_JSON_BYTES,
        "research timeline payload",
    )?;
    connection.execute("INSERT INTO research_timeline(id,project_id,ledger_sequence,session_entity_id,entity_id,event_type,payload_json,actor_id,occurred_at)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",params![new_id(),project_id,sequence,session_id,entity_id,event_type,payload,actor_id,now])?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_relationship(
    connection: &Connection,
    project_id: &str,
    command: &CommandContext,
    source_id: &str,
    source_type: &str,
    target_id: &str,
    target_type: &str,
    relation_type: &str,
    origin: OriginKind,
    direct_sources: &[String],
) -> Result<String> {
    validate_research_pair(source_type, relation_type, target_type)?;
    if source_id == target_id {
        return Err(CoreError::Validation(
            "Research relationships cannot target themselves".into(),
        ));
    }
    for source in direct_sources {
        entity_type(connection, project_id, source)?;
    }
    let direct_sources_json = bounded_json(
        &json!(direct_sources),
        MAX_RESEARCH_JSON_BYTES,
        "relationship direct sources",
    )?;
    let id = new_id();
    let now = Utc::now().to_rfc3339();
    connection.execute("INSERT INTO relationships(id,project_id,relation_type,relation_version,source_entity_id,source_entity_type,
        target_entity_id,target_entity_type,status,origin_type,actor_id,confidence,review_state,direct_source_ids_json,supersedes_id,created_at,updated_at)
        VALUES(?1,?2,?3,1,?4,?5,?6,?7,'active',?8,?9,NULL,'accepted',?10,NULL,?11,?11)",
        params![id,project_id,relation_type,source_id,source_type,target_id,target_type,origin.as_str(),command.actor.id,direct_sources_json,now])?;
    Ok(id)
}

fn supersedes_would_cycle(
    connection: &Connection,
    project_id: &str,
    source_id: &str,
    target_id: &str,
) -> Result<bool> {
    if source_id == target_id {
        return Ok(true);
    }
    connection.query_row("WITH RECURSIVE chain(id) AS (
        SELECT target_entity_id FROM relationships WHERE project_id=?1 AND source_entity_id=?2 AND relation_type='supersedes' AND status='active'
        UNION SELECT r.target_entity_id FROM relationships r JOIN chain c ON r.source_entity_id=c.id
        WHERE r.project_id=?1 AND r.relation_type='supersedes' AND r.status='active')
        SELECT EXISTS(SELECT 1 FROM chain WHERE id=?3)",params![project_id,target_id,source_id],|row|row.get(0)).map_err(Into::into)
}

fn validate_lifecycle_transition(kind: &str, current: &str, target: &str) -> Result<()> {
    let allowed = match kind {
        "research_session" => matches!(
            (current, target),
            ("active", "completed" | "cancelled" | "archived")
                | ("completed" | "cancelled", "archived")
        ),
        "research_question" => matches!(
            (current, target),
            ("draft", "active" | "archived")
                | (
                    "active",
                    "answered" | "deferred" | "superseded" | "archived"
                )
                | ("deferred", "active" | "superseded" | "archived")
                | ("answered", "superseded" | "archived")
        ),
        "evidence" => matches!(
            (current, target),
            (
                "available",
                "unavailable" | "superseded" | "archived" | "purged_payload"
            ) | ("unavailable", "available" | "archived" | "purged_payload")
                | ("superseded", "archived")
                | ("purged_payload", "archived")
        ),
        "experiment" => matches!(
            (current, target),
            ("planned", "running" | "cancelled" | "archived")
                | ("running", "completed" | "failed" | "cancelled")
                | ("failed", "running" | "archived")
                | ("completed" | "cancelled", "archived")
        ),
        "result" => matches!(
            (current, target),
            ("recorded", "superseded" | "archived") | ("superseded", "archived")
        ),
        "finding" => matches!(
            (current, target),
            (
                "candidate",
                "accepted" | "challenged" | "rejected" | "archived"
            ) | ("accepted", "challenged" | "superseded" | "archived")
                | ("challenged", "accepted" | "superseded" | "archived")
                | ("rejected", "archived")
                | ("superseded", "archived")
        ),
        "decision" => matches!(
            (current, target),
            ("proposed", "accepted" | "rejected" | "archived")
                | ("accepted", "superseded" | "retired" | "archived")
                | ("rejected", "superseded" | "archived")
                | ("superseded" | "retired", "archived")
        ),
        "requirement" => matches!(
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
        ),
        _ => false,
    };
    if !allowed {
        return Err(CoreError::Validation(format!(
            "invalid {kind} lifecycle transition {current} -> {target}"
        )));
    }
    Ok(())
}

fn is_unresolved(kind: &str, status: &str) -> bool {
    match kind {
        "research_session" => status == "active",
        "research_question" => matches!(status, "draft" | "active" | "deferred"),
        "experiment" => matches!(status, "planned" | "running" | "failed"),
        "finding" => matches!(status, "candidate" | "challenged"),
        "decision" => status == "proposed",
        "requirement" => matches!(
            status,
            "draft" | "accepted" | "in_progress" | "blocked" | "implemented"
        ),
        _ => false,
    }
}

fn unresolved_research_hits(
    connection: &Connection,
    project_id: &str,
    limit: u32,
) -> Result<(Vec<ResearchSearchHit>, u64)> {
    let predicate = "(
        (e.entity_type='research_session' AND e.status='active') OR
        (e.entity_type='research_question' AND e.status IN ('draft','active','deferred')) OR
        (e.entity_type='experiment' AND e.status IN ('planned','running','failed')) OR
        (e.entity_type='finding' AND e.status IN ('candidate','challenged')) OR
        (e.entity_type='decision' AND e.status='proposed') OR
        (e.entity_type='requirement' AND e.status IN ('draft','accepted','in_progress','blocked','implemented')
          AND EXISTS(SELECT 1 FROM requirements req WHERE req.entity_id=e.id AND req.created_in_space='research'))
    )";
    let count_sql =
        format!("SELECT count(*) FROM entities e WHERE e.project_id=?1 AND {predicate}");
    let count: i64 = connection.query_row(&count_sql, [project_id], |row| row.get(0))?;
    let query_sql = format!(
        "SELECT d.entity_id,d.entity_type,e.title,e.status,substr(d.body,1,300)
         FROM research_search_documents d JOIN entities e ON e.id=d.entity_id
         WHERE d.project_id=?1 AND {predicate}
         ORDER BY e.updated_at DESC,e.id LIMIT ?2"
    );
    let mut statement = connection.prepare(&query_sql)?;
    let items = statement
        .query_map(params![project_id, limit], |row| {
            Ok(ResearchSearchHit {
                entity_id: row.get(0)?,
                entity_type: row.get(1)?,
                title: row.get(2)?,
                status: row.get(3)?,
                snippet: row.get(4)?,
            })
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok((items, count as u64))
}

fn research_details(connection: &Connection, entity: &Entity) -> Result<Value> {
    match entity.entity_type.as_str(){
        "research_session"=>connection.query_row("SELECT objective,started_at,ended_at,completion_note FROM research_sessions WHERE entity_id=?1",[&entity.id],
            |row|Ok(json!({"objective":row.get::<_,String>(0)?,"started_at":row.get::<_,String>(1)?,"ended_at":row.get::<_,Option<String>>(2)?,"completion_note":row.get::<_,Option<String>>(3)?}))).map_err(Into::into),
        "research_question"=>connection.query_row("SELECT question_kind,question_text,context,desired_outcome,priority,due_at FROM research_questions WHERE entity_id=?1",[&entity.id],
            |row|Ok(json!({"kind":row.get::<_,String>(0)?,"question":row.get::<_,String>(1)?,"context":row.get::<_,String>(2)?,"desired_outcome":row.get::<_,String>(3)?,"priority":row.get::<_,i64>(4)?,"due_at":row.get::<_,Option<String>>(5)?}))).map_err(Into::into),
        "evidence"=>connection.query_row("SELECT evidence_kind,source_uri,source_title,source_author,captured_at,capture_method,stable_reference,source_content,annotation_text,summary_text,relevance,original_artifact_id FROM evidence WHERE entity_id=?1",[&entity.id],
            |row|Ok(json!({"kind":row.get::<_,String>(0)?,"source_uri":row.get::<_,Option<String>>(1)?,"source_title":row.get::<_,Option<String>>(2)?,"source_author":row.get::<_,Option<String>>(3)?,"captured_at":row.get::<_,String>(4)?,"capture_method":row.get::<_,String>(5)?,"stable_reference":row.get::<_,Option<String>>(6)?,"source_content":row.get::<_,Option<String>>(7)?,"annotation":row.get::<_,String>(8)?,"summary":row.get::<_,String>(9)?,"relevance":row.get::<_,String>(10)?,"original_artifact_id":row.get::<_,Option<String>>(11)?}))).map_err(Into::into),
        "experiment"=>connection.query_row("SELECT hypothesis,method,inputs_json,expected_observations,started_at,ended_at FROM experiments WHERE entity_id=?1",[&entity.id],|row|{
            let inputs:String=row.get(2)?; Ok(json!({"hypothesis":row.get::<_,String>(0)?,"method":row.get::<_,String>(1)?,"inputs":serde_json::from_str::<Value>(&inputs).map_err(|e|rusqlite::Error::FromSqlConversionFailure(2,rusqlite::types::Type::Text,Box::new(e)))?,"expected_observations":row.get::<_,String>(3)?,"started_at":row.get::<_,Option<String>>(4)?,"ended_at":row.get::<_,Option<String>>(5)?}))}).map_err(Into::into),
        "result"=>connection.query_row("SELECT experiment_id,observation_text,outcome,measurements_json,observed_at,artifact_id FROM results WHERE entity_id=?1",[&entity.id],|row|{
            let measurements:String=row.get(3)?; Ok(json!({"experiment_id":row.get::<_,String>(0)?,"observation":row.get::<_,String>(1)?,"outcome":row.get::<_,String>(2)?,"measurements":serde_json::from_str::<Value>(&measurements).map_err(|e|rusqlite::Error::FromSqlConversionFailure(3,rusqlite::types::Type::Text,Box::new(e)))?,"observed_at":row.get::<_,String>(4)?,"artifact_id":row.get::<_,Option<String>>(5)?}))}).map_err(Into::into),
        "finding"=>connection.query_row("SELECT claim,interpretation,uncertainty,confidence,accepted_at FROM findings WHERE entity_id=?1",[&entity.id],
            |row|Ok(json!({"claim":row.get::<_,String>(0)?,"interpretation":row.get::<_,String>(1)?,"uncertainty":row.get::<_,String>(2)?,"confidence":row.get::<_,Option<f64>>(3)?,"accepted_at":row.get::<_,Option<String>>(4)?}))).map_err(Into::into),
        "decision"=>connection.query_row("SELECT selected_option,rationale,alternatives_json,constraints_json,decided_at FROM decisions WHERE entity_id=?1",[&entity.id],|row|{
            let alternatives:String=row.get(2)?;let constraints:String=row.get(3)?;Ok(json!({"selected_option":row.get::<_,String>(0)?,"rationale":row.get::<_,String>(1)?,"alternatives":serde_json::from_str::<Value>(&alternatives).map_err(|e|rusqlite::Error::FromSqlConversionFailure(2,rusqlite::types::Type::Text,Box::new(e)))?,"constraints":serde_json::from_str::<Value>(&constraints).map_err(|e|rusqlite::Error::FromSqlConversionFailure(3,rusqlite::types::Type::Text,Box::new(e)))?,"decided_at":row.get::<_,Option<String>>(4)?}))}).map_err(Into::into),
        "requirement"=>connection.query_row("SELECT statement,acceptance_criteria_json,priority,rationale_origin,verification_method FROM requirements WHERE entity_id=?1",[&entity.id],|row|{
            let criteria:String=row.get(1)?;Ok(json!({"statement":row.get::<_,String>(0)?,"acceptance_criteria":serde_json::from_str::<Value>(&criteria).map_err(|e|rusqlite::Error::FromSqlConversionFailure(1,rusqlite::types::Type::Text,Box::new(e)))?,"priority":row.get::<_,i64>(2)?,"rationale_origin":row.get::<_,String>(3)?,"verification_method":row.get::<_,String>(4)?}))}).map_err(Into::into),
        _=>Err(CoreError::Validation("unsupported Research entity type".into())),
    }
}

fn research_report_detail(
    connection: &Connection,
    project_id: &str,
    entity_id: &str,
    kind: &str,
) -> Result<String> {
    match kind {
        "research_question" => connection
            .query_row(
                "SELECT question_text FROM research_questions WHERE entity_id=?1",
                [entity_id],
                |row| row.get(0),
            )
            .map_err(Into::into),
        "evidence" => connection
            .query_row(
                "SELECT summary_text,relevance,COALESCE(source_uri,stable_reference),original_artifact_id
                 FROM evidence WHERE entity_id=?1",
                [entity_id],
                |row| {
                    let summary: String = row.get(0)?;
                    let relevance: String = row.get(1)?;
                    let reference: Option<String> = row.get(2)?;
                    let artifact: Option<String> = row.get(3)?;
                    Ok(format!(
                        "Summary: {} Reference: `{}` Artifact: `{}` Relevance: {}",
                        if summary.is_empty() { "Not provided." } else { &summary },
                        reference.as_deref().unwrap_or("not-provided"),
                        artifact.as_deref().unwrap_or("not-provided"),
                        if relevance.is_empty() { "Not provided." } else { &relevance }
                    ))
                },
            )
            .map_err(Into::into),
        "experiment" => connection
            .query_row(
                "SELECT hypothesis,method FROM experiments WHERE entity_id=?1",
                [entity_id],
                |row| {
                    Ok(format!(
                        "Hypothesis: {} Method: {}",
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?
                    ))
                },
            )
            .map_err(Into::into),
        "result" => connection
            .query_row(
                "SELECT observation_text,outcome,experiment_id FROM results WHERE entity_id=?1",
                [entity_id],
                |row| {
                    Ok(format!(
                        "Observation: {} Outcome: **{}**. Experiment: `{}`.",
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?
                    ))
                },
            )
            .map_err(Into::into),
        "finding" => {
            let claim: String = connection.query_row(
                "SELECT claim FROM findings WHERE entity_id=?1",
                [entity_id],
                |row| row.get(0),
            )?;
            let mut statement = connection.prepare(
                "SELECT source_entity_id,relation_type FROM relationships
                 WHERE project_id=?1 AND target_entity_id=?2 AND target_entity_type='finding'
                   AND relation_type IN ('supports','challenges','contextualizes','inconclusive_for')
                   AND status='active' ORDER BY relation_type,source_entity_id",
            )?;
            let sources = statement
                .query_map(params![project_id, entity_id], |row| {
                    Ok(format!(
                        "{} `{}`",
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(0)?
                    ))
                })?
                .collect::<std::result::Result<Vec<_>, _>>()?;
            Ok(format!("Claim: {claim} Sources: {}.", sources.join(", ")))
        }
        "decision" => connection
            .query_row(
                "SELECT selected_option,rationale FROM decisions WHERE entity_id=?1",
                [entity_id],
                |row| {
                    Ok(format!(
                        "Selected: {} Rationale: {}",
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?
                    ))
                },
            )
            .map_err(Into::into),
        "requirement" => connection
            .query_row(
                "SELECT statement,rationale_origin FROM requirements WHERE entity_id=?1",
                [entity_id],
                |row| {
                    Ok(format!(
                        "Statement: {} Rationale origin: **{}**.",
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?
                    ))
                },
            )
            .map_err(Into::into),
        _ => Ok(String::new()),
    }
}

fn report_excerpt(value: &str, max_chars: usize) -> String {
    let flattened = value.replace(['\r', '\n'], " ");
    let escaped = flattened
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let mut output: String = escaped.chars().take(max_chars).collect();
    if escaped.chars().count() > max_chars {
        output.push('…');
    }
    output
}

pub(crate) fn append_research_integrity_issues(
    connection: &Connection,
    project_id: &str,
    report: &mut IntegrityReport,
) -> Result<()> {
    let mappings = [
        ("research_session", "research_sessions"),
        ("research_question", "research_questions"),
        ("evidence", "evidence"),
        ("experiment", "experiments"),
        ("result", "results"),
        ("finding", "findings"),
        ("decision", "decisions"),
        ("requirement", "requirements"),
    ];
    for (kind, table) in mappings {
        let sql = format!(
            "SELECT e.id FROM entities e LEFT JOIN {table} d ON d.entity_id=e.id WHERE e.project_id=?1 AND e.entity_type=?2 AND d.entity_id IS NULL"
        );
        let mut statement = connection.prepare(&sql)?;
        let ids = statement
            .query_map(params![project_id, kind], |row| row.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        for id in ids {
            report.issues.push(IntegrityIssue{code:"missing_research_detail".into(),path_or_id:id,
            guidance:format!("Restore the normalized {kind} row from a verified backup; do not infer missing content.")});
        }
    }
    let mut statement=connection.prepare("SELECT e.id FROM entities e LEFT JOIN research_search_documents d ON d.entity_id=e.id
        LEFT JOIN requirements req ON req.entity_id=e.id
        WHERE e.project_id=?1
        AND (e.entity_type IN ('research_session','research_question','evidence','experiment','result','finding','decision')
             OR (e.entity_type='requirement' AND req.created_in_space='research'))
        AND d.entity_id IS NULL")?;
    let ids = statement
        .query_map([project_id], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for id in ids {
        report.issues.push(IntegrityIssue{code:"missing_research_search_projection".into(),path_or_id:id,
        guidance:"Rebuild the deterministic Research search projection from normalized canonical rows.".into()});
    }
    let mut statement=connection.prepare("SELECT id,source_entity_type,relation_type,target_entity_type FROM relationships WHERE project_id=?1
        AND (source_entity_type IN ('research_session','research_question','evidence','experiment','result','finding','decision','requirement')
          OR target_entity_type IN ('research_session','research_question','evidence','experiment','result','finding','decision','requirement'))")?;
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
        if validate_registered_relationship_pair(&source, &relation, &target).is_err() {
            report.issues.push(IntegrityIssue{code:"invalid_research_relationship".into(),path_or_id:id,
            guidance:"Archive or repair the relationship through a reviewed migration; preserve its audit history.".into()});
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documented_research_relationship_matrix_is_explicit() {
        let valid = [
            ("experiment", "addresses", "research_question"),
            ("experiment", "produces", "result"),
            ("evidence", "supports", "finding"),
            ("result", "challenges", "finding"),
            ("evidence", "contextualizes", "research_question"),
            ("finding", "answers", "research_question"),
            ("finding", "informs", "decision"),
            ("decision", "creates", "requirement"),
            ("decision", "supersedes", "decision"),
        ];
        for (source, relation, target) in valid {
            assert!(
                validate_research_pair(source, relation, target).is_ok(),
                "expected {source} --{relation}--> {target} to be valid"
            );
        }
        let invalid = [
            ("research_question", "supports", "finding"),
            ("evidence", "creates", "requirement"),
            ("result", "informs", "decision"),
            ("decision", "supersedes", "finding"),
            ("finding", "implements", "requirement"),
        ];
        for (source, relation, target) in invalid {
            assert!(
                validate_research_pair(source, relation, target).is_err(),
                "expected {source} --{relation}--> {target} to be rejected"
            );
        }
    }

    #[test]
    fn terminal_and_observation_lifecycles_cannot_be_reopened_silently() {
        assert!(validate_lifecycle_transition("experiment", "planned", "running").is_ok());
        assert!(validate_lifecycle_transition("experiment", "planned", "completed").is_err());
        assert!(validate_lifecycle_transition("result", "recorded", "superseded").is_ok());
        assert!(validate_lifecycle_transition("result", "superseded", "recorded").is_err());
        assert!(validate_lifecycle_transition("decision", "accepted", "proposed").is_err());
        assert!(validate_lifecycle_transition("finding", "challenged", "accepted").is_ok());
    }

    #[test]
    fn registered_policy_preserves_declared_cp4_and_cp6_extension_seams() {
        assert!(
            validate_registered_relationship_pair("change_set", "implements", "requirement")
                .is_ok()
        );
        assert!(validate_registered_relationship_pair("test", "verifies", "requirement").is_ok());
        assert!(
            validate_registered_relationship_pair(
                "learning_feedback",
                "requests_revision_of",
                "finding"
            )
            .is_ok()
        );
        assert!(
            validate_registered_relationship_pair("core.fixture", "supports", "requirement")
                .is_err()
        );
    }

    #[test]
    fn report_projection_flattens_and_escapes_untrusted_text() {
        assert_eq!(
            report_excerpt("line one\n<script>alert(1)</script>", 100),
            "line one &lt;script&gt;alert(1)&lt;/script&gt;"
        );
        assert_eq!(report_excerpt("abcdef", 3), "abc…");
    }
}
