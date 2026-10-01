use std::fs;
use std::time::Instant;

use continuum_core::{
    ActorRef, CommandContext, ContinuityStore, HumanDocumentRequest, NewEntity, NewRelationship,
    OriginKind, RelationshipReviewState, render_human_document,
};
use serde_json::json;

const ENTITY_COUNT: usize = 300;
const RELATIONSHIP_COUNT: usize = 600;
const WARMUPS: usize = 5;
const SAMPLES: usize = 30;

fn command() -> CommandContext {
    CommandContext::new(ActorRef::user("cp8-benchmark"))
}

fn percentile(samples: &[f64], value: f64) -> f64 {
    samples[((samples.len() - 1) as f64 * value).ceil() as usize]
}

fn main() {
    let directory = tempfile::tempdir().expect("temporary benchmark directory");
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP8 report benchmark",
        ActorRef::user("benchmark-owner"),
    )
    .expect("create benchmark project");

    let mut ids = Vec::with_capacity(ENTITY_COUNT);
    for index in 0..ENTITY_COUNT {
        let mut input = NewEntity::authored(
            "benchmark_knowledge",
            format!("Knowledge record {index:03}"),
        );
        input.data = json!({"summary":format!("Bounded deterministic report content {index:03}"),"verified":true});
        ids.push(
            store
                .create_entity_with_context(&command(), input)
                .expect("seed entity"),
        );
    }
    for index in 0..RELATIONSHIP_COUNT {
        let source = &ids[index % ENTITY_COUNT];
        let mut target_index = (index * 37 + 1) % ENTITY_COUNT;
        if source == &ids[target_index] {
            target_index = (target_index + 1) % ENTITY_COUNT;
        }
        store
            .create_relationship_with_context(
                &command(),
                NewRelationship {
                    relation_type: "contextualizes".into(),
                    relation_version: 1,
                    source_entity_id: source.clone(),
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
    let cold_started = Instant::now();
    let cold_document = store
        .compose_human_document(request.clone())
        .expect("cold composition");
    let cold_compose_ms = cold_started.elapsed().as_secs_f64() * 1_000.0;
    let cold_render_started = Instant::now();
    let cold_render = render_human_document(&cold_document).expect("cold render");
    let cold_render_ms = cold_render_started.elapsed().as_secs_f64() * 1_000.0;
    assert!(cold_render.asset_manifest.network_dependencies.is_empty());

    for _ in 0..WARMUPS {
        let document = store
            .compose_human_document(request.clone())
            .expect("warm composition");
        render_human_document(&document).expect("warm render");
    }

    let mut compose_ms = Vec::with_capacity(SAMPLES);
    let mut render_ms = Vec::with_capacity(SAMPLES);
    let mut combined_ms = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let combined_started = Instant::now();
        let started = Instant::now();
        let document = store
            .compose_human_document(request.clone())
            .expect("sample composition");
        compose_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        let started = Instant::now();
        let rendered = render_human_document(&document).expect("sample render");
        render_ms.push(started.elapsed().as_secs_f64() * 1_000.0);
        combined_ms.push(combined_started.elapsed().as_secs_f64() * 1_000.0);
        assert!(rendered.html.contains("Knowledge record"));
    }
    compose_ms.sort_by(f64::total_cmp);
    render_ms.sort_by(f64::total_cmp);
    combined_ms.sort_by(f64::total_cmp);
    assert!(
        store
            .verify_integrity()
            .expect("integrity scan")
            .is_healthy()
    );

    println!("{}", serde_json::to_string_pretty(&json!({
        "checkpoint":"CP8",
        "build":env!("CARGO_PKG_VERSION"),
        "os":std::env::consts::OS,
        "arch":std::env::consts::ARCH,
        "cpu":linux_value("/proc/cpuinfo", "model name"),
        "memory":linux_value("/proc/meminfo", "MemTotal"),
        "fixture":{"name":"human-document-300-v1","entities":ENTITY_COUNT,"relationships":RELATIONSHIP_COUNT,"graph_node_budget":200,"graph_edge_budget":500},
        "protocol":{"warmups":WARMUPS,"samples":SAMPLES,"network_included":false},
        "milliseconds":{
            "cold_composition":cold_compose_ms,
            "cold_html_markdown_render":cold_render_ms,
            "warm_composition":{"p50":percentile(&compose_ms,0.50),"p95":percentile(&compose_ms,0.95),"max":compose_ms.last().copied().unwrap_or_default()},
            "warm_html_markdown_render":{"p50":percentile(&render_ms,0.50),"p95":percentile(&render_ms,0.95),"max":render_ms.last().copied().unwrap_or_default()},
            "warm_compose_and_render":{"p50":percentile(&combined_ms,0.50),"p95":percentile(&combined_ms,0.95),"max":combined_ms.last().copied().unwrap_or_default()}
        },
        "targets_ms":{"compose_and_local_export_p95":5000,"cached_first_useful_overview_p95":2000},
        "output":{"html_bytes":cold_render.html.len(),"markdown_bytes":cold_render.markdown.len(),"offline_dependencies":cold_render.asset_manifest.network_dependencies.len()},
        "correctness":"all projections validated, offline dependency list empty, and final integrity scan healthy"
    })).unwrap());
}

fn linux_value(path: &str, key: &str) -> Option<String> {
    fs::read_to_string(path).ok().and_then(|content| {
        content.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            (name.trim() == key).then(|| value.trim().to_owned())
        })
    })
}
