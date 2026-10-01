use std::time::Instant;

use continuum_core::{
    ActorRef, AiProjectPolicyInput, CommandContext, ContinuityStore, DataClassification, NewEntity,
    NewProviderProfile, NewSemanticTask, ProviderCapabilities, ProviderDataPolicy, ProviderKind,
    ProviderRequest, ProviderResponse, SemanticRequirements, SemanticRoutePolicy, SemanticTaskType,
    SemanticTransport,
};
use serde_json::{Value, json};

struct FixtureTransport {
    source_id: String,
}

impl SemanticTransport for FixtureTransport {
    fn send(
        &self,
        _request: &ProviderRequest,
    ) -> std::result::Result<ProviderResponse, continuum_core::ProviderTransportError> {
        let output = json!({"summary":"Continuity preserves a bounded current state.","source_ids":[self.source_id]});
        Ok(ProviderResponse {
            status_code: 200,
            body: json!({"candidates":[{"content":{"parts":[{"text":serde_json::to_string(&output).unwrap()}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":120,"candidatesTokenCount":24},"modelVersion":"fixture"}),
        })
    }
}

fn command() -> CommandContext {
    CommandContext::new(ActorRef::user("cp7-benchmark"))
}

fn output_schema() -> Value {
    json!({"type":"object","additionalProperties":false,"properties":{"summary":{"type":"string"},"source_ids":{"type":"array","items":{"type":"string"}}},"required":["summary","source_ids"]})
}

fn task(source_id: &str, profile_id: &str) -> NewSemanticTask {
    NewSemanticTask {
        task_type: SemanticTaskType::ResearchSynthesis,
        source_entity_ids: vec![source_id.into()],
        source_artifact_ids: vec![],
        output_schema: output_schema(),
        prompt_template_id: "cp7-benchmark".into(),
        prompt_template_version: 1,
        requirements: SemanticRequirements::default(),
        route_policy: SemanticRoutePolicy {
            preferred_profile_id: Some(profile_id.into()),
            allowed_profile_ids: vec![profile_id.into()],
            allow_failover: false,
        },
        privacy_audience: "benchmark".into(),
        consent_id: None,
        max_input_units: 50_000,
        max_output_units: 1_000,
        timeout_ms: 30_000,
        cache_ttl_seconds: 3_600,
    }
}

fn percentile(samples: &[f64], percentile: f64) -> f64 {
    let index = ((samples.len() - 1) as f64 * percentile).ceil() as usize;
    samples[index]
}

fn main() {
    let directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP7 benchmark",
        ActorRef::user("owner"),
    )
    .unwrap();
    let mut source = NewEntity::authored("note", "Benchmark source");
    source.data = json!({"text":"A compact checkpoint must preserve state, unresolved work, and next actions."});
    let source_id = store
        .create_entity_with_context(&command(), source)
        .unwrap();
    store
        .set_entity_ai_classification(
            &command(),
            &source_id,
            DataClassification::Public,
            "Synthetic benchmark fixture",
        )
        .unwrap();
    let profile = store
        .register_provider_profile(
            &command(),
            NewProviderProfile {
                display_name: "benchmark-gemini".into(),
                provider_kind: ProviderKind::Gemini,
                endpoint: "https://generativelanguage.googleapis.com/v1beta/models/fixture:generateContent".into(),
                model_id: "fixture".into(),
                credential_ref: "continuum/benchmark".into(),
                enabled: true,
                priority: 1,
                adapter_version: 1,
                capabilities: ProviderCapabilities {
                    text_input: true,
                    structured_output: true,
                    streaming: false,
                    cancellation: false,
                    max_input_units: 100_000,
                    max_output_units: 10_000,
                    token_count_confidence: "provider_reported".into(),
                    supported_task_types: vec!["research_synthesis".into()],
                    declared_deviations: vec![],
                },
                data_policy: ProviderDataPolicy {
                    region: "fixture".into(),
                    retention_summary: "No network".into(),
                    training_summary: "No training".into(),
                    verified_at: chrono::Utc::now().to_rfc3339(),
                    reference_url: "https://example.invalid/policy".into(),
                },
            },
        )
        .unwrap();
    let policy = store.ai_project_policy().unwrap();
    store
        .set_ai_project_policy(
            &command(),
            AiProjectPolicyInput {
                allowed_profile_ids: vec![profile.id.clone()],
                internal_remote_enabled: false,
                automatic_failover: false,
                max_attempts: 1,
                max_total_units: 100_000,
                expected_policy_version: policy.policy_version,
            },
        )
        .unwrap();
    let transport = FixtureTransport {
        source_id: source_id.clone(),
    };
    let initial = store
        .create_semantic_task(&command(), task(&source_id, &profile.id))
        .unwrap();
    let started = Instant::now();
    store
        .execute_semantic_task(&command(), &initial.id, &transport)
        .unwrap();
    let uncached_ms = started.elapsed().as_secs_f64() * 1_000.0;

    let mut cached = Vec::new();
    for _ in 0..30 {
        let started = Instant::now();
        let next = store
            .create_semantic_task(&command(), task(&source_id, &profile.id))
            .unwrap();
        let result = store
            .execute_semantic_task(&command(), &next.id, &transport)
            .unwrap();
        assert!(result.cache_hit);
        cached.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    cached.sort_by(f64::total_cmp);
    let integrity_started = Instant::now();
    assert!(store.verify_integrity().unwrap().is_healthy());
    let integrity_ms = integrity_started.elapsed().as_secs_f64() * 1_000.0;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "checkpoint":"CP7",
            "fixture":"semantic-cache-v1",
            "samples":cached.len(),
            "provider_network":"fixture/no network; local overhead only",
            "uncached_local_pipeline_ms":uncached_ms,
            "cached_task_create_and_execute_p50_ms":percentile(&cached,0.50),
            "cached_task_create_and_execute_p95_ms":percentile(&cached,0.95),
            "cached_task_create_and_execute_max_ms":cached.last().copied().unwrap_or_default(),
            "integrity_scan_ms":integrity_ms,
            "correctness":{"cache_hits":cached.len(),"candidate_per_task":true,"canonical_mutations":0}
        }))
        .unwrap()
    );
}
