use std::fs;
use std::time::{Duration, Instant};

use continuum_core::{CheckpointScope, ContinuityStore, NewEntity, OriginKind, new_id};
use rusqlite::params;
use serde_json::json;

const ENTITY_COUNT: usize = 50_000;
const RELATIONSHIP_COUNT: usize = 150_000;
const ARTIFACT_COUNT: usize = 100;
const WARMUPS: usize = 5;
const SAMPLES: usize = 30;

fn main() {
    let temporary = tempfile::tempdir().expect("benchmark temporary directory");
    let project_root = temporary.path().join("standard-core-fixture");
    let store = ContinuityStore::create(&project_root, "CP2 Standard Core Fixture")
        .expect("create benchmark project");
    seed_standard_metadata(&store);
    for index in 0..ARTIFACT_COUNT {
        let payload = format!("continuum-benchmark-artifact-{index:04}").into_bytes();
        store
            .ingest_artifact(&new_id(), &payload, "application/octet-stream")
            .expect("seed artifact");
    }
    let checkpoint = store
        .create_checkpoint(
            &new_id(),
            CheckpointScope::Core,
            &json!({"fixture": "standard-core", "next_actions": ["continue benchmark"]}),
            &[],
        )
        .expect("seed checkpoint");

    for _ in 0..WARMUPS {
        let _ = ContinuityStore::open(&project_root).expect("warm open");
        let _ = store
            .get_checkpoint(&checkpoint.id)
            .expect("warm checkpoint");
    }

    let open = sample(|| {
        let _ = ContinuityStore::open(&project_root).expect("sample open");
    });
    let writes = sample(|| {
        store
            .create_entity(
                &new_id(),
                NewEntity {
                    entity_type: "benchmark.write".into(),
                    schema_version: 1,
                    title: "Measured entity write".into(),
                    origin: OriginKind::Deterministic,
                    metadata: json!({}),
                    data: json!({"bounded": true}),
                },
            )
            .expect("sample write");
    });
    let checkpoint_load = sample(|| {
        let _ = store
            .get_checkpoint(&checkpoint.id)
            .expect("sample checkpoint");
    });
    let integrity = sample(|| {
        assert!(
            store
                .verify_integrity()
                .expect("sample integrity")
                .is_healthy()
        );
    });

    let output = json!({
        "benchmark_version": 1,
        "build": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "cpu": linux_value("/proc/cpuinfo", "model name"),
        "memory": linux_value("/proc/meminfo", "MemTotal"),
        "fixture": {
            "name": "standard-core-metadata-v1",
            "entities": ENTITY_COUNT,
            "relationships": RELATIONSHIP_COUNT,
            "artifacts": ARTIFACT_COUNT,
            "artifact_payload_note": "CP2 metadata/integrity harness; the CP12 5 GB payload corpus is deferred"
        },
        "protocol": {"warmups": WARMUPS, "samples": SAMPLES, "network_included": false},
        "milliseconds": {
            "warm_project_open": summarize(open),
            "entity_write": summarize(writes),
            "checkpoint_load": summarize(checkpoint_load),
            "integrity_scan_100_artifacts": summarize(integrity)
        },
        "targets_ms": {
            "warm_project_overview": 3000,
            "entity_write": 150,
            "checkpoint_load": 1000
        },
        "correctness": "all sampled operations completed; every integrity scan was healthy"
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}

fn seed_standard_metadata(store: &ContinuityStore) {
    let mut connection = store.debug_connection().expect("fixture connection");
    let tx = connection.transaction().expect("fixture transaction");
    let now = chrono::Utc::now().to_rfc3339();
    {
        let mut insert = tx
            .prepare_cached(
                "INSERT INTO entities(
                    id,project_id,entity_type,title,status,version,legacy_origin,data_json,created_at,updated_at,
                    origin_type,created_by,updated_by
                 ) VALUES(?1,?2,'benchmark.fixture',?3,'active',1,'system','{}',?4,?4,
                          'deterministic','benchmark','benchmark')",
            )
            .expect("prepare entity fixture");
        for index in 0..ENTITY_COUNT {
            insert
                .execute(params![
                    fixture_entity_id(index),
                    store.manifest().project_id,
                    format!("Fixture {index}"),
                    now
                ])
                .expect("insert entity fixture");
        }
    }
    {
        let mut insert = tx
            .prepare_cached(
                "INSERT INTO relationships(
                    id,project_id,relation_type,source_entity_id,target_entity_id,created_at,
                    relation_version,source_entity_type,target_entity_type,status,origin_type,actor_id,
                    review_state,direct_source_ids_json,updated_at
                 ) VALUES(?1,?2,'benchmark_link',?3,?4,?5,
                          1,'benchmark.fixture','benchmark.fixture','active','deterministic','benchmark',
                          'unreviewed','[]',?5)",
            )
            .expect("prepare relationship fixture");
        for index in 0..RELATIONSHIP_COUNT {
            let source = index % ENTITY_COUNT;
            let cycle = index / ENTITY_COUNT;
            let mut target = (index * 17 + cycle + 1) % ENTITY_COUNT;
            if source == target {
                target = (target + 1) % ENTITY_COUNT;
            }
            insert
                .execute(params![
                    format!("fixture-r-{index:08}"),
                    store.manifest().project_id,
                    fixture_entity_id(source),
                    fixture_entity_id(target),
                    now
                ])
                .expect("insert relationship fixture");
        }
    }
    tx.commit().expect("commit fixture");
}

fn fixture_entity_id(index: usize) -> String {
    format!("fixture-e-{index:08}")
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
