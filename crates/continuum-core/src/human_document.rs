use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as _;

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::store::{
    append_event_with_context, bounded_json, prior_result, record_command_with_context,
    validate_command_context, validate_nonempty,
};
use crate::{
    ActorKind, ArtifactClassification, CommandContext, ContinuityStore, CoreError,
    DataClassification, Entity, ExternalProposalKind, IntegrityIssue, IntegrityReport, OriginKind,
    PageRequest, Result, SemanticTaskType, new_id,
};

pub const HUMAN_DOCUMENT_SCHEMA_VERSION: u32 = 1;
pub const HUMAN_DOCUMENT_RENDERER_VERSION: u32 = 4;
pub const HUMAN_DOCUMENT_TEMPLATE_VERSION: u32 = 4;
const MERMAID_RUNTIME: &str = include_str!("../assets/mermaid-11.17.2.min.js");
const REPORT_INTERACTIONS: &str = include_str!("../assets/report-interactions.js");
const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_BLOCKS: usize = 500;
const MAX_CITATIONS: usize = 5_000;
const MAX_RECORDS: usize = 5_000;
const MAX_AI_CANDIDATES: usize = 20;
const MAX_GRAPH_NODES: usize = 200;
const MAX_GRAPH_EDGES: usize = 500;
const MAX_MERMAID_NODES: usize = 50;
const MAX_MERMAID_EDGES: usize = 100;
const MAX_TEXT: usize = 50_000;
const MAX_CELL_TEXT: usize = 4_000;

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

string_enum!(HumanDocumentKind {
    ResearchReport => "research_report",
    DevelopmentReport => "development_report",
    IntegratedReport => "integrated_report",
    ArchitectureExplanation => "architecture_explanation",
    Timeline => "timeline",
    Handover => "handover",
});

string_enum!(HumanDocumentAudience {
    LocalProject => "local_project",
    PrivatePortable => "private_portable",
    PublicPortable => "public_portable",
});

string_enum!(ContributionKind {
    Deterministic => "deterministic",
    AiAssisted => "ai_assisted",
    UserAuthored => "user_authored",
    Imported => "imported",
});

string_enum!(CalloutTone {
    Info => "info",
    Success => "success",
    Warning => "warning",
    Danger => "danger",
    Uncertainty => "uncertainty",
});

string_enum!(DiagramDirection {
    TopDown => "top_down",
    LeftRight => "left_right",
});

string_enum!(HumanDocumentFormat {
    Html => "html",
    Markdown => "markdown",
});

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanDocumentRequest {
    pub kind: HumanDocumentKind,
    pub title: Option<String>,
    pub audience: HumanDocumentAudience,
    pub checkpoint_id: Option<String>,
    #[serde(default)]
    pub root_entity_ids: Vec<String>,
    #[serde(default)]
    pub accepted_ai_candidate_ids: Vec<String>,
    pub max_records: usize,
    pub max_graph_nodes: usize,
    pub max_graph_edges: usize,
}

impl HumanDocumentRequest {
    pub fn research() -> Self {
        Self {
            kind: HumanDocumentKind::ResearchReport,
            title: None,
            audience: HumanDocumentAudience::LocalProject,
            checkpoint_id: None,
            root_entity_ids: Vec::new(),
            accepted_ai_candidate_ids: Vec::new(),
            max_records: 1_000,
            max_graph_nodes: 150,
            max_graph_edges: 400,
        }
    }

    pub fn development() -> Self {
        Self {
            kind: HumanDocumentKind::DevelopmentReport,
            ..Self::research()
        }
    }

    pub fn integrated() -> Self {
        Self {
            kind: HumanDocumentKind::IntegratedReport,
            ..Self::research()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentContribution {
    pub kind: ContributionKind,
    pub actor_id: Option<String>,
    pub provider_profile_id: Option<String>,
    pub attempt_id: Option<String>,
    pub candidate_id: Option<String>,
}

impl DocumentContribution {
    fn deterministic() -> Self {
        Self {
            kind: ContributionKind::Deterministic,
            actor_id: Some("continuum-core".into()),
            provider_profile_id: None,
            attempt_id: None,
            candidate_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanDocumentSource {
    pub source_kind: String,
    pub source_id: String,
    pub source_version: String,
    pub classification: DataClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanCitation {
    pub source_kind: String,
    pub source_id: String,
    pub source_type: String,
    pub title: String,
    pub status: String,
    pub origin: String,
    pub version: String,
    pub classification: DataClassification,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockMeta {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub source_ids: Vec<String>,
    pub contribution: DocumentContribution,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StatusCard {
    pub label: String,
    pub value: String,
    pub state: String,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KeyValueFact {
    pub key: String,
    pub value: String,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentTableColumn {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentTableRow {
    pub id: String,
    pub cells: BTreeMap<String, String>,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentTimelineEntry {
    pub id: String,
    pub occurred_at: String,
    pub label: String,
    pub detail: String,
    pub state: String,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub status: String,
    pub source_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_data_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DiagramEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub label: String,
    pub source_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GraphSpecification {
    pub renderer: String,
    pub layout: String,
    pub direction: DiagramDirection,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
    pub truncated: bool,
    pub textual_alternative: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MermaidSpecification {
    pub diagram_kind: String,
    pub direction: DiagramDirection,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
    pub textual_alternative: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockContent {
    Overview {
        summary: String,
        status: String,
        blockers: Vec<String>,
        next_actions: Vec<String>,
    },
    StatusCards {
        cards: Vec<StatusCard>,
    },
    Heading {
        level: u8,
    },
    Prose {
        text: String,
    },
    Callout {
        tone: CalloutTone,
        text: String,
    },
    ResearchSynthesis {
        summary: String,
        key_points: Vec<String>,
        limitations: Vec<String>,
        recommendations: Vec<String>,
    },
    KeyValue {
        facts: Vec<KeyValueFact>,
    },
    Table {
        columns: Vec<DocumentTableColumn>,
        rows: Vec<DocumentTableRow>,
        truncated: bool,
    },
    Timeline {
        entries: Vec<DocumentTimelineEntry>,
        truncated: bool,
    },
    CitationList {
        citation_ids: Vec<String>,
    },
    ArtifactReference {
        artifact_id: String,
        media_type: String,
        availability: String,
        description: String,
    },
    MermaidDiagram {
        specification: MermaidSpecification,
    },
    Graph {
        specification: GraphSpecification,
    },
    DetailGroup {
        summary: String,
        details: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanDocumentBlock {
    pub meta: BlockMeta,
    pub content: BlockContent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanDocument {
    pub id: String,
    pub schema_version: u32,
    pub project_id: String,
    pub kind: HumanDocumentKind,
    pub title: String,
    pub audience: HumanDocumentAudience,
    pub source_checkpoint_id: Option<String>,
    pub source_ledger_sequence: i64,
    pub generated_at: String,
    pub classification: DataClassification,
    pub omissions: Vec<String>,
    pub limitations: Vec<String>,
    pub sources: Vec<HumanDocumentSource>,
    pub citations: Vec<HumanCitation>,
    pub blocks: Vec<HumanDocumentBlock>,
    pub material_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanDocumentFreshness {
    pub fresh: bool,
    pub current_ledger_sequence: i64,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanDocumentPage {
    pub items: Vec<HumanDocument>,
    pub next_offset: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HumanDocumentAssetManifest {
    pub schema_version: u32,
    pub renderer_version: u32,
    pub template_version: u32,
    pub network_dependencies: Vec<String>,
    pub inline_style_sha256: String,
    pub interactive_renderer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RenderedHumanDocument {
    pub html: String,
    pub markdown: String,
    pub material_fingerprint: String,
    pub html_sha256: String,
    pub markdown_sha256: String,
    pub asset_manifest: HumanDocumentAssetManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PublishedHumanDocument {
    pub export_id: String,
    pub document_id: String,
    pub format: HumanDocumentFormat,
    pub artifact_id: String,
    pub content_sha256: String,
    pub asset_manifest: HumanDocumentAssetManifest,
    pub created_at: String,
}

impl ContinuityStore {
    pub fn compose_human_document(&self, request: HumanDocumentRequest) -> Result<HumanDocument> {
        validate_request(&request)?;
        let connection = self.connection()?;
        let project_id = &self.manifest().project_id;
        let project_name = &self.manifest().name;
        let ledger_sequence: i64 = connection.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [project_id],
            |row| row.get(0),
        )?;

        let checkpoint = request
            .checkpoint_id
            .as_ref()
            .map(|id| self.get_checkpoint(id))
            .transpose()?;
        if let Some(checkpoint) = &checkpoint
            && !checkpoint_matches_kind(&checkpoint.scope, request.kind)
        {
            return Err(CoreError::Validation(format!(
                "checkpoint scope {} is incompatible with {}",
                checkpoint.scope,
                request.kind.as_str()
            )));
        }

        let mut entity_ids =
            select_entity_ids(&connection, project_id, &request, checkpoint.as_ref())?;
        entity_ids.sort();
        entity_ids.dedup();
        let source_was_truncated = entity_ids.len() > request.max_records;
        entity_ids.truncate(request.max_records);

        let mut omissions = Vec::new();
        if source_was_truncated {
            omissions.push(format!(
                "entity scope exceeded the declared {}-record limit",
                request.max_records
            ));
        }
        let mut entities = Vec::new();
        for id in entity_ids {
            let entity = self.get_entity(&id)?;
            if !entity_allowed_for_kind(&entity.entity_type, request.kind) {
                continue;
            }
            let classification = entity_classification(&connection, project_id, &id)?;
            if !audience_allows(request.audience, classification) {
                omissions.push(format!(
                    "{} source {} omitted by {} audience policy",
                    classification.as_str(),
                    id,
                    request.audience.as_str()
                ));
                continue;
            }
            entities.push((entity, classification));
        }
        entities.sort_by(|left, right| {
            left.0
                .entity_type
                .cmp(&right.0.entity_type)
                .then_with(|| left.0.created_at.cmp(&right.0.created_at))
                .then_with(|| left.0.id.cmp(&right.0.id))
        });

        let previews = collect_evidence_previews(self, &connection, &entities, request.audience);
        let mut citations = entities
            .iter()
            .map(|(entity, classification)| HumanCitation {
                source_kind: "entity".into(),
                source_id: entity.id.clone(),
                source_type: entity.entity_type.clone(),
                title: entity.title.clone(),
                status: entity.status.clone(),
                origin: entity.origin.clone(),
                version: entity.version.to_string(),
                classification: *classification,
            })
            .collect::<Vec<_>>();
        citations.extend(previews.values().map(|preview| HumanCitation {
            source_kind: "artifact".into(),
            source_id: preview.artifact_id.clone(),
            source_type: preview.media_type.clone(),
            title: format!("Screenshot preview for {}", preview.entity_title),
            status: "available".into(),
            origin: "deterministic".into(),
            version: preview.sha256.clone(),
            classification: preview.classification,
        }));
        let mut sources = citations
            .iter()
            .map(|citation| HumanDocumentSource {
                source_kind: citation.source_kind.clone(),
                source_id: citation.source_id.clone(),
                source_version: citation.version.clone(),
                classification: citation.classification,
            })
            .collect::<Vec<_>>();

        let mut blocks = Vec::new();
        let (blockers, next_actions) = checkpoint_actions(checkpoint.as_ref());
        let report_status = if blockers.is_empty() {
            "active"
        } else {
            "attention_required"
        };
        let all_entity_ids = entities
            .iter()
            .map(|value| value.0.id.clone())
            .collect::<Vec<_>>();
        blocks.push(block(
            "overview",
            "Overview",
            all_entity_ids.iter().take(50).cloned().collect(),
            BlockContent::Overview {
                summary: report_summary(request.kind, project_name, entities.len()),
                status: report_status.into(),
                blockers,
                next_actions,
            },
        ));

        let mut type_counts = BTreeMap::<String, usize>::new();
        let mut status_counts = BTreeMap::<String, usize>::new();
        for (entity, _) in &entities {
            *type_counts.entry(entity.entity_type.clone()).or_default() += 1;
            *status_counts.entry(entity.status.clone()).or_default() += 1;
        }
        let mut cards = type_counts
            .iter()
            .map(|(label, count)| StatusCard {
                label: humanize(label),
                value: count.to_string(),
                state: "neutral".into(),
                source_ids: entities
                    .iter()
                    .filter(|value| &value.0.entity_type == label)
                    .take(20)
                    .map(|value| value.0.id.clone())
                    .collect(),
            })
            .collect::<Vec<_>>();
        for (label, count) in status_counts {
            cards.push(StatusCard {
                label: format!("Status: {}", humanize(&label)),
                value: count.to_string(),
                state: status_state(&label).into(),
                source_ids: entities
                    .iter()
                    .filter(|value| value.0.status == label)
                    .take(20)
                    .map(|value| value.0.id.clone())
                    .collect(),
            });
        }
        blocks.push(block(
            "status",
            "Current State",
            all_entity_ids.iter().take(100).cloned().collect(),
            BlockContent::StatusCards { cards },
        ));

        for (entity_type, label) in report_sections(request.kind) {
            let matching = entities
                .iter()
                .filter(|value| value.0.entity_type == entity_type)
                .collect::<Vec<_>>();
            if matching.is_empty() {
                continue;
            }
            let rows = matching
                .iter()
                .map(|(entity, _classification)| {
                    let detail = if matches!(
                        entity.entity_type.as_str(),
                        "research_session"
                            | "research_question"
                            | "evidence"
                            | "experiment"
                            | "result"
                            | "finding"
                            | "decision"
                            | "requirement"
                    ) {
                        self.get_research_item(&entity.id)
                            .map(|item| item.details)
                            .unwrap_or_else(|_| entity.data.clone())
                    } else {
                        entity.data.clone()
                    };
                    DocumentTableRow {
                        id: entity.id.clone(),
                        cells: BTreeMap::from([
                            ("title".into(), bounded_text(&entity.title, 500)),
                            ("status".into(), humanize(&entity.status)),
                            ("updated".into(), friendly_timestamp(&entity.updated_at)),
                            ("detail".into(), human_detail_text(&detail)),
                        ]),
                        source_ids: vec![entity.id.clone()],
                    }
                })
                .collect::<Vec<_>>();
            blocks.push(block(
                &format!("table-{entity_type}"),
                label,
                matching.iter().map(|value| value.0.id.clone()).collect(),
                BlockContent::Table {
                    columns: vec![
                        column("title", "Title"),
                        column("status", "Status"),
                        column("updated", "Updated"),
                        column("detail", "What this tells us"),
                    ],
                    rows,
                    truncated: false,
                },
            ));
        }

        let mut timeline = entities
            .iter()
            .map(|(entity, _)| DocumentTimelineEntry {
                id: format!("timeline-{}", entity.id),
                occurred_at: entity.updated_at.clone(),
                label: entity.title.clone(),
                detail: format!(
                    "{} · {} · {}",
                    humanize(&entity.entity_type),
                    entity.status,
                    entity.origin
                ),
                state: entity.status.clone(),
                source_ids: vec![entity.id.clone()],
            })
            .collect::<Vec<_>>();
        timeline.sort_by(|left, right| {
            left.occurred_at
                .cmp(&right.occurred_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        let timeline_truncated = timeline.len() > 1_000;
        timeline.truncate(1_000);
        if !timeline.is_empty() {
            blocks.push(block(
                "timeline",
                "Timeline",
                timeline
                    .iter()
                    .flat_map(|entry| entry.source_ids.clone())
                    .collect(),
                BlockContent::Timeline {
                    entries: timeline,
                    truncated: timeline_truncated,
                },
            ));
        }

        let (graph, mermaid) = compose_diagrams(
            &connection,
            project_id,
            &entities,
            &previews,
            request.max_graph_nodes,
            request.max_graph_edges,
        )?;
        if graph.truncated {
            omissions.push("interactive graph was scoped to its declared node/edge budget".into());
        }
        if !graph.nodes.is_empty() {
            blocks.push(block(
                "knowledge-graph",
                "Research & Project Knowledge Map",
                graph
                    .nodes
                    .iter()
                    .flat_map(|node| node.source_ids.clone())
                    .collect(),
                BlockContent::Graph {
                    specification: graph,
                },
            ));
            let has_reasoned_flow = mermaid.nodes.iter().any(|node| {
                matches!(
                    node.kind.as_str(),
                    "finding" | "decision" | "requirement" | "change_set" | "checkpoint"
                )
            });
            if has_reasoned_flow || request.kind != HumanDocumentKind::ResearchReport {
                blocks.push(block(
                    "portable-flow",
                    "Evidence-to-Decision Flow",
                    mermaid
                        .nodes
                        .iter()
                        .flat_map(|node| node.source_ids.clone())
                        .collect(),
                    BlockContent::MermaidDiagram {
                        specification: mermaid,
                    },
                ));
            } else {
                omissions.push("Evidence-to-Decision flow is waiting for a reviewed Finding or Decision; the raw Evidence map remains available.".into());
            }
        }

        // Reviewed MCP proposals are the bridge between AI interpretation and a
        // trustworthy Human Document. Pending proposals never appear here.
        let accepted = self.list_external_proposals(
            Some("accepted"),
            PageRequest {
                limit: 100,
                offset: 0,
            },
        )?;
        let report_source_ids = citations
            .iter()
            .map(|citation| citation.source_id.clone())
            .collect::<HashSet<_>>();
        let mut ai_blocks = Vec::new();
        for summary in accepted.items {
            if !proposal_scope_matches_report(summary.scope.as_str(), request.kind) {
                continue;
            }
            let proposal = self.get_external_proposal(&summary.id)?;
            if !proposal
                .source_refs
                .iter()
                .all(|id| report_source_ids.contains(id))
            {
                omissions.push(format!(
                    "reviewed AI proposal {} cites sources outside this report",
                    proposal.id
                ));
                continue;
            }
            if !matches!(proposal.kind, ExternalProposalKind::ResearchSynthesis | ExternalProposalKind::DiagramPlan) { continue; }
            let content = (|| -> Result<BlockContent> { Ok(match proposal.kind {
                ExternalProposalKind::ResearchSynthesis => BlockContent::ResearchSynthesis {
                    summary: required_string(&proposal.payload, "summary", MAX_TEXT)?,
                    key_points: string_array(&proposal.payload, "key_points", 100)?,
                    limitations: string_array(&proposal.payload, "limitations", 100)?,
                    recommendations: string_array(&proposal.payload, "recommendations", 100)?,
                },
                ExternalProposalKind::DiagramPlan => {
                    let specification = diagram_from_candidate(&proposal.payload)?;
                    validate_proposal_diagram_sources(&specification, &proposal.source_refs)?;
                    BlockContent::MermaidDiagram { specification }
                }
                _ => unreachable!(),
            }) })();
            let content = match content {
                Ok(content) => content,
                Err(_) => {
                    omissions.push(format!("Reviewed AI draft '{}' has an invalid presentation payload; revise it before inclusion.", proposal.title));
                    continue;
                }
            };
            let block_title = match proposal.kind {
                ExternalProposalKind::ResearchSynthesis => "Research Synthesis & Recommendations",
                ExternalProposalKind::DiagramPlan => "Research Story Map",
                _ => unreachable!(),
            };
            ai_blocks.push(HumanDocumentBlock {
                meta: BlockMeta {
                    id: format!("reviewed-ai-{}", proposal.id),
                    title: block_title.into(),
                    source_ids: proposal.source_refs.iter().cloned().chain(std::iter::once(proposal.id.clone())).collect(),
                    contribution: DocumentContribution {
                        kind: ContributionKind::AiAssisted,
                        actor_id: proposal.reviewed_by.clone(),
                        provider_profile_id: None,
                        attempt_id: None,
                        candidate_id: Some(proposal.id.clone()),
                    },
                },
                content,
            });
            citations.push(HumanCitation {
                source_kind: "external_proposal".into(),
                source_id: proposal.id.clone(),
                source_type: proposal.kind.as_str().into(),
                title: proposal.title,
                status: proposal.status,
                origin: "ai_assisted_reviewed".into(),
                version: proposal.version.to_string(),
                classification: highest_classification(&sources),
            });
            sources.push(HumanDocumentSource {
                source_kind: "external_proposal".into(),
                source_id: proposal.id,
                source_version: proposal.version.to_string(),
                classification: highest_classification(&sources),
            });
        }
        // Put human meaning before inventories and technical provenance.
        ai_blocks.sort_by_key(|block| if matches!(block.content, BlockContent::ResearchSynthesis { .. }) { 0 } else { 1 });
        for block in ai_blocks.into_iter().rev() {
            blocks.insert(1, block);
        }

        let citation_id_set = citations
            .iter()
            .map(|citation| citation.source_id.clone())
            .collect::<HashSet<_>>();
        for candidate_id in &request.accepted_ai_candidate_ids {
            let candidate = self.get_semantic_candidate(candidate_id)?;
            if candidate.review_state != "accepted" {
                return Err(CoreError::Validation(format!(
                    "AI candidate {candidate_id} must be accepted before Human Document use"
                )));
            }
            if !self.semantic_candidate_freshness(candidate_id)?.fresh {
                omissions.push(format!("stale AI candidate {candidate_id} was omitted"));
                continue;
            }
            let task = self.get_semantic_task(&candidate.task_id)?;
            if !candidate
                .validation
                .cited_source_ids
                .iter()
                .all(|source| citation_id_set.contains(source))
            {
                omissions.push(format!(
                    "AI candidate {candidate_id} cites sources outside this document scope"
                ));
                continue;
            }
            match task.task_type {
                SemanticTaskType::ReportNarrative => {
                    if let Some(text) = candidate_text(&candidate.output) {
                        blocks.push(HumanDocumentBlock {
                            meta: BlockMeta {
                                id: format!("ai-narrative-{candidate_id}"),
                                title: "AI-assisted narrative".into(),
                                source_ids: candidate.validation.cited_source_ids.clone(),
                                contribution: DocumentContribution {
                                    kind: ContributionKind::AiAssisted,
                                    actor_id: None,
                                    provider_profile_id: Some(
                                        candidate.provider_profile_id.clone(),
                                    ),
                                    attempt_id: Some(candidate.attempt_id.clone()),
                                    candidate_id: Some(candidate.id.clone()),
                                },
                            },
                            content: BlockContent::Callout {
                                tone: CalloutTone::Info,
                                text,
                            },
                        });
                    } else {
                        omissions.push(format!(
                            "AI candidate {candidate_id} has no supported narrative field"
                        ));
                        continue;
                    }
                }
                SemanticTaskType::DiagramPlan => {
                    let spec = diagram_from_candidate(&candidate.output)?;
                    blocks.push(HumanDocumentBlock {
                        meta: BlockMeta {
                            id: format!("ai-diagram-{candidate_id}"),
                            title: "AI-assisted diagram plan".into(),
                            source_ids: candidate.validation.cited_source_ids.clone(),
                            contribution: DocumentContribution {
                                kind: ContributionKind::AiAssisted,
                                actor_id: None,
                                provider_profile_id: Some(candidate.provider_profile_id.clone()),
                                attempt_id: Some(candidate.attempt_id.clone()),
                                candidate_id: Some(candidate.id.clone()),
                            },
                        },
                        content: BlockContent::MermaidDiagram {
                            specification: spec,
                        },
                    });
                }
                _ => {
                    omissions.push(format!(
                        "AI candidate {candidate_id} has task type outside CP8 presentation scope"
                    ));
                    continue;
                }
            }
            citations.push(HumanCitation {
                source_kind: "ai_candidate".into(),
                source_id: candidate.id.clone(),
                source_type: task.task_type.as_str().into(),
                title: "Reviewed AI candidate".into(),
                status: candidate.review_state.clone(),
                origin: "ai_assisted_reviewed".into(),
                version: candidate.candidate_version.to_string(),
                classification: highest_classification(&sources),
            });
            sources.push(HumanDocumentSource {
                source_kind: "ai_candidate".into(),
                source_id: candidate.id,
                source_version: candidate.candidate_version.to_string(),
                classification: highest_classification(&sources),
            });
        }

        citations.sort_by(|left, right| {
            left.source_kind
                .cmp(&right.source_kind)
                .then_with(|| left.source_id.cmp(&right.source_id))
        });
        citations.dedup_by(|left, right| left.source_kind == right.source_kind && left.source_id == right.source_id);
        sources.sort_by(|left, right| {
            left.source_kind
                .cmp(&right.source_kind)
                .then_with(|| left.source_id.cmp(&right.source_id))
        });
        // Several evidence records can legitimately reference the same original
        // artifact. A report declares that artifact once, while retaining every
        // evidence record and its provenance elsewhere in the document.
        sources.dedup_by(|left, right| left.source_kind == right.source_kind && left.source_id == right.source_id);
        blocks.push(block(
            "citations",
            "Sources and Verification",
            citations
                .iter()
                .map(|citation| citation.source_id.clone())
                .collect(),
            BlockContent::CitationList {
                citation_ids: citations
                    .iter()
                    .map(|citation| citation.source_id.clone())
                    .collect(),
            },
        ));

        omissions.sort();
        omissions.dedup();
        let mut document = HumanDocument {
            id: new_id(),
            schema_version: HUMAN_DOCUMENT_SCHEMA_VERSION,
            project_id: project_id.clone(),
            kind: request.kind,
            title: request
                .title
                .unwrap_or_else(|| default_title(request.kind, project_name)),
            audience: request.audience,
            source_checkpoint_id: checkpoint.map(|value| value.id),
            source_ledger_sequence: ledger_sequence,
            generated_at: Utc::now().to_rfc3339(),
            classification: highest_classification(&sources),
            omissions,
            limitations: vec![
                "Human Documentation is a derived projection; canonical project records remain authoritative."
                    .into(),
                "Interactive layout coordinates are computed by the pinned UI renderer and are not canonical."
                    .into(),
            ],
            sources,
            citations,
            blocks,
            material_fingerprint: String::new(),
        };
        document.material_fingerprint = material_fingerprint(&document)?;
        validate_human_document(&document)?;
        Ok(document)
    }

    pub fn save_human_document(
        &self,
        command: &CommandContext,
        document: &HumanDocument,
    ) -> Result<HumanDocument> {
        validate_command_context(command)?;
        if command.actor.kind == ActorKind::AiProposal {
            return Err(CoreError::Validation(
                "AI actors cannot save Human Documents".into(),
            ));
        }
        validate_human_document(document)?;
        if document.project_id != self.manifest().project_id {
            return Err(CoreError::Validation(
                "Human Document belongs to another project".into(),
            ));
        }
        if document.material_fingerprint != material_fingerprint(document)? {
            return Err(CoreError::Validation(
                "Human Document material fingerprint does not match its content".into(),
            ));
        }
        let mut connection = self.connection()?;
        validate_source_snapshots(&connection, &document.project_id, &document.sources)?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(Some(id)) =
            prior_result(&tx, &document.project_id, command, "SaveHumanDocument")?
        {
            tx.commit()?;
            return self.get_human_document(&id);
        }
        tx.execute(
            "INSERT INTO human_documents(id,project_id,schema_version,document_kind,title,audience,source_checkpoint_id,source_ledger_sequence,classification,document_json,material_fingerprint,created_at,created_by) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
            params![
                document.id,
                document.project_id,
                document.schema_version,
                document.kind.as_str(),
                document.title,
                document.audience.as_str(),
                document.source_checkpoint_id,
                document.source_ledger_sequence,
                document.classification.as_str(),
                bounded_json(
                    &serde_json::to_value(document)?,
                    MAX_DOCUMENT_BYTES,
                    "Human Document",
                )?,
                document.material_fingerprint,
                document.generated_at,
                command.actor.id,
            ],
        )?;
        for source in &document.sources {
            tx.execute(
                "INSERT INTO human_document_sources(document_id,source_kind,source_id,source_version,classification) VALUES(?1,?2,?3,?4,?5)",
                params![
                    document.id,
                    source.source_kind,
                    source.source_id,
                    source.source_version,
                    source.classification.as_str()
                ],
            )?;
        }
        append_event_with_context(
            &tx,
            &document.project_id,
            command,
            Some(&document.id),
            "human_document.saved",
            &json!({
                "document_id": document.id,
                "document_kind": document.kind.as_str(),
                "source_ledger_sequence": document.source_ledger_sequence,
                "source_count": document.sources.len(),
                "material_fingerprint": document.material_fingerprint,
            }),
        )?;
        record_command_with_context(
            &tx,
            command,
            &document.project_id,
            "SaveHumanDocument",
            Some(&document.id),
            None,
            &json!({
                "document_kind": document.kind.as_str(),
                "audience": document.audience.as_str(),
            }),
        )?;
        tx.commit()?;
        self.get_human_document(&document.id)
    }

    pub fn get_human_document(&self, id: &str) -> Result<HumanDocument> {
        let raw: String = self
            .connection()?
            .query_row(
                "SELECT document_json FROM human_documents WHERE id=?1 AND project_id=?2",
                params![id, self.manifest().project_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| CoreError::NotFound(id.into()))?;
        let document: HumanDocument = serde_json::from_str(&raw)?;
        validate_human_document(&document)?;
        Ok(document)
    }

    pub fn list_human_documents(
        &self,
        kind: Option<HumanDocumentKind>,
        page: PageRequest,
    ) -> Result<HumanDocumentPage> {
        if page.limit == 0 || page.limit > 100 {
            return Err(CoreError::Validation(
                "Human Document page limit must be within 1..=100".into(),
            ));
        }
        let offset = i64::try_from(page.offset)
            .map_err(|_| CoreError::Validation("page offset is too large".into()))?;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id FROM human_documents WHERE project_id=?1 AND (?2 IS NULL OR document_kind=?2) ORDER BY created_at,id LIMIT ?3 OFFSET ?4",
        )?;
        let ids = statement
            .query_map(
                params![
                    self.manifest().project_id,
                    kind.map(HumanDocumentKind::as_str),
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
            .map(|id| self.get_human_document(&id))
            .collect::<Result<Vec<_>>>()?;
        Ok(HumanDocumentPage {
            next_offset: has_more.then_some(page.offset + items.len() as u64),
            items,
        })
    }

    pub fn human_document_freshness(&self, id: &str) -> Result<HumanDocumentFreshness> {
        let document = self.get_human_document(id)?;
        let connection = self.connection()?;
        let current_ledger_sequence: i64 = connection.query_row(
            "SELECT ledger_sequence FROM projects WHERE id=?1",
            [&document.project_id],
            |row| row.get(0),
        )?;
        let mut reasons =
            source_staleness_reasons(&connection, &document.project_id, &document.sources)?;
        let relevant_events: i64 = connection.query_row(
            "SELECT count(*) FROM audit_events WHERE project_id=?1 AND ledger_sequence>?2 AND event_type NOT LIKE 'human_document.%' AND event_type NOT LIKE 'artifact.%'",
            params![document.project_id, document.source_ledger_sequence],
            |row| row.get(0),
        )?;
        if relevant_events > 0 {
            reasons.push(format!(
                "{relevant_events} project event(s) occurred after the source ledger position"
            ));
        }
        reasons.sort();
        reasons.dedup();
        Ok(HumanDocumentFreshness {
            fresh: reasons.is_empty(),
            current_ledger_sequence,
            reasons,
        })
    }

    pub fn publish_human_document(
        &self,
        command: &CommandContext,
        document_id: &str,
        format: HumanDocumentFormat,
    ) -> Result<PublishedHumanDocument> {
        validate_command_context(command)?;
        if command.actor.kind == ActorKind::AiProposal {
            return Err(CoreError::Validation(
                "AI actors cannot publish Human Documents".into(),
            ));
        }
        let document = self.get_human_document(document_id)?;
        let bundle = render_human_document(&document)?;
        let (bytes, media_type, content_sha256) = match format {
            HumanDocumentFormat::Html => (
                bundle.html.as_bytes(),
                "text/html;charset=utf-8",
                bundle.html_sha256.clone(),
            ),
            HumanDocumentFormat::Markdown => (
                bundle.markdown.as_bytes(),
                "text/markdown;charset=utf-8",
                bundle.markdown_sha256.clone(),
            ),
        };
        let artifact = self.ingest_artifact_reader_with_context(
            command,
            bytes,
            media_type,
            artifact_classification(document.classification),
            OriginKind::Deterministic,
            &json!({
                "generated_artifact_kind": "human_document_export",
                "document_id": document.id,
                "document_kind": document.kind.as_str(),
                "format": format.as_str(),
                "source_ledger_sequence": document.source_ledger_sequence,
                "material_fingerprint": document.material_fingerprint,
                "renderer_version": HUMAN_DOCUMENT_RENDERER_VERSION,
                "template_version": HUMAN_DOCUMENT_TEMPLATE_VERSION,
                "offline": true,
            }),
        )?;
        let connection = self.connection()?;
        let existing: Option<String> = connection
            .query_row(
                "SELECT id FROM human_document_exports WHERE document_id=?1 AND format=?2 AND content_sha256=?3",
                params![document.id, format.as_str(), content_sha256],
                |row| row.get(0),
            )
            .optional()?;
        let export_id = existing.unwrap_or_else(new_id);
        let created_at = Utc::now().to_rfc3339();
        connection.execute(
            "INSERT OR IGNORE INTO human_document_exports(id,project_id,document_id,format,artifact_id,renderer_version,template_version,content_sha256,asset_manifest_json,created_at,created_by) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                export_id,
                document.project_id,
                document.id,
                format.as_str(),
                artifact.id,
                HUMAN_DOCUMENT_RENDERER_VERSION,
                HUMAN_DOCUMENT_TEMPLATE_VERSION,
                content_sha256,
                bounded_json(
                    &serde_json::to_value(&bundle.asset_manifest)?,
                    64 * 1024,
                    "asset manifest",
                )?,
                created_at,
                command.actor.id,
            ],
        )?;
        Ok(PublishedHumanDocument {
            export_id,
            document_id: document.id,
            format,
            artifact_id: artifact.id,
            content_sha256,
            asset_manifest: bundle.asset_manifest,
            created_at,
        })
    }
}

pub fn validate_human_document(document: &HumanDocument) -> Result<()> {
    if document.schema_version != HUMAN_DOCUMENT_SCHEMA_VERSION {
        return Err(CoreError::UnsupportedSchema {
            found: document.schema_version,
            supported: HUMAN_DOCUMENT_SCHEMA_VERSION,
        });
    }
    validate_nonempty(&document.id, 200, "Human Document ID")?;
    validate_nonempty(&document.project_id, 200, "Human Document project ID")?;
    validate_nonempty(&document.title, 500, "Human Document title")?;
    if document.source_ledger_sequence < 0
        || document.blocks.is_empty()
        || document.blocks.len() > MAX_BLOCKS
        || document.citations.len() > MAX_CITATIONS
        || document.sources.len() > MAX_CITATIONS
    {
        return Err(CoreError::Validation(
            "Human Document collection or ledger bounds are invalid".into(),
        ));
    }
    let mut source_keys = HashSet::new();
    for source in &document.sources {
        if !matches!(
            source.source_kind.as_str(),
            "entity" | "artifact" | "ai_candidate" | "external_proposal"
        ) || !source_keys.insert((source.source_kind.clone(), source.source_id.clone()))
        {
            return Err(CoreError::Validation(
                "Human Document sources must be unique and use known kinds".into(),
            ));
        }
        validate_nonempty(&source.source_id, 500, "Human Document source ID")?;
        validate_nonempty(&source.source_version, 500, "Human Document source version")?;
        if !audience_allows(document.audience, source.classification) {
            return Err(CoreError::Validation(
                "Human Document contains a source forbidden by its audience".into(),
            ));
        }
    }
    let mut citation_ids = HashSet::new();
    for citation in &document.citations {
        if !citation_ids.insert(citation.source_id.clone()) {
            return Err(CoreError::Validation("duplicate citation source ID".into()));
        }
        validate_text(&citation.title, 2_000, "citation title")?;
        if !source_keys.contains(&(citation.source_kind.clone(), citation.source_id.clone())) {
            return Err(CoreError::Validation(format!(
                "citation {} is outside the declared source boundary",
                citation.source_id
            )));
        }
    }
    let mut block_ids = HashSet::new();
    for block in &document.blocks {
        if !block_ids.insert(block.meta.id.clone()) {
            return Err(CoreError::Validation(
                "duplicate Human Document block ID".into(),
            ));
        }
        validate_text(&block.meta.id, 300, "block ID")?;
        validate_text(&block.meta.title, 1_000, "block title")?;
        validate_source_ids(&block.meta.source_ids, &citation_ids, "block")?;
        let reviewed_external = block.meta.contribution.candidate_id.as_ref().is_some_and(|id| {
            block.meta.source_ids.contains(id) && document.citations.iter().any(|citation| {
                citation.source_id == *id && citation.source_kind == "external_proposal" && citation.status == "accepted"
            })
        });
        validate_contribution(&block.meta.contribution, reviewed_external)?;
        validate_block(&block.content, &citation_ids)?;
    }
    for value in document.omissions.iter().chain(&document.limitations) {
        validate_text(value, MAX_TEXT, "omission or limitation")?;
    }
    bounded_json(
        &serde_json::to_value(document)?,
        MAX_DOCUMENT_BYTES,
        "Human Document",
    )?;
    Ok(())
}

pub fn render_human_document(document: &HumanDocument) -> Result<RenderedHumanDocument> {
    validate_human_document(document)?;
    let expected = material_fingerprint(document)?;
    if document.material_fingerprint != expected {
        return Err(CoreError::Validation(
            "Human Document material fingerprint is invalid".into(),
        ));
    }
    let html = render_html(document);
    let markdown = render_markdown(document);
    let asset_manifest = HumanDocumentAssetManifest {
        schema_version: 1,
        renderer_version: HUMAN_DOCUMENT_RENDERER_VERSION,
        template_version: HUMAN_DOCUMENT_TEMPLATE_VERSION,
        network_dependencies: Vec::new(),
        inline_style_sha256: sha256(STYLE.as_bytes()),
        interactive_renderer: "in-app: @xyflow/react + elkjs + mermaid; offline: bundled Mermaid 11.17.2 with pan/zoom and textual fallback"
            .into(),
    };
    Ok(RenderedHumanDocument {
        html_sha256: sha256(html.as_bytes()),
        markdown_sha256: sha256(markdown.as_bytes()),
        html,
        markdown,
        material_fingerprint: expected,
        asset_manifest,
    })
}

fn validate_request(request: &HumanDocumentRequest) -> Result<()> {
    if let Some(title) = &request.title {
        validate_text(title, 500, "report title")?;
    }
    if request.checkpoint_id.is_some() && !request.root_entity_ids.is_empty() {
        return Err(CoreError::Validation(
            "choose a Checkpoint or explicit roots, not both".into(),
        ));
    }
    if request.max_records == 0
        || request.max_records > MAX_RECORDS
        || request.max_graph_nodes == 0
        || request.max_graph_nodes > MAX_GRAPH_NODES
        || request.max_graph_edges == 0
        || request.max_graph_edges > MAX_GRAPH_EDGES
        || request.root_entity_ids.len() > 50
        || request.accepted_ai_candidate_ids.len() > MAX_AI_CANDIDATES
    {
        return Err(CoreError::Validation(
            "Human Document scope exceeds a declared CP8 bound".into(),
        ));
    }
    ensure_unique(&request.root_entity_ids, "report root")?;
    ensure_unique(&request.accepted_ai_candidate_ids, "AI candidate")?;
    Ok(())
}

fn select_entity_ids(
    connection: &Connection,
    project_id: &str,
    request: &HumanDocumentRequest,
    checkpoint: Option<&crate::Checkpoint>,
) -> Result<Vec<String>> {
    if let Some(checkpoint) = checkpoint {
        let mut statement = connection.prepare(
            "SELECT source_entity_id FROM checkpoint_sources WHERE checkpoint_id=?1 ORDER BY source_entity_id",
        )?;
        return statement
            .query_map([&checkpoint.id], |row| row.get(0))?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into);
    }
    if !request.root_entity_ids.is_empty() {
        let allowed_roots = request
            .root_entity_ids
            .iter()
            .map(|id| {
                let exists: bool = connection.query_row(
                    "SELECT EXISTS(SELECT 1 FROM entities WHERE id=?1 AND project_id=?2)",
                    params![id, project_id],
                    |row| row.get(0),
                )?;
                if !exists {
                    return Err(CoreError::NotFound(id.clone()));
                }
                Ok(id.clone())
            })
            .collect::<Result<Vec<_>>>()?;
        let allowed = allowed_roots.iter().cloned().collect::<HashSet<_>>();
        let mut seen = allowed.clone();
        let mut frontier = allowed_roots;
        for _ in 0..4 {
            let mut next = Vec::new();
            for current in frontier {
                let mut statement = connection.prepare(
                    "SELECT source_entity_id,target_entity_id FROM relationships WHERE project_id=?1 AND status='active' AND (source_entity_id=?2 OR target_entity_id=?2) ORDER BY created_at,id LIMIT 501",
                )?;
                let neighbors = statement
                    .query_map(params![project_id, current], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                for (source, target) in neighbors {
                    for id in [source, target] {
                        if seen.len() > request.max_records {
                            break;
                        }
                        if seen.insert(id.clone()) {
                            next.push(id);
                        }
                    }
                }
            }
            if next.is_empty() || seen.len() > request.max_records {
                break;
            }
            frontier = next;
        }
        return Ok(seen.into_iter().collect());
    }
    let mut statement = connection.prepare(
        "SELECT id FROM entities WHERE project_id=?1 AND status<>'archived' ORDER BY entity_type,created_at,id LIMIT ?2",
    )?;
    statement
        .query_map(params![project_id, request.max_records as i64 + 1], |row| {
            row.get(0)
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(Into::into)
}

#[derive(Debug, Clone)]
struct EvidencePreview {
    artifact_id: String,
    entity_title: String,
    media_type: String,
    sha256: String,
    classification: DataClassification,
    data_uri: String,
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or_default();
        let third = chunk.get(2).copied().unwrap_or_default();
        output.push(ALPHABET[(first >> 2) as usize] as char);
        output.push(ALPHABET[(((first & 0x03) << 4) | (second >> 4)) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[(((second & 0x0f) << 2) | (third >> 6)) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(third & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    output
}

fn collect_evidence_previews(
    store: &ContinuityStore,
    connection: &Connection,
    entities: &[(Entity, DataClassification)],
    audience: HumanDocumentAudience,
) -> HashMap<String, EvidencePreview> {
    const MAX_PREVIEWS: usize = 8;
    const MAX_PREVIEW_BYTES: u64 = 512 * 1024;
    if audience != HumanDocumentAudience::LocalProject {
        return HashMap::new();
    }
    let mut previews = HashMap::new();
    for (entity, classification) in entities
        .iter()
        .filter(|value| value.0.entity_type == "evidence")
    {
        if previews.len() >= MAX_PREVIEWS {
            break;
        }
        let artifact_id = connection
            .query_row(
                "SELECT original_artifact_id FROM evidence WHERE entity_id=?1",
                [&entity.id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()
            .ok()
            .flatten()
            .flatten();
        let Some(artifact_id) = artifact_id else {
            continue;
        };
        let Ok(artifact) = store.get_artifact(&artifact_id) else {
            continue;
        };
        if !matches!(
            artifact.media_type.as_str(),
            "image/png" | "image/jpeg" | "image/webp"
        ) || artifact.byte_size <= 0
            || artifact.byte_size as u64 > MAX_PREVIEW_BYTES
        {
            continue;
        }
        let Ok(bytes) = store.read_artifact_bounded(&artifact_id, MAX_PREVIEW_BYTES) else {
            continue;
        };
        previews.insert(
            entity.id.clone(),
            EvidencePreview {
                artifact_id,
                entity_title: entity.title.clone(),
                media_type: artifact.media_type.clone(),
                sha256: artifact.sha256,
                classification: *classification,
                data_uri: format!(
                    "data:{};base64,{}",
                    artifact.media_type,
                    base64_encode(&bytes)
                ),
            },
        );
    }
    previews
}

fn diagram_entity_label(connection: &Connection, entity: &Entity) -> Result<String> {
    if entity.entity_type != "evidence" {
        return Ok(bounded_text(&entity.title, 300));
    }
    let meaning = connection
        .query_row(
            "SELECT annotation_text,summary_text FROM evidence WHERE entity_id=?1",
            [&entity.id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?;
    let Some((annotation, summary)) = meaning else {
        return Ok("Evidence awaiting interpretation".into());
    };
    let annotation = annotation.trim();
    if !annotation.is_empty() {
        return Ok(bounded_text(annotation, 300));
    }
    let summary = summary.trim();
    if !summary.is_empty()
        && summary != "Captured screenshot; interpretation has not yet been reviewed."
        && summary != "Awaiting AI analysis and human review."
    {
        return Ok(bounded_text(summary, 300));
    }
    Ok("Screenshot awaiting AI interpretation".into())
}

fn compose_diagrams(
    connection: &Connection,
    project_id: &str,
    entities: &[(Entity, DataClassification)],
    previews: &HashMap<String, EvidencePreview>,
    max_nodes: usize,
    max_edges: usize,
) -> Result<(GraphSpecification, MermaidSpecification)> {
    // This is a project-comprehension view, not a dump of every parsed symbol.
    // Low-level code/test observations stay available in tables and provenance.
    let mut graph_entities = entities
        .iter()
        .filter(|value| graph_summary_entity(&value.0.entity_type))
        .collect::<Vec<_>>();
    if graph_entities.is_empty() {
        graph_entities = entities.iter().collect();
    }
    graph_entities.sort_by(|left, right| {
        graph_entity_priority(&left.0.entity_type)
            .cmp(&graph_entity_priority(&right.0.entity_type))
            .then_with(|| left.0.created_at.cmp(&right.0.created_at))
            .then_with(|| left.0.id.cmp(&right.0.id))
    });
    graph_entities.truncate(max_nodes);
    let node_ids = graph_entities
        .iter()
        .map(|value| value.0.id.clone())
        .collect::<HashSet<_>>();
    let nodes = graph_entities
        .iter()
        .map(|value| {
            let preview = previews.get(&value.0.id);
            let mut source_ids = vec![value.0.id.clone()];
            if let Some(preview) = preview {
                source_ids.push(preview.artifact_id.clone());
            }
            Ok(DiagramNode {
                id: value.0.id.clone(),
                label: diagram_entity_label(connection, &value.0)?,
                kind: value.0.entity_type.clone(),
                status: value.0.status.clone(),
                source_ids,
                preview_data_uri: preview.map(|value| value.data_uri.clone()),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let mut statement = connection.prepare(
        "SELECT id,source_entity_id,target_entity_id,relation_type,direct_source_ids_json FROM relationships WHERE project_id=?1 AND status='active' ORDER BY created_at,id LIMIT 5001",
    )?;
    let relationships = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let mut all_matching = Vec::new();
    for (id, source, target, label, direct_json) in relationships {
        if node_ids.contains(&source) && node_ids.contains(&target) {
            let mut source_ids = vec![source.clone(), target.clone()];
            source_ids.extend(serde_json::from_str::<Vec<String>>(&direct_json)?);
            source_ids.retain(|id| node_ids.contains(id));
            source_ids.sort();
            source_ids.dedup();
            all_matching.push(DiagramEdge {
                id,
                source,
                target,
                label,
                source_ids,
            });
        }
    }
    let entity_types = graph_entities
        .iter()
        .map(|value| (value.0.id.as_str(), value.0.entity_type.as_str()))
        .collect::<HashMap<_, _>>();
    let mut memberships = connection.prepare(
        "SELECT session_entity_id,entity_id FROM research_session_items ORDER BY added_at,entity_id",
    )?;
    for membership in memberships
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?
    {
        let (session, item) = membership;
        if !node_ids.contains(&session) || !node_ids.contains(&item) {
            continue;
        }
        let label = match entity_types.get(item.as_str()).copied().unwrap_or_default() {
            "research_question" => "research question",
            "evidence" => "evidence collected",
            "experiment" => "experiment",
            "result" => "observed result",
            "finding" => "research finding",
            "decision" => "decision",
            "requirement" => "development requirement",
            _ => "part of this work",
        };
        all_matching.push(DiagramEdge {
            id: format!("session-item-{session}-{item}"),
            source: session.clone(),
            target: item.clone(),
            label: label.into(),
            source_ids: vec![session, item],
        });
    }
    let truncated = entities.len() > nodes.len() || all_matching.len() > max_edges;
    all_matching.truncate(max_edges);
    let textual_alternative = diagram_alternative(&nodes, &all_matching, truncated);
    let graph = GraphSpecification {
        renderer: "react_flow_elk".into(),
        layout: "org.eclipse.elk.layered".into(),
        direction: DiagramDirection::LeftRight,
        nodes: nodes.clone(),
        edges: all_matching.clone(),
        truncated,
        textual_alternative: textual_alternative.clone(),
    };
    let mermaid_nodes = nodes
        .into_iter()
        .map(|mut node| {
            node.preview_data_uri = None;
            node
        })
        .take(MAX_MERMAID_NODES)
        .collect::<Vec<_>>();
    let mermaid_ids = mermaid_nodes
        .iter()
        .map(|node| node.id.clone())
        .collect::<HashSet<_>>();
    let mermaid_edges = all_matching
        .into_iter()
        .filter(|edge| mermaid_ids.contains(&edge.source) && mermaid_ids.contains(&edge.target))
        .take(MAX_MERMAID_EDGES)
        .collect::<Vec<_>>();
    let mermaid = MermaidSpecification {
        diagram_kind: "flowchart".into(),
        direction: DiagramDirection::LeftRight,
        textual_alternative: diagram_alternative(&mermaid_nodes, &mermaid_edges, truncated),
        nodes: mermaid_nodes,
        edges: mermaid_edges,
    };
    Ok((graph, mermaid))
}

fn diagram_from_candidate(value: &Value) -> Result<MermaidSpecification> {
    let object = value
        .as_object()
        .ok_or_else(|| CoreError::Validation("diagram candidate must be an object".into()))?;
    let raw_nodes = object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| CoreError::Validation("diagram candidate requires nodes".into()))?;
    let raw_edges = object
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| CoreError::Validation("diagram candidate requires edges".into()))?;
    if raw_nodes.is_empty()
        || raw_nodes.len() > MAX_MERMAID_NODES
        || raw_edges.len() > MAX_MERMAID_EDGES
    {
        return Err(CoreError::Validation(
            "diagram candidate exceeds the bounded portable diagram contract".into(),
        ));
    }
    let mut ids = HashSet::new();
    let mut nodes = Vec::new();
    for node in raw_nodes {
        let id = required_string(node, "id", 200)?;
        if !ids.insert(id.clone()) {
            return Err(CoreError::Validation("duplicate diagram node ID".into()));
        }
        nodes.push(DiagramNode {
            id: id.clone(),
            label: required_string(node, "label", 300)?,
            kind: optional_string(node, "kind", 100).unwrap_or_else(|| "concept".into()),
            status: optional_string(node, "status", 100).unwrap_or_else(|| "proposed".into()),
            source_ids: string_array(node, "source_ids", 20)?,
            preview_data_uri: None,
        });
    }
    let mut edges = Vec::new();
    for (index, edge) in raw_edges.iter().enumerate() {
        let source = required_string(edge, "source", 200)?;
        let target = required_string(edge, "target", 200)?;
        if !ids.contains(&source) || !ids.contains(&target) {
            return Err(CoreError::Validation(
                "diagram edge references an unknown node".into(),
            ));
        }
        edges.push(DiagramEdge {
            id: optional_string(edge, "id", 200).unwrap_or_else(|| format!("edge-{index}")),
            source,
            target,
            label: optional_string(edge, "label", 200).unwrap_or_default(),
            source_ids: string_array(edge, "source_ids", 20)?,
        });
    }
    let alternative = object
        .get("textual_alternative")
        .and_then(Value::as_str)
        .map(|value| bounded_text(value, 10_000))
        .unwrap_or_else(|| diagram_alternative(&nodes, &edges, false));
    Ok(MermaidSpecification {
        diagram_kind: "flowchart".into(),
        direction: match object.get("direction").and_then(Value::as_str) {
            Some("top_down") => DiagramDirection::TopDown,
            _ => DiagramDirection::LeftRight,
        },
        nodes,
        edges,
        textual_alternative: alternative,
    })
}

fn validate_proposal_diagram_sources(
    specification: &MermaidSpecification,
    proposal_sources: &[String],
) -> Result<()> {
    let declared = proposal_sources
        .iter()
        .map(String::as_str)
        .collect::<HashSet<_>>();
    let valid = specification
        .nodes
        .iter()
        .flat_map(|node| &node.source_ids)
        .chain(specification.edges.iter().flat_map(|edge| &edge.source_ids))
        .all(|source| declared.contains(source.as_str()));
    if !valid {
        return Err(CoreError::Validation(
            "reviewed diagram cites a source not declared by its AI proposal".into(),
        ));
    }
    Ok(())
}

fn render_html(document: &HumanDocument) -> String {
    let mut html = String::with_capacity(64 * 1024);
    let runtime = MERMAID_RUNTIME.replace("</script", "<\\/script");
    let scripts = format!("<script>{runtime}</script><script>{REPORT_INTERACTIONS}</script>");
    let script_policy = format!("script-src 'sha256-{}' 'sha256-{}'; ", base64_encode(&Sha256::digest(runtime.as_bytes())), base64_encode(&Sha256::digest(REPORT_INTERACTIONS.as_bytes())));
    write!(
        html,
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src 'none'; connect-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'\"><title>{}</title><style>{}</style></head><body><a class=\"skip\" href=\"#main\">Skip to report</a><header><p class=\"eyebrow\">Continuum project report</p><h1>{}</h1><p class=\"lead\">A readable explanation of what was researched, what the Evidence supports, what was decided, and how the project changed.</p><p class=\"meta\">Updated {} · Privacy {}</p><details><summary>Verification details</summary><p>Ledger position {} · {} audience · {} report. Canonical project records remain authoritative.</p></details></header><main id=\"main\">",
        escape_html(&document.title),
        STYLE,
        escape_html(&document.title),
        escape_html(&document.generated_at),
        escape_html(document.classification.as_str()),
        document.source_ledger_sequence,
        escape_html(document.audience.as_str()),
        escape_html(&humanize(document.kind.as_str()))
    )
    .expect("writing to String cannot fail");
    if !document.omissions.is_empty() || !document.limitations.is_empty() {
        html.push_str("<section aria-labelledby=\"scope-notes\" class=\"callout warning\"><h2 id=\"scope-notes\">Scope, omissions, and limitations</h2><ul>");
        for item in document.omissions.iter().chain(&document.limitations) {
            write!(html, "<li>{}</li>", escape_html(item)).expect("writing to String cannot fail");
        }
        html.push_str("</ul></section>");
    }
    html.push_str("<nav class=\"report-toc\" aria-label=\"Report contents\"><strong>In this report</strong><div>");
    for block in &document.blocks {
        write!(
            html,
            "<a href=\"#{}\">{}</a>",
            safe_anchor(&block.meta.id),
            escape_html(&block.meta.title)
        )
        .expect("writing to String cannot fail");
    }
    html.push_str("</div></nav>");
    for block in &document.blocks {
        render_html_block(&mut html, block, &document.citations);
    }
    write!(
        html,
        "</main><footer><p>Material fingerprint <code>{}</code></p><p>Human Document schema v{} · renderer v{} · template v{}</p></footer></body></html>",
        escape_html(&document.material_fingerprint),
        document.schema_version,
        HUMAN_DOCUMENT_RENDERER_VERSION,
        HUMAN_DOCUMENT_TEMPLATE_VERSION
    )
    .expect("writing to String cannot fail");
    // Only the two bundled scripts are executable. All project content remains
    // escaped text, including Mermaid source; network access stays disabled.
    html = html.replacen("default-src 'none'; ", &format!("default-src 'none'; {script_policy}"), 1);
    html = html.replacen("</body>", &format!("{scripts}</body>"), 1);
    html
}

fn render_html_block(output: &mut String, block: &HumanDocumentBlock, citations: &[HumanCitation]) {
    let id = escape_html(&block.meta.id);
    let title = escape_html(&block.meta.title);
    let authorship = escape_html(contribution_label(block.meta.contribution.kind));
    write!(output, "<section id=\"{}\" aria-labelledby=\"{}-title\"><div class=\"section-heading\"><h2 id=\"{}-title\">{}</h2><span class=\"badge\">{}</span></div>", id, id, id, title, authorship)
        .expect("writing to String cannot fail");
    match &block.content {
        BlockContent::Overview {
            summary,
            status,
            blockers,
            next_actions,
        } => {
            write!(
                output,
                "<p class=\"lead\">{}</p><p>Status: <strong>{}</strong></p>",
                escape_html(summary),
                escape_html(status)
            )
            .expect("writing to String cannot fail");
            render_html_list(output, "Blockers", blockers);
            render_html_list(output, "Next actions", next_actions);
        }
        BlockContent::StatusCards { cards } => {
            output.push_str("<div class=\"cards\">");
            for card in cards {
                write!(
                    output,
                    "<article class=\"card state-{}\"><span>{}</span><strong>{}</strong></article>",
                    escape_html(&card.state),
                    escape_html(&card.label),
                    escape_html(&card.value)
                )
                .expect("writing to String cannot fail");
            }
            output.push_str("</div>");
        }
        BlockContent::Heading { level } => {
            write!(output, "<p>Heading level {level} marker.</p>")
                .expect("writing to String cannot fail");
        }
        BlockContent::Prose { text } => {
            write!(output, "<p>{}</p>", escape_html(text)).expect("writing to String cannot fail");
        }
        BlockContent::Callout { tone, text } => {
            write!(
                output,
                "<aside class=\"callout {}\"><strong>{}</strong><p>{}</p></aside>",
                tone.as_str(),
                escape_html(&humanize(tone.as_str())),
                escape_html(text)
            )
            .expect("writing to String cannot fail");
        }
        BlockContent::ResearchSynthesis {
            summary,
            key_points,
            limitations,
            recommendations,
        } => {
            write!(output, "<div class=\"research-synthesis\"><p class=\"lead\">{}</p><div class=\"synthesis-grid\" style=\"display:grid;grid-template-columns:repeat(auto-fit,minmax(18rem,1fr));gap:1rem\">", escape_html(summary)).expect("writing to String cannot fail");
            render_html_synthesis_list(output, "What the evidence shows", key_points);
            render_html_synthesis_list(output, "Recommended next steps", recommendations);
            output.push_str("</div>");
            if !limitations.is_empty() {
                output.push_str("<aside class=\"callout uncertainty\">");
                render_html_list(output, "What still needs verification", limitations);
                output.push_str("</aside>");
            }
            output.push_str("</div>");
        }
        BlockContent::KeyValue { facts } => {
            output.push_str("<dl class=\"facts\">");
            for fact in facts {
                write!(
                    output,
                    "<div><dt>{}</dt><dd>{}</dd></div>",
                    escape_html(&fact.key),
                    escape_html(&fact.value)
                )
                .expect("writing to String cannot fail");
            }
            output.push_str("</dl>");
        }
        BlockContent::Table {
            columns,
            rows,
            truncated,
        } => {
            output.push_str("<div class=\"table-wrap\" tabindex=\"0\" role=\"region\" aria-label=\"Scrollable data table\"><table><caption>");
            output.push_str(&title);
            output.push_str("</caption><thead><tr>");
            for column in columns {
                write!(
                    output,
                    "<th scope=\"col\" data-column=\"{}\">{}</th>",
                    escape_html(&column.key),
                    escape_html(&column.label)
                )
                .expect("writing to String cannot fail");
            }
            output.push_str("</tr></thead><tbody>");
            for row in rows {
                output.push_str("<tr>");
                for (index, column) in columns.iter().enumerate() {
                    let value = row.cells.get(&column.key).map(String::as_str).unwrap_or("");
                    if index == 0 {
                        write!(
                            output,
                            "<th scope=\"row\" data-column=\"{}\">{}</th>",
                            escape_html(&column.key),
                            escape_html(value)
                        )
                        .expect("writing to String cannot fail");
                    } else {
                        write!(
                            output,
                            "<td data-column=\"{}\">{}</td>",
                            escape_html(&column.key),
                            escape_html(value)
                        )
                        .expect("writing to String cannot fail");
                    }
                }
                output.push_str("</tr>");
            }
            output.push_str("</tbody></table></div>");
            if *truncated {
                output.push_str(
                    "<p class=\"warning-text\">Table is truncated at its declared limit.</p>",
                );
            }
        }
        BlockContent::Timeline { entries, truncated } => {
            output.push_str("<ol class=\"timeline\">");
            for entry in entries {
                write!(
                    output,
                    "<li><time>{}</time><strong>{}</strong><p>{}</p></li>",
                    escape_html(&entry.occurred_at),
                    escape_html(&entry.label),
                    escape_html(&entry.detail)
                )
                .expect("writing to String cannot fail");
            }
            output.push_str("</ol>");
            if *truncated {
                output.push_str("<p class=\"warning-text\">Timeline is truncated.</p>");
            }
        }
        BlockContent::CitationList { citation_ids } => {
            output.push_str("<ol class=\"citations\">");
            for citation_id in citation_ids {
                if let Some(citation) = citations.iter().find(|item| &item.source_id == citation_id)
                {
                    write!(output, "<li id=\"source-{}\"><strong>{}</strong><br><span>{} · {} · version {} · {}</span><details><summary>Technical reference</summary><code>{}</code></details></li>", safe_anchor(&citation.source_id), escape_html(&citation.title), escape_html(&citation.source_type), escape_html(&citation.status), escape_html(&citation.version), escape_html(citation.classification.as_str()), escape_html(&citation.source_id)).expect("writing to String cannot fail");
                }
            }
            output.push_str("</ol>");
        }
        BlockContent::ArtifactReference {
            artifact_id,
            media_type,
            availability,
            description,
        } => {
            write!(output, "<article class=\"artifact\"><p>{}</p><dl><dt>Artifact ID</dt><dd><code>{}</code></dd><dt>Media type</dt><dd>{}</dd><dt>Availability</dt><dd>{}</dd></dl></article>", escape_html(description), escape_html(artifact_id), escape_html(media_type), escape_html(availability)).expect("writing to String cannot fail");
        }
        BlockContent::MermaidDiagram { specification } => render_diagram_fallback(
            output,
            &specification.nodes,
            &specification.edges,
            specification.direction,
            &specification.textual_alternative,
            "mermaid",
        ),
        BlockContent::Graph { specification } => render_diagram_fallback(
            output,
            &specification.nodes,
            &specification.edges,
            specification.direction,
            &specification.textual_alternative,
            "react-flow-elk",
        ),
        BlockContent::DetailGroup { summary, details } => {
            write!(
                output,
                "<details><summary>{}</summary>",
                escape_html(summary)
            )
            .expect("writing to String cannot fail");
            for detail in details {
                write!(output, "<p>{}</p>", escape_html(detail))
                    .expect("writing to String cannot fail");
            }
            output.push_str("</details>");
        }
    }
    render_source_links(output, &block.meta.source_ids);
    output.push_str("</section>");
}

fn render_html_synthesis_list(output: &mut String, title: &str, values: &[String]) {
    output.push_str("<div style=\"border:1px solid var(--line);border-radius:12px;padding:1rem\">");
    render_html_list(output, title, values);
    output.push_str("</div>");
}

fn render_diagram_fallback(
    output: &mut String,
    nodes: &[DiagramNode],
    edges: &[DiagramEdge],
    direction: DiagramDirection,
    alternative: &str,
    renderer: &str,
) {
    let mut layers = nodes
        .iter()
        .map(|node| (node.id.clone(), 0usize))
        .collect::<HashMap<_, _>>();
    for _ in 0..nodes.len() {
        let snapshot = layers.clone();
        let mut changed = false;
        for edge in edges {
            let next = snapshot
                .get(&edge.source)
                .copied()
                .unwrap_or_default()
                .saturating_add(1);
            let target = layers.entry(edge.target.clone()).or_default();
            let bounded = next.min(nodes.len().saturating_sub(1));
            if bounded > *target {
                *target = bounded;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let mut columns = BTreeMap::<usize, Vec<&DiagramNode>>::new();
    for node in nodes {
        columns
            .entry(*layers.get(&node.id).unwrap_or(&0))
            .or_default()
            .push(node);
    }
    let node_width = 282usize;
    let mut positions = HashMap::<String, (usize, usize, usize)>::new();
    let mut width = 760usize;
    let mut max_height = 320usize;
    match direction {
        DiagramDirection::LeftRight => {
            for (layer, column) in &columns {
                let mut y = 34usize;
                for node in column {
                    // A provenance thumbnail helps recognition without turning
                    // the semantic map back into a screenshot wall.
                    let height = if node.preview_data_uri.is_some() {
                        154
                    } else {
                        118
                    };
                    let x = 34 + layer * 408;
                    positions.insert(node.id.clone(), (x, y, height));
                    width = width.max(x + node_width + 72);
                    y += height + 64;
                }
                max_height = max_height.max(y + 30);
            }
        }
        DiagramDirection::TopDown => {
            for (layer, row) in &columns {
                let y = 34 + layer * 238;
                for (index, node) in row.iter().enumerate() {
                    let height = if node.preview_data_uri.is_some() {
                        154
                    } else {
                        118
                    };
                    let x = 34 + index * (node_width + 76);
                    positions.insert(node.id.clone(), (x, y, height));
                    width = width.max(x + node_width + 72);
                    max_height = max_height.max(y + height + 74);
                }
            }
        }
    }
    write!(output, "<figure data-renderer=\"{}\"><div class=\"diagram-tools\"><span>Drag to explore</span><button type=\"button\" data-diagram=\"out\" aria-label=\"Zoom out\">−</button><output class=\"diagram-status\">Preparing diagram…</output><button type=\"button\" data-diagram=\"in\" aria-label=\"Zoom in\">+</button><button type=\"button\" data-diagram=\"fit\">Fit</button><button type=\"button\" data-diagram=\"full\">Fullscreen</button></div><pre class=\"mermaid-source\" hidden>{}</pre><div class=\"diagram-scroll\" tabindex=\"0\" aria-label=\"Scrollable research diagram\"><svg class=\"relationship-map\" viewBox=\"0 0 {} {}\" role=\"img\" aria-label=\"{}\" xmlns=\"http://www.w3.org/2000/svg\"><defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"9\" refY=\"5\" markerWidth=\"7\" markerHeight=\"7\" orient=\"auto-start-reverse\"><path d=\"M 0 0 L 10 5 L 0 10 z\"/></marker></defs>", escape_html(renderer), escape_html(&mermaid_source(nodes, edges, direction)), width, max_height, escape_html(alternative)).expect("writing to String cannot fail");
    for edge in edges {
        let Some((source_x, source_y, source_height)) = positions.get(&edge.source) else {
            continue;
        };
        let Some((target_x, target_y, target_height)) = positions.get(&edge.target) else {
            continue;
        };
        let label = humanize(&edge.label);
        let label_width = label.chars().count().min(42) * 7 + 22;
        let (path, label_x, label_y) = match direction {
            DiagramDirection::LeftRight => {
                let x1 = source_x + node_width;
                let y1 = source_y + source_height / 2;
                let x2 = *target_x;
                let y2 = target_y + target_height / 2;
                let middle = (x1 + x2) / 2;
                (
                    format!("M {x1} {y1} C {middle} {y1}, {middle} {y2}, {x2} {y2}"),
                    middle,
                    (y1 + y2) / 2,
                )
            }
            DiagramDirection::TopDown => {
                let x1 = source_x + node_width / 2;
                let y1 = source_y + source_height;
                let x2 = target_x + node_width / 2;
                let y2 = *target_y;
                let middle = (y1 + y2) / 2;
                (
                    format!("M {x1} {y1} C {x1} {middle}, {x2} {middle}, {x2} {y2}"),
                    (x1 + x2) / 2,
                    middle,
                )
            }
        };
        write!(output, "<path class=\"relationship-line\" d=\"{}\" marker-end=\"url(#arrow)\"/><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"24\" rx=\"7\" fill=\"var(--panel)\" stroke=\"var(--line)\"/><text class=\"relationship-label\" x=\"{label_x}\" y=\"{}\" text-anchor=\"middle\">{}</text>", escape_html(&path), label_x.saturating_sub(label_width / 2), label_y.saturating_sub(16), label_width, label_y + 4, escape_html(&label)).expect("writing to String cannot fail");
    }
    for node in nodes {
        let Some((x, y, height)) = positions.get(&node.id) else {
            continue;
        };
        write!(output, "<foreignObject x=\"{x}\" y=\"{y}\" width=\"{node_width}\" height=\"{height}\"><div xmlns=\"http://www.w3.org/1999/xhtml\" class=\"diagram-node kind-{}\">", escape_html(&node.kind)).expect("writing to String cannot fail");
        if let Some(preview) = &node.preview_data_uri {
            write!(
                output,
                "<img src=\"{}\" alt=\"Evidence image: {}\"/>",
                escape_html(preview),
                escape_html(&node.label)
            )
            .expect("writing to String cannot fail");
        }
        write!(
            output,
            "<small>{} · {}</small><strong>{}</strong></div></foreignObject>",
            escape_html(&humanize(&node.kind)),
            escape_html(&humanize(&node.status)),
            escape_html(&node.label)
        )
        .expect("writing to String cannot fail");
    }
    write!(output, "</svg></div><figcaption>{}</figcaption><details><summary>Read the connections</summary><ul>", escape_html(alternative)).expect("String write");
    for edge in edges {
        let source = nodes
            .iter()
            .find(|node| node.id == edge.source)
            .map(|node| node.label.as_str())
            .unwrap_or(&edge.source);
        let target = nodes
            .iter()
            .find(|node| node.id == edge.target)
            .map(|node| node.label.as_str())
            .unwrap_or(&edge.target);
        write!(
            output,
            "<li>{} — {} → {}</li>",
            escape_html(source),
            escape_html(&edge.label),
            escape_html(target)
        )
        .expect("writing to String cannot fail");
    }
    output.push_str("</ul></details>");
    if nodes.iter().any(|node| node.preview_data_uri.is_some()) {
        output.push_str("<details><summary>View evidence images</summary><div class=\"evidence-gallery\">");
        for node in nodes {
            if let Some(preview) = &node.preview_data_uri {
                write!(output,"<figure><img loading=\"lazy\" src=\"{}\" alt=\"{}\"><figcaption>{}</figcaption></figure>",escape_html(preview),escape_html(&node.label),escape_html(&node.label)).expect("String write");
            }
        }
        output.push_str("</div></details>");
    }
    output.push_str("</figure>");
}

fn render_markdown(document: &HumanDocument) -> String {
    let mut output = format!(
        "# {}\n\n- Kind: `{}`\n- Project: `{}`\n- Source ledger sequence: `{}`\n- Audience: `{}`\n- Privacy: `{}`\n- Material fingerprint: `{}`\n\n> This is a derived Human Document. Canonical project records remain authoritative.\n\n",
        markdown_text(&document.title),
        document.kind.as_str(),
        document.project_id,
        document.source_ledger_sequence,
        document.audience.as_str(),
        document.classification.as_str(),
        document.material_fingerprint
    );
    if !document.omissions.is_empty() || !document.limitations.is_empty() {
        output.push_str("## Scope, omissions, and limitations\n\n");
        for item in document.omissions.iter().chain(&document.limitations) {
            writeln!(output, "- {}", markdown_text(item)).expect("writing to String cannot fail");
        }
        output.push('\n');
    }
    for block in &document.blocks {
        writeln!(output, "## {}\n", markdown_text(&block.meta.title))
            .expect("writing to String cannot fail");
        writeln!(
            output,
            "_Contribution: {}_\n",
            block.meta.contribution.kind.as_str()
        )
        .expect("writing to String cannot fail");
        match &block.content {
            BlockContent::Overview {
                summary,
                status,
                blockers,
                next_actions,
            } => {
                writeln!(
                    output,
                    "{}\n\n**Status:** `{}`\n",
                    markdown_text(summary),
                    markdown_text(status)
                )
                .expect("writing to String cannot fail");
                render_markdown_list(&mut output, "Blockers", blockers);
                render_markdown_list(&mut output, "Next actions", next_actions);
            }
            BlockContent::StatusCards { cards } => {
                for card in cards {
                    writeln!(
                        output,
                        "- **{}:** {} (`{}`)",
                        markdown_text(&card.label),
                        markdown_text(&card.value),
                        markdown_text(&card.state)
                    )
                    .expect("writing to String cannot fail");
                }
            }
            BlockContent::Heading { level } => {
                writeln!(output, "Heading level {level} marker.")
                    .expect("writing to String cannot fail");
            }
            BlockContent::Prose { text } => {
                writeln!(output, "{}", markdown_text(text)).expect("writing to String cannot fail");
            }
            BlockContent::Callout { tone, text } => {
                writeln!(
                    output,
                    "> **{}:** {}",
                    humanize(tone.as_str()),
                    markdown_text(text)
                )
                .expect("writing to String cannot fail");
            }
            BlockContent::ResearchSynthesis {
                summary,
                key_points,
                limitations,
                recommendations,
            } => {
                writeln!(output, "{}\n", markdown_text(summary))
                    .expect("writing to String cannot fail");
                render_markdown_list(&mut output, "What the evidence shows", key_points);
                render_markdown_list(&mut output, "Recommended next steps", recommendations);
                if !limitations.is_empty() {
                    render_markdown_list(&mut output, "What still needs verification", limitations);
                }
            }
            BlockContent::KeyValue { facts } => {
                for fact in facts {
                    writeln!(
                        output,
                        "- **{}:** {}",
                        markdown_text(&fact.key),
                        markdown_text(&fact.value)
                    )
                    .expect("writing to String cannot fail");
                }
            }
            BlockContent::Table {
                columns,
                rows,
                truncated,
            } => {
                writeln!(
                    output,
                    "| {} |",
                    columns
                        .iter()
                        .map(|column| markdown_table_text(&column.label))
                        .collect::<Vec<_>>()
                        .join(" | ")
                )
                .expect("writing to String cannot fail");
                writeln!(
                    output,
                    "| {} |",
                    columns
                        .iter()
                        .map(|_| "---")
                        .collect::<Vec<_>>()
                        .join(" | ")
                )
                .expect("writing to String cannot fail");
                for row in rows {
                    writeln!(
                        output,
                        "| {} |",
                        columns
                            .iter()
                            .map(|column| markdown_table_text(
                                row.cells.get(&column.key).map(String::as_str).unwrap_or("")
                            ))
                            .collect::<Vec<_>>()
                            .join(" | ")
                    )
                    .expect("writing to String cannot fail");
                }
                if *truncated {
                    output.push_str("\n_Table truncated at declared limit._\n");
                }
            }
            BlockContent::Timeline { entries, truncated } => {
                for entry in entries {
                    writeln!(
                        output,
                        "1. **{}** — {} — {}",
                        markdown_text(&entry.occurred_at),
                        markdown_text(&entry.label),
                        markdown_text(&entry.detail)
                    )
                    .expect("writing to String cannot fail");
                }
                if *truncated {
                    output.push_str("\n_Timeline truncated._\n");
                }
            }
            BlockContent::CitationList { citation_ids } => {
                for id in citation_ids {
                    if let Some(citation) = document
                        .citations
                        .iter()
                        .find(|value| &value.source_id == id)
                    {
                        writeln!(
                            output,
                            "- **{}** — `{}` — {} / {} / version {} / {}",
                            markdown_text(&citation.title),
                            markdown_text(&citation.source_id),
                            markdown_text(&citation.source_type),
                            markdown_text(&citation.status),
                            markdown_text(&citation.version),
                            citation.classification.as_str()
                        )
                        .expect("writing to String cannot fail");
                    }
                }
            }
            BlockContent::ArtifactReference {
                artifact_id,
                media_type,
                availability,
                description,
            } => {
                writeln!(
                    output,
                    "- {} — `{}` — `{}` — `{}`",
                    markdown_text(description),
                    markdown_text(artifact_id),
                    markdown_text(media_type),
                    markdown_text(availability)
                )
                .expect("writing to String cannot fail");
            }
            BlockContent::MermaidDiagram { specification } => render_markdown_diagram(
                &mut output,
                &specification.nodes,
                &specification.edges,
                specification.direction,
                &specification.textual_alternative,
            ),
            BlockContent::Graph { specification } => render_markdown_diagram(
                &mut output,
                &specification.nodes,
                &specification.edges,
                specification.direction,
                &specification.textual_alternative,
            ),
            BlockContent::DetailGroup { summary, details } => {
                writeln!(output, "**{}**", markdown_text(summary))
                    .expect("writing to String cannot fail");
                for detail in details {
                    writeln!(output, "- {}", markdown_text(detail))
                        .expect("writing to String cannot fail");
                }
            }
        }
        render_markdown_sources(&mut output, &block.meta.source_ids);
        output.push('\n');
    }
    output
}

fn validate_block(content: &BlockContent, citation_ids: &HashSet<String>) -> Result<()> {
    match content {
        BlockContent::Overview {
            summary,
            blockers,
            next_actions,
            ..
        } => {
            validate_text(summary, MAX_TEXT, "overview summary")?;
            validate_strings(blockers, 200, 10_000, "blocker")?;
            validate_strings(next_actions, 200, 10_000, "next action")?;
        }
        BlockContent::StatusCards { cards } => {
            if cards.len() > 200 {
                return Err(CoreError::Validation("too many status cards".into()));
            }
            for card in cards {
                validate_text(&card.label, 500, "card label")?;
                validate_text(&card.value, 500, "card value")?;
                validate_source_ids(&card.source_ids, citation_ids, "status card")?;
            }
        }
        BlockContent::Heading { level } if !(2..=6).contains(level) => {
            return Err(CoreError::Validation(
                "heading level must be within 2..=6".into(),
            ));
        }
        BlockContent::Heading { .. } => {}
        BlockContent::Prose { text } | BlockContent::Callout { text, .. } => {
            validate_text(text, MAX_TEXT, "prose")?
        }
        BlockContent::ResearchSynthesis {
            summary,
            key_points,
            limitations,
            recommendations,
        } => {
            validate_text(summary, MAX_TEXT, "research synthesis")?;
            validate_strings(key_points, 100, 10_000, "research synthesis key point")?;
            validate_strings(limitations, 100, 10_000, "research synthesis limitation")?;
            validate_strings(
                recommendations,
                100,
                10_000,
                "research synthesis recommendation",
            )?;
        }
        BlockContent::KeyValue { facts } => {
            if facts.len() > 500 {
                return Err(CoreError::Validation("too many facts".into()));
            }
            for fact in facts {
                validate_text(&fact.key, 500, "fact key")?;
                validate_text(&fact.value, MAX_CELL_TEXT, "fact value")?;
                validate_source_ids(&fact.source_ids, citation_ids, "fact")?;
            }
        }
        BlockContent::Table { columns, rows, .. } => validate_table(columns, rows, citation_ids)?,
        BlockContent::Timeline { entries, .. } => {
            if entries.len() > 1_000 {
                return Err(CoreError::Validation(
                    "timeline exceeds 1000 entries".into(),
                ));
            }
            for entry in entries {
                validate_text(&entry.label, 1_000, "timeline label")?;
                validate_text(&entry.detail, MAX_CELL_TEXT, "timeline detail")?;
                validate_source_ids(&entry.source_ids, citation_ids, "timeline entry")?;
            }
        }
        BlockContent::CitationList { citation_ids: ids } => {
            validate_source_ids(ids, citation_ids, "citation list")?
        }
        BlockContent::ArtifactReference {
            artifact_id,
            media_type,
            availability,
            description,
        } => {
            validate_text(artifact_id, 500, "artifact ID")?;
            validate_text(media_type, 200, "media type")?;
            validate_text(availability, 100, "availability")?;
            validate_text(description, MAX_TEXT, "artifact description")?;
        }
        BlockContent::MermaidDiagram { specification } => {
            validate_mermaid(specification, citation_ids)?
        }
        BlockContent::Graph { specification } => validate_graph(specification, citation_ids)?,
        BlockContent::DetailGroup { summary, details } => {
            validate_text(summary, 2_000, "detail summary")?;
            validate_strings(details, 500, MAX_TEXT, "detail")?;
        }
    }
    Ok(())
}

fn validate_table(
    columns: &[DocumentTableColumn],
    rows: &[DocumentTableRow],
    citations: &HashSet<String>,
) -> Result<()> {
    if columns.is_empty() || columns.len() > 20 || rows.len() > 1_000 {
        return Err(CoreError::Validation(
            "table dimensions exceed CP8 limits".into(),
        ));
    }
    let mut keys = HashSet::new();
    for column in columns {
        validate_text(&column.key, 100, "column key")?;
        validate_text(&column.label, 500, "column label")?;
        if !keys.insert(column.key.clone()) {
            return Err(CoreError::Validation("duplicate table column".into()));
        }
    }
    let mut row_ids = HashSet::new();
    for row in rows {
        if !row_ids.insert(row.id.clone()) {
            return Err(CoreError::Validation("duplicate table row".into()));
        }
        validate_source_ids(&row.source_ids, citations, "table row")?;
        for (key, value) in &row.cells {
            if !keys.contains(key) {
                return Err(CoreError::Validation(
                    "table row contains an unknown column".into(),
                ));
            }
            validate_text(value, MAX_CELL_TEXT, "table cell")?;
        }
    }
    Ok(())
}

fn validate_graph(graph: &GraphSpecification, citations: &HashSet<String>) -> Result<()> {
    if graph.renderer != "react_flow_elk"
        || graph.layout != "org.eclipse.elk.layered"
        || graph.nodes.len() > MAX_GRAPH_NODES
        || graph.edges.len() > MAX_GRAPH_EDGES
    {
        return Err(CoreError::Validation(
            "graph renderer/layout or dimensions are invalid".into(),
        ));
    }
    validate_diagram_parts(&graph.nodes, &graph.edges, citations)?;
    validate_text(&graph.textual_alternative, 50_000, "graph alternative")
}

fn validate_mermaid(graph: &MermaidSpecification, citations: &HashSet<String>) -> Result<()> {
    if graph.diagram_kind != "flowchart"
        || graph.nodes.len() > MAX_MERMAID_NODES
        || graph.edges.len() > MAX_MERMAID_EDGES
    {
        return Err(CoreError::Validation(
            "portable diagram contract is invalid".into(),
        ));
    }
    validate_diagram_parts(&graph.nodes, &graph.edges, citations)?;
    validate_text(&graph.textual_alternative, 50_000, "diagram alternative")
}

fn validate_diagram_parts(
    nodes: &[DiagramNode],
    edges: &[DiagramEdge],
    citations: &HashSet<String>,
) -> Result<()> {
    let mut ids = HashSet::new();
    for node in nodes {
        if !ids.insert(node.id.clone()) {
            return Err(CoreError::Validation("duplicate diagram node".into()));
        }
        validate_text(&node.label, 500, "diagram label")?;
        validate_source_ids(&node.source_ids, citations, "diagram node")?;
        if let Some(preview) = &node.preview_data_uri
            && (preview.len() > 800_000
                || !matches!(
                    preview.get(..preview.find(',').unwrap_or_default()),
                    Some(
                        "data:image/png;base64"
                            | "data:image/jpeg;base64"
                            | "data:image/webp;base64"
                    )
                ))
        {
            return Err(CoreError::Validation(
                "diagram preview must be a bounded PNG, JPEG, or WebP data URI".into(),
            ));
        }
    }
    for edge in edges {
        if !ids.contains(&edge.source) || !ids.contains(&edge.target) {
            return Err(CoreError::Validation(
                "diagram edge endpoint is missing".into(),
            ));
        }
        validate_text(&edge.label, 300, "diagram edge label")?;
        validate_source_ids(&edge.source_ids, citations, "diagram edge")?;
    }
    Ok(())
}

fn validate_contribution(value: &DocumentContribution, reviewed_external: bool) -> Result<()> {
    if value.kind == ContributionKind::AiAssisted && !(reviewed_external && value.actor_id.is_some())
        && (value.provider_profile_id.is_none()
            || value.attempt_id.is_none()
            || value.candidate_id.is_none())
    {
        return Err(CoreError::Validation(
            "AI-assisted blocks require provider, attempt, and candidate provenance".into(),
        ));
    }
    if value.kind != ContributionKind::AiAssisted
        && (value.provider_profile_id.is_some()
            || value.attempt_id.is_some()
            || value.candidate_id.is_some())
    {
        return Err(CoreError::Validation(
            "non-AI blocks cannot claim provider provenance".into(),
        ));
    }
    Ok(())
}

fn material_fingerprint(document: &HumanDocument) -> Result<String> {
    let value = json!({
        "schema_version": document.schema_version,
        "project_id": document.project_id,
        "kind": document.kind.as_str(),
        "title": document.title,
        "audience": document.audience.as_str(),
        "source_checkpoint_id": document.source_checkpoint_id,
        "source_ledger_sequence": document.source_ledger_sequence,
        "classification": document.classification.as_str(),
        "omissions": document.omissions,
        "limitations": document.limitations,
        "sources": document.sources,
        "citations": document.citations,
        "blocks": document.blocks,
    });
    Ok(sha256(&serde_json::to_vec(&value)?))
}

fn validate_source_snapshots(
    connection: &Connection,
    project_id: &str,
    sources: &[HumanDocumentSource],
) -> Result<()> {
    let stale = source_staleness_reasons(connection, project_id, sources)?;
    if stale.is_empty() {
        Ok(())
    } else {
        Err(CoreError::Conflict(format!(
            "Human Document sources changed before save: {}",
            stale.join("; ")
        )))
    }
}

fn source_staleness_reasons(
    connection: &Connection,
    project_id: &str,
    sources: &[HumanDocumentSource],
) -> Result<Vec<String>> {
    let mut reasons = Vec::new();
    for source in sources {
        let current = match source.source_kind.as_str() {
            "entity" => connection.query_row("SELECT CAST(version AS TEXT) FROM entities WHERE id=?1 AND project_id=?2", params![source.source_id, project_id], |row| row.get::<_, String>(0)).optional()?,
            "artifact" => connection.query_row("SELECT sha256 FROM artifacts WHERE id=?1 AND project_id=?2", params![source.source_id, project_id], |row| row.get::<_, String>(0)).optional()?,
            "ai_candidate" => connection.query_row("SELECT CAST(candidate_version AS TEXT) FROM ai_candidates WHERE id=?1 AND project_id=?2", params![source.source_id, project_id], |row| row.get::<_, String>(0)).optional()?,
            "external_proposal" => connection.query_row("SELECT CAST(version AS TEXT) FROM external_proposals WHERE id=?1 AND project_id=?2 AND status='accepted'", params![source.source_id, project_id], |row| row.get::<_, String>(0)).optional()?,
            _ => None,
        };
        match current {
            None => reasons.push(format!("source {} is unavailable", source.source_id)),
            Some(version) if version != source.source_version => reasons.push(format!(
                "source {} changed from {} to {}",
                source.source_id, source.source_version, version
            )),
            Some(_) => {}
        }
    }
    Ok(reasons)
}

fn checkpoint_actions(checkpoint: Option<&crate::Checkpoint>) -> (Vec<String>, Vec<String>) {
    let strings = |key: &str| {
        checkpoint
            .and_then(|value| value.summary.get(key))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(|value| bounded_text(value, 10_000))
                    .collect()
            })
            .unwrap_or_default()
    };
    (strings("blockers"), strings("next_actions"))
}

fn entity_classification(
    connection: &Connection,
    project_id: &str,
    entity_id: &str,
) -> Result<DataClassification> {
    let value: Option<String> = connection.query_row("SELECT classification FROM ai_entity_classification WHERE entity_id=?1 AND project_id=?2", params![entity_id, project_id], |row| row.get(0)).optional()?;
    DataClassification::parse(value.as_deref().unwrap_or("internal"))
}

fn highest_classification(sources: &[HumanDocumentSource]) -> DataClassification {
    sources
        .iter()
        .fold(DataClassification::Public, |highest, source| {
            highest.max(source.classification)
        })
}

fn audience_allows(audience: HumanDocumentAudience, classification: DataClassification) -> bool {
    match audience {
        HumanDocumentAudience::LocalProject => true,
        HumanDocumentAudience::PrivatePortable => {
            classification.rank() <= DataClassification::Sensitive.rank()
        }
        HumanDocumentAudience::PublicPortable => classification == DataClassification::Public,
    }
}

fn artifact_classification(value: DataClassification) -> ArtifactClassification {
    match value {
        DataClassification::Public => ArtifactClassification::Public,
        DataClassification::Internal => ArtifactClassification::Internal,
        DataClassification::Sensitive => ArtifactClassification::Confidential,
        DataClassification::Secret => ArtifactClassification::Secret,
        DataClassification::NeverSend => ArtifactClassification::NeverSend,
    }
}

fn checkpoint_matches_kind(scope: &str, kind: HumanDocumentKind) -> bool {
    match kind {
        HumanDocumentKind::ResearchReport => scope == "research",
        HumanDocumentKind::DevelopmentReport => scope == "development",
        HumanDocumentKind::IntegratedReport => scope == "integrated",
        _ => matches!(scope, "research" | "development" | "integrated" | "core"),
    }
}

fn proposal_scope_matches_report(scope: &str, kind: HumanDocumentKind) -> bool {
    match kind {
        HumanDocumentKind::ResearchReport => matches!(scope, "research" | "integrated"),
        HumanDocumentKind::DevelopmentReport => matches!(scope, "development" | "integrated"),
        HumanDocumentKind::IntegratedReport => {
            matches!(scope, "research" | "development" | "integrated")
        }
        _ => matches!(scope, "research" | "development" | "integrated" | "core"),
    }
}

fn entity_allowed_for_kind(entity_type: &str, kind: HumanDocumentKind) -> bool {
    match kind {
        HumanDocumentKind::ResearchReport => matches!(
            entity_type,
            "research_session"
                | "research_question"
                | "evidence"
                | "experiment"
                | "result"
                | "finding"
                | "decision"
                | "requirement"
                | "learning_feedback"
        ),
        HumanDocumentKind::DevelopmentReport => matches!(
            entity_type,
            "repository"
                | "repository_baseline"
                | "commit_observation"
                | "requirement"
                | "change_set"
                | "analysis_run"
                | "code_entity"
                | "test"
                | "test_run"
                | "learning_feedback"
        ),
        HumanDocumentKind::ArchitectureExplanation => matches!(
            entity_type,
            "decision"
                | "requirement"
                | "repository"
                | "change_set"
                | "code_entity"
                | "test"
                | "learning_feedback"
        ),
        HumanDocumentKind::IntegratedReport
        | HumanDocumentKind::Timeline
        | HumanDocumentKind::Handover => true,
    }
}

fn report_sections(kind: HumanDocumentKind) -> Vec<(&'static str, &'static str)> {
    let research = vec![
        ("research_question", "Research Questions"),
        ("evidence", "Evidence"),
        ("experiment", "Experiments"),
        ("result", "Results"),
        ("finding", "Findings"),
        ("decision", "Decisions"),
    ];
    let development = vec![
        ("requirement", "Requirements"),
        ("change_set", "ChangeSets"),
        ("repository", "Repositories"),
        ("repository_baseline", "Repository Baselines"),
        ("analysis_run", "Code Intelligence Runs"),
        ("code_entity", "Code"),
        ("test", "Tests"),
        ("test_run", "Test Runs"),
    ];
    match kind {
        HumanDocumentKind::ResearchReport => research
            .into_iter()
            .chain([
                ("requirement", "Optional Development Handoff"),
                ("learning_feedback", "Learning Feedback"),
            ])
            .collect(),
        HumanDocumentKind::DevelopmentReport => development
            .into_iter()
            .chain([("learning_feedback", "Learning Feedback")])
            .collect(),
        HumanDocumentKind::ArchitectureExplanation => vec![
            ("decision", "Architecture Decisions"),
            ("requirement", "Architecture Requirements"),
            ("change_set", "Implementation Changes"),
            ("code_entity", "Architecture Components"),
            ("test", "Architecture Validation"),
        ],
        _ => research
            .into_iter()
            .chain(development)
            .chain([("learning_feedback", "Learning Feedback")])
            .collect(),
    }
}

fn graph_summary_entity(entity_type: &str) -> bool {
    matches!(
        entity_type,
        "research_session"
            | "research_question"
            | "evidence"
            | "experiment"
            | "result"
            | "finding"
            | "decision"
            | "requirement"
            | "repository"
            | "change_set"
            | "checkpoint"
            | "learning_feedback"
    )
}

fn graph_entity_priority(entity_type: &str) -> u8 {
    match entity_type {
        "research_session" => 0,
        "research_question" => 1,
        "evidence" | "experiment" => 2,
        "result" | "finding" => 3,
        "decision" => 4,
        "requirement" => 5,
        "change_set" | "repository" => 6,
        "checkpoint" | "learning_feedback" => 7,
        _ => 8,
    }
}

fn report_summary(kind: HumanDocumentKind, project: &str, count: usize) -> String {
    format!(
        "{} presents {count} source-backed project records for {project}. Important state appears first; verification detail and citations remain available below.",
        humanize(kind.as_str())
    )
}

fn default_title(kind: HumanDocumentKind, project: &str) -> String {
    format!("{} — {project}", humanize(kind.as_str()))
}
fn column(key: &str, label: &str) -> DocumentTableColumn {
    DocumentTableColumn {
        key: key.into(),
        label: label.into(),
    }
}
fn block(
    id: &str,
    title: &str,
    mut source_ids: Vec<String>,
    content: BlockContent,
) -> HumanDocumentBlock {
    source_ids.sort();
    source_ids.dedup();
    HumanDocumentBlock {
        meta: BlockMeta {
            id: id.into(),
            title: title.into(),
            source_ids,
            contribution: DocumentContribution::deterministic(),
        },
        content,
    }
}
fn status_state(status: &str) -> &'static str {
    match status {
        "failed" | "blocked" | "rejected" | "invalid" => "danger",
        "accepted" | "completed" | "passed" | "active" => "success",
        "pending" | "draft" | "proposed" => "warning",
        _ => "neutral",
    }
}
fn contribution_label(kind: ContributionKind) -> &'static str {
    match kind {
        ContributionKind::Deterministic => "Verified project data",
        ContributionKind::AiAssisted => "AI-assisted · reviewed",
        ContributionKind::UserAuthored => "Written by the user",
        ContributionKind::Imported => "Imported source",
    }
}
fn humanize(value: &str) -> String {
    let mut result = value.replace('_', " ");
    if let Some(first) = result.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    result
}
fn bounded_text(value: &str, max: usize) -> String {
    let mut output = value.chars().take(max).collect::<String>();
    if value.chars().count() > max {
        output.push('…');
    }
    output
}
fn friendly_timestamp(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| {
            timestamp
                .with_timezone(&Utc)
                .format("%d %b %Y, %H:%M UTC")
                .to_string()
        })
        .unwrap_or_else(|_| bounded_text(value, 100))
}
fn bounded_json_text(value: &Value) -> String {
    bounded_text(
        &serde_json::to_string(value).unwrap_or_else(|_| "{\"unavailable\":true}".into()),
        MAX_CELL_TEXT,
    )
}
fn human_detail_text(value: &Value) -> String {
    const FIELDS: [(&str, &str); 15] = [
        ("objective", "Objective"),
        ("question", "Question"),
        ("context", "Context"),
        ("desired_outcome", "Desired outcome"),
        ("annotation", "Description"),
        ("summary", "Summary"),
        ("relevance", "Why it matters"),
        ("hypothesis", "Hypothesis"),
        ("method", "Method"),
        ("observations", "Observations"),
        ("claim", "Finding"),
        ("interpretation", "Interpretation"),
        ("uncertainty", "Uncertainty"),
        ("rationale", "Rationale"),
        ("requirement_text", "Requirement"),
    ];
    let mut lines = Vec::new();
    for (key, label) in FIELDS {
        let Some(raw) = value.get(key) else { continue };
        let text = match raw {
            Value::String(text) => text.trim().to_owned(),
            Value::Number(number) => number.to_string(),
            Value::Bool(boolean) => boolean.to_string(),
            Value::Array(values) => values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "),
            _ => String::new(),
        };
        if matches!(
            text.as_str(),
            "Captured screenshot; interpretation has not yet been reviewed."
                | "Captured for the active research session."
                | "Awaiting AI analysis and human review."
                | "Evidence collected for this research session."
        ) {
            continue;
        }
        if !text.is_empty() {
            lines.push(format!("{label}: {}", bounded_text(&text, 4_000)));
        }
    }
    if lines.is_empty() && value.get("kind").and_then(Value::as_str) == Some("screenshot") {
        return "Awaiting AI interpretation and human review. Use ‘Analyze research with AI’ to prepare a factual description and recommendations.".into();
    }
    if lines.is_empty() {
        bounded_json_text(value)
    } else {
        bounded_text(&lines.join("\n"), MAX_CELL_TEXT)
    }
}
fn candidate_text(value: &Value) -> Option<String> {
    ["summary", "narrative", "text"]
        .iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .map(|value| bounded_text(value, MAX_TEXT))
}
fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
fn required_string(value: &Value, key: &str, max: usize) -> Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|value| bounded_text(value, max))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| CoreError::Validation(format!("diagram field {key} is required")))
}
fn optional_string(value: &Value, key: &str, max: usize) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(|value| bounded_text(value, max))
}
fn string_array(value: &Value, key: &str, max: usize) -> Result<Vec<String>> {
    let items = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| CoreError::Validation(format!("diagram field {key} must be an array")))?;
    if items.len() > max {
        return Err(CoreError::Validation(format!(
            "diagram field {key} exceeds its limit"
        )));
    }
    items
        .iter()
        .map(|item| {
            item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                CoreError::Validation(format!("diagram field {key} must contain strings"))
            })
        })
        .collect()
}
fn diagram_alternative(nodes: &[DiagramNode], edges: &[DiagramEdge], truncated: bool) -> String {
    let node_by_id = nodes
        .iter()
        .map(|node| (node.id.as_str(), node.label.as_str()))
        .collect::<HashMap<_, _>>();
    let relationships = edges
        .iter()
        .take(30)
        .filter_map(|edge| {
            Some(format!(
                "{} —{}→ {}",
                node_by_id.get(edge.source.as_str())?,
                edge.label,
                node_by_id.get(edge.target.as_str())?
            ))
        })
        .collect::<Vec<_>>();
    let detail = if relationships.is_empty() {
        " Add reviewed findings and relationships to build the reasoning flow.".into()
    } else {
        format!(" Key paths: {}.", relationships.join("; "))
    };
    format!(
        "Project knowledge map with {} concepts and {} relationships{}.{}",
        nodes.len(),
        edges.len(),
        if truncated {
            "; low-level records are intentionally omitted from this overview"
        } else {
            ""
        },
        detail
    )
}

fn validate_source_ids(ids: &[String], allowed: &HashSet<String>, label: &str) -> Result<()> {
    let mut seen = HashSet::new();
    for id in ids {
        if !seen.insert(id) {
            return Err(CoreError::Validation(format!(
                "duplicate source in {label}"
            )));
        }
        if !allowed.contains(id) {
            return Err(CoreError::Validation(format!(
                "{label} cites source {id} outside the document boundary"
            )));
        }
    }
    Ok(())
}
fn validate_text(value: &str, max: usize, label: &str) -> Result<()> {
    if value.chars().count() > max {
        Err(CoreError::Validation(format!(
            "{label} exceeds {max} characters"
        )))
    } else {
        Ok(())
    }
}
fn validate_strings(
    values: &[String],
    max_items: usize,
    max_text: usize,
    label: &str,
) -> Result<()> {
    if values.len() > max_items {
        return Err(CoreError::Validation(format!("too many {label} values")));
    }
    for value in values {
        validate_text(value, max_text, label)?;
    }
    Ok(())
}
fn ensure_unique(values: &[String], label: &str) -> Result<()> {
    let mut seen = HashSet::new();
    for value in values {
        validate_nonempty(value, 500, label)?;
        if !seen.insert(value) {
            return Err(CoreError::Validation(format!("duplicate {label}")));
        }
    }
    Ok(())
}

fn escape_html(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' => output.push_str("&quot;"),
            '\'' => output.push_str("&#39;"),
            _ => output.push(character),
        }
    }
    output
}
fn safe_anchor(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect()
}
fn markdown_text(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('`', "\\`")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn markdown_table_text(value: &str) -> String {
    markdown_text(value)
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
}
fn render_html_list(output: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    write!(output, "<h3>{}</h3><ul>", escape_html(label)).expect("writing to String cannot fail");
    for value in values {
        write!(output, "<li>{}</li>", escape_html(value)).expect("writing to String cannot fail");
    }
    output.push_str("</ul>");
}
fn render_source_links(output: &mut String, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    write!(
        output,
        "<details class=\"source-links\"><summary>View {} verified source reference{}</summary><p>",
        ids.len(),
        if ids.len() == 1 { "" } else { "s" }
    )
    .expect("writing to String cannot fail");
    for (index, id) in ids.iter().take(100).enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        write!(
            output,
            "<a href=\"#source-{}\">Source {}</a>",
            safe_anchor(id),
            index + 1
        )
        .expect("writing to String cannot fail");
    }
    output.push_str("</p></details>");
}
fn render_markdown_list(output: &mut String, label: &str, values: &[String]) {
    if values.is_empty() {
        return;
    }
    writeln!(output, "**{label}:**").expect("writing to String cannot fail");
    for value in values {
        writeln!(output, "- {}", markdown_text(value)).expect("writing to String cannot fail");
    }
    output.push('\n');
}
fn render_markdown_sources(output: &mut String, ids: &[String]) {
    if ids.is_empty() {
        return;
    }
    writeln!(
        output,
        "\nSources: {}",
        ids.iter()
            .map(|id| format!("`{}`", markdown_text(id)))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .expect("writing to String cannot fail");
}
fn render_markdown_diagram(
    output: &mut String,
    nodes: &[DiagramNode],
    edges: &[DiagramEdge],
    direction: DiagramDirection,
    alternative: &str,
) {
    writeln!(output, "{}\n", markdown_text(alternative)).expect("writing to String cannot fail");
    writeln!(output, "```mermaid\n{}```\n", mermaid_source(nodes, edges, direction)).expect("String write");
    for edge in edges {
        let source = nodes.iter().find(|node| node.id == edge.source).map(|node| node.label.as_str()).unwrap_or("Unavailable source");
        let target = nodes.iter().find(|node| node.id == edge.target).map(|node| node.label.as_str()).unwrap_or("Unavailable target");
        writeln!(
            output,
            "- Edge `{}` — {} → {}",
            markdown_text(&edge.label),
            markdown_text(source),
            markdown_text(target)
        )
        .expect("writing to String cannot fail");
    }
}

fn mermaid_source(nodes: &[DiagramNode], edges: &[DiagramEdge], direction: DiagramDirection) -> String {
    let label = |value: &str| value.chars().take(180).map(|c| if c.is_control() || "\"<>`{}|[]\\#".contains(c) { ' ' } else { c }).collect::<String>();
    let mut source = format!("flowchart {}\n", if matches!(direction, DiagramDirection::TopDown) { "TD" } else { "LR" });
    let aliases: HashMap<_,_> = nodes.iter().enumerate().map(|(i, node)| (node.id.as_str(), format!("n{i}"))).collect();
    for node in nodes { writeln!(source, "  {}[\"{}\"]", aliases[node.id.as_str()], label(&node.label)).expect("String write"); }
    for edge in edges {
        if let (Some(from), Some(to)) = (aliases.get(edge.source.as_str()), aliases.get(edge.target.as_str())) {
            writeln!(source, "  {from} -->|\"{}\"| {to}", label(&edge.label)).expect("String write");
        }
    }
    source
}

pub(crate) fn append_human_document_integrity_issues(
    connection: &Connection,
    project_id: &str,
    report: &mut IntegrityReport,
) -> Result<()> {
    let mut statement = connection.prepare(
        "SELECT id,document_json,material_fingerprint FROM human_documents WHERE project_id=?1",
    )?;
    let rows = statement
        .query_map([project_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    for (id, raw, stored_fingerprint) in rows {
        match serde_json::from_str::<HumanDocument>(&raw) {
            Ok(document) => {
                if validate_human_document(&document).is_err()
                    || material_fingerprint(&document).ok().as_deref()
                        != Some(stored_fingerprint.as_str())
                {
                    report.issues.push(IntegrityIssue {
                        code: "invalid_human_document".into(),
                        path_or_id: id,
                        guidance: "Retain the record for audit and regenerate it from canonical sources; do not trust or render the corrupted projection.".into(),
                    });
                }
            }
            Err(_) => report.issues.push(IntegrityIssue {
                code: "malformed_human_document".into(),
                path_or_id: id,
                guidance: "Recover the saved projection from a verified backup or regenerate it from canonical sources.".into(),
            }),
        }
    }
    let dangling_sources: i64 = connection.query_row(
        "SELECT count(*) FROM human_document_sources s JOIN human_documents d ON d.id=s.document_id WHERE d.project_id=?1 AND ((s.source_kind='entity' AND NOT EXISTS(SELECT 1 FROM entities e WHERE e.id=s.source_id AND e.project_id=d.project_id)) OR (s.source_kind='artifact' AND NOT EXISTS(SELECT 1 FROM artifacts a WHERE a.id=s.source_id AND a.project_id=d.project_id)) OR (s.source_kind='ai_candidate' AND NOT EXISTS(SELECT 1 FROM ai_candidates c WHERE c.id=s.source_id AND c.project_id=d.project_id)) OR (s.source_kind='external_proposal' AND NOT EXISTS(SELECT 1 FROM external_proposals p WHERE p.id=s.source_id AND p.project_id=d.project_id)))",
        [project_id],
        |row| row.get(0),
    )?;
    if dangling_sources > 0 {
        report.issues.push(IntegrityIssue {
            code: "dangling_human_document_source".into(),
            path_or_id: project_id.into(),
            guidance: "Preserve the report for audit and recover its missing source boundary from a verified project backup.".into(),
        });
    }
    let invalid_exports: i64 = connection.query_row(
        "SELECT count(*) FROM human_document_exports x LEFT JOIN artifacts a ON a.id=x.artifact_id AND a.project_id=x.project_id WHERE x.project_id=?1 AND (a.id IS NULL OR a.sha256<>x.content_sha256)",
        [project_id],
        |row| row.get(0),
    )?;
    if invalid_exports > 0 {
        report.issues.push(IntegrityIssue {
            code: "invalid_human_document_export".into(),
            path_or_id: project_id.into(),
            guidance: "Quarantine the export and republish it from the saved renderer-neutral Human Document.".into(),
        });
    }
    Ok(())
}

const STYLE: &str = r#"
body .diagram-tools select{padding:.5rem;background:var(--panel);color:var(--text);border:1px solid var(--line);border-radius:7px}body .evidence-gallery{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:1rem}body .evidence-gallery img{display:block;width:100%;max-height:340px;object-fit:contain}
body main>section{background:transparent;border:0;border-bottom:1px solid var(--line);border-radius:0;box-shadow:none;padding:2rem 0}body main,body header,body footer{max-width:1000px}body figure{margin:1rem 0}body figure figcaption{font-size:.9rem;color:var(--muted);line-height:1.7;margin-top:1rem}body .diagram-scroll{height:480px;max-height:65vh;padding:16px;cursor:grab;touch-action:none}body .diagram-scroll:active{cursor:grabbing}body .diagram-scroll>svg{display:block;min-width:0;max-width:none}body .diagram-tools{display:flex;align-items:center;gap:.55rem;flex-wrap:wrap;padding:.7rem;border:1px solid var(--line);border-radius:10px 10px 0 0;background:var(--panel)}body .diagram-tools>span{margin-right:auto;font-size:.85rem;color:var(--muted)}body .diagram-tools button{background:var(--panel);color:var(--text);font:inherit;border:1px solid var(--line);border-radius:7px;min-width:38px;padding:.4rem .7rem;cursor:pointer}body .diagram-tools output{font-size:.85rem;min-width:3rem;text-align:center}body figure:fullscreen{background:var(--bg);padding:1rem;overflow:auto}body figure:fullscreen .diagram-scroll{height:80vh;max-height:80vh}body section p,body section li{line-height:1.75}body .report-toc{position:static}body [hidden]{display:none!important}
:root{color-scheme:light dark;--bg:#f5f7f5;--panel:#fff;--text:#18221d;--muted:#59645e;--line:#d8dfda;--accent:#176b52;--accent-soft:#e2f3ec;--warn:#8a5400;--danger:#a22b2b;--focus:#0b6cff;font:16px/1.55 system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif}*{box-sizing:border-box}body{margin:0;background:var(--bg);color:var(--text)}header,main,footer{width:min(1120px,calc(100% - 2rem));margin:auto}header{padding:3rem 0 1.5rem}.eyebrow{text-transform:uppercase;letter-spacing:.12em;color:var(--accent);font-weight:700}.meta{color:var(--muted)}h1{font-size:clamp(2rem,5vw,3.5rem);line-height:1.08;margin:.3rem 0 1rem}h2{font-size:1.45rem;margin:0}h3{font-size:1rem}section{background:var(--panel);border:1px solid var(--line);border-radius:16px;padding:1.25rem;margin:1rem 0;box-shadow:0 6px 24px rgba(20,40,30,.04)}.section-heading{display:flex;gap:1rem;align-items:center;justify-content:space-between;margin-bottom:1rem}.badge{font-size:.75rem;padding:.25rem .6rem;border-radius:99px;background:var(--accent-soft);color:var(--accent);font-weight:700}.lead{font-size:1.15rem;max-width:72ch}.cards{display:grid;grid-template-columns:repeat(auto-fit,minmax(150px,1fr));gap:.75rem}.card{border:1px solid var(--line);border-radius:12px;padding:1rem;display:flex;flex-direction:column}.card strong{font-size:1.8rem}.state-danger{border-color:var(--danger)}.state-warning{border-color:var(--warn)}.state-success{border-color:var(--accent)}.table-wrap{position:relative;width:100%;max-width:100%;overflow:auto;overscroll-behavior-inline:contain;scrollbar-gutter:stable;max-height:70vh;border:1px solid var(--line);border-radius:10px;-webkit-overflow-scrolling:touch}table{width:max-content;min-width:78rem;border-collapse:collapse;table-layout:auto}caption{text-align:left;font-weight:700;padding:.8rem}th,td{padding:.7rem;text-align:left;vertical-align:top;border-top:1px solid var(--line);max-width:none;overflow-wrap:normal;word-break:normal}thead th{position:sticky;top:0;z-index:2;background:var(--panel)}tr>:first-child{position:sticky;left:0;z-index:1;background:var(--panel);box-shadow:1px 0 0 var(--line)}thead th:first-child{z-index:3}[data-column="title"]{min-width:14rem;max-width:19rem;white-space:normal;overflow-wrap:break-word}[data-column="status"]{min-width:8rem;white-space:nowrap}[data-column="origin"]{min-width:9rem;white-space:nowrap}[data-column="updated"]{min-width:15rem;white-space:nowrap}[data-column="classification"]{min-width:8rem;white-space:nowrap}[data-column="detail"]{min-width:28rem;max-width:38rem;white-space:pre-wrap;overflow-wrap:break-word}.timeline{list-style:none;padding:0;border-left:2px solid var(--line);margin-left:.5rem}.timeline li{padding:0 0 1.25rem 1.25rem}.timeline time{display:block;color:var(--muted);font-size:.85rem}.timeline strong{display:block}.callout{border-left:5px solid var(--accent);padding:1rem}.warning{border-left-color:var(--warn)}.danger{border-left-color:var(--danger)}.uncertainty{border-left-color:#7657a8}.diagram-scroll{overflow:auto;max-height:70vh;padding:.5rem;background:var(--bg);border:1px solid var(--line);border-radius:12px}.relationship-map{display:block;min-width:760px;width:100%;height:auto}.relationship-map marker path{fill:var(--muted)}.relationship-line{fill:none;stroke:var(--muted);stroke-width:2.5}.relationship-label{fill:var(--text);font:700 12px system-ui}.diagram-node{height:100%;overflow:hidden;background:var(--panel);border:2px solid var(--line);border-radius:12px;padding:.7rem;display:flex;flex-direction:column;box-shadow:0 7px 20px rgba(20,40,30,.12)}.diagram-node.kind-evidence{border-color:#187a55}.diagram-node.kind-research_question{border-color:#7651b5}.diagram-node.kind-research_session{border-color:#157a96}.diagram-node.kind-finding{border-color:#a26a00}.diagram-node img{width:42px;height:32px;object-fit:cover;background:#0d1410;border-radius:6px;margin-bottom:.35rem;border:1px solid var(--line)}.diagram-node small{display:block;color:var(--muted);font-size:.72rem;text-transform:uppercase;letter-spacing:.05em}.diagram-node strong{display:block;margin-top:.25rem;line-height:1.25;overflow-wrap:anywhere}.source-links{font-size:.8rem;color:var(--muted);overflow-wrap:anywhere}.source-links a{color:var(--accent)}code{overflow-wrap:anywhere}.citations li{margin-bottom:.8rem}.citations span{color:var(--muted)}details{border-top:1px solid var(--line);padding-top:.75rem;margin-top:.75rem}summary{cursor:pointer;font-weight:700}.skip{position:absolute;left:-9999px;top:1rem}.skip:focus{left:1rem;background:var(--panel);padding:.7rem;z-index:10}a:focus-visible,button:focus-visible,summary:focus-visible,[tabindex]:focus-visible{outline:3px solid var(--focus);outline-offset:3px}footer{padding:2rem 0 4rem;color:var(--muted);font-size:.85rem}@media(prefers-color-scheme:dark){:root{--bg:#101512;--panel:#18201b;--text:#eef5f0;--muted:#aab7af;--line:#344139;--accent:#79d9b5;--accent-soft:#203c31;--warn:#f2ba64;--danger:#ff8e8e}}@media(prefers-reduced-motion:reduce){*,*::before,*::after{scroll-behavior:auto!important;animation:none!important;transition:none!important}}@media print{body{background:#fff;color:#111}section{box-shadow:none;break-inside:avoid}.table-wrap,.diagram-scroll{max-height:none;overflow:visible}.table-wrap table{width:100%;min-width:0}header,main,footer{width:100%}}
.report-toc{position:sticky;top:.5rem;z-index:2;margin:1rem 0;padding:.85rem 1rem;border:1px solid var(--line);border-radius:14px;background:color-mix(in srgb,var(--panel) 94%,transparent);backdrop-filter:blur(12px)}.report-toc>strong{display:block;margin-bottom:.45rem;color:var(--accent);font-size:.78rem;letter-spacing:.08em;text-transform:uppercase}.report-toc>div{display:flex;gap:.45rem;overflow:auto}.report-toc a{flex:0 0 auto;padding:.35rem .65rem;border:1px solid var(--line);border-radius:999px;background:var(--bg);color:var(--text);font-size:.76rem;text-decoration:none}.report-toc a:hover{border-color:var(--accent);color:var(--accent)}
"#;
