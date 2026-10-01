use continuum_core::{
    ActorRef, AiProjectPolicyInput, ArtifactClassification, BlockContent, CORE_SCHEMA_VERSION,
    CandidateReviewAction, CommandContext, ContinuityStore, CoreError, DataClassification,
    EntityUpdate, EvidenceKind, HumanDocumentAudience, HumanDocumentFormat, HumanDocumentKind,
    HumanDocumentRequest, NewEntity, NewEvidence, NewProviderProfile, NewRelationship,
    NewResearchQuestion, NewResearchSession, NewSemanticTask, OriginKind, PageRequest,
    ProviderCapabilities, ProviderDataPolicy, ProviderKind, ProviderRequest, ProviderResponse,
    ProviderTransportError, QuestionKind, RelationshipReviewState, SemanticRequirements,
    SemanticRoutePolicy, SemanticTaskType, SemanticTransport, Space, render_human_document,
};
use serde_json::json;

fn user() -> CommandContext {
    CommandContext::new(ActorRef::user("cp8-user"))
}

fn setup() -> (tempfile::TempDir, ContinuityStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP8 visual fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    (directory, store)
}

fn entity(store: &ContinuityStore, entity_type: &str, title: &str) -> String {
    let mut input = NewEntity::authored(entity_type, title);
    input.data = json!({"summary": format!("Verified detail for {title}")});
    store.create_entity_with_context(&user(), input).unwrap()
}

fn classify(store: &ContinuityStore, id: &str, classification: DataClassification) {
    store
        .set_entity_ai_classification(
            &user(),
            id,
            classification,
            "CP8 audience-policy acceptance fixture",
        )
        .unwrap();
}

struct NarrativeTransport {
    source_id: String,
}

struct DiagramTransport {
    source_id: String,
}

impl SemanticTransport for DiagramTransport {
    fn send(
        &self,
        _request: &ProviderRequest,
    ) -> std::result::Result<ProviderResponse, ProviderTransportError> {
        let output = json!({
            "nodes":[{
                "id":"source",
                "label":"Reviewed source",
                "kind":"concept",
                "status":"verified",
                "source_ids":[self.source_id]
            }],
            "edges":[],
            "textual_alternative":"One reviewed source node."
        });
        Ok(ProviderResponse {
            status_code: 200,
            body: json!({
                "candidates":[{"content":{"parts":[{"text":serde_json::to_string(&output).unwrap()}]},"finishReason":"STOP"}],
                "usageMetadata":{"promptTokenCount":30,"candidatesTokenCount":12},
                "modelVersion":"cp8-fixture"
            }),
        })
    }
}

impl SemanticTransport for NarrativeTransport {
    fn send(
        &self,
        _request: &ProviderRequest,
    ) -> std::result::Result<ProviderResponse, ProviderTransportError> {
        let output = json!({
            "summary":"The reviewed source supports a concise, bounded continuity narrative.",
            "source_ids":[self.source_id]
        });
        Ok(ProviderResponse {
            status_code: 200,
            body: json!({
                "candidates":[{"content":{"parts":[{"text":serde_json::to_string(&output).unwrap()}]},"finishReason":"STOP"}],
                "usageMetadata":{"promptTokenCount":30,"candidatesTokenCount":12},
                "modelVersion":"cp8-fixture"
            }),
        })
    }
}

#[test]
fn integrated_document_is_source_backed_bounded_and_renderer_neutral() {
    let (_directory, store) = setup();
    let research = entity(&store, "concept", "Why local continuity matters");
    let development = entity(&store, "component", "Continuity Core");
    store
        .create_relationship_with_context(
            &user(),
            NewRelationship {
                relation_type: "contextualizes".into(),
                relation_version: 1,
                source_entity_id: research.clone(),
                target_entity_id: development.clone(),
                origin: OriginKind::User,
                confidence: None,
                review_state: RelationshipReviewState::Accepted,
                direct_source_ids: vec![],
                supersedes_id: None,
            },
        )
        .unwrap();

    let document = store
        .compose_human_document(HumanDocumentRequest::integrated())
        .unwrap();
    assert_eq!(document.kind, HumanDocumentKind::IntegratedReport);
    assert!(
        document
            .sources
            .iter()
            .any(|source| source.source_id == research)
    );
    assert!(
        document
            .sources
            .iter()
            .any(|source| source.source_id == development)
    );
    assert!(
        document
            .blocks
            .iter()
            .any(|block| block.meta.id == "knowledge-graph")
    );
    assert!(
        document
            .blocks
            .iter()
            .any(|block| block.meta.id == "portable-flow")
    );
    assert!(!document.material_fingerprint.is_empty());

    let rendered = render_human_document(&document).unwrap();
    assert_eq!(rendered.material_fingerprint, document.material_fingerprint);
    assert!(rendered.asset_manifest.network_dependencies.is_empty());
    assert!(rendered.html.contains("Continuity Core"));
    assert!(rendered.html.contains("script-src 'sha256-"));
    assert!(rendered.html.contains("mermaid-source"));
    assert!(rendered.html.contains("data-diagram=\"fit\""));
    assert!(rendered.markdown.contains("```mermaid\nflowchart"));
    assert!(rendered.markdown.contains("Continuity Core"));
}

#[test]
fn research_session_membership_and_screenshot_preview_form_an_informative_graph() {
    let (_directory, store) = setup();
    store
        .set_space_capability_with_context(&user(), Space::Research, true)
        .unwrap();
    let session = store
        .create_research_session(
            &user(),
            NewResearchSession {
                title: "Second Brain exploration".into(),
                objective: "Understand how a durable shared AI memory should work".into(),
                started_at: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let question = store
        .create_research_question(
            &user(),
            NewResearchQuestion {
                title: "Portable project memory".into(),
                kind: QuestionKind::Question,
                question: "How can project memory survive an AI handover?".into(),
                context: "The owner may change tools or collaborators.".into(),
                desired_outcome: "A verified, portable continuity design.".into(),
                priority: 3,
                due_at: None,
                session_id: Some(session.entity.id.clone()),
                metadata: json!({}),
            },
        )
        .unwrap();
    let screenshot = store
        .ingest_artifact_reader_with_context(
            &user(),
            &b"\x89PNG\r\n\x1a\ncontinuum-preview"[..],
            "image/png",
            ArtifactClassification::Internal,
            OriginKind::User,
            &json!({"purpose":"cp8-preview-regression"}),
        )
        .unwrap();
    let evidence = store
        .create_evidence(
            &user(),
            NewEvidence {
                title: "Collective memory architecture screenshot".into(),
                kind: EvidenceKind::Screenshot,
                origin: OriginKind::User,
                source_uri: None,
                source_title: Some("Captured architecture".into()),
                source_author: Some("cp8-user".into()),
                captured_at: None,
                capture_method: "screenshot".into(),
                stable_reference: None,
                source_content: None,
                annotation: "Shows the candidate project-memory layers.".into(),
                summary: "A visual model for shared project memory.".into(),
                relevance: "Evidence for the research question.".into(),
                original_artifact_id: Some(screenshot.id.clone()),
                question_id: Some(question.entity.id.clone()),
                session_id: Some(session.entity.id.clone()),
                metadata: json!({}),
            },
        )
        .unwrap();

    let second_evidence = store
        .create_evidence(
            &user(),
            NewEvidence {
                title: "Second observation from the same image".into(),
                kind: EvidenceKind::Screenshot,
                origin: OriginKind::User,
                source_uri: None,
                source_title: Some("Captured architecture".into()),
                source_author: Some("cp8-user".into()),
                captured_at: None,
                capture_method: "screenshot".into(),
                stable_reference: None,
                source_content: None,
                annotation: "Independent note using the same original image.".into(),
                summary: "Second evidence record.".into(),
                relevance: "Another reading of the same source.".into(),
                original_artifact_id: Some(screenshot.id.clone()),
                question_id: Some(question.entity.id.clone()),
                session_id: Some(session.entity.id.clone()),
                metadata: json!({}),
            },
        )
        .unwrap();

    let document = store
        .compose_human_document(HumanDocumentRequest::research())
        .unwrap();
    assert_eq!(document.sources.iter().filter(|source| source.source_kind == "artifact" && source.source_id == screenshot.id).count(), 1);
    assert!(document.sources.iter().any(|source| source.source_id == second_evidence.entity.id));
    let graph = document
        .blocks
        .iter()
        .find_map(|block| match &block.content {
            BlockContent::Graph { specification } => Some(specification),
            _ => None,
        })
        .unwrap();
    assert!(
        !document
            .blocks
            .iter()
            .any(|block| block.meta.id == "portable-flow"),
        "an evidence-only research report must not duplicate the same raw graph as an evidence-to-decision flow"
    );
    assert!(graph.edges.iter().any(|edge| {
        edge.source == session.entity.id
            && edge.target == question.entity.id
            && edge.label == "research question"
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.source == session.entity.id
            && edge.target == evidence.entity.id
            && edge.label == "evidence collected"
    }));
    let evidence_node = graph
        .nodes
        .iter()
        .find(|node| node.id == evidence.entity.id)
        .unwrap();
    assert!(
        evidence_node
            .preview_data_uri
            .as_deref()
            .is_some_and(|value| value.starts_with("data:image/png;base64,"))
    );
    assert!(evidence_node.source_ids.contains(&screenshot.id));

    let rendered = render_human_document(&document).unwrap();
    assert!(
        rendered
            .html
            .contains("aria-label=\"Scrollable data table\"")
    );
    assert!(rendered.html.contains("data-column=\"updated\""));
    assert!(rendered.html.contains("min-width:78rem"));
    assert!(rendered.html.contains("relationship-map"));
    assert!(rendered.html.contains("evidence collected"));
    assert!(rendered.html.contains("data:image/png;base64,"));
}

#[test]
fn standalone_research_and_development_reports_do_not_require_the_other_space() {
    let (_directory, store) = setup();

    let research = store
        .compose_human_document(HumanDocumentRequest::research())
        .unwrap();
    let development = store
        .compose_human_document(HumanDocumentRequest::development())
        .unwrap();

    assert_eq!(research.kind, HumanDocumentKind::ResearchReport);
    assert_eq!(development.kind, HumanDocumentKind::DevelopmentReport);
    assert!(!research.blocks.is_empty());
    assert!(!development.blocks.is_empty());
    assert!(
        render_human_document(&research)
            .unwrap()
            .html
            .contains("Research report")
    );
    assert!(
        render_human_document(&development)
            .unwrap()
            .html
            .contains("Development report")
    );
}

#[test]
fn public_and_portable_audiences_never_leak_disallowed_sources() {
    let (_directory, store) = setup();
    let public = entity(&store, "concept", "Public finding");
    let internal = entity(&store, "concept", "Internal roadmap");
    let secret = entity(&store, "concept", "Secret credential plan");
    classify(&store, &public, DataClassification::Public);
    classify(&store, &internal, DataClassification::Internal);
    classify(&store, &secret, DataClassification::Secret);

    let mut public_request = HumanDocumentRequest::integrated();
    public_request.audience = HumanDocumentAudience::PublicPortable;
    let public_document = store.compose_human_document(public_request).unwrap();
    assert_eq!(public_document.sources.len(), 1);
    assert_eq!(public_document.sources[0].source_id, public);
    let public_html = render_human_document(&public_document).unwrap().html;
    assert!(!public_html.contains("Internal roadmap"));
    assert!(!public_html.contains("Secret credential plan"));

    let mut private_request = HumanDocumentRequest::integrated();
    private_request.audience = HumanDocumentAudience::PrivatePortable;
    let private_document = store.compose_human_document(private_request).unwrap();
    assert!(
        private_document
            .sources
            .iter()
            .any(|source| source.source_id == internal)
    );
    assert!(
        !private_document
            .sources
            .iter()
            .any(|source| source.source_id == secret)
    );
}

#[test]
fn hostile_source_text_is_inert_in_the_offline_html_export() {
    let (_directory, store) = setup();
    let hostile = entity(
        &store,
        "concept",
        "<script>alert('x')</script><img src=https://attacker.invalid/x>",
    );
    classify(&store, &hostile, DataClassification::Public);
    let document = store
        .compose_human_document(HumanDocumentRequest::integrated())
        .unwrap();
    let rendered = render_human_document(&document).unwrap();

    assert!(rendered.html.contains("Content-Security-Policy"));
    assert!(rendered.html.contains("default-src 'none'"));
    assert!(!rendered.html.contains("<script>alert"));
    assert!(!rendered.html.contains("<img src=https://attacker.invalid"));
    assert!(rendered.html.contains("&lt;script&gt;"));
    assert!(rendered.asset_manifest.network_dependencies.is_empty());
}

#[test]
fn saved_document_detects_source_changes_and_rejects_stale_snapshot_save() {
    let (_directory, store) = setup();
    let source = entity(&store, "concept", "Original title");
    let document = store
        .compose_human_document(HumanDocumentRequest::integrated())
        .unwrap();
    let saved = store.save_human_document(&user(), &document).unwrap();
    assert!(store.human_document_freshness(&saved.id).unwrap().fresh);

    let current = store.get_entity(&source).unwrap();
    store
        .update_entity_with_context(
            &user(),
            &source,
            EntityUpdate {
                title: "Updated title".into(),
                status: current.status,
                metadata: current.metadata,
                data: current.data,
                expected_version: current.version,
            },
        )
        .unwrap();

    let freshness = store.human_document_freshness(&saved.id).unwrap();
    assert!(!freshness.fresh);
    assert!(
        freshness
            .reasons
            .iter()
            .any(|reason| reason.contains("changed"))
    );
    assert!(matches!(
        store.save_human_document(&user(), &document),
        Err(CoreError::Conflict(_))
    ));
}

#[test]
fn html_and_markdown_publications_are_content_addressed_auditable_artifacts() {
    let (_directory, store) = setup();
    entity(&store, "concept", "Publishable knowledge");
    let document = store
        .compose_human_document(HumanDocumentRequest::integrated())
        .unwrap();
    let saved = store.save_human_document(&user(), &document).unwrap();
    let html = store
        .publish_human_document(&user(), &saved.id, HumanDocumentFormat::Html)
        .unwrap();
    let markdown = store
        .publish_human_document(&user(), &saved.id, HumanDocumentFormat::Markdown)
        .unwrap();

    assert_ne!(html.artifact_id, markdown.artifact_id);
    assert_ne!(html.content_sha256, markdown.content_sha256);
    assert!(html.asset_manifest.network_dependencies.is_empty());
    assert!(store.verify_integrity().unwrap().is_healthy());
}

#[test]
fn only_reviewed_fresh_ai_presentation_enters_the_report_with_full_authorship() {
    let (_directory, store) = setup();
    let source = entity(&store, "concept", "Grounded narrative source");
    classify(&store, &source, DataClassification::Public);
    let profile = store
        .register_provider_profile(
            &user(),
            NewProviderProfile {
                display_name: "CP8 Gemini fixture".into(),
                provider_kind: ProviderKind::Gemini,
                endpoint:
                    "https://generativelanguage.googleapis.com/v1beta/models/cp8:generateContent"
                        .into(),
                model_id: "cp8-fixture".into(),
                credential_ref: "continuum/cp8-fixture".into(),
                enabled: true,
                priority: 1,
                adapter_version: 1,
                capabilities: ProviderCapabilities {
                    text_input: true,
                    structured_output: true,
                    streaming: false,
                    cancellation: false,
                    max_input_units: 10_000,
                    max_output_units: 1_000,
                    token_count_confidence: "provider_reported".into(),
                    supported_task_types: vec!["report_narrative".into(), "diagram_plan".into()],
                    declared_deviations: vec![],
                },
                data_policy: ProviderDataPolicy {
                    region: "fixture".into(),
                    retention_summary: "No network fixture".into(),
                    training_summary: "No training fixture".into(),
                    verified_at: chrono::Utc::now().to_rfc3339(),
                    reference_url: "https://example.invalid/cp8-policy".into(),
                },
            },
        )
        .unwrap();
    let policy = store.ai_project_policy().unwrap();
    store
        .set_ai_project_policy(
            &user(),
            AiProjectPolicyInput {
                allowed_profile_ids: vec![profile.id.clone()],
                internal_remote_enabled: false,
                automatic_failover: false,
                max_attempts: 1,
                max_total_units: 20_000,
                expected_policy_version: policy.policy_version,
            },
        )
        .unwrap();
    let task = store
        .create_semantic_task(
            &user(),
            NewSemanticTask {
                task_type: SemanticTaskType::ReportNarrative,
                source_entity_ids: vec![source.clone()],
                source_artifact_ids: vec![],
                output_schema: json!({
                    "type":"object",
                    "additionalProperties":false,
                    "properties":{
                        "summary":{"type":"string","minLength":1,"maxLength":1000},
                        "source_ids":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":20}
                    },
                    "required":["summary","source_ids"]
                }),
                prompt_template_id: "cp8-report-narrative".into(),
                prompt_template_version: 1,
                requirements: SemanticRequirements::default(),
                route_policy: SemanticRoutePolicy {
                    preferred_profile_id: Some(profile.id.clone()),
                    allowed_profile_ids: vec![profile.id.clone()],
                    allow_failover: false,
                },
                privacy_audience: "project-owner".into(),
                consent_id: None,
                max_input_units: 5_000,
                max_output_units: 500,
                timeout_ms: 10_000,
                cache_ttl_seconds: 3_600,
            },
        )
        .unwrap();
    let execution = store
        .execute_semantic_task(
            &user(),
            &task.id,
            &NarrativeTransport {
                source_id: source.clone(),
            },
        )
        .unwrap();

    let mut pending_request = HumanDocumentRequest::integrated();
    pending_request.accepted_ai_candidate_ids = vec![execution.candidate.id.clone()];
    assert!(matches!(
        store.compose_human_document(pending_request),
        Err(CoreError::Validation(_))
    ));

    let accepted = store
        .review_semantic_candidate(
            &user(),
            &execution.candidate.id,
            execution.candidate.candidate_version,
            CandidateReviewAction::Accept {
                edited_output: None,
            },
            "Reviewed and approved for the Human Document.",
        )
        .unwrap();
    let mut accepted_request = HumanDocumentRequest::integrated();
    accepted_request.accepted_ai_candidate_ids = vec![accepted.id.clone()];
    let document = store.compose_human_document(accepted_request).unwrap();
    let ai_block = document
        .blocks
        .iter()
        .find(|block| block.meta.id == format!("ai-narrative-{}", accepted.id))
        .expect("reviewed AI narrative block");
    assert_eq!(
        ai_block.meta.contribution.candidate_id.as_deref(),
        Some(accepted.id.as_str())
    );
    assert!(ai_block.meta.contribution.attempt_id.is_some());
    assert!(
        document
            .citations
            .iter()
            .any(|citation| citation.source_id == accepted.id)
    );

    let diagram_task = store
        .create_semantic_task(
            &user(),
            NewSemanticTask {
                task_type: SemanticTaskType::DiagramPlan,
                source_entity_ids: vec![source.clone()],
                source_artifact_ids: vec![],
                output_schema: json!({
                    "type":"object",
                    "additionalProperties":false,
                    "properties":{
                        "nodes":{"type":"array","minItems":1,"maxItems":50,"items":{
                            "type":"object","additionalProperties":false,
                            "properties":{
                                "id":{"type":"string","minLength":1,"maxLength":200},
                                "label":{"type":"string","minLength":1,"maxLength":300},
                                "kind":{"type":"string","maxLength":100},
                                "status":{"type":"string","maxLength":100},
                                "source_ids":{"type":"array","items":{"type":"string"},"minItems":1,"maxItems":20}
                            },
                            "required":["id","label","source_ids"]
                        }},
                        "edges":{"type":"array","maxItems":100,"items":{
                            "type":"object","additionalProperties":false,
                            "properties":{
                                "source":{"type":"string","minLength":1,"maxLength":200},
                                "target":{"type":"string","minLength":1,"maxLength":200},
                                "source_ids":{"type":"array","items":{"type":"string"},"maxItems":20}
                            },
                            "required":["source","target","source_ids"]
                        }},
                        "textual_alternative":{"type":"string","minLength":1,"maxLength":10000}
                    },
                    "required":["nodes","edges","textual_alternative"]
                }),
                prompt_template_id: "cp8-diagram-plan".into(),
                prompt_template_version: 1,
                requirements: SemanticRequirements::default(),
                route_policy: SemanticRoutePolicy {
                    preferred_profile_id: Some(profile.id.clone()),
                    allowed_profile_ids: vec![profile.id.clone()],
                    allow_failover: false,
                },
                privacy_audience: "project-owner".into(),
                consent_id: None,
                max_input_units: 5_000,
                max_output_units: 500,
                timeout_ms: 10_000,
                cache_ttl_seconds: 3_600,
            },
        )
        .unwrap();
    let diagram_execution = store
        .execute_semantic_task(
            &user(),
            &diagram_task.id,
            &DiagramTransport { source_id: source },
        )
        .unwrap();
    let accepted_diagram = store
        .review_semantic_candidate(
            &user(),
            &diagram_execution.candidate.id,
            diagram_execution.candidate.candidate_version,
            CandidateReviewAction::Accept {
                edited_output: None,
            },
            "Reviewed diagram plan for the Human Document.",
        )
        .unwrap();
    let mut diagram_request = HumanDocumentRequest::integrated();
    diagram_request.accepted_ai_candidate_ids = vec![accepted_diagram.id.clone()];
    let diagram_document = store.compose_human_document(diagram_request).unwrap();
    let diagram_block = diagram_document
        .blocks
        .iter()
        .find(|block| block.meta.id == format!("ai-diagram-{}", accepted_diagram.id))
        .expect("reviewed AI diagram block");
    assert_eq!(
        diagram_block.meta.contribution.candidate_id.as_deref(),
        Some(accepted_diagram.id.as_str())
    );
    assert!(diagram_block.meta.contribution.attempt_id.is_some());
}

#[test]
fn request_limits_pagination_and_schema_migration_are_enforced() {
    let (directory, store) = setup();
    entity(&store, "concept", "Bounded report");
    let mut invalid = HumanDocumentRequest::integrated();
    invalid.max_graph_nodes = 201;
    assert!(matches!(
        store.compose_human_document(invalid),
        Err(CoreError::Validation(_))
    ));

    let document = store
        .compose_human_document(HumanDocumentRequest::integrated())
        .unwrap();
    store.save_human_document(&user(), &document).unwrap();
    let page = store
        .list_human_documents(
            Some(HumanDocumentKind::IntegratedReport),
            PageRequest {
                limit: 1,
                offset: 0,
            },
        )
        .unwrap();
    assert_eq!(page.items.len(), 1);
    let version: u32 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, CORE_SCHEMA_VERSION);
    drop(store);

    let reopened = ContinuityStore::open(directory.path().join("project")).unwrap();
    let version: u32 = reopened
        .debug_connection()
        .unwrap()
        .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, CORE_SCHEMA_VERSION);
    assert!(reopened.verify_integrity().unwrap().is_healthy());
}
