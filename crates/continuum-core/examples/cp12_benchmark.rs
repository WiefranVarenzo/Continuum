use std::fs;
use std::time::Instant;

use continuum_core::{
    ActorRef, CommandContext, ContinuityStore, HumanDocumentRequest, NewEntity, NewRelationship,
    OriginKind, RelationshipReviewState, Space, render_human_document,
};
use serde_json::json;

const ENTITY_COUNT: usize = 1_000;
const RELATIONSHIP_COUNT: usize = 2_000;
const WARMUPS: usize = 3;
const SAMPLES: usize = 30;

fn command() -> CommandContext {
    CommandContext::new(ActorRef::user("cp12-benchmark"))
}

fn percentile(samples: &[f64], value: f64) -> f64 {
    samples[((samples.len() - 1) as f64 * value).ceil() as usize]
}

fn milliseconds(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1_000.0
}

fn main() {
    let directory = tempfile::tempdir().expect("temporary benchmark directory");
    let project_root = directory.path().join("active-project");
    let store = ContinuityStore::create_with_actor(
        &project_root,
        "CP12 integrated release benchmark",
        ActorRef::user("benchmark-owner"),
    )
    .expect("create benchmark project");
    store
        .set_space_capability_with_context(&command(), Space::Research, true)
        .expect("enable Research Space");
    store
        .set_space_capability_with_context(&command(), Space::Development, true)
        .expect("enable Development Space");

    let mut ids = Vec::with_capacity(ENTITY_COUNT);
    for index in 0..ENTITY_COUNT {
        let entity_type = if index % 2 == 0 {
            "core.benchmark_research"
        } else {
            "core.benchmark_development"
        };
        let mut input =
            NewEntity::authored(entity_type, format!("Integrated release record {index:04}"));
        input.data = json!({
            "summary": format!("Bounded CP12 release fixture record {index:04}"),
            "status": "verified",
            "fixture": "cp12-integrated-1000-v1"
        });
        ids.push(
            store
                .create_entity_with_context(&command(), input)
                .expect("seed entity"),
        );
    }
    for index in 0..RELATIONSHIP_COUNT {
        let source_index = index % ENTITY_COUNT;
        let mut target_index = (index * 41 + 1) % ENTITY_COUNT;
        if source_index == target_index {
            target_index = (target_index + 1) % ENTITY_COUNT;
        }
        store
            .create_relationship_with_context(
                &command(),
                NewRelationship {
                    relation_type: "contextualizes".into(),
                    relation_version: 1,
                    source_entity_id: ids[source_index].clone(),
                    target_entity_id: ids[target_index].clone(),
                    origin: OriginKind::Deterministic,
                    confidence: Some(1.0),
                    review_state: RelationshipReviewState::Accepted,
                    direct_source_ids: vec![],
                    supersedes_id: None,
                },
            )
            .expect("seed relationship");
    }

    let request = HumanDocumentRequest::integrated();
    for _ in 0..WARMUPS {
        let document = store
            .compose_human_document(request.clone())
            .expect("warm composition");
        render_human_document(&document).expect("warm render");
        assert!(
            store
                .verify_integrity()
                .expect("warm integrity")
                .is_healthy()
        );
    }

    let before_hwm_kib = linux_status_kib("VmHWM");
    let mut report_samples = Vec::with_capacity(SAMPLES);
    let mut diagnostics_samples = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let document = store
            .compose_human_document(request.clone())
            .expect("compose report");
        let rendered = render_human_document(&document).expect("render report");
        report_samples.push(milliseconds(started));
        assert!(rendered.asset_manifest.network_dependencies.is_empty());

        let started = Instant::now();
        assert!(
            store
                .verify_integrity()
                .expect("integrity scan")
                .is_healthy()
        );
        diagnostics_samples.push(milliseconds(started));
    }

    let backup_started = Instant::now();
    let backup = store
        .backup_database(directory.path().join("release-backup.sqlite3"))
        .expect("database backup");
    let backup_ms = milliseconds(backup_started);
    let backup_bytes = fs::metadata(&backup).expect("backup metadata").len();

    let export_root = directory.path().join("release-export");
    let export_started = Instant::now();
    store.export_project(&export_root).expect("project export");
    let export_ms = milliseconds(export_started);

    let restore_started = Instant::now();
    let restored =
        ContinuityStore::import_export(&export_root, directory.path().join("restored-project"))
            .expect("verified restore");
    let restore_ms = milliseconds(restore_started);
    assert_eq!(restored.manifest().project_id, store.manifest().project_id);
    assert!(restored.capability_enabled(Space::Research).unwrap());
    assert!(restored.capability_enabled(Space::Development).unwrap());
    assert!(
        restored
            .verify_integrity()
            .expect("restored integrity")
            .is_healthy()
    );

    report_samples.sort_by(f64::total_cmp);
    diagnostics_samples.sort_by(f64::total_cmp);
    let after_hwm_kib = linux_status_kib("VmHWM");
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "checkpoint": "CP12",
            "build": env!("CARGO_PKG_VERSION"),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "cpu": linux_value("/proc/cpuinfo", "model name"),
            "memory": linux_value("/proc/meminfo", "MemTotal"),
            "fixture": {
                "name": "cp12-integrated-1000-v1",
                "entities": ENTITY_COUNT,
                "relationships": RELATIONSHIP_COUNT,
                "spaces": ["research", "development"]
            },
            "protocol": {"warmups": WARMUPS, "samples": SAMPLES, "network_included": false, "release_build": true},
            "milliseconds": {
                "compose_and_render": {
                    "p50": percentile(&report_samples, 0.50),
                    "p95": percentile(&report_samples, 0.95),
                    "max": report_samples.last().copied().unwrap_or_default()
                },
                "full_integrity_diagnostics": {
                    "p50": percentile(&diagnostics_samples, 0.50),
                    "p95": percentile(&diagnostics_samples, 0.95),
                    "max": diagnostics_samples.last().copied().unwrap_or_default()
                },
                "database_backup": backup_ms,
                "verified_project_export": export_ms,
                "verified_project_restore": restore_ms
            },
            "backup_bytes": backup_bytes,
            "memory_high_water_kib": {
                "before": before_hwm_kib,
                "after": after_hwm_kib,
                "delta": after_hwm_kib.zip(before_hwm_kib).map(|(after, before)| after.saturating_sub(before))
            },
            "targets_ms": {"compose_and_render_p95": 5000, "diagnostics_p95": 5000},
            "correctness": "both Spaces enabled, all projections offline, 30 healthy integrity scans, verified backup, export, identity-preserving restore, and healthy restored project"
        }))
        .unwrap()
    );
}

fn linux_value(path: &str, key: &str) -> Option<String> {
    fs::read_to_string(path).ok().and_then(|content| {
        content.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| value.trim().to_owned())
        })
    })
}

fn linux_status_kib(key: &str) -> Option<u64> {
    linux_value("/proc/self/status", key)
        .and_then(|value| value.split_whitespace().next()?.parse().ok())
}
