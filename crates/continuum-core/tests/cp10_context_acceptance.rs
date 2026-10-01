use std::fs;
use std::path::Path;
use std::process::Command;

use continuum_core::{
    ActorRef, CORE_SCHEMA_VERSION, CheckpointScope, CheckpointTrigger, CommandContext,
    ContextAudience, ContextBudget, ContextPackRequest, ContinuityStore, CoreError,
    CurrentProjectStateRequest, DataClassification, EvidenceAnnotationUpdate, FreshnessRequirement,
    NewEvidence, NewResearchSession, OriginKind, PageRequest, ResearchCheckpointInput,
    RetrievalProfile, SemanticCheckpointInput, Space,
};
use continuum_core::{EvidenceKind, NewEntity};
use serde_json::json;

fn user() -> CommandContext {
    CommandContext::new(ActorRef::user("cp10-user"))
}

fn project(research: bool, development: bool) -> (tempfile::TempDir, ContinuityStore) {
    let directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP10 context fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    if research {
        store
            .set_space_capability_with_context(&user(), Space::Research, true)
            .unwrap();
    }
    if development {
        store
            .set_space_capability_with_context(&user(), Space::Development, true)
            .unwrap();
    }
    (directory, store)
}

fn research_session(store: &ContinuityStore, title: &str) -> String {
    store
        .create_research_session(
            &user(),
            NewResearchSession {
                title: title.into(),
                objective: "Resume the exact research state without reconstructing it from chat."
                    .into(),
                started_at: None,
                metadata: json!({"fixture":"cp10"}),
            },
        )
        .unwrap()
        .entity
        .id
}

fn evidence(store: &ContinuityStore, session_id: &str, title: &str, content: &str) -> String {
    store
        .create_evidence(
            &user(),
            NewEvidence {
                title: title.into(),
                kind: EvidenceKind::Note,
                origin: OriginKind::User,
                source_uri: None,
                source_title: Some("CP10 fixture".into()),
                source_author: Some("cp10-user".into()),
                captured_at: None,
                capture_method: "manual_note".into(),
                stable_reference: None,
                source_content: Some(content.into()),
                annotation: "Checkpoint source".into(),
                summary: content.into(),
                relevance: "Context retrieval".into(),
                original_artifact_id: None,
                question_id: None,
                session_id: Some(session_id.into()),
                metadata: json!({}),
            },
        )
        .unwrap()
        .entity
        .id
}

fn semantic_checkpoint(
    store: &ContinuityStore,
    scope: CheckpointScope,
    note: &str,
) -> continuum_core::CheckpointEnvelope {
    store
        .create_semantic_checkpoint(
            &user(),
            SemanticCheckpointInput {
                scope,
                trigger: CheckpointTrigger::Interruption,
                note: note.into(),
                blockers: vec!["Awaiting one deterministic validation".into()],
                risks: vec!["Do not lose the source boundary".into()],
                next_actions: vec!["Continue from the saved source boundary".into()],
                semantic_candidate_id: None,
                supersedes_checkpoint_id: None,
            },
        )
        .unwrap()
}

fn context_request(
    scope: CheckpointScope,
    checkpoint_id: Option<String>,
    audience: ContextAudience,
) -> ContextPackRequest {
    ContextPackRequest {
        task: "Resume continuity checkpoint and inspect current evidence".into(),
        audience,
        consumer_target: "cp10-acceptance".into(),
        scope,
        checkpoint_id,
        freshness_requirement: FreshnessRequirement::AllowStaleWithWarning,
        retrieval_profile: match scope {
            CheckpointScope::Research => RetrievalProfile::Research,
            CheckpointScope::Development => RetrievalProfile::Development,
            CheckpointScope::Integrated => RetrievalProfile::Integrated,
            CheckpointScope::Core => RetrievalProfile::Resume,
        },
        root_entity_ids: Vec::new(),
        exclude_source_ids: Vec::new(),
        include_artifact_content: false,
        budget: ContextBudget::default(),
    }
}

fn run_git(root: &Path, args: &[&str]) -> String {
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
    String::from_utf8(output.stdout).unwrap().trim().into()
}

#[test]
fn independent_spaces_and_integrated_scope_obey_the_cp1_modularity_contract() {
    let (_research_dir, research) = project(true, false);
    research_session(&research, "Research-only session");
    let research_checkpoint = semantic_checkpoint(
        &research,
        CheckpointScope::Research,
        "Pause research without a repository.",
    );
    assert_eq!(research_checkpoint.checkpoint.scope, "research");
    assert!(
        research_checkpoint.repository_snapshot["repositories"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(matches!(
        research.create_semantic_checkpoint(
            &user(),
            SemanticCheckpointInput {
                scope: CheckpointScope::Integrated,
                trigger: CheckpointTrigger::Manual,
                note: String::new(),
                blockers: Vec::new(),
                risks: Vec::new(),
                next_actions: Vec::new(),
                semantic_candidate_id: None,
                supersedes_checkpoint_id: None,
            }
        ),
        Err(CoreError::Validation(_))
    ));

    let (_development_dir, development) = project(false, true);
    let development_checkpoint = semantic_checkpoint(
        &development,
        CheckpointScope::Development,
        "Pause development without research.",
    );
    assert_eq!(development_checkpoint.checkpoint.scope, "development");

    let (_integrated_dir, integrated) = project(true, true);
    research_session(&integrated, "Integrated session");
    let integrated_checkpoint = semantic_checkpoint(
        &integrated,
        CheckpointScope::Integrated,
        "Connected R&D bookmark.",
    );
    assert_eq!(integrated_checkpoint.checkpoint.scope, "integrated");
    assert!(integrated.verify_integrity().unwrap().is_healthy());
}

#[test]
fn current_state_explains_then_since_now_next_and_checkpoint_drift() {
    let (_directory, store) = project(true, false);
    let session_id = research_session(&store, "Continuity research");
    let first_source = evidence(
        &store,
        &session_id,
        "First observation",
        "Recovery succeeds.",
    );
    let first = semantic_checkpoint(&store, CheckpointScope::Research, "First bookmark");

    let initial = store.checkpoint_freshness(&first.checkpoint.id).unwrap();
    assert!(initial.fresh);
    assert!(initial.reasons.is_empty());
    let initial_state = store
        .current_project_state(CurrentProjectStateRequest {
            scope: CheckpointScope::Research,
            checkpoint_id: Some(first.checkpoint.id.clone()),
        })
        .unwrap();
    assert!(initial_state.since.is_empty());

    let entity = store.get_entity(&first_source).unwrap();
    store
        .update_evidence_annotation(
            &user(),
            &first_source,
            EvidenceAnnotationUpdate {
                annotation: "Reviewed after the checkpoint".into(),
                summary: "A newer observation changes the resume state.".into(),
                relevance: "High".into(),
                expected_version: entity.version,
            },
        )
        .unwrap();
    let second_source = evidence(
        &store,
        &session_id,
        "Second observation",
        "A new source was added after the bookmark.",
    );

    let stale = store.checkpoint_freshness(&first.checkpoint.id).unwrap();
    assert!(!stale.fresh);
    assert!(stale.changed_source_ids.contains(&first_source));
    assert!(stale.events_since_checkpoint > 0);

    let state = store
        .current_project_state(CurrentProjectStateRequest {
            scope: CheckpointScope::Research,
            checkpoint_id: Some(first.checkpoint.id.clone()),
        })
        .unwrap();
    assert!(state.then.is_object());
    assert!(!state.since.is_empty());
    assert!(state.now.is_object());
    assert_eq!(
        state.next_actions,
        vec!["Continue from the saved source boundary"]
    );

    let second = semantic_checkpoint(&store, CheckpointScope::Research, "Second bookmark");
    let comparison = store
        .compare_checkpoints(&first.checkpoint.id, &second.checkpoint.id)
        .unwrap();
    assert!(comparison.changed_source_ids.contains(&first_source));
    assert!(comparison.added_source_ids.contains(&second_source));
}

#[test]
fn context_pack_is_deterministic_bounded_previewable_and_atomically_saved() {
    let (_directory, store) = project(true, false);
    let session_id = research_session(&store, "Pack research");
    let source = evidence(
        &store,
        &session_id,
        "Public resume evidence",
        "A concise source for deterministic retrieval.",
    );
    store
        .set_entity_ai_classification(
            &user(),
            &source,
            DataClassification::Public,
            "Safe fixture content",
        )
        .unwrap();
    let checkpoint = semantic_checkpoint(&store, CheckpointScope::Research, "Pack boundary");
    let request = context_request(
        CheckpointScope::Research,
        Some(checkpoint.checkpoint.id.clone()),
        ContextAudience::LocalUser,
    );
    let first = store.build_context_pack(request.clone()).unwrap();
    let repeated = store.build_context_pack(request).unwrap();
    assert_eq!(first.request_fingerprint, repeated.request_fingerprint);
    assert_eq!(first.content_fingerprint, repeated.content_fingerprint);
    assert_eq!(
        first
            .items
            .iter()
            .map(|item| (&item.source_kind, &item.source_id))
            .collect::<Vec<_>>(),
        repeated
            .items
            .iter()
            .map(|item| (&item.source_kind, &item.source_id))
            .collect::<Vec<_>>()
    );
    assert!(first.included_bytes <= first.budget.max_bytes);
    assert!(first.estimated_tokens <= first.budget.hard_tokens);
    assert!(first.items.len() <= first.budget.max_items as usize);

    let save_command = user();
    let saved = store.save_context_pack(&save_command, &first).unwrap();
    let retry = store.save_context_pack(&save_command, &first).unwrap();
    assert_eq!(saved.pack.id, retry.pack.id);
    assert_eq!(saved.generated_artifact_id, retry.generated_artifact_id);
    assert_eq!(store.get_saved_context_pack(&saved.pack.id).unwrap(), saved);
    assert_eq!(
        store
            .list_saved_context_packs(PageRequest::default())
            .unwrap()
            .items
            .len(),
        1
    );

    let mut stale_preview = store
        .build_context_pack(context_request(
            CheckpointScope::Research,
            Some(checkpoint.checkpoint.id),
            ContextAudience::LocalUser,
        ))
        .unwrap();
    evidence(
        &store,
        &session_id,
        "State changed after preview",
        "The preview must now be rebuilt.",
    );
    assert!(matches!(
        store.save_context_pack(&user(), &stale_preview),
        Err(CoreError::Conflict(_))
    ));

    stale_preview.budget.max_items = 0;
    assert!(matches!(
        store.save_context_pack(&user(), &stale_preview),
        Err(CoreError::Validation(_))
    ));
}

#[test]
fn privacy_policy_fails_closed_for_external_context_and_explicit_roots() {
    let (_directory, store) = project(true, false);
    let session_id = research_session(&store, "Privacy research");
    let public = evidence(
        &store,
        &session_id,
        "Public finding",
        "This source can be shared publicly.",
    );
    let secret = evidence(
        &store,
        &session_id,
        "Private credential observation",
        "password=must-never-leave-the-device",
    );
    store
        .set_entity_ai_classification(
            &user(),
            &public,
            DataClassification::Public,
            "Public fixture",
        )
        .unwrap();
    store
        .set_entity_ai_classification(
            &user(),
            &secret,
            DataClassification::NeverSend,
            "Contains local-only material",
        )
        .unwrap();
    let checkpoint = semantic_checkpoint(&store, CheckpointScope::Research, "Privacy boundary");

    let local = store
        .build_context_pack(context_request(
            CheckpointScope::Research,
            Some(checkpoint.checkpoint.id.clone()),
            ContextAudience::LocalUser,
        ))
        .unwrap();
    assert!(local.items.iter().any(|item| item.source_id == secret));

    let portable = store
        .build_context_pack(context_request(
            CheckpointScope::Research,
            Some(checkpoint.checkpoint.id),
            ContextAudience::PublicPortable,
        ))
        .unwrap();
    assert!(portable.items.iter().any(|item| item.source_id == public));
    assert!(!portable.items.iter().any(|item| item.source_id == secret));
    assert!(portable.omissions.iter().any(|item| {
        item.source_id.as_deref() == Some(secret.as_str()) && item.reason == "privacy_policy"
    }));

    let mut denied_root =
        context_request(CheckpointScope::Research, None, ContextAudience::ExternalAi);
    denied_root.root_entity_ids.push(secret);
    assert!(matches!(
        store.build_context_pack(denied_root),
        Err(CoreError::Validation(_))
    ));

    let (_private_directory, private_store) = project(true, false);
    let private_session = research_session(&private_store, "Private-only research");
    let private_source = evidence(
        &private_store,
        &private_session,
        "Private-only source",
        "This remains project-private.",
    );
    let private_pack = private_store
        .build_context_pack(context_request(
            CheckpointScope::Research,
            None,
            ContextAudience::PublicPortable,
        ))
        .unwrap();
    assert_eq!(private_pack.items.len(), 1);
    assert_eq!(private_pack.items[0].source_kind, "project_state");
    assert_eq!(
        private_pack.items[0].content["next_actions_withheld"],
        json!(true)
    );
    assert!(private_pack.omissions.iter().any(|item| {
        item.source_id.as_deref() == Some(private_source.as_str())
            && item.reason == "privacy_policy"
    }));
}

#[test]
fn current_freshness_requirement_blocks_stale_context_until_user_allows_warning() {
    let (_directory, store) = project(true, false);
    let session_id = research_session(&store, "Freshness research");
    evidence(&store, &session_id, "Initial source", "Initial state");
    let checkpoint = semantic_checkpoint(&store, CheckpointScope::Research, "Fresh boundary");
    evidence(&store, &session_id, "Later source", "Later state");

    let mut request = context_request(
        CheckpointScope::Research,
        Some(checkpoint.checkpoint.id),
        ContextAudience::LocalUser,
    );
    request.freshness_requirement = FreshnessRequirement::Current;
    assert!(matches!(
        store.build_context_pack(request.clone()),
        Err(CoreError::Conflict(_))
    ));
    request.freshness_requirement = FreshnessRequirement::AllowStaleWithWarning;
    let pack = store.build_context_pack(request).unwrap();
    assert_eq!(
        pack.freshness.as_ref().map(|value| value.fresh),
        Some(false)
    );
}

#[test]
fn legacy_checkpoint_remains_readable_and_usable_after_cp10() {
    let (_directory, store) = project(true, false);
    let session_id = research_session(&store, "Legacy research");
    let source = evidence(&store, &session_id, "Legacy source", "Legacy content");
    let legacy = store
        .create_checkpoint_with_context(
            &user(),
            CheckpointScope::Research,
            &json!({"note":"Created through the stable CP2 contract"}),
            &[source],
        )
        .unwrap();
    let envelope = store.get_checkpoint_envelope(&legacy.id).unwrap();
    assert_eq!(envelope.privacy_policy_version, "legacy-unspecified");
    let pack = store
        .build_context_pack(context_request(
            CheckpointScope::Research,
            Some(legacy.id),
            ContextAudience::LocalUser,
        ))
        .unwrap();
    assert!(!pack.items.is_empty());
}

#[test]
fn repository_divergence_is_visible_without_mutating_the_repository() {
    let repository_directory = tempfile::tempdir().unwrap();
    let repository_root = repository_directory.path().join("repo");
    fs::create_dir_all(&repository_root).unwrap();
    run_git(&repository_root, &["init", "-b", "main"]);
    run_git(&repository_root, &["config", "user.name", "Continuum Test"]);
    run_git(
        &repository_root,
        &["config", "user.email", "continuum@example.invalid"],
    );
    run_git(&repository_root, &["config", "commit.gpgsign", "false"]);
    fs::write(repository_root.join("README.md"), "# Initial\n").unwrap();
    run_git(&repository_root, &["add", "--", "README.md"]);
    run_git(&repository_root, &["commit", "-m", "initial"]);

    let (_directory, store) = project(false, true);
    store
        .attach_repository(
            &user(),
            continuum_core::RepositoryAttachInput {
                path: repository_root.clone(),
                title: Some("CP10 repository".into()),
                metadata: json!({}),
            },
        )
        .unwrap();
    let checkpoint =
        semantic_checkpoint(&store, CheckpointScope::Development, "Repository boundary");
    assert!(
        store
            .checkpoint_freshness(&checkpoint.checkpoint.id)
            .unwrap()
            .fresh
    );

    fs::write(
        repository_root.join("README.md"),
        "# Working tree changed\n",
    )
    .unwrap();
    let freshness = store
        .checkpoint_freshness(&checkpoint.checkpoint.id)
        .unwrap();
    assert!(!freshness.fresh);
    assert!(!freshness.repository_diverged_ids.is_empty());
}

#[test]
fn invalid_budgets_and_ai_authored_canonical_checkpoints_are_rejected() {
    let (_directory, store) = project(true, false);
    research_session(&store, "Validation research");
    let mut invalid = context_request(CheckpointScope::Research, None, ContextAudience::LocalUser);
    invalid.budget.max_bytes = 512;
    assert!(matches!(
        store.build_context_pack(invalid),
        Err(CoreError::Validation(_))
    ));

    let mut ai_command = user();
    ai_command.actor.kind = continuum_core::ActorKind::AiProposal;
    assert!(matches!(
        store.create_semantic_checkpoint(
            &ai_command,
            SemanticCheckpointInput {
                scope: CheckpointScope::Research,
                trigger: CheckpointTrigger::Manual,
                note: "AI must not commit canonical truth".into(),
                blockers: Vec::new(),
                risks: Vec::new(),
                next_actions: Vec::new(),
                semantic_candidate_id: None,
                supersedes_checkpoint_id: None,
            }
        ),
        Err(CoreError::Validation(_))
    ));
}

#[test]
fn schema_v10_upgrades_atomically_to_cp10_without_rewriting_legacy_data() {
    let (directory, store) = project(true, false);
    let session_id = research_session(&store, "Migration research");
    let source = evidence(
        &store,
        &session_id,
        "Migration source",
        "Preserve this source",
    );
    let legacy = store
        .create_research_checkpoint(
            &user(),
            ResearchCheckpointInput {
                note: "Pre-CP10 checkpoint".into(),
                next_actions: vec!["Upgrade schema".into()],
            },
        )
        .unwrap();
    let project_root = store.root().to_path_buf();
    drop(store);

    let database = project_root.join("ledger.sqlite3");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "DROP TRIGGER context_pack_generated_artifacts_immutable_delete;
             DROP TRIGGER context_pack_generated_artifacts_immutable_update;
             DROP TRIGGER context_pack_sources_immutable_delete;
             DROP TRIGGER context_pack_sources_immutable_update;
             DROP TRIGGER context_pack_records_immutable_delete;
             DROP TRIGGER context_pack_records_immutable_update;
             DROP TRIGGER context_packs_immutable_delete;
             DROP TRIGGER context_packs_immutable_update;
             DROP TRIGGER checkpoint_artifact_sources_immutable_delete;
             DROP TRIGGER checkpoint_artifact_sources_immutable_update;
             DROP TRIGGER checkpoint_envelopes_immutable_delete;
             DROP TRIGGER checkpoint_envelopes_immutable_update;
             DROP TRIGGER checkpoint_sources_immutable_delete;
             DROP TRIGGER checkpoint_sources_immutable_update;
             DROP TABLE context_pack_generated_artifacts;
             DROP TABLE context_pack_sources;
             DROP TABLE context_pack_records;
             DROP TABLE checkpoint_artifact_sources;
             DROP TABLE checkpoint_envelopes;
             DELETE FROM schema_migrations WHERE version=11;",
        )
        .unwrap();
    drop(connection);

    let reopened = ContinuityStore::open(&project_root).unwrap();
    assert_eq!(reopened.get_checkpoint(&legacy.id).unwrap(), legacy);
    assert_eq!(
        reopened.get_entity(&source).unwrap().title,
        "Migration source"
    );
    let version: i64 = reopened
        .debug_connection()
        .unwrap()
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(version >= 11, "CP10 migration must remain present");
    assert_eq!(version, i64::from(CORE_SCHEMA_VERSION));
    assert!(reopened.verify_integrity().unwrap().is_healthy());
    drop(directory);
}

#[test]
fn ordinary_core_entities_remain_available_to_core_resume_packs() {
    let (_directory, store) = project(false, false);
    let entity_id = store
        .create_entity_with_context(
            &user(),
            NewEntity {
                entity_type: "core.note".into(),
                schema_version: 1,
                title: "Cross-space project note".into(),
                origin: OriginKind::User,
                metadata: json!({}),
                data: json!({"text":"Remember the global project constraint"}),
            },
        )
        .unwrap();
    let checkpoint = semantic_checkpoint(&store, CheckpointScope::Core, "Core bookmark");
    let pack = store
        .build_context_pack(context_request(
            CheckpointScope::Core,
            Some(checkpoint.checkpoint.id),
            ContextAudience::LocalUser,
        ))
        .unwrap();
    assert!(pack.items.iter().any(|item| item.source_id == entity_id));
}

#[test]
fn integrity_scan_detects_incomplete_saved_context_publication() {
    let (_directory, store) = project(true, false);
    let session_id = research_session(&store, "Integrity research");
    evidence(
        &store,
        &session_id,
        "Integrity source",
        "Immutable source snapshot",
    );
    let checkpoint = semantic_checkpoint(&store, CheckpointScope::Research, "Integrity boundary");
    let pack = store
        .build_context_pack(context_request(
            CheckpointScope::Research,
            Some(checkpoint.checkpoint.id),
            ContextAudience::LocalUser,
        ))
        .unwrap();
    let saved = store.save_context_pack(&user(), &pack).unwrap();
    assert!(store.verify_integrity().unwrap().is_healthy());

    let connection = store.debug_connection().unwrap();
    connection
        .execute_batch("DROP TRIGGER context_pack_sources_immutable_delete;")
        .unwrap();
    connection
        .execute(
            "DELETE FROM context_pack_sources WHERE context_pack_id=?1 AND ordinal=(SELECT MIN(ordinal) FROM context_pack_sources WHERE context_pack_id=?1)",
            [&saved.pack.id],
        )
        .unwrap();
    drop(connection);
    let report = store.verify_integrity().unwrap();
    assert!(report.issues.iter().any(|issue| {
        issue.code == "context_pack_sources_invalid" && issue.path_or_id == saved.pack.id
    }));
}
