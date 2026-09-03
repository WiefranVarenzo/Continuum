use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use continuum_core::{
    ActorRef, ChangeSetKind, CommandContext, CommitIngestionInput, ContinuityStore,
    DevelopmentCheckpointInput, DevelopmentIntentOrigin, NewChangeSet, RepositoryAttachInput,
    Space,
};
use serde_json::json;

const INCREMENTAL_COMMITS: usize = 100;
const WARMUPS: usize = 5;
const SAMPLES: usize = 30;

fn main() {
    let temporary = tempfile::tempdir().expect("benchmark temporary directory");
    let repository_root = temporary.path().join("repository");
    initialize_repository(&repository_root);
    let store = ContinuityStore::create_with_actor(
        temporary.path().join("continuum-project"),
        "CP4 Git Benchmark",
        ActorRef::user("benchmark-owner"),
    )
    .expect("create benchmark project");
    store
        .set_space_capability_with_context(&command(), Space::Development, true)
        .expect("enable Development Space");
    let repository = store
        .attach_repository(
            &command(),
            RepositoryAttachInput {
                path: repository_root.clone(),
                title: Some("Benchmark repository".into()),
                metadata: json!({"fixture":"incremental-git-100-v1"}),
            },
        )
        .expect("attach repository");
    let initial = store
        .observe_repository_baseline(&command(), &repository.entity.id)
        .expect("observe initial baseline");
    store
        .ingest_git_commits(
            &command(),
            CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: initial.entity.id,
                max_commits: 10_000,
            },
        )
        .expect("ingest initial commit");

    seed_incremental_commits(&repository_root);
    let current = store
        .observe_repository_baseline(&command(), &repository.entity.id)
        .expect("observe incremental baseline");
    let ingestion_started = Instant::now();
    let ingestion = store
        .ingest_git_commits(
            &command(),
            CommitIngestionInput {
                repository_id: repository.entity.id.clone(),
                baseline_id: current.entity.id.clone(),
                max_commits: 10_000,
            },
        )
        .expect("ingest 100 new commits");
    let ingestion_elapsed = ingestion_started.elapsed();
    assert_eq!(ingestion.ingested_commit_ids.len(), INCREMENTAL_COMMITS);
    let change_set_started = Instant::now();
    let change_set = store
        .create_change_set(
            &command(),
            NewChangeSet {
                title: "100-commit benchmark ChangeSet".into(),
                summary: "Groups the deterministic incremental benchmark observations.".into(),
                repository_id: repository.entity.id,
                baseline_id: current.entity.id,
                kind: ChangeSetKind::Committed,
                commit_entity_ids: ingestion.ingested_commit_ids,
                requirement_links: vec![],
                intent_origin: DevelopmentIntentOrigin::User,
                supersedes_change_set_id: None,
                metadata: json!({}),
            },
        )
        .expect("create benchmark ChangeSet");
    let change_set_elapsed = change_set_started.elapsed();
    let checkpoint = store
        .create_development_checkpoint(
            &command(),
            DevelopmentCheckpointInput {
                note: "Benchmark checkpoint".into(),
                blockers: vec![],
                next_actions: vec!["Continue benchmark".into()],
            },
        )
        .expect("create benchmark checkpoint");

    for _ in 0..WARMUPS {
        let _ = store.resume_development().expect("warm resume");
        let _ = store.generate_development_report().expect("warm report");
        let _ = store
            .get_checkpoint(&checkpoint.id)
            .expect("warm checkpoint load");
    }
    let resume = sample(|| {
        let state = store.resume_development().expect("sample resume");
        assert!(!state.checkpoint_is_stale);
    });
    let report = sample(|| {
        let report = store.generate_development_report().expect("sample report");
        assert!(report.markdown.contains(&change_set.entity.id));
    });
    let checkpoint_load = sample(|| {
        let _ = store
            .get_checkpoint(&checkpoint.id)
            .expect("sample checkpoint load");
    });
    assert!(
        store
            .verify_integrity()
            .expect("integrity scan")
            .is_healthy()
    );

    let output = json!({
        "benchmark_version": 1,
        "build": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpu": linux_value("/proc/cpuinfo", "model name"),
        "memory": linux_value("/proc/meminfo", "MemTotal"),
        "git_version": git(&repository_root, &["--version"]),
        "fixture": {
            "name": "incremental-git-100-v1",
            "initial_commits": 1,
            "new_commits": INCREMENTAL_COMMITS,
            "changed_files_per_commit": 1,
            "change_set_commits": INCREMENTAL_COMMITS
        },
        "protocol": {"warmups":WARMUPS,"samples":SAMPLES,"network_included":false},
        "milliseconds": {
            "incremental_git_ingestion_100_commits": ingestion_elapsed.as_secs_f64() * 1000.0,
            "create_100_commit_change_set": change_set_elapsed.as_secs_f64() * 1000.0,
            "development_resume_state": summarize(resume),
            "deterministic_development_report": summarize(report),
            "checkpoint_load": summarize(checkpoint_load)
        },
        "targets_ms": {
            "incremental_git_ingestion_100_commits": 60000,
            "development_resume_state": 1000,
            "deterministic_development_report": 3000,
            "checkpoint_load": 1000
        },
        "correctness": "100 new commits and file diffs ingested exactly once; ChangeSet created; final integrity scan healthy"
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn initialize_repository(root: &Path) {
    fs::create_dir_all(root).expect("create repository directory");
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Continuum Benchmark"]);
    git(root, &["config", "user.email", "benchmark@example.invalid"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    fs::write(root.join("fixture.txt"), "initial\n").expect("write initial fixture");
    git(root, &["add", "--", "fixture.txt"]);
    git(root, &["commit", "-m", "Initial benchmark state"]);
}

fn seed_incremental_commits(root: &Path) {
    for index in 0..INCREMENTAL_COMMITS {
        fs::write(
            root.join("fixture.txt"),
            format!("incremental state {index:03}\n"),
        )
        .expect("write incremental fixture");
        git(root, &["add", "--", "fixture.txt"]);
        git(
            root,
            &["commit", "-m", &format!("Incremental change {index:03}")],
        );
    }
}

fn command() -> CommandContext {
    CommandContext::new(ActorRef::user("benchmark"))
}

fn git(root: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .output()
        .expect("run Git fixture command");
    assert!(
        output.status.success(),
        "Git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git fixture output is UTF-8")
        .trim()
        .to_owned()
}

fn sample(mut operation: impl FnMut()) -> Vec<Duration> {
    (0..SAMPLES)
        .map(|_| {
            let started = Instant::now();
            operation();
            started.elapsed()
        })
        .collect()
}

fn summarize(mut samples: Vec<Duration>) -> serde_json::Value {
    samples.sort_unstable();
    let percentile = |fraction: f64| {
        let index = ((samples.len() - 1) as f64 * fraction).ceil() as usize;
        samples[index].as_secs_f64() * 1000.0
    };
    json!({
        "p50": percentile(0.50),
        "p95": percentile(0.95),
        "max": samples.last().unwrap().as_secs_f64() * 1000.0
    })
}

fn linux_value(path: &str, key: &str) -> Option<String> {
    fs::read_to_string(path).ok().and_then(|content| {
        content.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| value.trim().to_owned())
        })
    })
}
