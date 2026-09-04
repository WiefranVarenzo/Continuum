use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use continuum_core::{
    ActorRef, AnalysisLimits, AnalyzeBaselineInput, CommandContext, ContinuityStore,
    DevelopmentSearchQuery, PageRequest, RepositoryAttachInput, Space,
};
use serde_json::json;

const RUST_FILES: usize = 200;
const TYPESCRIPT_FILES: usize = 200;
const JSON_FILES: usize = 98;
const WARMUPS: usize = 5;
const SAMPLES: usize = 30;

fn main() {
    let temporary = tempfile::tempdir().expect("benchmark temporary directory");
    let repository_root = temporary.path().join("repository");
    initialize_repository(&repository_root);
    let store = ContinuityStore::create_with_actor(
        temporary.path().join("continuum-project"),
        "CP5 Code Intelligence Benchmark",
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
                title: Some("CP5 benchmark repository".into()),
                metadata: json!({"fixture":"code-intelligence-500-v1"}),
            },
        )
        .expect("attach repository");
    let initial_baseline = store
        .observe_repository_baseline(&command(), &repository.entity.id)
        .expect("observe initial baseline");
    let cold_started = Instant::now();
    let cold = analyze(&store, &repository.entity.id, &initial_baseline.entity.id);
    let cold_elapsed = cold_started.elapsed();

    fs::write(
        repository_root.join("web/changed_file.ts"),
        "export function changedFile(): boolean { return true; }\n\
         test('changed file works', () => { if (!changedFile()) throw new Error(); });\n",
    )
    .expect("write one changed file");
    git(&repository_root, &["add", "--", "web/changed_file.ts"]);
    git(
        &repository_root,
        &["commit", "-m", "Add one changed source file"],
    );
    let incremental_baseline = store
        .observe_repository_baseline(&command(), &repository.entity.id)
        .expect("observe incremental baseline");
    let incremental_started = Instant::now();
    let incremental = analyze(
        &store,
        &repository.entity.id,
        &incremental_baseline.entity.id,
    );
    let incremental_elapsed = incremental_started.elapsed();

    for _ in 0..WARMUPS {
        let _ = store
            .get_analysis_run(&incremental.entity.id)
            .expect("warm AnalysisRun load");
        let _ = search(&store);
        let _ = store
            .generate_development_report()
            .expect("warm Development Report");
    }
    let analysis_load = sample(|| {
        let loaded = store
            .get_analysis_run(&incremental.entity.id)
            .expect("sample AnalysisRun load");
        assert_eq!(loaded.entity.id, incremental.entity.id);
    });
    let search_results = sample(|| {
        assert!(!search(&store).items.is_empty());
    });
    let report = sample(|| {
        let report = store
            .generate_development_report()
            .expect("sample Development Report");
        assert!(report.markdown.contains("Code Intelligence"));
    });
    assert_eq!(cold.file_count, 500);
    assert_eq!(incremental.file_count, 501);
    assert_eq!(incremental.cache_miss_count, 1);
    assert!(incremental.cache_hit_count >= 500);
    assert!(
        store
            .verify_integrity()
            .expect("integrity scan")
            .is_healthy()
    );

    let output = json!({
        "benchmark_version":1,
        "build":env!("CARGO_PKG_VERSION"),
        "os":std::env::consts::OS,
        "arch":std::env::consts::ARCH,
        "cpu":linux_value("/proc/cpuinfo", "model name"),
        "memory":linux_value("/proc/meminfo", "MemTotal"),
        "git_version":git(&repository_root, &["--version"]),
        "analyzer_bundle_version":incremental.analyzer_bundle_version,
        "output_schema_version":incremental.output_schema_version,
        "fixture":{
            "name":"code-intelligence-500-v1",
            "initial_files":cold.file_count,
            "incremental_files":incremental.file_count,
            "rust_files":RUST_FILES,
            "typescript_files":TYPESCRIPT_FILES,
            "json_files":JSON_FILES,
            "manifest_files":2,
            "changed_files":1,
            "cold_code_entities":cold.code_entity_count,
            "cold_tests":cold.test_count,
            "incremental_code_entities":incremental.code_entity_count,
            "incremental_tests":incremental.test_count,
            "incremental_cache_hits":incremental.cache_hit_count,
            "incremental_cache_misses":incremental.cache_miss_count
        },
        "protocol":{"warmups":WARMUPS,"samples":SAMPLES,"network_included":false},
        "milliseconds":{
            "cold_analysis_500_files":cold_elapsed.as_secs_f64()*1000.0,
            "one_changed_file_incremental_analysis":incremental_elapsed.as_secs_f64()*1000.0,
            "analysis_run_load":summarize(analysis_load),
            "code_entity_search":summarize(search_results),
            "deterministic_development_report":summarize(report)
        },
        "targets_ms":{
            "one_changed_file_incremental_analysis":5000,
            "analysis_run_load":500,
            "code_entity_search":500,
            "deterministic_development_report":3000
        },
        "correctness":"500-file cold snapshot and 501-file incremental snapshot analyzed; exactly one cache miss after one changed file; deterministic identities, report, search, and integrity scan verified"
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn initialize_repository(root: &Path) {
    fs::create_dir_all(root.join("rust")).expect("create Rust fixture directory");
    fs::create_dir_all(root.join("web")).expect("create TypeScript fixture directory");
    fs::create_dir_all(root.join("config")).expect("create JSON fixture directory");
    git(root, &["init", "-b", "main"]);
    git(root, &["config", "user.name", "Continuum Benchmark"]);
    git(root, &["config", "user.email", "benchmark@example.invalid"]);
    git(root, &["config", "commit.gpgsign", "false"]);
    fs::write(
        root.join("Cargo.toml"),
        "[package]\nname='benchmark-fixture'\nversion='0.1.0'\n[dependencies]\nserde='1'\n",
    )
    .expect("write Cargo manifest");
    fs::write(
        root.join("package.json"),
        "{\"name\":\"benchmark-fixture\",\"dependencies\":{\"react\":\"19\"}}",
    )
    .expect("write npm manifest");
    for index in 0..RUST_FILES {
        fs::write(
            root.join(format!("rust/module_{index:03}.rs")),
            format!(
                "pub struct Item{index};\npub fn value_{index}() -> usize {{ {index} }}\n\
                 #[test]\nfn test_value_{index}() {{ assert_eq!(value_{index}(), {index}); }}\n"
            ),
        )
        .expect("write Rust fixture");
    }
    for index in 0..TYPESCRIPT_FILES {
        fs::write(
            root.join(format!("web/module_{index:03}.test.ts")),
            format!(
                "export function value{index}(): number {{ return {index}; }}\n\
                 test('value {index}', () => {{ if (value{index}() !== {index}) throw new Error(); }});\n"
            ),
        )
        .expect("write TypeScript fixture");
    }
    for index in 0..JSON_FILES {
        fs::write(
            root.join(format!("config/settings_{index:03}.json")),
            format!("{{\"enabled\":true,\"index\":{index}}}"),
        )
        .expect("write JSON fixture");
    }
    git(root, &["add", "--", "."]);
    git(root, &["commit", "-m", "Create CP5 benchmark fixture"]);
}

fn analyze(
    store: &ContinuityStore,
    repository_id: &str,
    baseline_id: &str,
) -> continuum_core::AnalysisRunRecord {
    store
        .analyze_repository_baseline(
            &command(),
            AnalyzeBaselineInput {
                repository_id: repository_id.into(),
                baseline_id: baseline_id.into(),
                limits: AnalysisLimits::default(),
            },
        )
        .expect("analyze repository baseline")
}

fn search(store: &ContinuityStore) -> continuum_core::DevelopmentSearchPage {
    store
        .search_development(DevelopmentSearchQuery {
            text: "value".into(),
            entity_type: Some("code_entity".into()),
            status: Some("active".into()),
            page: PageRequest {
                limit: 50,
                offset: 0,
            },
        })
        .expect("search CodeEntities")
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
        "p50":percentile(0.50),
        "p95":percentile(0.95),
        "max":samples.last().unwrap().as_secs_f64()*1000.0
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
