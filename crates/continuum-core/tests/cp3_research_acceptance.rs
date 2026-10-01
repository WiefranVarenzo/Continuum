use continuum_core::{
    ActorKind, ActorRef, CommandContext, ContinuityStore, CoreError, EvidenceAnnotationUpdate,
    EvidenceDetailsUpdate, EvidenceKind, FindingSource, FindingSourceAssessment, NewDecision,
    NewEntity, NewEvidence, NewExperiment, NewFinding, NewRelationship, NewRequirement,
    NewResearchQuestion, NewResearchResult, NewResearchSession, OriginKind, PageRequest,
    QuestionKind, RelationshipReviewState, RequirementRationaleOrigin, ResearchCheckpointInput,
    ResearchEntityKind, ResearchQuestionUpdate, ResearchResultOutcome, ResearchSearchQuery,
    ResearchTimelineFilter, Space, new_id,
};
use serde_json::json;
use tempfile::TempDir;

fn user_command() -> CommandContext {
    CommandContext::new(ActorRef::user("researcher-1"))
}

fn enabled_project() -> (TempDir, ContinuityStore) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("project");
    let store =
        ContinuityStore::create_with_actor(&root, "CP3 fixture", ActorRef::user("owner")).unwrap();
    store
        .set_space_capability_with_context(&user_command(), Space::Research, true)
        .unwrap();
    (directory, store)
}

fn session(store: &ContinuityStore) -> String {
    store
        .create_research_session(
            &user_command(),
            NewResearchSession {
                title: "Storage research".into(),
                objective: "Determine a durable local storage strategy".into(),
                started_at: None,
                metadata: json!({"owner":"researcher-1"}),
            },
        )
        .unwrap()
        .entity
        .id
}

fn question(store: &ContinuityStore, session_id: Option<&str>, title: &str) -> String {
    store
        .create_research_question(
            &user_command(),
            NewResearchQuestion {
                title: title.into(),
                kind: QuestionKind::Question,
                question: "Which local storage design remains recoverable?".into(),
                context: "Continuum must work offline.".into(),
                desired_outcome: "A verified design choice.".into(),
                priority: 3,
                due_at: None,
                session_id: session_id.map(str::to_owned),
                metadata: json!({}),
            },
        )
        .unwrap()
        .entity
        .id
}

fn note_evidence(
    store: &ContinuityStore,
    session_id: Option<&str>,
    question_id: Option<&str>,
    title: &str,
) -> String {
    store
        .create_evidence(
            &user_command(),
            NewEvidence {
                title: title.into(),
                kind: EvidenceKind::Note,
                origin: OriginKind::User,
                source_uri: None,
                source_title: Some("Direct observation".into()),
                source_author: Some("researcher-1".into()),
                captured_at: None,
                capture_method: "manual_note".into(),
                stable_reference: None,
                source_content: Some("SQLite recovery succeeded after process termination.".into()),
                annotation: "Relevant to local durability.".into(),
                summary: "Recovery succeeded.".into(),
                relevance: "Tests the primary failure mode.".into(),
                original_artifact_id: None,
                question_id: question_id.map(str::to_owned),
                session_id: session_id.map(str::to_owned),
                metadata: json!({}),
            },
        )
        .unwrap()
        .entity
        .id
}

fn running_experiment(
    store: &ContinuityStore,
    session_id: Option<&str>,
    question_id: Option<&str>,
) -> String {
    let experiment = store
        .create_experiment(
            &user_command(),
            NewExperiment {
                title: "Crash recovery experiment".into(),
                hypothesis: "A committed transaction survives restart.".into(),
                method: "Commit, terminate the process, then reopen and verify.".into(),
                inputs: json!({"iterations":10}),
                expected_observations: "All committed records remain present.".into(),
                question_id: question_id.map(str::to_owned),
                session_id: session_id.map(str::to_owned),
                metadata: json!({}),
            },
        )
        .unwrap();
    store
        .transition_research_item(
            &user_command(),
            &experiment.entity.id,
            experiment.entity.version,
            "running",
            None,
        )
        .unwrap();
    experiment.entity.id
}

#[test]
fn research_only_cycle_is_auditable_resumable_and_requires_no_development() {
    let (_directory, store) = enabled_project();
    let session_id = session(&store);
    let question_id = question(&store, Some(&session_id), "Durable local storage");
    let evidence_id = note_evidence(
        &store,
        Some(&session_id),
        Some(&question_id),
        "Recovery observation",
    );
    let experiment_id = running_experiment(&store, Some(&session_id), Some(&question_id));
    let result = store
        .create_research_result(
            &user_command(),
            NewResearchResult {
                title: "Ten successful recoveries".into(),
                experiment_id: experiment_id.clone(),
                observation: "10 of 10 committed transactions were recovered.".into(),
                outcome: ResearchResultOutcome::Positive,
                measurements: json!({"successful":10,"attempted":10}),
                observed_at: None,
                artifact_id: None,
                session_id: Some(session_id.clone()),
                metadata: json!({}),
            },
        )
        .unwrap();
    let finding = store
        .create_finding(
            &user_command(),
            NewFinding {
                title: "SQLite transaction recovery is sufficient".into(),
                claim: "Committed local transactions survived the tested termination mode.".into(),
                interpretation: "SQLite is suitable for the canonical metadata ledger.".into(),
                uncertainty: "Power-loss behavior remains a separate test.".into(),
                confidence: Some(0.9),
                sources: vec![
                    FindingSource {
                        entity_id: evidence_id.clone(),
                        assessment: FindingSourceAssessment::Supports,
                    },
                    FindingSource {
                        entity_id: result.entity.id.clone(),
                        assessment: FindingSourceAssessment::InconclusiveFor,
                    },
                ],
                answers_question_id: Some(question_id.clone()),
                session_id: Some(session_id.clone()),
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
            Some("Supported by the recovery observation."),
        )
        .unwrap();
    let decision = store
        .create_decision(
            &user_command(),
            NewDecision {
                title: "Use SQLite for canonical metadata".into(),
                selected_option: "SQLite WAL".into(),
                rationale: "It satisfies the tested durability and local-first constraints.".into(),
                alternatives: vec!["Flat JSON files".into(), "Remote database".into()],
                constraints: vec!["Foreign keys remain enabled".into()],
                finding_ids: vec![finding.entity.id.clone()],
                supersedes_decision_id: None,
                supersedes_expected_version: None,
                session_id: Some(session_id.clone()),
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
                title: "Transactional metadata persistence".into(),
                statement: "Canonical metadata writes must be transactional.".into(),
                acceptance_criteria: vec!["Interrupted writes leave no partial state.".into()],
                priority: 4,
                rationale_origin: RequirementRationaleOrigin::Research,
                verification_method: "Integration test".into(),
                decision_id: Some(decision.entity.id.clone()),
                session_id: Some(session_id.clone()),
                metadata: json!({}),
            },
        )
        .unwrap();
    assert_eq!(requirement.entity.status, "draft");

    let checkpoint = store
        .create_research_checkpoint(
            &user_command(),
            ResearchCheckpointInput {
                note: "Pause after selecting storage.".into(),
                next_actions: vec!["Test simulated power loss.".into()],
            },
        )
        .unwrap();
    assert_eq!(checkpoint.scope, "research");
    assert_eq!(checkpoint.summary["development_required"], json!(false));
    let resume = store.resume_research().unwrap();
    assert!(!resume.checkpoint_is_stale);
    assert_eq!(resume.next_actions, vec!["Test simulated power loss."]);
    assert!(!store.capability_enabled(Space::Development).unwrap());

    let report = store.generate_research_report().unwrap();
    assert!(report.markdown.contains(&evidence_id));
    assert!(report.markdown.contains(&decision.entity.id));
    assert!(report.cited_entity_ids.contains(&finding.entity.id));
    assert!(store.verify_integrity().unwrap().is_healthy());
}

#[test]
fn result_can_exist_without_a_finding_and_finding_can_mix_source_assessments() {
    let (_directory, store) = enabled_project();
    let question_id = question(&store, None, "Source assessment");
    let supporting = note_evidence(&store, None, Some(&question_id), "Supporting evidence");
    let challenging = note_evidence(&store, None, Some(&question_id), "Challenging evidence");
    let experiment_id = running_experiment(&store, None, Some(&question_id));
    let result = store
        .create_research_result(
            &user_command(),
            NewResearchResult {
                title: "Independent observation".into(),
                experiment_id,
                observation: "The outcome varied across runs.".into(),
                outcome: ResearchResultOutcome::Mixed,
                measurements: json!({}),
                observed_at: None,
                artifact_id: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    assert!(
        store
            .search_research(ResearchSearchQuery {
                text: String::new(),
                entity_type: Some(ResearchEntityKind::Finding),
                status: None,
                session_id: None,
                page: PageRequest::default(),
            })
            .unwrap()
            .items
            .is_empty()
    );

    let finding = store
        .create_finding(
            &user_command(),
            NewFinding {
                title: "Mixed conclusion".into(),
                claim: "Recovery behavior depends on the failure mode.".into(),
                interpretation: "More targeted experiments are required.".into(),
                uncertainty: "Power-loss mode is not covered.".into(),
                confidence: Some(0.5),
                sources: vec![
                    FindingSource {
                        entity_id: supporting,
                        assessment: FindingSourceAssessment::Supports,
                    },
                    FindingSource {
                        entity_id: challenging,
                        assessment: FindingSourceAssessment::Challenges,
                    },
                    FindingSource {
                        entity_id: result.entity.id,
                        assessment: FindingSourceAssessment::InconclusiveFor,
                    },
                ],
                answers_question_id: Some(question_id),
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let relationships = store
        .list_relationships_for_entity(&finding.entity.id, PageRequest::default())
        .unwrap();
    let types: Vec<_> = relationships
        .items
        .iter()
        .map(|relationship| relationship.relation_type.as_str())
        .collect();
    assert!(types.contains(&"supports"));
    assert!(types.contains(&"challenges"));
    assert!(types.contains(&"inconclusive_for"));
}

#[test]
fn evidence_original_source_is_immutable_while_annotations_are_versioned() {
    let (_directory, store) = enabled_project();
    let artifact = store
        .ingest_artifact(&new_id(), b"immutable source bytes", "text/plain")
        .unwrap();
    let evidence = store
        .create_evidence(
            &user_command(),
            NewEvidence {
                title: "Imported source".into(),
                kind: EvidenceKind::File,
                origin: OriginKind::Import,
                source_uri: Some("file:///source.txt".into()),
                source_title: Some("Source file".into()),
                source_author: None,
                captured_at: None,
                capture_method: "file_import".into(),
                stable_reference: Some(artifact.sha256.clone()),
                source_content: None,
                annotation: "First annotation".into(),
                summary: String::new(),
                relevance: String::new(),
                original_artifact_id: Some(artifact.id.clone()),
                question_id: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let updated = store
        .update_evidence_annotation(
            &user_command(),
            &evidence.entity.id,
            EvidenceAnnotationUpdate {
                annotation: "Reviewed annotation".into(),
                summary: "Source remains unchanged.".into(),
                relevance: "High".into(),
                expected_version: evidence.entity.version,
            },
        )
        .unwrap();
    assert_eq!(updated.entity.version, 2);
    assert_eq!(updated.details["original_artifact_id"], artifact.id);
    assert_eq!(updated.details["stable_reference"], artifact.sha256);
    assert_eq!(updated.details["annotation"], "Reviewed annotation");

    let renamed = store
        .update_evidence_details(
            &user_command(),
            &updated.entity.id,
            EvidenceDetailsUpdate {
                title: "Pricing evidence for realtime transcription".into(),
                annotation: "User intent: compare realtime pricing.".into(),
                summary: "The captured image shows a pricing table.".into(),
                relevance: "Supports the active cost comparison.".into(),
                expected_version: updated.entity.version,
            },
        )
        .unwrap();
    assert_eq!(
        renamed.entity.title,
        "Pricing evidence for realtime transcription"
    );
    assert_eq!(renamed.entity.version, 3);
    assert_eq!(renamed.details["original_artifact_id"], artifact.id);
    assert_eq!(
        renamed.details["annotation"],
        "User intent: compare realtime pricing."
    );
}

#[test]
fn decision_supersession_preserves_prior_rationale_and_requirement_is_optional() {
    let (_directory, store) = enabled_project();
    let first = store
        .create_decision(
            &user_command(),
            NewDecision {
                title: "First decision".into(),
                selected_option: "Option A".into(),
                rationale: "Initial constraint set.".into(),
                alternatives: vec!["Option B".into()],
                constraints: vec![],
                finding_ids: vec![],
                supersedes_decision_id: None,
                supersedes_expected_version: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let first = store
        .transition_research_item(
            &user_command(),
            &first.entity.id,
            first.entity.version,
            "accepted",
            None,
        )
        .unwrap();
    let stale_supersession = store.create_decision(
        &user_command(),
        NewDecision {
            title: "Stale revised decision".into(),
            selected_option: "Option B".into(),
            rationale: "This request observed an old version.".into(),
            alternatives: vec!["Option A".into()],
            constraints: vec![],
            finding_ids: vec![],
            supersedes_decision_id: Some(first.entity.id.clone()),
            supersedes_expected_version: Some(1),
            session_id: None,
            metadata: json!({}),
        },
    );
    assert!(matches!(stale_supersession, Err(CoreError::Conflict(_))));
    let decision_count: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM decisions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        decision_count, 1,
        "failed supersession must roll back its new Decision"
    );
    let second = store
        .create_decision(
            &user_command(),
            NewDecision {
                title: "Revised decision".into(),
                selected_option: "Option B".into(),
                rationale: "A new constraint invalidated Option A.".into(),
                alternatives: vec!["Option A".into()],
                constraints: vec!["New constraint".into()],
                finding_ids: vec![],
                supersedes_decision_id: Some(first.entity.id.clone()),
                supersedes_expected_version: Some(first.entity.version),
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let prior = store.get_research_item(&first.entity.id).unwrap();
    assert_eq!(prior.entity.status, "superseded");
    assert_eq!(prior.details["rationale"], "Initial constraint set.");
    let links = store
        .list_relationships_for_entity(&second.entity.id, PageRequest::default())
        .unwrap();
    assert!(links.items.iter().any(|link| {
        link.relation_type == "supersedes" && link.target_entity_id == first.entity.id
    }));
    assert!(
        store
            .search_research(ResearchSearchQuery {
                text: String::new(),
                entity_type: Some(ResearchEntityKind::Requirement),
                status: None,
                session_id: None,
                page: PageRequest::default(),
            })
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn checkpoint_is_fresh_until_later_research_and_then_resumes_with_delta() {
    let (_directory, store) = enabled_project();
    question(&store, None, "Before checkpoint");
    let checkpoint = store
        .create_research_checkpoint(
            &user_command(),
            ResearchCheckpointInput {
                note: "Bookmark the current investigation.".into(),
                next_actions: vec!["Collect another source.".into()],
            },
        )
        .unwrap();
    let root = store.root().to_path_buf();
    drop(store);
    let store = ContinuityStore::open(root).unwrap();
    let fresh = store.resume_research().unwrap();
    assert_eq!(fresh.checkpoint.unwrap().id, checkpoint.id);
    assert!(!fresh.checkpoint_is_stale);
    assert_eq!(fresh.events_since_checkpoint, 0);
    question(&store, None, "After checkpoint");
    let stale = store.resume_research().unwrap();
    assert!(stale.checkpoint_is_stale);
    assert_eq!(stale.events_since_checkpoint, 1);
}

#[test]
fn search_and_timeline_are_bounded_filterable_and_deterministic() {
    let (_directory, store) = enabled_project();
    let session_id = session(&store);
    let first = question(&store, Some(&session_id), "Needle alpha");
    question(&store, Some(&session_id), "Needle beta");
    question(&store, None, "Unrelated session");
    let page = store
        .search_research(ResearchSearchQuery {
            text: "needle".into(),
            entity_type: Some(ResearchEntityKind::ResearchQuestion),
            status: Some("active".into()),
            session_id: Some(session_id.clone()),
            page: PageRequest {
                limit: 1,
                offset: 0,
            },
        })
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.next_offset, Some(1));
    let timeline = store
        .research_timeline(
            ResearchTimelineFilter {
                session_id: Some(session_id),
                entity_id: None,
                event_type: Some("research.question.created".into()),
            },
            PageRequest::default(),
        )
        .unwrap();
    assert_eq!(timeline.items.len(), 2);
    assert!(
        timeline
            .items
            .windows(2)
            .all(|pair| pair[0].ledger_sequence < pair[1].ledger_sequence)
    );
    assert!(
        timeline
            .items
            .iter()
            .any(|entry| { entry.entity_id.as_deref() == Some(first.as_str()) })
    );
}

#[test]
fn active_question_edit_is_versioned_searchable_and_preserves_stale_write_protection() {
    let (_directory, store) = enabled_project();
    let id = question(&store, None, "Original title");
    let current = store.get_research_item(&id).unwrap();
    let update = ResearchQuestionUpdate {
        title: "Updated recovery question".into(),
        kind: QuestionKind::Hypothesis,
        question: "WAL recovery remains deterministic after abrupt termination.".into(),
        context: "Updated context".into(),
        desired_outcome: "A reproducible result".into(),
        priority: 4,
        due_at: None,
        metadata: json!({"revision_reason":"scope clarified"}),
        expected_version: current.entity.version,
    };
    let updated = store
        .update_research_question(&user_command(), &id, update.clone())
        .unwrap();
    assert_eq!(updated.entity.version, 2);
    assert_eq!(updated.details["kind"], "hypothesis");
    assert!(
        store
            .search_research(ResearchSearchQuery {
                text: "abrupt termination".into(),
                entity_type: Some(ResearchEntityKind::ResearchQuestion),
                status: None,
                session_id: None,
                page: PageRequest::default(),
            })
            .unwrap()
            .items
            .iter()
            .any(|hit| hit.entity_id == id)
    );
    assert!(matches!(
        store.update_research_question(&user_command(), &id, update),
        Err(CoreError::Conflict(_))
    ));
}

#[test]
fn existing_research_item_can_join_an_active_session_idempotently() {
    let (_directory, store) = enabled_project();
    let session_id = session(&store);
    let question_id = question(&store, None, "Attach later");
    let mut command = user_command();
    command.idempotency_key = "associate-session-item".into();
    store
        .associate_research_item_with_session(&command, &session_id, &question_id)
        .unwrap();
    let mut retry = user_command();
    retry.idempotency_key = command.idempotency_key;
    store
        .associate_research_item_with_session(&retry, &session_id, &question_id)
        .unwrap();
    let hits = store
        .search_research(ResearchSearchQuery {
            text: String::new(),
            entity_type: Some(ResearchEntityKind::ResearchQuestion),
            status: None,
            session_id: Some(session_id),
            page: PageRequest::default(),
        })
        .unwrap();
    assert_eq!(hits.items.len(), 1);
    assert_eq!(hits.items[0].entity_id, question_id);
}

#[test]
fn invalid_lifecycle_stale_version_and_unsupported_finding_acceptance_are_atomic() {
    let (_directory, store) = enabled_project();
    let experiment = store
        .create_experiment(
            &user_command(),
            NewExperiment {
                title: "Planned experiment".into(),
                hypothesis: "Hypothesis".into(),
                method: "Method".into(),
                inputs: json!({}),
                expected_observations: "Observation".into(),
                question_id: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    assert!(
        store
            .transition_research_item(
                &user_command(),
                &experiment.entity.id,
                experiment.entity.version,
                "completed",
                None,
            )
            .is_err()
    );
    let running = store
        .transition_research_item(
            &user_command(),
            &experiment.entity.id,
            experiment.entity.version,
            "running",
            None,
        )
        .unwrap();
    assert!(matches!(
        store.transition_research_item(
            &user_command(),
            &experiment.entity.id,
            experiment.entity.version,
            "completed",
            None,
        ),
        Err(CoreError::Conflict(_))
    ));
    assert_eq!(
        store
            .get_research_item(&running.entity.id)
            .unwrap()
            .entity
            .status,
        "running"
    );
    let finding = store
        .create_finding(
            &user_command(),
            NewFinding {
                title: "Unsupported candidate".into(),
                claim: "A claim".into(),
                interpretation: "An interpretation".into(),
                uncertainty: String::new(),
                confidence: None,
                sources: vec![],
                answers_question_id: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    assert!(
        store
            .transition_research_item(&user_command(), &finding.entity.id, 1, "accepted", None)
            .is_err()
    );
    assert_eq!(
        store
            .get_research_item(&finding.entity.id)
            .unwrap()
            .entity
            .version,
        1
    );
}

#[test]
fn research_capability_and_typed_apis_prevent_bypass_or_placeholder_records() {
    let directory = tempfile::tempdir().unwrap();
    let store =
        ContinuityStore::create(directory.path().join("project"), "Capability fixture").unwrap();
    let input = NewResearchQuestion {
        title: "Blocked".into(),
        kind: QuestionKind::Question,
        question: "Should this be written?".into(),
        context: String::new(),
        desired_outcome: String::new(),
        priority: 1,
        due_at: None,
        session_id: None,
        metadata: json!({}),
    };
    assert!(
        store
            .create_research_question(&user_command(), input.clone())
            .is_err()
    );
    assert_eq!(
        store
            .debug_connection()
            .unwrap()
            .query_row("SELECT count(*) FROM entities", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let project_id = store.summary().unwrap().project_id;
    store
        .set_space_capability_with_context(&user_command(), Space::Research, true)
        .unwrap();
    assert_eq!(store.summary().unwrap().project_id, project_id);
    assert!(matches!(
        store.create_entity_with_context(
            &user_command(),
            NewEntity {
                entity_type: "research_question".into(),
                schema_version: 1,
                title: "Bypass".into(),
                origin: OriginKind::User,
                metadata: json!({}),
                data: json!({}),
            },
        ),
        Err(CoreError::Validation(_))
    ));
    let first = store
        .create_research_question(&user_command(), input)
        .unwrap();
    let second = question(&store, None, "Second question");
    assert!(
        store
            .create_relationship_with_context(
                &user_command(),
                NewRelationship {
                    relation_type: "supports".into(),
                    relation_version: 1,
                    source_entity_id: first.entity.id,
                    target_entity_id: second,
                    origin: OriginKind::User,
                    confidence: None,
                    review_state: RelationshipReviewState::Accepted,
                    direct_source_ids: vec![],
                    supersedes_id: None,
                },
            )
            .is_err()
    );
}

#[test]
fn ai_actor_cannot_write_canonical_research_state() {
    let (_directory, store) = enabled_project();
    let mut command = user_command();
    command.actor = ActorRef {
        kind: ActorKind::AiProposal,
        id: "semantic-provider".into(),
    };
    let result = store.create_research_question(
        &command,
        NewResearchQuestion {
            title: "AI candidate".into(),
            kind: QuestionKind::Question,
            question: "Should this become canonical?".into(),
            context: String::new(),
            desired_outcome: String::new(),
            priority: 1,
            due_at: None,
            session_id: None,
            metadata: json!({}),
        },
    );
    assert!(matches!(result, Err(CoreError::Validation(_))));
    let count: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM research_questions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn idempotent_retry_with_new_command_id_creates_one_research_record_and_event() {
    let (_directory, store) = enabled_project();
    let mut first_command = user_command();
    first_command.idempotency_key = "create-question-stable-key".into();
    let input = NewResearchQuestion {
        title: "Idempotent question".into(),
        kind: QuestionKind::Uncertainty,
        question: "Can a retry duplicate this?".into(),
        context: String::new(),
        desired_outcome: "Exactly one record.".into(),
        priority: 2,
        due_at: None,
        session_id: None,
        metadata: json!({}),
    };
    let first = store
        .create_research_question(&first_command, input.clone())
        .unwrap();
    let mut retry = user_command();
    retry.idempotency_key = first_command.idempotency_key;
    let second = store.create_research_question(&retry, input).unwrap();
    assert_eq!(first.entity.id, second.entity.id);
    let connection = store.debug_connection().unwrap();
    let records: i64 = connection
        .query_row("SELECT count(*) FROM research_questions", [], |row| {
            row.get(0)
        })
        .unwrap();
    let events: i64 = connection
        .query_row(
            "SELECT count(*) FROM research_timeline WHERE event_type='research.question.created'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!((records, events), (1, 1));
}

#[test]
fn source_validation_and_closed_session_fail_without_partial_state() {
    let (_directory, store) = enabled_project();
    let before = store.summary().unwrap().ledger_sequence;
    let invalid = store.create_evidence(
        &user_command(),
        NewEvidence {
            title: "Missing source".into(),
            kind: EvidenceKind::Web,
            origin: OriginKind::External,
            source_uri: None,
            source_title: None,
            source_author: None,
            captured_at: None,
            capture_method: "web".into(),
            stable_reference: None,
            source_content: None,
            annotation: String::new(),
            summary: String::new(),
            relevance: String::new(),
            original_artifact_id: None,
            question_id: None,
            session_id: None,
            metadata: json!({}),
        },
    );
    assert!(invalid.is_err());
    assert_eq!(store.summary().unwrap().ledger_sequence, before);

    let session_id = session(&store);
    let current = store.get_research_item(&session_id).unwrap();
    store
        .transition_research_item(
            &user_command(),
            &session_id,
            current.entity.version,
            "completed",
            Some("Session complete."),
        )
        .unwrap();
    let count_before: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM research_questions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            store.create_research_question(
                &user_command(),
                NewResearchQuestion {
                    title: "Cannot attach".into(),
                    kind: QuestionKind::Question,
                    question: "Closed session?".into(),
                    context: String::new(),
                    desired_outcome: String::new(),
                    priority: 1,
                    due_at: None,
                    session_id: Some(session_id),
                    metadata: json!({}),
                },
            )
        }))
        .unwrap()
        .is_err()
    );
    let count_after: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM research_questions", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count_before, count_after);
}

#[test]
fn export_restore_preserves_research_identity_provenance_and_standalone_mode() {
    let (directory, store) = enabled_project();
    let evidence_id = note_evidence(&store, None, None, "Exported evidence");
    let export = directory.path().join("export");
    store.export_project(&export).unwrap();
    let restored_root = directory.path().join("restored");
    let restored = ContinuityStore::import_export(&export, &restored_root).unwrap();
    let evidence = restored.get_research_item(&evidence_id).unwrap();
    assert_eq!(evidence.entity.origin, "user");
    assert_eq!(
        evidence.details["source_content"],
        "SQLite recovery succeeded after process termination."
    );
    assert!(restored.capability_enabled(Space::Research).unwrap());
    assert!(!restored.capability_enabled(Space::Development).unwrap());
    assert!(restored.verify_integrity().unwrap().is_healthy());
}

#[test]
fn integrity_scan_detects_missing_normalized_research_state_with_recovery_guidance() {
    let (_directory, store) = enabled_project();
    let question_id = question(&store, None, "Corruption fixture");
    store
        .debug_connection()
        .unwrap()
        .execute(
            "DELETE FROM research_questions WHERE entity_id=?1",
            [&question_id],
        )
        .unwrap();
    let report = store.verify_integrity().unwrap();
    assert!(report.issues.iter().any(|issue| {
        issue.code == "missing_research_detail" && issue.path_or_id == question_id
    }));
}
