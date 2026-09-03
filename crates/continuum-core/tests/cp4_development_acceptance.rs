use std::fs;
use std::path::Path;
use std::process::Command;

use continuum_core::{
    ActorKind, ActorRef, BaselineRelation, ChangeSetKind, CommandContext, ContinuityStore,
    CoreError, DevelopmentCheckpointInput, DevelopmentIntentOrigin, DevelopmentSearchQuery,
    DevelopmentTimelineFilter, NewChangeSet, NewDecision, NewDevelopmentRequirement, NewEntity,
    NewRequirement, OriginKind, PageRequest, RepositoryAttachInput, RequirementImplementation,
    RequirementLinkInput, RequirementRationaleOrigin, Space,
};
use serde_json::json;
use tempfile::TempDir;

fn user_command() -> CommandContext {
    CommandContext::new(ActorRef::user("developer-1"))
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
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn init_repository(directory: &TempDir) -> std::path::PathBuf {
    let root = directory.path().join("repository");
    fs::create_dir_all(&root).unwrap();
    run_git(&root, &["init", "-b", "main"]);
    run_git(&root, &["config", "user.name", "Continuum Test"]);
    run_git(
        &root,
        &["config", "user.email", "continuum@example.invalid"],
    );
    run_git(&root, &["config", "commit.gpgsign", "false"]);
    fs::write(root.join("README.md"), "# Fixture\n").unwrap();
    run_git(&root, &["add", "--", "README.md"]);
    run_git(&root, &["commit", "-m", "Initial fixture"]);
    root
}

fn development_project(directory: &TempDir) -> ContinuityStore {
    let store = ContinuityStore::create_with_actor(
        directory.path().join("continuum-project"),
        "CP4 fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    store
        .set_space_capability_with_context(&user_command(), Space::Development, true)
        .unwrap();
    store
}

fn attach_and_observe(
    store: &ContinuityStore,
    root: &Path,
) -> (
    continuum_core::RepositoryRecord,
    continuum_core::RepositoryBaselineRecord,
) {
    let repository = store
        .attach_repository(
            &user_command(),
            RepositoryAttachInput {
                path: root.to_path_buf(),
                title: Some("Fixture repository".into()),
                metadata: json!({}),
            },
        )
        .unwrap();
    let baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    (repository, baseline)
}

#[test]
fn development_only_cycle_is_reproducible_resumable_and_has_honest_origin() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_and_observe(&store, &root);
    assert_eq!(
        baseline.relation_to_previous,
        BaselineRelation::Initial.as_str()
    );

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
    assert_eq!(ingestion.reachable_commits, 1);
    assert_eq!(ingestion.ingested_commit_ids.len(), 1);

    let requirement = store
        .create_development_requirement(
            &user_command(),
            NewDevelopmentRequirement {
                title: "Implement imported brief".into(),
                statement: "The repository must retain an auditable implementation record.".into(),
                acceptance_criteria: vec![
                    "A committed ChangeSet links to this Requirement.".into(),
                ],
                priority: 3,
                rationale_origin: RequirementRationaleOrigin::External,
                verification_method: "Inspect deterministic provenance".into(),
                metadata: json!({"external_reference":"brief-42"}),
            },
        )
        .unwrap();
    assert_eq!(requirement.origin, "external");
    let requirement = store
        .transition_development_requirement(
            &user_command(),
            &requirement.id,
            requirement.version,
            "accepted",
        )
        .unwrap();
    let requirement = store
        .transition_development_requirement(
            &user_command(),
            &requirement.id,
            requirement.version,
            "in_progress",
        )
        .unwrap();

    let change_set = store
        .create_change_set(
            &user_command(),
            NewChangeSet {
                title: "Initial implementation".into(),
                summary: "Records the existing implementation without inventing research.".into(),
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                kind: ChangeSetKind::Committed,
                commit_entity_ids: ingestion.ingested_commit_ids.clone(),
                requirement_links: vec![RequirementLinkInput {
                    requirement_id: requirement.id.clone(),
                    relationship: RequirementImplementation::Implements,
                }],
                intent_origin: DevelopmentIntentOrigin::External,
                supersedes_change_set_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    assert_eq!(change_set.entity.status, "observed");
    assert_eq!(change_set.file_changes.len(), 1);
    let links = store
        .list_relationships_for_entity(
            &change_set.entity.id,
            continuum_core::PageRequest::default(),
        )
        .unwrap();
    assert!(links.items.iter().any(|link| {
        link.relation_type == "implements" && link.target_entity_id == requirement.id
    }));
    let search = store
        .search_development(DevelopmentSearchQuery {
            text: "existing implementation".into(),
            entity_type: Some("change_set".into()),
            status: Some("observed".into()),
            page: PageRequest::default(),
        })
        .unwrap();
    assert_eq!(search.items.len(), 1);
    assert_eq!(search.items[0].entity_id, change_set.entity.id);
    let timeline = store
        .development_timeline(
            DevelopmentTimelineFilter {
                repository_id: Some(repository.entity.id.clone()),
                entity_id: None,
                event_type: None,
            },
            PageRequest::default(),
        )
        .unwrap();
    assert!(
        timeline
            .items
            .windows(2)
            .all(|pair| pair[0].ledger_sequence < pair[1].ledger_sequence)
    );

    let checkpoint = store
        .create_development_checkpoint(
            &user_command(),
            DevelopmentCheckpointInput {
                note: "Pause after documenting the current implementation.".into(),
                blockers: vec!["Verification is not complete.".into()],
                next_actions: vec!["Run the planned verification.".into()],
            },
        )
        .unwrap();
    assert_eq!(checkpoint.scope, "development");
    assert_eq!(checkpoint.summary["research_required"], json!(false));
    let resume = store.resume_development().unwrap();
    assert!(!resume.checkpoint_is_stale);
    assert!(!resume.repository_diverged);
    assert_eq!(resume.next_actions, vec!["Run the planned verification."]);
    assert!(!store.capability_enabled(Space::Research).unwrap());

    let report = store.generate_development_report().unwrap();
    assert!(report.markdown.contains(&change_set.entity.id));
    assert!(report.markdown.contains("rationale `external`"));
    let research_report = store.generate_research_report().unwrap();
    assert!(!research_report.markdown.contains(&requirement.id));
    assert!(store.verify_integrity().unwrap().is_healthy());

    let export = project_directory.path().join("cp4-export");
    store.export_project(&export).unwrap();
    let restored_root = project_directory.path().join("cp4-restored");
    let restored = ContinuityStore::import_export(&export, &restored_root).unwrap();
    assert_eq!(
        restored.get_change_set(&change_set.entity.id).unwrap(),
        change_set
    );
    assert!(restored.verify_integrity().unwrap().is_healthy());
}

#[test]
fn unchanged_baseline_and_commit_reingestion_are_idempotent() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_and_observe(&store, &root);
    let first = store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    let repeated_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    assert_eq!(repeated_baseline.entity.id, baseline.entity.id);
    let repeated = store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    assert_eq!(first.ingested_commit_ids.len(), 1);
    assert!(repeated.ingested_commit_ids.is_empty());
    assert_eq!(repeated.previously_known_commits, 1);
    let count: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM git_commit_observations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 1);
}

#[test]
fn working_tree_draft_can_be_explicitly_superseded_by_reachable_commits() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, clean_baseline) = attach_and_observe(&store, &root);
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: clean_baseline.entity.id,
                max_commits: 100,
            },
        )
        .unwrap();

    fs::write(root.join("README.md"), "# Fixture\n\nDraft change.\n").unwrap();
    let dirty_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    assert_eq!(
        dirty_baseline.relation_to_previous,
        BaselineRelation::WorktreeChanged.as_str()
    );
    fs::write(
        root.join("README.md"),
        "# Fixture\n\nA different draft with the same Git status.\n",
    )
    .unwrap();
    let current_dirty_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    assert_eq!(
        current_dirty_baseline.relation_to_previous,
        BaselineRelation::WorktreeChanged.as_str()
    );
    assert_ne!(
        current_dirty_baseline.worktree_fingerprint,
        dirty_baseline.worktree_fingerprint
    );
    let draft = store
        .create_change_set(
            &user_command(),
            NewChangeSet {
                title: "Draft README change".into(),
                summary: "Uncommitted documentation update.".into(),
                repository_id: repository.entity.id.clone(),
                baseline_id: current_dirty_baseline.entity.id.clone(),
                kind: ChangeSetKind::WorkingTree,
                commit_entity_ids: vec![],
                requirement_links: vec![],
                intent_origin: DevelopmentIntentOrigin::User,
                supersedes_change_set_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    assert_eq!(
        draft.worktree_fingerprint,
        Some(current_dirty_baseline.worktree_fingerprint)
    );
    assert_eq!(draft.file_changes[0].new_path, "README.md");

    run_git(&root, &["add", "--", "README.md"]);
    run_git(&root, &["commit", "-m", "Document draft change"]);
    let committed_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    assert_eq!(
        committed_baseline.relation_to_previous,
        BaselineRelation::FastForward.as_str()
    );
    let ingestion = store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: committed_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    assert_eq!(ingestion.ingested_commit_ids.len(), 1);
    let committed = store
        .create_change_set(
            &user_command(),
            NewChangeSet {
                title: "Committed README change".into(),
                summary: "Explicitly realizes the earlier working-tree draft.".into(),
                repository_id: repository.entity.id,
                baseline_id: committed_baseline.entity.id,
                kind: ChangeSetKind::Committed,
                commit_entity_ids: ingestion.ingested_commit_ids,
                requirement_links: vec![],
                intent_origin: DevelopmentIntentOrigin::User,
                supersedes_change_set_id: Some(draft.entity.id.clone()),
                metadata: json!({}),
            },
        )
        .unwrap();
    assert_eq!(
        committed.supersedes_change_set_id,
        Some(draft.entity.id.clone())
    );
    assert_eq!(
        store.get_entity(&draft.entity.id).unwrap().status,
        "superseded"
    );
}

#[test]
fn rename_branch_switch_and_amend_preserve_distinct_observations() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, initial) = attach_and_observe(&store, &root);
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: initial.entity.id,
                max_commits: 100,
            },
        )
        .unwrap();

    run_git(&root, &["mv", "README.md", "OVERVIEW.md"]);
    run_git(&root, &["commit", "-m", "Rename project overview"]);
    let renamed_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    let renamed = store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: renamed_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    let commit = store
        .get_commit_observation(&renamed.ingested_commit_ids[0])
        .unwrap();
    assert!(commit.file_changes.iter().any(|change| {
        change.change_kind == "renamed"
            && change.old_path.as_deref() == Some("README.md")
            && change.new_path == "OVERVIEW.md"
    }));

    run_git(&root, &["checkout", "-b", "experiment"]);
    let branch = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    assert_eq!(
        branch.relation_to_previous,
        BaselineRelation::BranchSwitch.as_str()
    );
    fs::write(root.join("OVERVIEW.md"), "# Amended fixture\n").unwrap();
    run_git(&root, &["add", "--", "OVERVIEW.md"]);
    run_git(&root, &["commit", "--amend", "-m", "Amended overview"]);
    let amended = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    assert_eq!(
        amended.relation_to_previous,
        BaselineRelation::HistoryRewrite.as_str()
    );
    assert_ne!(amended.head_oid, branch.head_oid);
    assert_eq!(amended.previous_baseline_id, Some(branch.entity.id));
}

#[test]
fn unsafe_or_invalid_development_writes_fail_closed_and_atomically() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store =
        ContinuityStore::create(project_directory.path().join("project"), "Guard fixture").unwrap();
    let disabled = store.attach_repository(
        &user_command(),
        RepositoryAttachInput {
            path: root.clone(),
            title: None,
            metadata: json!({}),
        },
    );
    assert!(matches!(disabled, Err(CoreError::Validation(_))));
    store
        .set_space_capability_with_context(&user_command(), Space::Development, true)
        .unwrap();
    let subdirectory = root.join("nested");
    fs::create_dir_all(&subdirectory).unwrap();
    let nested = store.attach_repository(
        &user_command(),
        RepositoryAttachInput {
            path: subdirectory,
            title: None,
            metadata: json!({}),
        },
    );
    assert!(matches!(nested, Err(CoreError::Validation(_))));
    let ai_command = CommandContext::new(continuum_core::ActorRef {
        kind: ActorKind::AiProposal,
        id: "model-candidate".into(),
    });
    let ai = store.attach_repository(
        &ai_command,
        RepositoryAttachInput {
            path: root.clone(),
            title: None,
            metadata: json!({}),
        },
    );
    assert!(matches!(ai, Err(CoreError::Validation(_))));
    let generic = store.create_entity_with_context(
        &user_command(),
        NewEntity {
            entity_type: "change_set".into(),
            schema_version: 1,
            title: "Bypass".into(),
            origin: OriginKind::User,
            metadata: json!({}),
            data: json!({}),
        },
    );
    assert!(matches!(generic, Err(CoreError::Validation(_))));

    let (repository, baseline) = attach_and_observe(&store, &root);
    let update = store.debug_connection().unwrap().execute(
        "UPDATE repository_baselines SET head_oid='bad' WHERE entity_id=?1",
        [&baseline.entity.id],
    );
    assert!(update.is_err());
    let empty = store.create_change_set(
        &user_command(),
        NewChangeSet {
            title: "Empty draft".into(),
            summary: "Must fail atomically.".into(),
            repository_id: repository.entity.id,
            baseline_id: baseline.entity.id,
            kind: ChangeSetKind::WorkingTree,
            commit_entity_ids: vec![],
            requirement_links: vec![],
            intent_origin: DevelopmentIntentOrigin::Unknown,
            supersedes_change_set_id: None,
            metadata: json!({}),
        },
    );
    assert!(matches!(empty, Err(CoreError::Validation(_))));
    let count: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM change_sets", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn checkpoint_detects_unobserved_repository_divergence_without_losing_bookmark() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (_repository, _baseline) = attach_and_observe(&store, &root);
    let checkpoint = store
        .create_development_checkpoint(
            &user_command(),
            DevelopmentCheckpointInput {
                note: "Bookmark the clean worktree.".into(),
                blockers: vec![],
                next_actions: vec!["Continue implementation.".into()],
            },
        )
        .unwrap();
    fs::write(root.join("README.md"), "# Fixture\n\nUnobserved edit.\n").unwrap();
    let resume = store.resume_development().unwrap();
    assert_eq!(
        resume.checkpoint.as_ref().map(|value| value.id.as_str()),
        Some(checkpoint.id.as_str())
    );
    assert!(resume.checkpoint_is_stale);
    assert!(resume.repository_diverged);
    assert_eq!(resume.events_since_checkpoint, 0);
    assert_eq!(resume.next_actions, vec!["Continue implementation."]);
}

#[test]
fn explicit_repository_relocation_preserves_continuum_identity_and_history() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_and_observe(&store, &root);
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
    let moved = repository_directory.path().join("moved-repository");
    fs::rename(&root, &moved).unwrap();
    let relocated = store
        .relocate_repository(
            &user_command(),
            &repository.entity.id,
            repository.entity.version,
            &moved,
        )
        .unwrap();
    assert_eq!(relocated.entity.id, repository.entity.id);
    assert_eq!(relocated.entity.version, repository.entity.version + 1);
    assert_ne!(relocated.root_fingerprint, repository.root_fingerprint);
    assert_eq!(
        store
            .get_commit_observation(&ingestion.ingested_commit_ids[0])
            .unwrap()
            .first_observed_baseline_id,
        baseline.entity.id
    );
    let repeated_baseline = store
        .observe_repository_baseline(&user_command(), &relocated.entity.id)
        .unwrap();
    assert_eq!(repeated_baseline.entity.id, baseline.entity.id);

    let unrelated_directory = tempfile::tempdir().unwrap();
    let unrelated = init_repository(&unrelated_directory);
    let wrong = store.relocate_repository(
        &user_command(),
        &relocated.entity.id,
        relocated.entity.version,
        &unrelated,
    );
    assert!(matches!(wrong, Err(CoreError::Validation(_))));
    assert_eq!(
        store
            .get_repository(&relocated.entity.id)
            .unwrap()
            .root_path,
        relocated.root_path
    );
}

#[test]
fn unborn_and_detached_repository_states_are_explicit() {
    let repository_directory = tempfile::tempdir().unwrap();
    let unborn_root = repository_directory.path().join("unborn");
    fs::create_dir_all(&unborn_root).unwrap();
    run_git(&unborn_root, &["init", "-b", "main"]);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (unborn_repository, unborn) = attach_and_observe(&store, &unborn_root);
    assert_eq!(
        unborn.relation_to_previous,
        BaselineRelation::Unborn.as_str()
    );
    assert!(unborn.head_oid.is_none());
    let empty = store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: unborn_repository.entity.id,
                baseline_id: unborn.entity.id,
                max_commits: 100,
            },
        )
        .unwrap();
    assert_eq!(empty.reachable_commits, 0);

    let committed_directory = tempfile::tempdir().unwrap();
    let committed_root = init_repository(&committed_directory);
    let (repository, _attached) = attach_and_observe(&store, &committed_root);
    run_git(&committed_root, &["checkout", "--detach", "HEAD"]);
    let detached = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    assert_eq!(
        detached.relation_to_previous,
        BaselineRelation::DetachedHead.as_str()
    );
    assert!(detached.head_ref.is_none());
    assert!(detached.head_oid.is_some());
}

#[test]
fn committed_change_set_rejects_commit_not_reachable_from_selected_baseline() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, main_baseline) = attach_and_observe(&store, &root);
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: main_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    run_git(&root, &["checkout", "-b", "side"]);
    fs::write(root.join("side.txt"), "side branch only\n").unwrap();
    run_git(&root, &["add", "--", "side.txt"]);
    run_git(&root, &["commit", "-m", "Side branch commit"]);
    let side_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    let side_ingestion = store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: side_baseline.entity.id,
                max_commits: 100,
            },
        )
        .unwrap();
    assert_eq!(side_ingestion.ingested_commit_ids.len(), 1);
    run_git(&root, &["checkout", "main"]);
    let selected_main = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    let return_to_main = store
        .latest_repository_reconciliation(&repository.entity.id)
        .unwrap();
    assert_eq!(
        return_to_main.relation_kind,
        BaselineRelation::BranchSwitch.as_str()
    );
    assert_eq!(return_to_main.current_baseline_id, selected_main.entity.id);
    let invalid = store.create_change_set(
        &user_command(),
        NewChangeSet {
            title: "Invalid cross-branch grouping".into(),
            summary: "The side commit is not reachable from this main baseline.".into(),
            repository_id: repository.entity.id,
            baseline_id: selected_main.entity.id,
            kind: ChangeSetKind::Committed,
            commit_entity_ids: side_ingestion.ingested_commit_ids,
            requirement_links: vec![],
            intent_origin: DevelopmentIntentOrigin::User,
            supersedes_change_set_id: None,
            metadata: json!({}),
        },
    );
    assert!(matches!(invalid, Err(CoreError::Validation(_))));
}

#[test]
fn accepted_research_decision_flows_into_development_without_copying_requirement() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    store
        .set_space_capability_with_context(&user_command(), Space::Research, true)
        .unwrap();
    let decision = store
        .create_decision(
            &user_command(),
            NewDecision {
                title: "Adopt auditable repository history".into(),
                selected_option: "Use deterministic Git observations".into(),
                rationale: "The implementation must remain reproducible.".into(),
                alternatives: vec![],
                constraints: vec!["Git remains authoritative".into()],
                finding_ids: vec![],
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
                title: "Preserve implementation history".into(),
                statement: "Every implementation group must cite exact Git observations.".into(),
                acceptance_criteria: vec!["ChangeSet resolves to immutable commits.".into()],
                priority: 4,
                rationale_origin: RequirementRationaleOrigin::Research,
                verification_method: "Inspect links".into(),
                decision_id: Some(decision.entity.id),
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let (repository, baseline) = attach_and_observe(&store, &root);
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
                title: "Decision-backed implementation".into(),
                summary: "Uses the same Requirement identity produced by Research.".into(),
                repository_id: repository.entity.id,
                baseline_id: baseline.entity.id,
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
    let links = store
        .list_relationships_for_entity(&requirement.entity.id, PageRequest::default())
        .unwrap();
    assert!(links.items.iter().any(|link| {
        link.source_entity_id == change_set.entity.id && link.relation_type == "implements"
    }));
    assert_eq!(
        store.get_entity(&requirement.entity.id).unwrap().id,
        requirement.entity.id
    );
}

#[test]
fn integrity_scan_reports_missing_development_detail_with_recovery_guidance() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let repository = store
        .attach_repository(
            &user_command(),
            RepositoryAttachInput {
                path: root,
                title: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let connection = store.debug_connection().unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .unwrap();
    connection
        .execute(
            "DELETE FROM repositories WHERE entity_id=?1",
            [&repository.entity.id],
        )
        .unwrap();
    drop(connection);
    let report = store.verify_integrity().unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| {
            issue.code == "missing_development_detail" && issue.path_or_id == repository.entity.id
        })
        .unwrap();
    assert!(issue.guidance.contains("verified backup"));
}

#[test]
fn repository_local_executable_git_configuration_is_rejected() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    run_git(&root, &["config", "filter.evil.clean", "false"]);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);

    let error = store
        .attach_repository(
            &user_command(),
            RepositoryAttachInput {
                path: root,
                title: None,
                metadata: json!({}),
            },
        )
        .unwrap_err();

    assert!(matches!(error, CoreError::Validation(_)));
    assert!(
        error
            .to_string()
            .contains("repository-local executable filter/diff configuration")
    );
}
