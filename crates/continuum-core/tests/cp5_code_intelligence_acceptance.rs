use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use continuum_core::{
    ActorRef, AnalysisLimits, AnalyzeBaselineInput, ChangeSetKind, CommandContext, ContinuityStore,
    CoreError, DevelopmentCheckpointInput, DevelopmentIntentOrigin, DevelopmentSearchQuery,
    DevelopmentTimelineFilter, LinkTestVerificationInput, NewChangeSet, NewDevelopmentRequirement,
    NewEntity, NewTestRun, OriginKind, PageRequest, RepositoryAttachInput,
    RequirementRationaleOrigin, Space, TestOutcome, TestResultInput, TestRunSource, new_id,
};
use rusqlite::OptionalExtension;
use serde_json::{Value, json};
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

fn init_repository(directory: &TempDir) -> PathBuf {
    let root = directory.path().join("repository");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("web")).unwrap();
    fs::create_dir_all(root.join("python")).unwrap();
    run_git(&root, &["init", "-b", "main"]);
    run_git(&root, &["config", "user.name", "Continuum Test"]);
    run_git(
        &root,
        &["config", "user.email", "continuum@example.invalid"],
    );
    run_git(&root, &["config", "commit.gpgsign", "false"]);
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname=\"fixture\"\nversion=\"0.1.0\"\n\
         [dependencies]\nserde=\"1\"\n\
         [dev-dependencies]\ntempfile=\"3\"\n",
    )
    .unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub struct Engine;\n\
         impl Engine { pub fn run(&self) -> bool { true } }\n\
         #[test]\npub fn engine_runs() { assert!(Engine.run()); }\n",
    )
    .unwrap();
    fs::write(
        root.join("web/math.test.ts"),
        "export function add(a: number, b: number): number { return a + b; }\n\
         test('adds numbers', () => { if (add(1, 2) !== 3) throw new Error(); });\n",
    )
    .unwrap();
    fs::write(
        root.join("python/test_math.py"),
        "class TestMath:\n    def test_add(self):\n        assert 1 + 2 == 3\n",
    )
    .unwrap();
    fs::write(
        root.join("package.json"),
        "{\"name\":\"fixture\",\"dependencies\":{\"react\":\"19\"},\
         \"devDependencies\":{\"vitest\":\"3\"}}",
    )
    .unwrap();
    fs::write(
        root.join("settings.json"),
        "{\"feature\":true,\"retries\":3}",
    )
    .unwrap();
    fs::write(
        root.join(".env.fixture"),
        "TOKEN=do-not-persist\nPORT=3000\n",
    )
    .unwrap();
    fs::write(root.join("notes.custom"), "unsupported but addressable\n").unwrap();
    fs::write(root.join("image.bin"), [0_u8, 1, 2, 3]).unwrap();
    run_git(&root, &["add", "-f", "."]);
    run_git(&root, &["commit", "-m", "Add multi-language fixture"]);
    root
}

fn development_project(directory: &TempDir) -> ContinuityStore {
    let store = ContinuityStore::create_with_actor(
        directory.path().join("continuum-project"),
        "CP5 fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    store
        .set_space_capability_with_context(&user_command(), Space::Development, true)
        .unwrap();
    store
}

fn attach_observe(
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
                path: root.to_owned(),
                title: Some("Code fixture".into()),
                metadata: json!({}),
            },
        )
        .unwrap();
    let baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    (repository, baseline)
}

fn analyze(
    store: &ContinuityStore,
    repository_id: &str,
    baseline_id: &str,
) -> continuum_core::AnalysisRunRecord {
    store
        .analyze_repository_baseline(
            &user_command(),
            AnalyzeBaselineInput {
                repository_id: repository_id.into(),
                baseline_id: baseline_id.into(),
                limits: AnalysisLimits::default(),
            },
        )
        .unwrap()
}

fn entity_id_by_title(store: &ContinuityStore, kind: &str, title: &str) -> String {
    store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT id FROM entities WHERE entity_type=?1 AND title=?2 ORDER BY id LIMIT 1",
            [kind, title],
            |row| row.get(0),
        )
        .unwrap_or_else(|error| panic!("missing {kind} titled {title:?}: {error}"))
}

fn file_id(store: &ContinuityStore, repository_id: &str, path: &str) -> String {
    store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT cea.code_entity_id FROM code_entity_aliases cea
             JOIN code_entities ce ON ce.entity_id=cea.code_entity_id
             WHERE cea.repository_id=?1 AND cea.alias_kind='path' AND cea.alias_value=?2
               AND cea.retired_at_baseline_id IS NULL AND ce.entity_kind='file'",
            [repository_id, path],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn development_only_code_intelligence_is_resumable_auditable_and_exportable() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_observe(&store, &root);
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
    let requirement = store
        .create_development_requirement(
            &user_command(),
            NewDevelopmentRequirement {
                title: "Keep code behavior verified".into(),
                statement: "Detected tests must remain separate from observed runs.".into(),
                acceptance_criteria: vec!["A TestRun cites a detected Test.".into()],
                priority: 3,
                rationale_origin: RequirementRationaleOrigin::External,
                verification_method: "Inspect CP5 links".into(),
                metadata: json!({}),
            },
        )
        .unwrap();
    let change_set = store
        .create_change_set(
            &user_command(),
            NewChangeSet {
                title: "Initial code fixture".into(),
                summary: "Introduces code, configuration, dependencies, and tests.".into(),
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                kind: ChangeSetKind::Committed,
                commit_entity_ids: ingestion.ingested_commit_ids,
                requirement_links: Vec::new(),
                intent_origin: DevelopmentIntentOrigin::External,
                supersedes_change_set_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();

    let analysis = analyze(&store, &repository.entity.id, &baseline.entity.id);
    assert_eq!(analysis.entity.origin, "deterministic");
    assert_eq!(analysis.analyzer_executions.len(), 4);
    assert!(analysis.code_entity_count >= 15);
    assert!(analysis.test_count >= 3);
    assert_eq!(analysis.completeness, "partial");
    assert!(analysis.limitations.iter().any(|item| {
        item.code == "unsupported_language" && item.source_path.as_deref() == Some("notes.custom")
    }));
    assert!(
        analysis
            .limitations
            .iter()
            .any(|item| item.code == "binary_content")
    );

    let source_file_id = file_id(&store, &repository.entity.id, "src/lib.rs");
    let source_file = store.get_code_entity(&source_file_id).unwrap();
    assert_eq!(source_file.entity_kind, "file");
    assert_eq!(
        source_file.latest_observation.unwrap().baseline_id,
        baseline.entity.id
    );
    let modifies: i64 = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM relationships WHERE source_entity_id=?1
             AND target_entity_id=?2 AND relation_type='modifies'",
            [&change_set.entity.id, &source_file_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(modifies, 1);

    let rust_test_id = entity_id_by_title(&store, "test", "engine_runs");
    let rust_test = store.get_test(&rust_test_id).unwrap();
    assert_eq!(rust_test.framework, "rust-test");
    store
        .link_test_verification(
            &user_command(),
            LinkTestVerificationInput {
                test_id: rust_test_id.clone(),
                target_entity_id: requirement.id.clone(),
            },
        )
        .unwrap();
    let test_run = store
        .record_test_run(
            &user_command(),
            NewTestRun {
                title: "Imported local test result".into(),
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                outcome: TestOutcome::Passed,
                command_label: "cargo test".into(),
                exit_code: Some(0),
                duration_ms: Some(42),
                source: TestRunSource::User,
                results: vec![TestResultInput {
                    test_id: rust_test_id,
                    outcome: TestOutcome::Passed,
                    duration_ms: Some(2),
                    message: None,
                }],
                details: json!({"runner":"external-observation"}),
                metadata: json!({}),
            },
        )
        .unwrap();
    assert_eq!(test_run.entity.entity_type, "test_run");
    assert_eq!(test_run.results.len(), 1);

    let search = store
        .search_development(DevelopmentSearchQuery {
            text: "engine_runs".into(),
            entity_type: Some("test".into()),
            status: None,
            page: PageRequest::default(),
        })
        .unwrap();
    assert_eq!(search.items.len(), 1);
    let timeline = store
        .development_timeline(
            DevelopmentTimelineFilter {
                event_type: Some("code_intelligence.analysis.completed".into()),
                ..Default::default()
            },
            PageRequest::default(),
        )
        .unwrap();
    assert_eq!(timeline.items.len(), 1);
    let checkpoint = store
        .create_development_checkpoint(
            &user_command(),
            DevelopmentCheckpointInput {
                note: "Code analysis and tests captured".into(),
                blockers: Vec::new(),
                next_actions: vec!["Continue provenance work in CP6".into()],
            },
        )
        .unwrap();
    assert_eq!(checkpoint.summary["schema_version"], 2);
    assert!(
        checkpoint.summary["code_intelligence"]
            .as_array()
            .is_some_and(|items| items.len() >= 2)
    );
    assert!(!store.resume_development().unwrap().checkpoint_is_stale);
    let report = store.generate_development_report().unwrap();
    assert!(report.markdown.contains("## Code Intelligence"));
    assert!(report.markdown.contains("## Test Runs"));
    assert!(report.cited_entity_ids.contains(&analysis.entity.id));
    assert!(report.cited_entity_ids.contains(&test_run.entity.id));
    assert!(store.verify_integrity().unwrap().is_healthy());

    let export = project_directory.path().join("export");
    store.export_project(&export).unwrap();
    let restored =
        ContinuityStore::import_export(&export, project_directory.path().join("restored-project"))
            .unwrap();
    assert_eq!(
        restored
            .get_analysis_run(&analysis.entity.id)
            .unwrap()
            .source_fingerprint,
        analysis.source_fingerprint
    );
    assert_eq!(
        restored.get_test_run(&test_run.entity.id).unwrap().results,
        test_run.results
    );
    assert!(restored.verify_integrity().unwrap().is_healthy());
    assert!(!restored.capability_enabled(Space::Research).unwrap());
}

#[test]
fn identical_analysis_is_idempotent_and_unchanged_blobs_use_versioned_cache() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, first_baseline) = attach_observe(&store, &root);
    let ingestion = store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: first_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    let first = analyze(&store, &repository.entity.id, &first_baseline.entity.id);
    let post_analysis_change_set = store
        .create_change_set(
            &user_command(),
            NewChangeSet {
                title: "Created after code analysis".into(),
                summary: "Proves ChangeSet-to-CodeEntity linking is order independent.".into(),
                repository_id: repository.entity.id.clone(),
                baseline_id: first_baseline.entity.id.clone(),
                kind: ChangeSetKind::Committed,
                commit_entity_ids: ingestion.ingested_commit_ids,
                requirement_links: Vec::new(),
                intent_origin: DevelopmentIntentOrigin::User,
                supersedes_change_set_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let linked_files: i64 = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM relationships WHERE source_entity_id=?1
             AND relation_type='modifies' AND target_entity_type='code_entity'",
            [&post_analysis_change_set.entity.id],
            |row| row.get(0),
        )
        .unwrap();
    assert!(linked_files > 0);
    let second = analyze(&store, &repository.entity.id, &first_baseline.entity.id);
    assert_eq!(first.entity.id, second.entity.id);
    let count: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM analysis_runs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 1);

    fs::write(root.join("new.custom"), "another unsupported file\n").unwrap();
    run_git(&root, &["add", "--", "new.custom"]);
    run_git(&root, &["commit", "-m", "Add unrelated file"]);
    let next_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    let next = analyze(&store, &repository.entity.id, &next_baseline.entity.id);
    assert_ne!(next.entity.id, first.entity.id);
    assert!(next.cache_hit_count >= first.analyzed_file_count);
    assert!(next.cache_miss_count <= 1);
}

#[test]
fn dirty_worktree_and_unborn_repository_degrade_without_stale_structural_claims() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, clean_baseline) = attach_observe(&store, &root);
    let clean_analysis = analyze(&store, &repository.entity.id, &clean_baseline.entity.id);
    let engine_id = entity_id_by_title(&store, "code_entity", "Engine");
    let original_source = fs::read(root.join("src/lib.rs")).unwrap();
    fs::write(
        root.join("src/lib.rs"),
        "pub fn changed_but_uncommitted() {}\n",
    )
    .unwrap();
    fs::write(root.join("untracked.rs"), "pub fn untracked() {}\n").unwrap();
    let dirty = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    let analysis = analyze(&store, &repository.entity.id, &dirty.entity.id);
    assert_eq!(analysis.completeness, "partial");
    let limitation_paths = analysis
        .limitations
        .iter()
        .filter(|item| item.code == "working_tree_content_not_captured")
        .filter_map(|item| item.source_path.as_deref())
        .collect::<Vec<_>>();
    assert!(limitation_paths.contains(&"src/lib.rs"));
    assert!(limitation_paths.contains(&"untracked.rs"));
    let untracked_id = file_id(&store, &repository.entity.id, "untracked.rs");
    assert_eq!(
        store
            .get_code_entity(&untracked_id)
            .unwrap()
            .latest_observation
            .unwrap()
            .observation_status,
        "fallback"
    );
    let stale_symbol_count: i64 = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM code_entity_observations
             WHERE source_path='src/lib.rs' AND qualified_name='changed_but_uncommitted'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stale_symbol_count, 0);
    assert_eq!(store.get_entity(&engine_id).unwrap().status, "unavailable");

    fs::write(root.join("src/lib.rs"), original_source).unwrap();
    fs::remove_file(root.join("untracked.rs")).unwrap();
    let clean_again = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    assert_eq!(clean_again.entity.id, clean_baseline.entity.id);
    let reapplied = analyze(&store, &repository.entity.id, &clean_again.entity.id);
    assert_eq!(reapplied.entity.id, clean_analysis.entity.id);
    assert_eq!(store.get_entity(&engine_id).unwrap().status, "active");
    let reapply_events = store
        .development_timeline(
            DevelopmentTimelineFilter {
                event_type: Some("code_intelligence.analysis.reapplied".into()),
                ..Default::default()
            },
            PageRequest::default(),
        )
        .unwrap();
    assert_eq!(reapply_events.items.len(), 1);

    let unborn_directory = tempfile::tempdir().unwrap();
    let unborn_root = unborn_directory.path().join("empty");
    fs::create_dir_all(&unborn_root).unwrap();
    run_git(&unborn_root, &["init", "-b", "main"]);
    let unborn = store
        .attach_repository(
            &user_command(),
            RepositoryAttachInput {
                path: unborn_root,
                title: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let unborn_baseline = store
        .observe_repository_baseline(&user_command(), &unborn.entity.id)
        .unwrap();
    let unborn_analysis = analyze(&store, &unborn.entity.id, &unborn_baseline.entity.id);
    assert!(
        unborn_analysis
            .limitations
            .iter()
            .any(|item| item.code == "unborn_repository")
    );
}

#[test]
fn committed_rename_preserves_file_and_symbol_identity_when_content_is_unchanged() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, first_baseline) = attach_observe(&store, &root);
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: first_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &first_baseline.entity.id);
    let old_file_id = file_id(&store, &repository.entity.id, "src/lib.rs");
    let old_symbol_id = entity_id_by_title(&store, "code_entity", "Engine");

    run_git(&root, &["mv", "src/lib.rs", "src/engine.rs"]);
    run_git(&root, &["commit", "-m", "Rename engine source"]);
    let next_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: next_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &next_baseline.entity.id);
    assert_eq!(
        file_id(&store, &repository.entity.id, "src/engine.rs"),
        old_file_id
    );
    assert_eq!(
        entity_id_by_title(&store, "code_entity", "Engine"),
        old_symbol_id
    );
    let aliases: i64 = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM code_entity_aliases WHERE code_entity_id=?1 AND alias_kind='path'",
            [&old_file_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(aliases, 2);
}

#[test]
fn supported_languages_manifests_and_secret_safe_configuration_are_deterministic() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    fs::write(
        root.join("package.json"),
        "{\"dependencies\":{\"safe\":\"https://token@example.invalid/repo.git\"}}",
    )
    .unwrap();
    run_git(&root, &["add", "--", "package.json"]);
    run_git(&root, &["commit", "-m", "Add credential-shaped dependency"]);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_observe(&store, &root);
    analyze(&store, &repository.entity.id, &baseline.entity.id);
    for symbol in ["Engine", "add", "TestMath", "TestMath::test_add"] {
        assert!(!entity_id_by_title(&store, "code_entity", symbol).is_empty());
    }
    for test in ["engine_runs", "adds numbers", "test_add"] {
        assert!(!entity_id_by_title(&store, "test", test).is_empty());
    }
    for dependency in [
        "serde (runtime)",
        "tempfile (development)",
        "safe (runtime)",
    ] {
        assert!(!entity_id_by_title(&store, "code_entity", dependency).is_empty());
    }
    let all_details = store
        .debug_connection()
        .unwrap()
        .prepare("SELECT details_json FROM code_entity_observations")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<Vec<_>, _>>()
        .unwrap()
        .join("\n");
    assert!(!all_details.contains("token@example"));
    assert!(all_details.contains("<redacted-uri>"));
    assert!(!all_details.contains("do-not-persist"));
    let dotenv_details: String = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT o.details_json FROM code_entity_observations o
             JOIN code_entities ce ON ce.entity_id=o.code_entity_id
             WHERE ce.entity_kind='configuration' AND o.source_path='.env.fixture'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let dotenv: Value = serde_json::from_str(&dotenv_details).unwrap();
    assert_eq!(dotenv["sensitive_values_omitted"], true);
    assert_eq!(dotenv["top_level_keys"], json!(["TOKEN", "PORT"]));
}

#[test]
fn invalid_test_runs_limits_ai_and_generic_bypass_fail_atomically() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_observe(&store, &root);
    analyze(&store, &repository.entity.id, &baseline.entity.id);
    let test_id = entity_id_by_title(&store, "test", "engine_runs");
    let before: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM test_runs", [], |row| row.get(0))
        .unwrap();
    let inconsistent = store.record_test_run(
        &user_command(),
        NewTestRun {
            title: "Impossible pass".into(),
            repository_id: repository.entity.id.clone(),
            baseline_id: baseline.entity.id.clone(),
            outcome: TestOutcome::Passed,
            command_label: "cargo test".into(),
            exit_code: Some(0),
            duration_ms: None,
            source: TestRunSource::User,
            results: vec![TestResultInput {
                test_id,
                outcome: TestOutcome::Failed,
                duration_ms: None,
                message: Some("failed".into()),
            }],
            details: json!({}),
            metadata: json!({}),
        },
    );
    assert!(matches!(inconsistent, Err(CoreError::Validation(_))));
    let after: i64 = store
        .debug_connection()
        .unwrap()
        .query_row("SELECT count(*) FROM test_runs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(before, after);

    let invalid_limits = store.analyze_repository_baseline(
        &user_command(),
        AnalyzeBaselineInput {
            repository_id: repository.entity.id.clone(),
            baseline_id: baseline.entity.id.clone(),
            limits: AnalysisLimits {
                max_files: 0,
                ..Default::default()
            },
        },
    );
    assert!(matches!(invalid_limits, Err(CoreError::Validation(_))));

    let generic = store.create_entity_with_context(
        &user_command(),
        NewEntity {
            entity_type: "code_entity".into(),
            schema_version: 1,
            title: "Bypass".into(),
            origin: OriginKind::User,
            metadata: json!({}),
            data: json!({}),
        },
    );
    assert!(matches!(generic, Err(CoreError::Validation(_))));

    let ai = store.analyze_repository_baseline(
        &CommandContext::new(continuum_core::ActorRef {
            kind: continuum_core::ActorKind::AiProposal,
            id: "model".into(),
        }),
        AnalyzeBaselineInput {
            repository_id: repository.entity.id,
            baseline_id: baseline.entity.id,
            limits: AnalysisLimits::default(),
        },
    );
    assert!(matches!(ai, Err(CoreError::Validation(_))));
}

#[test]
fn cross_repository_test_membership_and_invalid_verification_targets_are_rejected() {
    let first_directory = tempfile::tempdir().unwrap();
    let first_root = init_repository(&first_directory);
    let second_directory = tempfile::tempdir().unwrap();
    let second_root = init_repository(&second_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (first_repository, first_baseline) = attach_observe(&store, &first_root);
    let (second_repository, second_baseline) = attach_observe(&store, &second_root);
    analyze(
        &store,
        &first_repository.entity.id,
        &first_baseline.entity.id,
    );
    analyze(
        &store,
        &second_repository.entity.id,
        &second_baseline.entity.id,
    );
    let first_test = entity_id_by_title(&store, "test", "engine_runs");
    let second_code_entity: String = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT ce.entity_id FROM code_entities ce JOIN entities e ON e.id=ce.entity_id
             WHERE ce.repository_id=?1 AND ce.entity_kind='file' AND e.title='src/lib.rs'",
            [&second_repository.entity.id],
            |row| row.get(0),
        )
        .unwrap();
    let cross_repository = store.record_test_run(
        &user_command(),
        NewTestRun {
            title: "Wrong repository".into(),
            repository_id: second_repository.entity.id,
            baseline_id: second_baseline.entity.id,
            outcome: TestOutcome::Failed,
            command_label: "external runner".into(),
            exit_code: Some(1),
            duration_ms: None,
            source: TestRunSource::External,
            results: vec![TestResultInput {
                test_id: first_test.clone(),
                outcome: TestOutcome::Failed,
                duration_ms: None,
                message: None,
            }],
            details: json!({}),
            metadata: json!({}),
        },
    );
    assert!(matches!(cross_repository, Err(CoreError::Validation(_))));
    let cross_repository_verification = store.link_test_verification(
        &user_command(),
        LinkTestVerificationInput {
            test_id: first_test.clone(),
            target_entity_id: second_code_entity,
        },
    );
    assert!(matches!(
        cross_repository_verification,
        Err(CoreError::Validation(_))
    ));
    let invalid_target = store.link_test_verification(
        &user_command(),
        LinkTestVerificationInput {
            test_id: first_test,
            target_entity_id: first_repository.entity.id,
        },
    );
    assert!(matches!(invalid_target, Err(CoreError::Validation(_))));
}

#[test]
fn integrity_scan_reports_corrupt_code_intelligence_rows_with_recovery_guidance() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_observe(&store, &root);
    analyze(&store, &repository.entity.id, &baseline.entity.id);
    let test_id = entity_id_by_title(&store, "test", "engine_runs");
    let connection = store.debug_connection().unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .unwrap();
    connection
        .execute("DELETE FROM tests WHERE entity_id=?1", [&test_id])
        .unwrap();
    drop(connection);
    let report = store.verify_integrity().unwrap();
    let issue = report
        .issues
        .iter()
        .find(|issue| {
            issue.code == "missing_code_intelligence_detail" && issue.path_or_id == test_id
        })
        .unwrap();
    assert!(issue.guidance.contains("verified backup"));
}

#[test]
fn analysis_limit_failure_commits_no_partial_run_or_entity() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_observe(&store, &root);
    let result = store.analyze_repository_baseline(
        &user_command(),
        AnalyzeBaselineInput {
            repository_id: repository.entity.id,
            baseline_id: baseline.entity.id,
            limits: AnalysisLimits {
                max_files: 1,
                max_file_bytes: 1024,
                max_total_bytes: 1024,
                max_entities: 10,
            },
        },
    );
    assert!(matches!(result, Err(CoreError::Conflict(_))));
    let connection = store.debug_connection().unwrap();
    let run_count: i64 = connection
        .query_row("SELECT count(*) FROM analysis_runs", [], |row| row.get(0))
        .unwrap();
    let code_count: i64 = connection
        .query_row("SELECT count(*) FROM code_entities", [], |row| row.get(0))
        .unwrap();
    assert_eq!((run_count, code_count), (0, 0));
}

#[test]
fn parse_errors_are_explicit_and_do_not_hide_file_identity() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    fs::write(root.join("src/broken.rs"), "pub fn broken( {\n").unwrap();
    run_git(&root, &["add", "--", "src/broken.rs"]);
    run_git(&root, &["commit", "-m", "Add incomplete syntax"]);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_observe(&store, &root);
    let analysis = analyze(&store, &repository.entity.id, &baseline.entity.id);
    assert!(analysis.limitations.iter().any(|item| {
        item.code == "parse_error" && item.source_path.as_deref() == Some("src/broken.rs")
    }));
    assert!(!file_id(&store, &repository.entity.id, "src/broken.rs").is_empty());

    fs::write(root.join("unrelated.custom"), "advance baseline\n").unwrap();
    run_git(&root, &["add", "--", "unrelated.custom"]);
    run_git(&root, &["commit", "-m", "Advance after parse error"]);
    let next_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    let cached = analyze(&store, &repository.entity.id, &next_baseline.entity.id);
    assert!(cached.cache_hit_count > 0);
    assert!(cached.limitations.iter().any(|item| {
        item.code == "parse_error" && item.source_path.as_deref() == Some("src/broken.rs")
    }));
}

#[cfg(unix)]
#[test]
fn git_symlink_blob_is_never_parsed_as_source_code() {
    use std::os::unix::fs::symlink;

    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    symlink(
        "pub fn injected_from_link_target() {}",
        root.join("link.rs"),
    )
    .unwrap();
    run_git(&root, &["add", "--", "link.rs"]);
    run_git(&root, &["commit", "-m", "Add source-shaped symlink target"]);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_observe(&store, &root);
    let analysis = analyze(&store, &repository.entity.id, &baseline.entity.id);
    assert!(analysis.limitations.iter().any(|item| {
        item.code == "symlink_entry" && item.source_path.as_deref() == Some("link.rs")
    }));
    let parsed: i64 = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM entities WHERE entity_type='code_entity'
             AND title='injected_from_link_target'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(parsed, 0);
}

#[test]
fn disappeared_and_reappearing_code_and_tests_keep_identity_with_honest_presence() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_observe(&store, &root);
    analyze(&store, &repository.entity.id, &baseline.entity.id);
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    let file = file_id(&store, &repository.entity.id, "web/math.test.ts");
    let symbol = entity_id_by_title(&store, "code_entity", "add");
    let test = entity_id_by_title(&store, "test", "adds numbers");
    let original = fs::read(root.join("web/math.test.ts")).unwrap();

    fs::remove_file(root.join("web/math.test.ts")).unwrap();
    run_git(&root, &["add", "--", "web/math.test.ts"]);
    run_git(&root, &["commit", "-m", "Remove TypeScript test"]);
    let missing_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    let deletion = store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: missing_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &missing_baseline.entity.id);
    let deletion_change_set = store
        .create_change_set(
            &user_command(),
            NewChangeSet {
                title: "Remove TypeScript test".into(),
                summary: "Deletion remains connected to the historical file identity.".into(),
                repository_id: repository.entity.id.clone(),
                baseline_id: missing_baseline.entity.id.clone(),
                kind: ChangeSetKind::Committed,
                commit_entity_ids: deletion.ingested_commit_ids,
                requirement_links: Vec::new(),
                intent_origin: DevelopmentIntentOrigin::User,
                supersedes_change_set_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let deletion_link: i64 = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM relationships WHERE source_entity_id=?1
             AND target_entity_id=?2 AND relation_type='modifies'",
            [&deletion_change_set.entity.id, &file],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(deletion_link, 1);
    for id in [&file, &symbol, &test] {
        let entity = store.get_entity(id).unwrap();
        assert_eq!(entity.status, "unavailable");
        assert_eq!(entity.data["presence"], "unavailable");
        assert_eq!(
            entity.data["unavailable_since_baseline_id"],
            missing_baseline.entity.id
        );
    }

    fs::write(root.join("web/math.test.ts"), original).unwrap();
    run_git(&root, &["add", "--", "web/math.test.ts"]);
    run_git(&root, &["commit", "-m", "Restore TypeScript test"]);
    let restored_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    analyze(&store, &repository.entity.id, &restored_baseline.entity.id);
    assert_eq!(
        file_id(&store, &repository.entity.id, "web/math.test.ts"),
        file
    );
    assert_eq!(entity_id_by_title(&store, "code_entity", "add"), symbol);
    assert_eq!(entity_id_by_title(&store, "test", "adds numbers"), test);
    for id in [&file, &symbol, &test] {
        let entity = store.get_entity(id).unwrap();
        assert_eq!(entity.status, "active");
        assert_eq!(entity.data["presence"], "present");
        assert_eq!(
            entity.data["last_observed_baseline_id"],
            restored_baseline.entity.id
        );
    }
}

#[test]
fn analysis_queries_remain_project_scoped() {
    let first_directory = tempfile::tempdir().unwrap();
    let first_root = init_repository(&first_directory);
    let first_project_directory = tempfile::tempdir().unwrap();
    let first_store = development_project(&first_project_directory);
    let (repository, baseline) = attach_observe(&first_store, &first_root);
    let analysis = analyze(&first_store, &repository.entity.id, &baseline.entity.id);

    let second_project_directory = tempfile::tempdir().unwrap();
    let second_store = development_project(&second_project_directory);
    assert!(matches!(
        second_store.get_analysis_run(&analysis.entity.id),
        Err(CoreError::NotFound(_))
    ));
    let leaked: Option<String> = second_store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT entity_id FROM analysis_runs WHERE entity_id=?1",
            [&analysis.entity.id],
            |row| row.get(0),
        )
        .optional()
        .unwrap();
    assert!(leaked.is_none());
}

#[test]
fn historical_reanalysis_never_rewinds_current_projection() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, first_baseline) = attach_observe(&store, &root);
    analyze(&store, &repository.entity.id, &first_baseline.entity.id);
    let old_symbol = entity_id_by_title(&store, "code_entity", "Engine");

    fs::write(
        root.join("src/lib.rs"),
        "pub struct CurrentEngine;\nimpl CurrentEngine { pub fn run(&self) -> bool { true } }\n",
    )
    .unwrap();
    run_git(&root, &["add", "--", "src/lib.rs"]);
    run_git(&root, &["commit", "-m", "Replace current engine symbol"]);
    let current_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    analyze(&store, &repository.entity.id, &current_baseline.entity.id);
    let current_symbol = entity_id_by_title(&store, "code_entity", "CurrentEngine");
    let old_before = store.get_entity(&old_symbol).unwrap();
    let current_before = store.get_entity(&current_symbol).unwrap();
    assert_eq!(old_before.status, "unavailable");
    assert_eq!(current_before.status, "active");

    analyze(&store, &repository.entity.id, &first_baseline.entity.id);
    let old_after = store.get_entity(&old_symbol).unwrap();
    let current_after = store.get_entity(&current_symbol).unwrap();
    assert_eq!(old_after.status, "unavailable");
    assert_eq!(current_after.status, "active");
    assert_eq!(old_after.version, old_before.version);
    assert_eq!(current_after.version, current_before.version);
    assert_eq!(
        current_after.data["last_observed_baseline_id"],
        current_baseline.entity.id
    );
}

#[test]
fn renamed_file_keeps_identity_but_reused_old_path_gets_a_new_identity() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, first_baseline) = attach_observe(&store, &root);
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: first_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &first_baseline.entity.id);
    let original_id = file_id(&store, &repository.entity.id, "src/lib.rs");

    run_git(&root, &["mv", "src/lib.rs", "src/engine.rs"]);
    run_git(&root, &["commit", "-m", "Rename engine source"]);
    let renamed_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: renamed_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &renamed_baseline.entity.id);
    assert_eq!(
        file_id(&store, &repository.entity.id, "src/engine.rs"),
        original_id
    );

    fs::write(
        root.join("src/lib.rs"),
        "pub fn entirely_new_file_at_reused_path() -> u8 { 7 }\n",
    )
    .unwrap();
    run_git(&root, &["add", "--", "src/lib.rs"]);
    run_git(&root, &["commit", "-m", "Reuse old source path"]);
    let reused_baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: reused_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &reused_baseline.entity.id);
    let reused_id = file_id(&store, &repository.entity.id, "src/lib.rs");
    assert_ne!(reused_id, original_id);
    assert_eq!(
        file_id(&store, &repository.entity.id, "src/engine.rs"),
        original_id
    );
    let active_old_path_aliases: i64 = store
        .debug_connection()
        .unwrap()
        .query_row(
            "SELECT count(*) FROM code_entity_aliases
             WHERE repository_id=?1 AND alias_kind='path' AND alias_value='src/lib.rs'
               AND retired_at_baseline_id IS NULL",
            [&repository.entity.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(active_old_path_aliases, 1);
}

#[test]
fn archived_project_rejects_code_intelligence_writes() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, baseline) = attach_observe(&store, &root);
    store.set_project_archived(&new_id(), true).unwrap();
    let result = store.analyze_repository_baseline(
        &user_command(),
        AnalyzeBaselineInput {
            repository_id: repository.entity.id,
            baseline_id: baseline.entity.id,
            limits: AnalysisLimits::default(),
        },
    );
    assert!(matches!(result, Err(CoreError::Conflict(_))));
    assert_eq!(store.summary().unwrap().status, "archived");
}

#[test]
fn rename_destination_is_resolved_before_same_commit_old_path_reuse() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, first_baseline) = attach_observe(&store, &root);
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: first_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &first_baseline.entity.id);
    let original_id = file_id(&store, &repository.entity.id, "src/lib.rs");

    run_git(&root, &["mv", "src/lib.rs", "src/z_engine.rs"]);
    fs::write(
        root.join("src/lib.rs"),
        "pub fn new_file_created_in_same_commit() -> bool { true }\n",
    )
    .unwrap();
    run_git(&root, &["add", "--", "src/lib.rs", "src/z_engine.rs"]);
    run_git(
        &root,
        &["commit", "-m", "Rename and reuse old path atomically"],
    );
    let baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &baseline.entity.id);

    assert_eq!(
        file_id(&store, &repository.entity.id, "src/z_engine.rs"),
        original_id
    );
    assert_ne!(
        file_id(&store, &repository.entity.id, "src/lib.rs"),
        original_id
    );
    assert!(store.verify_integrity().unwrap().is_healthy());
}

#[test]
fn ordinary_copy_with_unchanged_source_receives_a_distinct_file_identity() {
    let repository_directory = tempfile::tempdir().unwrap();
    let root = init_repository(&repository_directory);
    let project_directory = tempfile::tempdir().unwrap();
    let store = development_project(&project_directory);
    let (repository, first_baseline) = attach_observe(&store, &root);
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: first_baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &first_baseline.entity.id);
    let source_id = file_id(&store, &repository.entity.id, "src/lib.rs");

    fs::copy(root.join("src/lib.rs"), root.join("src/copied.rs")).unwrap();
    run_git(&root, &["add", "--", "src/copied.rs"]);
    run_git(&root, &["commit", "-m", "Copy source without moving it"]);
    let baseline = store
        .observe_repository_baseline(&user_command(), &repository.entity.id)
        .unwrap();
    store
        .ingest_git_commits(
            &user_command(),
            continuum_core::CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: baseline.entity.id.clone(),
                max_commits: 100,
            },
        )
        .unwrap();
    analyze(&store, &repository.entity.id, &baseline.entity.id);

    assert_ne!(
        file_id(&store, &repository.entity.id, "src/copied.rs"),
        source_id
    );
    assert_eq!(
        file_id(&store, &repository.entity.id, "src/lib.rs"),
        source_id
    );
    assert!(store.verify_integrity().unwrap().is_healthy());
}
