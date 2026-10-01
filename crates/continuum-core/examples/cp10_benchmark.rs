use std::fs;
use std::time::Instant;

use continuum_core::{
    ActorRef, CheckpointScope, CheckpointTrigger, CommandContext, ContextAudience, ContextBudget,
    ContextPackRequest, ContinuityStore, CurrentProjectStateRequest, FreshnessRequirement,
    NewEntity, OriginKind, RetrievalProfile, SemanticCheckpointInput,
};
use serde_json::json;

const ENTITIES: usize = 750;
const SAMPLES: usize = 50;

fn command() -> CommandContext {
    CommandContext::new(ActorRef::user("cp10-benchmark"))
}

fn percentile(samples: &[f64], value: f64) -> f64 {
    samples[((samples.len() - 1) as f64 * value).ceil() as usize]
}

fn main() {
    let directory = tempfile::tempdir().expect("temporary benchmark directory");
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP10 context benchmark",
        ActorRef::user("benchmark-owner"),
    )
    .expect("create benchmark project");
    for index in 0..ENTITIES {
        store
            .create_entity_with_context(
                &command(),
                NewEntity {
                    entity_type: "core.benchmark_note".into(),
                    schema_version: 1,
                    title: format!("Continuity benchmark record {index:04}"),
                    origin: OriginKind::User,
                    metadata: json!({"fixture":"cp10-context-750-v1","index":index}),
                    data: json!({
                        "status":"active",
                        "summary":format!("Bounded deterministic context record {index:04}"),
                        "next":"Continue validation"
                    }),
                },
            )
            .expect("create benchmark entity");
    }
    let checkpoint = store
        .create_semantic_checkpoint(
            &command(),
            SemanticCheckpointInput {
                scope: CheckpointScope::Core,
                trigger: CheckpointTrigger::Milestone,
                note: "Benchmark checkpoint after 750 structured records".into(),
                blockers: vec!["None for benchmark".into()],
                risks: vec!["Bounded local hardware fixture".into()],
                next_actions: vec!["Build a deterministic resume pack".into()],
                semantic_candidate_id: None,
                supersedes_checkpoint_id: None,
            },
        )
        .expect("create checkpoint");
    let request = ContextPackRequest {
        task: "Resume the benchmark and continue deterministic validation".into(),
        audience: ContextAudience::LocalUser,
        consumer_target: "cp10-benchmark".into(),
        scope: CheckpointScope::Core,
        checkpoint_id: Some(checkpoint.checkpoint.id.clone()),
        freshness_requirement: FreshnessRequirement::Current,
        retrieval_profile: RetrievalProfile::Resume,
        root_entity_ids: Vec::new(),
        exclude_source_ids: Vec::new(),
        include_artifact_content: false,
        budget: ContextBudget::default(),
    };

    store
        .current_project_state(CurrentProjectStateRequest {
            scope: CheckpointScope::Core,
            checkpoint_id: Some(checkpoint.checkpoint.id.clone()),
        })
        .expect("warm current state");
    let reference_pack = store
        .build_context_pack(request.clone())
        .expect("warm context pack");
    let before_hwm_kib = linux_status_kib("VmHWM");
    let mut state_samples_ms = Vec::with_capacity(SAMPLES);
    let mut pack_samples_ms = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let started = Instant::now();
        let state = store
            .current_project_state(CurrentProjectStateRequest {
                scope: CheckpointScope::Core,
                checkpoint_id: Some(checkpoint.checkpoint.id.clone()),
            })
            .expect("load current state");
        state_samples_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        assert_eq!(
            state.freshness.as_ref().map(|value| value.fresh),
            Some(true)
        );

        let started = Instant::now();
        let pack = store
            .build_context_pack(request.clone())
            .expect("compose context pack");
        pack_samples_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        assert_eq!(pack.request_fingerprint, reference_pack.request_fingerprint);
        assert_eq!(pack.content_fingerprint, reference_pack.content_fingerprint);
    }
    state_samples_ms.sort_by(f64::total_cmp);
    pack_samples_ms.sort_by(f64::total_cmp);
    let after_hwm_kib = linux_status_kib("VmHWM");
    assert!(
        store
            .verify_integrity()
            .expect("integrity scan")
            .is_healthy()
    );

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "checkpoint":"CP10",
            "build":env!("CARGO_PKG_VERSION"),
            "os":std::env::consts::OS,
            "arch":std::env::consts::ARCH,
            "cpu":linux_value("/proc/cpuinfo", "model name"),
            "memory":linux_value("/proc/meminfo", "MemTotal"),
            "fixture":{"name":"core-context-750-v1","entities":ENTITIES,"checkpoint_sources":ENTITIES},
            "protocol":{"warmup":1,"samples":SAMPLES,"network_included":false,"ai_included":false,"release_build":true},
            "current_project_state_ms":{"p50":percentile(&state_samples_ms,0.50),"p95":percentile(&state_samples_ms,0.95),"max":state_samples_ms.last().copied().unwrap_or_default()},
            "context_pack_selection_ms":{"p50":percentile(&pack_samples_ms,0.50),"p95":percentile(&pack_samples_ms,0.95),"max":pack_samples_ms.last().copied().unwrap_or_default()},
            "context_pack":{"included_items":reference_pack.items.len(),"included_bytes":reference_pack.included_bytes,"estimated_tokens":reference_pack.estimated_tokens,"omissions":reference_pack.omissions.len()},
            "memory_high_water_kib":{"before":before_hwm_kib,"after":after_hwm_kib,"delta":after_hwm_kib.zip(before_hwm_kib).map(|(after,before)|after.saturating_sub(before))},
            "targets_ms":{"current_project_state_p95":1000,"context_pack_selection_p95":2000},
            "correctness":"stable request/content fingerprints across 50 runs, fresh source boundary, bounded Context Pack, and healthy final integrity scan"
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
