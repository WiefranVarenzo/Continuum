use std::io::{BufReader, Cursor};

use chrono::{Duration, Utc};
use continuum_core::{
    ActorRef, CheckpointScope, CommandContext, ContextAudience, ContextBudget, ContextPackRequest,
    ContinuityStore, DataClassification, EvidenceKind, FreshnessRequirement,
    McpClassificationCeiling, McpClientFamily, McpResourceFamily, McpToolName, McpTransport,
    NewEvidence, NewMcpClientGrant, NewResearchQuestion, OriginKind, PageRequest, QuestionKind,
    RetrievalProfile, Space,
};
use continuum_mcp::{ContinuumMcpServer, run_stdio};
use serde_json::{Value, json};

fn fixture(allow_proposals: bool) -> (tempfile::TempDir, ContinuityStore, String) {
    fixture_for_family(allow_proposals, McpClientFamily::Generic)
}

fn fixture_for_family(
    allow_proposals: bool,
    client_family: McpClientFamily,
) -> (tempfile::TempDir, ContinuityStore, String) {
    fixture_for_family_with_artifact_limit(allow_proposals, client_family, 0)
}

fn fixture_for_family_with_artifact_limit(
    allow_proposals: bool,
    client_family: McpClientFamily,
    max_artifact_bytes: u32,
) -> (tempfile::TempDir, ContinuityStore, String) {
    let directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP11 protocol fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    let tools = if allow_proposals {
        vec![
            McpToolName::ProjectGetState,
            McpToolName::ContextBuild,
            McpToolName::ProposalSubmit,
            McpToolName::ProposalGetStatus,
        ]
    } else {
        vec![McpToolName::ProjectGetState, McpToolName::ContextBuild]
    };
    let created = store
        .create_mcp_client_grant(
            &CommandContext::new(ActorRef::user("owner")),
            NewMcpClientGrant {
                client_label: "Generic protocol harness".into(),
                client_family,
                transport: McpTransport::Stdio,
                allowed_scopes: vec![CheckpointScope::Core],
                allowed_resources: vec![
                    McpResourceFamily::Project,
                    McpResourceFamily::Context,
                    McpResourceFamily::Schema,
                ],
                allowed_tools: tools,
                allow_proposals,
                classification_ceiling: McpClassificationCeiling::Internal,
                max_artifact_bytes,
                max_request_bytes: 64 * 1024,
                max_response_bytes: 512 * 1024,
                max_context_tokens: 8_000,
                max_calls_per_minute: 100,
                tool_timeout_ms: 10_000,
                expires_at: (Utc::now() + Duration::days(7)).to_rfc3339(),
            },
        )
        .unwrap();
    (directory, store, created.bearer_token)
}

#[test]
fn codex_claude_code_and_gemini_cli_transcripts_are_protocol_conformant() {
    let fixtures = [
        (McpClientFamily::Codex, include_str!("fixtures/codex.jsonl")),
        (
            McpClientFamily::ClaudeCode,
            include_str!("fixtures/claude-code.jsonl"),
        ),
        (
            McpClientFamily::GeminiCli,
            include_str!("fixtures/gemini-cli.jsonl"),
        ),
    ];
    for (family, transcript) in fixtures {
        let (_directory, store, token) = fixture_for_family(false, family);
        let mut server = ContinuumMcpServer::new(store, token);
        let mut responses = 0;
        for line in transcript.lines().filter(|line| !line.trim().is_empty()) {
            let parsed: Value = serde_json::from_str(line).unwrap();
            let response = server.handle_line(line);
            if parsed.get("id").is_some() {
                let response = response.expect("requests must receive a JSON-RPC response");
                assert_eq!(response["jsonrpc"], "2.0");
                assert!(response.get("error").is_none(), "{family:?}: {response}");
                responses += 1;
            } else {
                assert!(response.is_none(), "notifications have no response");
            }
        }
        assert!(responses >= 4);
    }
}

fn initialize(server: &mut ContinuumMcpServer) -> Value {
    let response = server
        .handle_line(
            &json!({
                "jsonrpc":"2.0","id":1,"method":"initialize","params":{
                    "protocolVersion":"2025-06-18","capabilities":{},
                    "clientInfo":{"name":"Protocol Harness","version":"1.0","title":"CP11"}
                }
            })
            .to_string(),
        )
        .unwrap();
    assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
    assert!(
        response["result"]["instructions"]
            .as_str()
            .unwrap()
            .starts_with("Continuum is a local")
    );
    assert!(
        server
            .handle_line(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string())
            .is_none()
    );
    response
}

#[test]
fn standard_request_metadata_is_accepted_without_relaxing_tool_arguments() {
    let (_directory, store, token) = fixture(false);
    let mut server = ContinuumMcpServer::new(store, token);
    initialize(&mut server);
    let call = |args: Value, meta: Value| json!({"jsonrpc":"2.0","id":17,"method":"tools/call","params":{
        "name":"continuum.project.get_state","arguments":args,"_meta":meta
    }}).to_string();
    let response = server.handle_line(&call(json!({"scope":"core"}), json!({"progressToken":"codex-17","client.extension":{}}))).unwrap();
    assert!(response.get("error").is_none(), "{response}");
    let invalid = server.handle_line(&call(json!({"scope":"core","unexpected":true}), json!({}))).unwrap();
    assert!(invalid.get("error").is_some() || invalid["result"]["isError"] == true);
    let invalid_meta = server.handle_line(&call(json!({"scope":"core"}), json!("invalid"))).unwrap();
    assert!(invalid_meta.get("error").is_some());
}

#[test]
fn lifecycle_catalog_and_bounded_state_work_over_generic_json_rpc() {
    let (_directory, store, token) = fixture(false);
    let mut server = ContinuumMcpServer::new(store, token.clone());
    let initialize_response = initialize(&mut server);
    assert!(!initialize_response.to_string().contains(&token));

    let list = server
        .handle_line(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}).to_string())
        .unwrap();
    let tools = list["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert!(
        tools
            .iter()
            .all(|tool| !tool["name"].as_str().unwrap().contains("proposal"))
    );

    let state = server
        .handle_line(
            &json!({
                "jsonrpc":"2.0","id":3,"method":"tools/call","params":{
                    "name":"continuum.project.get_state","arguments":{"scope":"core"}
                }
            })
            .to_string(),
        )
        .unwrap();
    assert_eq!(state["result"]["isError"], false);
    assert_eq!(
        state["result"]["structuredContent"]["audience"],
        "external_ai"
    );
    assert!(
        state["result"]["structuredContent"]["estimated_tokens"]
            .as_u64()
            .unwrap()
            <= 8_000
    );
}

#[test]
fn malformed_unknown_and_preinitialized_requests_fail_without_crashing() {
    let (_directory, store, token) = fixture(false);
    let mut server = ContinuumMcpServer::new(store, token);
    assert_eq!(server.handle_line("{").unwrap()["error"]["code"], -32700);
    let early = server
        .handle_line(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}).to_string())
        .unwrap();
    assert_eq!(early["error"]["data"]["kind"], "unauthorized");
    initialize(&mut server);
    let unknown = server
        .handle_line(
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"continuum.shell","arguments":{}}}).to_string(),
        )
        .unwrap();
    assert!(unknown.get("error").is_some());
}

#[test]
fn cancellation_is_bounded_and_prevents_a_not_yet_started_request() {
    let (_directory, store, token) = fixture(false);
    let mut server = ContinuumMcpServer::new(store, token);
    initialize(&mut server);
    assert!(
        server
            .handle_line(
                &json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"queued-1"}}).to_string()
            )
            .is_none()
    );
    let cancelled = server
        .handle_line(
            &json!({"jsonrpc":"2.0","id":"queued-1","method":"tools/list","params":{}}).to_string(),
        )
        .unwrap();
    assert_eq!(cancelled["error"]["code"], -32800);
    assert_eq!(cancelled["error"]["data"]["kind"], "cancelled");

    for id in 0..2_000 {
        assert!(
            server
                .handle_line(
                    &json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":format!("unused-{id}")}}).to_string()
                )
                .is_none()
        );
    }
    let ping = server
        .handle_line(&json!({"jsonrpc":"2.0","id":"alive","method":"ping"}).to_string())
        .unwrap();
    assert_eq!(ping["result"], json!({}));
}

#[test]
fn artifact_payload_request_never_rounds_a_small_grant_upward() {
    let (_directory, store, token) =
        fixture_for_family_with_artifact_limit(false, McpClientFamily::Generic, 256);
    let mut server = ContinuumMcpServer::new(store, token);
    initialize(&mut server);
    let response = server
        .handle_line(
            &json!({"jsonrpc":"2.0","id":"artifact","method":"tools/call","params":{"name":"continuum.context.build","arguments":{"task":"Read bounded artifact","scope":"core","include_artifact_content":true}}}).to_string(),
        )
        .unwrap();
    assert_eq!(response["result"]["isError"], true);
    assert_eq!(
        response["result"]["structuredContent"]["error"]["kind"],
        "invalid_request"
    );
}

#[test]
fn research_search_requires_research_scope_before_querying() {
    let directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP11 scope fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    let grant = store
        .create_mcp_client_grant(
            &CommandContext::new(ActorRef::user("owner")),
            NewMcpClientGrant {
                client_label: "Mismatched scope fixture".into(),
                client_family: McpClientFamily::Generic,
                transport: McpTransport::Stdio,
                allowed_scopes: vec![CheckpointScope::Core],
                allowed_resources: vec![McpResourceFamily::Research],
                allowed_tools: vec![McpToolName::ResearchSearch],
                allow_proposals: false,
                classification_ceiling: McpClassificationCeiling::Internal,
                max_artifact_bytes: 0,
                max_request_bytes: 64 * 1024,
                max_response_bytes: 512 * 1024,
                max_context_tokens: 8_000,
                max_calls_per_minute: 100,
                tool_timeout_ms: 10_000,
                expires_at: (Utc::now() + Duration::days(7)).to_rfc3339(),
            },
        )
        .unwrap();
    let mut server = ContinuumMcpServer::new(store, grant.bearer_token);
    initialize(&mut server);
    let response = server
        .handle_line(
            &json!({"jsonrpc":"2.0","id":"search","method":"tools/call","params":{"name":"continuum.research.search","arguments":{"text":"anything"}}}).to_string(),
        )
        .unwrap();
    assert_eq!(response["error"]["data"]["kind"], "unauthorized");
}

#[test]
fn denied_research_and_provenance_results_do_not_disclose_entity_ids() {
    let directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP11 privacy fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    store
        .set_space_capability_with_context(
            &CommandContext::new(ActorRef::user("owner")),
            Space::Research,
            true,
        )
        .unwrap();
    let question = store
        .create_research_question(
            &CommandContext::new(ActorRef::user("owner")),
            NewResearchQuestion {
                title: "Visible research root".into(),
                kind: QuestionKind::Question,
                question: "What may an external AI inspect?".into(),
                context: "Privacy projection fixture".into(),
                desired_outcome: "No denied identifier disclosure".into(),
                priority: 3,
                due_at: None,
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    let evidence = store
        .create_evidence(
            &CommandContext::new(ActorRef::user("owner")),
            NewEvidence {
                title: "Classified evidence marker".into(),
                kind: EvidenceKind::Observation,
                origin: OriginKind::User,
                source_uri: None,
                source_title: Some("Private fixture".into()),
                source_author: Some("owner".into()),
                captured_at: None,
                capture_method: "manual_observation".into(),
                stable_reference: Some("fixture:classified-evidence".into()),
                source_content: Some("content that must remain private".into()),
                annotation: "private annotation".into(),
                summary: "private summary".into(),
                relevance: "privacy test".into(),
                original_artifact_id: None,
                question_id: Some(question.entity.id.clone()),
                session_id: None,
                metadata: json!({}),
            },
        )
        .unwrap();
    store
        .set_entity_ai_classification(
            &CommandContext::new(ActorRef::user("owner")),
            &evidence.entity.id,
            DataClassification::Secret,
            "Never expose through this grant",
        )
        .unwrap();
    let grant = store
        .create_mcp_client_grant(
            &CommandContext::new(ActorRef::user("owner")),
            NewMcpClientGrant {
                client_label: "Privacy protocol fixture".into(),
                client_family: McpClientFamily::Generic,
                transport: McpTransport::Stdio,
                allowed_scopes: vec![CheckpointScope::Research],
                allowed_resources: vec![McpResourceFamily::Research, McpResourceFamily::Provenance],
                allowed_tools: vec![McpToolName::ResearchSearch, McpToolName::ProvenanceTrace],
                allow_proposals: false,
                classification_ceiling: McpClassificationCeiling::Internal,
                max_artifact_bytes: 0,
                max_request_bytes: 64 * 1024,
                max_response_bytes: 512 * 1024,
                max_context_tokens: 8_000,
                max_calls_per_minute: 100,
                tool_timeout_ms: 10_000,
                expires_at: (Utc::now() + Duration::days(7)).to_rfc3339(),
            },
        )
        .unwrap();
    let mut server = ContinuumMcpServer::new(store, grant.bearer_token);
    initialize(&mut server);

    let search = server
        .handle_line(
            &json!({"jsonrpc":"2.0","id":"search-private","method":"tools/call","params":{"name":"continuum.research.search","arguments":{"text":"Classified evidence marker"}}}).to_string(),
        )
        .unwrap();
    let search_text = search.to_string();
    assert!(!search_text.contains(&evidence.entity.id));
    assert!(!search_text.contains("Classified evidence marker"));
    assert!(search_text.contains("omitted by privacy or scope policy"));

    let trace = server
        .handle_line(
            &json!({"jsonrpc":"2.0","id":"trace-private","method":"tools/call","params":{"name":"continuum.provenance.trace","arguments":{"root_entity_ids":[question.entity.id],"scope":"research","direction":"both"}}}).to_string(),
        )
        .unwrap();
    let trace_text = trace.to_string();
    assert!(!trace_text.contains(&evidence.entity.id));
    assert!(!trace_text.contains("Classified evidence marker"));
    assert!(trace_text.contains("omitted by privacy or scope policy"));
}

#[test]
fn unknown_prompt_is_safely_audited_and_resource_suffix_is_rejected() {
    let (_directory, store, token) = fixture(false);
    let observer = store.clone();
    let mut server = ContinuumMcpServer::new(store, token);
    initialize(&mut server);
    let prompt = server
        .handle_line(
            &json!({"jsonrpc":"2.0","id":"prompt","method":"prompts/get","params":{"name":"secret_key=do-not-store"}}).to_string(),
        )
        .unwrap();
    assert!(prompt.get("error").is_some());
    let resource = server
        .handle_line(
            &json!({"jsonrpc":"2.0","id":"resource","method":"resources/read","params":{"uri":"continuum://project/current-state-evil"}}).to_string(),
        )
        .unwrap();
    assert!(resource.get("error").is_some());

    let audit = observer
        .list_mcp_audit(PageRequest {
            limit: 50,
            offset: 0,
        })
        .unwrap();
    let serialized = serde_json::to_string(&audit).unwrap();
    assert!(!serialized.contains("secret_key=do-not-store"));
    assert!(audit.items.iter().any(|item| {
        item.event_type == "request.completed"
            && item.target == "unknown-prompt"
            && item.outcome == "failed"
    }));
}

#[test]
fn context_pack_index_hides_local_tasks_and_filters_non_external_audiences() {
    let (_directory, store, token) = fixture(false);
    for (task, audience) in [
        ("private local task secret", ContextAudience::LocalUser),
        ("approved external task", ContextAudience::ExternalAi),
    ] {
        let pack = store
            .build_context_pack(ContextPackRequest {
                task: task.into(),
                audience,
                consumer_target: "cp11-test".into(),
                scope: CheckpointScope::Core,
                checkpoint_id: None,
                freshness_requirement: FreshnessRequirement::AllowStaleWithWarning,
                retrieval_profile: RetrievalProfile::Resume,
                root_entity_ids: vec![],
                exclude_source_ids: vec![],
                include_artifact_content: false,
                budget: ContextBudget::default(),
            })
            .unwrap();
        store
            .save_context_pack(&CommandContext::new(ActorRef::user("owner")), &pack)
            .unwrap();
    }

    let mut server = ContinuumMcpServer::new(store, token);
    initialize(&mut server);
    let response = server
        .handle_line(
            &json!({"jsonrpc":"2.0","id":"packs","method":"resources/read","params":{"uri":"continuum://context-packs"}}).to_string(),
        )
        .unwrap();
    let text = response["result"]["contents"][0]["text"].as_str().unwrap();
    let index: Value = serde_json::from_str(text).unwrap();
    assert_eq!(index["items"].as_array().unwrap().len(), 1);
    assert!(!text.contains("private local task secret"));
    assert!(!text.contains("approved external task"));
    assert!(!index["items"][0].as_object().unwrap().contains_key("task"));
}

#[test]
fn proposal_tool_returns_review_required_without_claiming_canonical_change() {
    let (_directory, store, token) = fixture(true);
    let before = store.summary().unwrap().ledger_sequence;
    let mut server = ContinuumMcpServer::new(store.clone(), token);
    initialize(&mut server);
    let response = server
        .handle_line(
            &json!({
                "jsonrpc":"2.0","id":"proposal","method":"tools/call","params":{
                    "name":"continuum.proposal.submit","arguments":{
                        "idempotency_key":"wire-proposal-1","kind":"next_action","scope":"core",
                        "title":"Inspect the next checkpoint","rationale":"A bounded next step for user review.",
                        "payload":{"action":"inspect"},"source_refs":[]
                    }
                }
            })
            .to_string(),
        )
        .unwrap();
    let result = &response["result"]["structuredContent"];
    assert_eq!(response["result"]["isError"], false);
    assert_eq!(result["canonical_state_changed"], false);
    assert_eq!(result["review_required"], true);
    assert_eq!(before, store.summary().unwrap().ledger_sequence);
}

#[test]
fn stdio_transport_writes_only_newline_delimited_json_rpc() {
    let (_directory, store, token) = fixture(false);
    let input = [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"Harness","version":"1"}}}).to_string(),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
        json!({"jsonrpc":"2.0","id":2,"method":"resources/list","params":{}}).to_string(),
    ].join("\n") + "\n";
    let mut output = Vec::new();
    run_stdio(
        ContinuumMcpServer::new(store, token),
        BufReader::new(Cursor::new(input)),
        &mut output,
    )
    .unwrap();
    let text = String::from_utf8(output).unwrap();
    let lines = text.lines().collect::<Vec<_>>();
    assert_eq!(lines.len(), 2);
    assert!(
        lines
            .iter()
            .all(|line| serde_json::from_str::<Value>(line).is_ok())
    );
}
