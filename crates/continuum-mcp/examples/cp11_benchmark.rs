use std::fs;
use std::time::{Duration, Instant};

use chrono::{Duration as ChronoDuration, Utc};
use continuum_core::{
    ActorRef, CheckpointScope, CommandContext, ContinuityStore, McpClassificationCeiling,
    McpClientFamily, McpResourceFamily, McpToolName, McpTransport, NewMcpClientGrant, new_id,
};
use continuum_mcp::ContinuumMcpServer;
use serde_json::{Value, json};

fn percentile(mut values: Vec<Duration>, percent: usize) -> f64 {
    values.sort_unstable();
    let index = ((values.len() - 1) * percent).div_ceil(100);
    values[index].as_secs_f64() * 1_000.0
}

fn resident_kib() -> u64 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("VmRSS:")?
                    .split_whitespace()
                    .next()?
                    .parse()
                    .ok()
            })
        })
        .unwrap_or(0)
}

fn request(server: &mut ContinuumMcpServer, value: Value) -> Duration {
    let started = Instant::now();
    let response = server
        .handle_line(&value.to_string())
        .expect("benchmark request must receive a response");
    assert!(response.get("error").is_none(), "{response}");
    started.elapsed()
}

fn main() {
    let root = std::env::temp_dir().join(format!("continuum-cp11-benchmark-{}", new_id()));
    let store = ContinuityStore::create_with_actor(
        &root,
        "CP11 benchmark",
        ActorRef::user("benchmark-owner"),
    )
    .expect("create benchmark project");
    let grant = store
        .create_mcp_client_grant(
            &CommandContext::new(ActorRef::user("benchmark-owner")),
            NewMcpClientGrant {
                client_label: "CP11 benchmark harness".into(),
                client_family: McpClientFamily::Generic,
                transport: McpTransport::Stdio,
                allowed_scopes: vec![CheckpointScope::Core],
                allowed_resources: vec![
                    McpResourceFamily::Project,
                    McpResourceFamily::Context,
                    McpResourceFamily::Schema,
                ],
                allowed_tools: vec![McpToolName::ProjectGetState, McpToolName::ContextBuild],
                allow_proposals: false,
                classification_ceiling: McpClassificationCeiling::Internal,
                max_artifact_bytes: 0,
                max_request_bytes: 64 * 1024,
                max_response_bytes: 512 * 1024,
                max_context_tokens: 8_000,
                max_calls_per_minute: 600,
                tool_timeout_ms: 10_000,
                expires_at: (Utc::now() + ChronoDuration::days(1)).to_rfc3339(),
            },
        )
        .expect("create benchmark grant");
    let baseline_kib = resident_kib();
    let mut server = ContinuumMcpServer::new(store, grant.bearer_token);
    let initialize_ms = request(
        &mut server,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"Benchmark","version":"1"}}}),
    )
    .as_secs_f64()
        * 1_000.0;
    assert!(
        server
            .handle_line(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string())
            .is_none()
    );

    for id in 0..10 {
        request(
            &mut server,
            json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":{}}),
        );
        request(
            &mut server,
            json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"continuum.project.get_state","arguments":{"scope":"core"}}}),
        );
    }
    let mut catalog = Vec::new();
    let mut state = Vec::new();
    for id in 0..100 {
        catalog.push(request(
            &mut server,
            json!({"jsonrpc":"2.0","id":id,"method":"tools/list","params":{}}),
        ));
        state.push(request(
            &mut server,
            json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":"continuum.project.get_state","arguments":{"scope":"core"}}}),
        ));
    }
    let peak_kib = resident_kib();
    let build_profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    let output = json!({
        "checkpoint":"CP11",
        "date":"2026-09-10",
        "profile":format!("{build_profile} local single-process STDIO JSON-RPC; 10 warmups + 100 measured operations"),
        "target_hardware":"AMD Ryzen 5 5600H / 16 GB RAM",
        "initialize_ms":initialize_ms,
        "tool_catalog":{"p50_ms":percentile(catalog.clone(),50),"p95_ms":percentile(catalog,95),"target_p95_ms":50},
        "bounded_current_state":{"p50_ms":percentile(state.clone(),50),"p95_ms":percentile(state,95),"target_p95_ms":500},
        "resident_memory":{"baseline_kib":baseline_kib,"peak_kib":peak_kib,"delta_kib":peak_kib.saturating_sub(baseline_kib),"target_delta_kib":131072}
    });
    println!("{}", serde_json::to_string_pretty(&output).unwrap());
    drop(server);
    let _ = fs::remove_dir_all(&root);
}
