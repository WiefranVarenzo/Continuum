use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use continuum_core::{
    ActorRef, AnalysisLimits, AnalyzeBaselineInput, ChangeSetKind, CheckpointScope, CommandContext,
    ContinuityStore, CoreError, DevelopmentIntentOrigin, EvidenceKind, FeedbackKind,
    FeedbackRelation, FeedbackSeverity, FeedbackTarget, FindingSource, FindingSourceAssessment,
    GraphDirection, GraphQuery, LearningFeedbackTransition, LinkTestVerificationInput,
    NewChangeSet, NewDecision, NewDevelopmentRequirement, NewEvidence, NewFinding,
    NewLearningFeedback, NewRelationship, NewRequirement, NewResearchQuestion, NewTestRun,
    OriginKind, PageRequest, QuestionKind, RelationshipAction, RelationshipReviewState,
    RelationshipStateChange, RepositoryAttachInput, RequirementImplementation,
    RequirementLinkInput, RequirementRationaleOrigin, Space, TestOutcome, TestResultInput,
    TestRunSource,
};
use serde_json::json;
use tempfile::TempDir;

fn user_command() -> CommandContext {
    CommandContext::new(ActorRef::user("cp6-user"))
}

fn run_git(root: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_repository(directory: &TempDir) -> PathBuf {
    let root = directory.path().join("repository");
    fs::create_dir_all(root.join("src")).unwrap();
    run_git(&root, &["init", "-b", "main"]);
    run_git(&root, &["config", "user.name", "Continuum Test"]);
    run_git(
        &root,
        &["config", "user.email", "continuum@example.invalid"],
    );
    run_git(&root, &["config", "commit.gpgsign", "false"]);
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"cp6-fixture\"\nversion=\"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn resume() -> bool { true }\n\
         #[test]\nfn resume_keeps_context() { assert!(resume()); }\n",
    )
    .unwrap();
    run_git(&root, &["add", "-f", "."]);
    run_git(&root, &["commit", "-m", "Implement resumable context"]);
    root
}

struct ConnectedFixture {
    _project_directory: TempDir,
    _repository_directory: TempDir,
    store: ContinuityStore,
    evidence_id: String,
    finding_id: String,
    decision_id: String,
    requirement_id: String,
    change_set_id: String,
    code_entity_id: String,
    test_id: String,
    test_run_id: String,
}

fn connected_fixture() -> ConnectedFixture {
    let project_directory = tempfile::tempdir().unwrap();
    let repository_directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create_with_actor(
        project_directory.path().join("continuum-project"),
        "CP6 connected fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    store
        .set_space_capability_with_context(&user_command(), Space::Research, true)
        .unwrap();
    store
        .set_space_capability_with_context(&user_command(), Space::Development, true)
        .unwrap();

    let question = store
        .create_research_question(
            &user_command(),
            NewResearchQuestion {
                title: "How can work resume safely?".into(),
                kind: QuestionKind::Question,
                question: "Which state must Continuum preserve for a reliable resume?".into(),
                context: "Users pause research and development frequently.".into(),
                desired_outcome: "A testable continuity requirement.".into(),
                priority: 4,
                due_at: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let evidence = store
        .create_evidence(
            &user_command(),
            NewEvidence {
                title: "Resume observation".into(),
                kind: EvidenceKind::Observation,
                origin: OriginKind::User,
                source_uri: None,
                source_title: Some("Observed workflow".into()),
                source_author: Some("cp6-user".into()),
                captured_at: None,
                capture_method: "manual_observation".into(),
                stable_reference: Some("fixture:resume-observation".into()),
                source_content: Some(
                    "A useful resume needs the last state, unresolved work, and next action."
                        .into(),
                ),
                annotation: "This is source material, not an AI summary.".into(),
                summary: "Resume requires explicit context.".into(),
                relevance: "Defines the continuity need.".into(),
                original_artifact_id: None,
                question_id: Some(question.entity.id),
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let finding = store
        .create_finding(
            &user_command(),
            NewFinding {
                title: "Explicit context enables resumption".into(),
                claim: "State, unresolved work, and next actions must be preserved together."
                    .into(),
                interpretation: "A timestamp or file bookmark alone is insufficient.".into(),
                uncertainty: "Presentation is deferred to CP8.".into(),
                confidence: Some(0.95),
                sources: vec![FindingSource {
                    entity_id: evidence.entity.id.clone(),
                    assessment: FindingSourceAssessment::Supports,
                }],
                answers_question_id: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let finding = store
        .transition_research_item(
            &user_command(),
            &finding.entity.id,
            finding.entity.version,
            "accepted",
            None,
        )
        .unwrap();
    let decision = store
        .create_decision(
            &user_command(),
            NewDecision {
                title: "Persist a source-backed resume state".into(),
                selected_option: "Use canonical provenance and immutable checkpoints".into(),
                rationale: "The accepted finding requires inspectable context.".into(),
                alternatives: vec!["Store only the last opened file".into()],
                constraints: vec!["AI remains optional".into()],
                finding_ids: vec![finding.entity.id.clone()],
                supersedes_decision_id: None,
                supersedes_expected_version: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let decision = store
        .transition_research_item(
            &user_command(),
            &decision.entity.id,
            decision.entity.version,
            "accepted",
            None,
        )
        .unwrap();
    let requirement = store
        .create_requirement(
            &user_command(),
            NewRequirement {
                title: "Resume keeps actionable context".into(),
                statement: "Resume must expose saved state, unresolved work, and next actions."
                    .into(),
                acceptance_criteria: vec!["A deterministic test verifies resume state.".into()],
                priority: 4,
                rationale_origin: RequirementRationaleOrigin::Research,
                verification_method: "Automated test observation".into(),
                decision_id: Some(decision.entity.id.clone()),
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let root = init_repository(&repository_directory);
    let repository = store
        .attach_repository(
            &user_command(),
            RepositoryAttachInput {
                path: root,
                title: Some("CP6 fixture repository".into()),
                metadata: json!({}),
            },
        )
        .unwrap();
    let baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    let ingestion = store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    let change_set = store
        .create_change_set(
            &user_command(),
            NewChangeSet {
                title: "Implement resumable context".into(),
                summary: "Adds deterministic resume behavior and its test.".into(),
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                kind: ChangeSetKind::Committed,
                commit_entity_ids: ingestion.ingested_commit_ids,
                requirement_links: vec![RequirementLinkInput {
                    requirement_id: requirement.entity.id.clone(),
                    relationship: RequirementImplementation::Implements,
                }],
                intent_origin: DevelopmentIntentOrigin::Research,
                supersedes_change_set_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    store
        .analyze_repository_baseline(
            &user_command(),
            AnalyzeBaselineInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                limits: AnalysisLimits::default(),
            },
        )
        .unwrap();
    let connection = store.debug_connection().unwrap();
    let code_entity_id: String = connection
        .query_row(
            "SELECT ce.entity_id FROM code_entities ce
             JOIN code_entity_observations o ON o.code_entity_id=ce.entity_id
             WHERE ce.repository_id=?1 AND ce.entity_kind='file' AND o.source_path='src/lib.rs'
             ORDER BY o.observed_at DESC LIMIT 1",
            [&repository.entity.id],
            |row| row.get(0),
        )
        .unwrap();
    let test_id: String = connection
        .query_row(
            "SELECT e.id FROM entities e WHERE e.project_id=?1 AND e.entity_type='test'
             AND e.title='resume_keeps_context' LIMIT 1",
            [&store.manifest().project_id],
            |row| row.get(0),
        )
        .unwrap();
    drop(connection);
    store
        .link_test_verification(
            &user_command(),
            LinkTestVerificationInput {
                test_id: test_id.clone(),
                target_entity_id: requirement.entity.id.clone(),
            },
        )
        .unwrap();
    let test_run = store
        .record_test_run(
            &user_command(),
            NewTestRun {
                title: "Resume acceptance run".into(),
                repository_id: repository.entity.id,
                baseline_id: baseline.entity.id,
                outcome: TestOutcome::Passed,
                command_label: "cargo test resume_keeps_context".into(),
                exit_code: Some(0),
                duration_ms: Some(8),
                source: TestRunSource::User,
                results: vec![TestResultInput {
                    test_id: test_id.clone(),
                    outcome: TestOutcome::Passed,
                    duration_ms: Some(2),
                    message: None,
                }],
                details: json!({"fixture":"cp6-connected-v1"}),
                metadata: json!({}),
            },
        )
        .unwrap();

    ConnectedFixture {
        _project_directory: project_directory,
        _repository_directory: repository_directory,
        store,
        evidence_id: evidence.entity.id,
        finding_id: finding.entity.id,
        decision_id: decision.entity.id,
        requirement_id: requirement.entity.id,
        change_set_id: change_set.entity.id,
        code_entity_id,
        test_id,
        test_run_id: test_run.entity.id,
    }
}

#[test]
fn connected_chain_is_bidirectional_validated_exportable_and_feedback_is_non_mutating() {
    let fixture = connected_fixture();
    let store = &fixture.store;
    let feedback = store
        .create_learning_feedback(
            &user_command(),
            NewLearningFeedback {
                title: "Validation reveals a documentation caveat".into(),
                kind: FeedbackKind::Observation,
                summary: "The code passes, but the decision should describe stale context.".into(),
                details: json!({"test_outcome":"passed"}),
                severity: FeedbackSeverity::Warning,
                origin: OriginKind::User,
                source_entity_ids: vec![fixture.test_run_id.clone()],
                targets: vec![FeedbackTarget {
                    entity_id: fixture.decision_id.clone(),
                    relation: FeedbackRelation::RequestsRevisionOf,
                }],
                metadata: json!({}),
            },
        )
        .unwrap();
    assert_eq!(feedback.entity.status, "open");
    assert_eq!(
        store.get_entity(&fixture.decision_id).unwrap().status,
        "accepted",
        "Learning Feedback must not mutate its target automatically"
    );

    let graph = store
        .traverse_provenance(GraphQuery {
            root_entity_ids: vec![fixture.evidence_id.clone()],
            direction: GraphDirection::Both,
            max_depth: 8,
            max_nodes: 500,
            max_edges: 1_500,
            ..Default::default()
        })
        .unwrap();
    for expected in [
        &fixture.evidence_id,
        &fixture.finding_id,
        &fixture.decision_id,
        &fixture.requirement_id,
        &fixture.change_set_id,
        &fixture.code_entity_id,
        &fixture.test_id,
        &fixture.test_run_id,
        &feedback.entity.id,
    ] {
        assert!(
            graph.nodes.iter().any(|node| &node.entity.id == expected),
            "missing graph node {expected}"
        );
    }
    for relation in [
        "supports",
        "informs",
        "creates",
        "implements",
        "modifies",
        "verifies",
        "executes",
        "produces",
        "requests_revision_of",
    ] {
        assert!(
            graph
                .edges
                .iter()
                .any(|edge| edge.relation_type == relation),
            "missing graph relation {relation}"
        );
    }
    assert!(!graph.truncated);

    let checkpoint = store
        .create_checkpoint_with_context(
            &user_command(),
            CheckpointScope::Integrated,
            &json!({"next_actions":["Review validation feedback"]}),
            &[fixture.evidence_id.clone(), fixture.requirement_id.clone()],
        )
        .unwrap();
    let checkpoint_graph = store
        .traverse_provenance(GraphQuery {
            checkpoint_id: Some(checkpoint.id),
            max_depth: 1,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(checkpoint_graph.root_entity_ids.len(), 2);
    assert!(
        checkpoint_graph
            .omissions
            .iter()
            .any(|value| { value.contains("current, not a reconstructed historical snapshot") })
    );

    let target_edge = graph
        .edges
        .iter()
        .find(|edge| {
            edge.source_entity_id == feedback.entity.id
                && edge.target_entity_id == fixture.decision_id
        })
        .unwrap();
    let rejected = store
        .change_relationship_state(
            &user_command(),
            &target_edge.id,
            RelationshipStateChange {
                action: RelationshipAction::Reject,
                annotation: "The Decision already documents the caveat.".into(),
                expected_state_version: target_edge.state_version,
            },
        )
        .unwrap();
    assert_eq!(rejected.status, "rejected");
    assert_eq!(rejected.review_state, "rejected");
    let history = store
        .list_relationship_history(&rejected.id, PageRequest::default())
        .unwrap();
    assert_eq!(history.items.len(), 2);
    assert_eq!(history.items[0].action, "created");
    assert_eq!(history.items[1].action, "rejected");

    let feedback = store
        .transition_learning_feedback(
            &user_command(),
            &feedback.entity.id,
            LearningFeedbackTransition {
                target_status: "resolved".into(),
                note: "Reviewed without changing the accepted Decision.".into(),
                expected_version: feedback.entity.version,
            },
        )
        .unwrap();
    assert_eq!(feedback.entity.status, "resolved");
    assert!(store.validate_provenance_graph().unwrap().is_healthy());
    assert!(store.verify_integrity().unwrap().is_healthy());

    let export = fixture._project_directory.path().join("cp6-export");
    store.export_project(&export).unwrap();
    let restored = ContinuityStore::import_export(
        &export,
        fixture._project_directory.path().join("cp6-restored"),
    )
    .unwrap();
    assert_eq!(
        restored
            .get_learning_feedback(&feedback.entity.id)
            .unwrap()
            .resolution_note,
        "Reviewed without changing the accepted Decision."
    );
    assert!(restored.verify_integrity().unwrap().is_healthy());
}

#[test]
fn standalone_partial_graphs_are_valid_without_synthetic_cross_space_records() {
    let directory = tempfile::tempdir().unwrap();
    let research =
        ContinuityStore::create(directory.path().join("research-only"), "CP6 Research only")
            .unwrap();
    research
        .set_space_capability_with_context(&user_command(), Space::Research, true)
        .unwrap();
    let question = research
        .create_research_question(
            &user_command(),
            NewResearchQuestion {
                title: "Standalone research".into(),
                kind: QuestionKind::Question,
                question: "Can this research stop before a Requirement?".into(),
                context: String::new(),
                desired_outcome: "A documented finding".into(),
                priority: 2,
                due_at: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let research_graph = research
        .traverse_provenance(GraphQuery {
            root_entity_ids: vec![question.entity.id],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(research_graph.nodes.len(), 1);
    assert!(!research.capability_enabled(Space::Development).unwrap());
    assert!(research.validate_provenance_graph().unwrap().is_healthy());

    let development = ContinuityStore::create(
        directory.path().join("development-only"),
        "CP6 Development only",
    )
    .unwrap();
    development
        .set_space_capability_with_context(&user_command(), Space::Development, true)
        .unwrap();
    let requirement = development
        .create_development_requirement(
            &user_command(),
            NewDevelopmentRequirement {
                title: "External requirement".into(),
                statement: "An existing brief may enter Development directly.".into(),
                acceptance_criteria: vec![],
                priority: 2,
                rationale_origin: RequirementRationaleOrigin::External,
                verification_method: String::new(),
                metadata: json!({"source":"legacy tracker"}),
            },
        )
        .unwrap();
    let report = development.validate_provenance_graph().unwrap();
    assert!(report.is_healthy());
    assert!(!report.issues.iter().any(|issue| {
        issue.code == "research_requirement_missing_decision"
            && issue.entity_ids.contains(&requirement.id)
    }));
    assert!(!development.capability_enabled(Space::Research).unwrap());
}

#[test]
fn graph_limits_cross_project_links_cycles_and_stale_writes_fail_closed() {
    let directory = tempfile::tempdir().unwrap();
    let first = ContinuityStore::create(directory.path().join("first"), "First").unwrap();
    let second = ContinuityStore::create(directory.path().join("second"), "Second").unwrap();
    first
        .set_space_capability_with_context(&user_command(), Space::Research, true)
        .unwrap();
    second
        .set_space_capability_with_context(&user_command(), Space::Research, true)
        .unwrap();
    let decision = |store: &ContinuityStore, title: &str| {
        store
            .create_decision(
                &user_command(),
                NewDecision {
                    title: title.into(),
                    selected_option: title.into(),
                    rationale: "Cycle fixture".into(),
                    alternatives: vec![],
                    constraints: vec![],
                    finding_ids: vec![],
                    supersedes_decision_id: None,
                    supersedes_expected_version: None,
                    session_id: None,
                    metadata: json!({}),
                },
            )
            .unwrap()
    };
    let a = decision(&first, "A");
    let b = decision(&first, "B");
    let foreign = decision(&second, "Foreign");
    let ab = first
        .create_relationship_with_context(
            &user_command(),
            NewRelationship {
                relation_type: "supersedes".into(),
                relation_version: 1,
                source_entity_id: a.entity.id.clone(),
                target_entity_id: b.entity.id.clone(),
                origin: OriginKind::User,
                confidence: None,
                review_state: RelationshipReviewState::Unreviewed,
                direct_source_ids: vec![],
                supersedes_id: None,
            },
        )
        .unwrap();
    let before = first.summary().unwrap().ledger_sequence;
    let cycle = first
        .create_relationship_with_context(
            &user_command(),
            NewRelationship {
                relation_type: "supersedes".into(),
                relation_version: 1,
                source_entity_id: b.entity.id.clone(),
                target_entity_id: a.entity.id.clone(),
                origin: OriginKind::User,
                confidence: None,
                review_state: RelationshipReviewState::Unreviewed,
                direct_source_ids: vec![],
                supersedes_id: None,
            },
        )
        .unwrap_err();
    assert!(matches!(cycle, CoreError::Validation(_)));
    assert_eq!(first.summary().unwrap().ledger_sequence, before);
    let cross_project = first
        .create_relationship_with_context(
            &user_command(),
            NewRelationship {
                relation_type: "supersedes".into(),
                relation_version: 1,
                source_entity_id: a.entity.id.clone(),
                target_entity_id: foreign.entity.id,
                origin: OriginKind::User,
                confidence: None,
                review_state: RelationshipReviewState::Unreviewed,
                direct_source_ids: vec![],
                supersedes_id: None,
            },
        )
        .unwrap_err();
    assert!(matches!(cross_project, CoreError::Validation(_)));

    let current = first.get_relationship(&ab).unwrap();
    let annotated = first
        .change_relationship_state(
            &user_command(),
            &ab,
            RelationshipStateChange {
                action: RelationshipAction::Annotate,
                annotation: "First review".into(),
                expected_state_version: current.state_version,
            },
        )
        .unwrap();
    let stale = first
        .change_relationship_state(
            &user_command(),
            &ab,
            RelationshipStateChange {
                action: RelationshipAction::Retire,
                annotation: String::new(),
                expected_state_version: current.state_version,
            },
        )
        .unwrap_err();
    assert!(matches!(stale, CoreError::Conflict(_)));
    assert_eq!(
        first.get_relationship(&ab).unwrap().annotation,
        annotated.annotation
    );
    let replacement = first
        .create_relationship_with_context(
            &user_command(),
            NewRelationship {
                relation_type: "supersedes".into(),
                relation_version: 1,
                source_entity_id: a.entity.id.clone(),
                target_entity_id: b.entity.id,
                origin: OriginKind::User,
                confidence: Some(1.0),
                review_state: RelationshipReviewState::Accepted,
                direct_source_ids: vec![],
                supersedes_id: Some(ab.clone()),
            },
        )
        .unwrap();
    assert_eq!(first.get_relationship(&ab).unwrap().status, "superseded");
    assert_eq!(
        first
            .list_relationship_history(&ab, PageRequest::default())
            .unwrap()
            .items
            .last()
            .unwrap()
            .action,
        "superseded"
    );
    assert_eq!(
        first.get_relationship(&replacement).unwrap().status,
        "active"
    );

    let oversized = first
        .traverse_provenance(GraphQuery {
            root_entity_ids: vec![a.entity.id],
            max_nodes: 501,
            ..Default::default()
        })
        .unwrap_err();
    assert!(matches!(oversized, CoreError::Validation(_)));
    assert!(first.verify_integrity().unwrap().is_healthy());
}

#[test]
fn provenance_validator_reports_real_connected_gaps_without_rejecting_partial_work() {
    let directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create(directory.path().join("project"), "Gap fixture").unwrap();
    store
        .set_space_capability_with_context(&user_command(), Space::Development, true)
        .unwrap();
    let requirement = store
        .create_development_requirement(
            &user_command(),
            NewDevelopmentRequirement {
                title: "Broken research handoff".into(),
                statement: "This claims research provenance without a Decision.".into(),
                acceptance_criteria: vec![],
                priority: 2,
                rationale_origin: RequirementRationaleOrigin::External,
                verification_method: String::new(),
                metadata: json!({}),
            },
        )
        .unwrap();
    // Simulate a corrupt/legacy ledger that claims research provenance without
    // using the typed Research command which normally prevents this state.
    store
        .debug_connection()
        .unwrap()
        .execute(
            "UPDATE requirements SET rationale_origin='research' WHERE entity_id=?1",
            [&requirement.id],
        )
        .unwrap();
    let report = store.validate_provenance_graph().unwrap();
    assert!(!report.is_healthy());
    assert!(report.issues.iter().any(|issue| {
        issue.code == "research_requirement_missing_decision"
            && issue.entity_ids.contains(&requirement.id)
    }));
}
