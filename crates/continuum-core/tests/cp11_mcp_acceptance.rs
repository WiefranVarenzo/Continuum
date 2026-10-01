use chrono::{Duration, Utc};
use continuum_core::{
    ActorKind, ActorRef, CheckpointScope, CommandContext, ContinuityStore, CoreError,
    DataClassification, ExternalProposalKind, MCP_PROTOCOL_VERSION, McpClassificationCeiling,
    McpClientFamily, McpClientInfo, McpResourceFamily, McpToolName, McpTransport,
    NewExternalProposal, NewMcpClientGrant, NewResearchSession, PageRequest,
    ProposalReviewDecision, Space,
};
use serde_json::json;

fn user() -> CommandContext {
    CommandContext::new(ActorRef::user("cp11-user"))
}

fn project() -> (tempfile::TempDir, ContinuityStore, String) {
    let directory = tempfile::tempdir().unwrap();
    let store = ContinuityStore::create_with_actor(
        directory.path().join("project"),
        "CP11 MCP fixture",
        ActorRef::user("owner"),
    )
    .unwrap();
    store
        .set_space_capability_with_context(&user(), Space::Research, true)
        .unwrap();
    let session_id = store
        .create_research_session(
            &user(),
            NewResearchSession {
                title: "MCP-safe research".into(),
                objective: "Prove permissioned external continuity without canonical AI writes."
                    .into(),
                started_at: None,
                metadata: json!({"classification":"internal"}),
            },
        )
        .unwrap()
        .entity
        .id;
    (directory, store, session_id)
}

fn grant_input(family: McpClientFamily, calls: u32) -> NewMcpClientGrant {
    NewMcpClientGrant {
        client_label: format!("{family:?} acceptance"),
        client_family: family,
        transport: McpTransport::Stdio,
        allowed_scopes: vec![CheckpointScope::Research, CheckpointScope::Core],
        allowed_resources: vec![
            McpResourceFamily::Project,
            McpResourceFamily::Checkpoint,
            McpResourceFamily::Context,
            McpResourceFamily::Entity,
            McpResourceFamily::Research,
            McpResourceFamily::Provenance,
            McpResourceFamily::Schema,
        ],
        allowed_tools: vec![
            McpToolName::ProjectGetState,
            McpToolName::CheckpointGet,
            McpToolName::CheckpointList,
            McpToolName::ContextBuild,
            McpToolName::EntityGet,
            McpToolName::ResearchSearch,
            McpToolName::ProvenanceTrace,
            McpToolName::ProposalSubmit,
            McpToolName::ProposalGetStatus,
        ],
        allow_proposals: true,
        classification_ceiling: McpClassificationCeiling::Internal,
        max_artifact_bytes: 0,
        max_request_bytes: 64 * 1024,
        max_response_bytes: 512 * 1024,
        max_context_tokens: 8_000,
        max_calls_per_minute: calls,
        tool_timeout_ms: 10_000,
        expires_at: (Utc::now() + Duration::days(7)).to_rfc3339(),
    }
}

fn open_access(
    store: &ContinuityStore,
    family: McpClientFamily,
    calls: u32,
    tool: McpToolName,
) -> (
    String,
    continuum_core::McpSession,
    continuum_core::McpAuthorizedRequest,
) {
    let created = store
        .create_mcp_client_grant(&user(), grant_input(family, calls))
        .unwrap();
    let client_name = match family {
        McpClientFamily::Codex => "Codex CLI",
        McpClientFamily::ClaudeCode => "Claude Code",
        McpClientFamily::GeminiCli => "Gemini CLI",
        McpClientFamily::Generic => "Protocol Harness",
    };
    let session = store
        .open_mcp_session(
            &created.bearer_token,
            McpTransport::Stdio,
            continuum_core::MCP_PROTOCOL_VERSION,
            McpClientInfo {
                name: client_name.into(),
                version: "1.0-test".into(),
                capabilities: json!({}),
            },
        )
        .unwrap();
    store
        .activate_mcp_session(&session.id, &created.bearer_token)
        .unwrap();
    let access = store
        .authorize_mcp_request(
            &session.id,
            &created.bearer_token,
            "tools/call",
            tool.as_str(),
            Some(CheckpointScope::Research),
            None,
            Some(tool),
            512,
        )
        .unwrap();
    (created.bearer_token, session, access)
}

#[test]
fn grants_are_human_created_hashed_bounded_and_http_fails_closed() {
    let (_directory, store, _) = project();
    let created = store
        .create_mcp_client_grant(&user(), grant_input(McpClientFamily::Codex, 10))
        .unwrap();
    assert!(created.bearer_token.starts_with("ctmcp_"));
    assert!(
        !serde_json::to_string(&created.grant)
            .unwrap()
            .contains(&created.bearer_token)
    );
    let connection = store.debug_connection().unwrap();
    let stored: String = connection
        .query_row(
            "SELECT token_sha256 FROM mcp_client_grants WHERE id=?1",
            [&created.grant.id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(stored.len(), 64);
    assert_ne!(stored, created.bearer_token);

    let mut ai = user();
    ai.actor.kind = ActorKind::AiProposal;
    assert!(matches!(
        store.create_mcp_client_grant(&ai, grant_input(McpClientFamily::Codex, 10)),
        Err(CoreError::Unauthorized(_))
    ));
    let mut http = grant_input(McpClientFamily::Generic, 10);
    http.transport = McpTransport::StreamableHttp;
    assert!(matches!(
        store.create_mcp_client_grant(&user(), http),
        Err(CoreError::Validation(_))
    ));

    assert!(matches!(
        store.open_mcp_session(
            "wrong-token",
            McpTransport::Stdio,
            MCP_PROTOCOL_VERSION,
            McpClientInfo {
                name: "Codex".into(),
                version: "test".into(),
                capabilities: json!({}),
            },
        ),
        Err(CoreError::Unauthorized(_))
    ));
    let audit = store
        .list_mcp_audit(PageRequest {
            limit: 20,
            offset: 0,
        })
        .unwrap();
    assert!(audit.items.iter().any(|record| {
        record.event_type == "session.rejected"
            && record.outcome == "denied"
            && record.grant_id.is_none()
    }));
    assert!(
        !serde_json::to_string(&audit)
            .unwrap()
            .contains("wrong-token")
    );
}

#[test]
fn codex_claude_gemini_and_generic_profiles_share_one_protocol_contract() {
    for family in [
        McpClientFamily::Codex,
        McpClientFamily::ClaudeCode,
        McpClientFamily::GeminiCli,
        McpClientFamily::Generic,
    ] {
        let (_directory, store, _) = project();
        let (token, session, _) = open_access(&store, family, 10, McpToolName::ProjectGetState);
        assert_eq!(
            session.protocol_version,
            continuum_core::MCP_PROTOCOL_VERSION
        );
        assert_eq!(session.transport, McpTransport::Stdio);
        store
            .close_mcp_session(&session.id, "test complete")
            .unwrap();
        assert!(matches!(
            store.authorize_mcp_request(
                &session.id,
                &token,
                "tools/call",
                McpToolName::ProjectGetState.as_str(),
                Some(CheckpointScope::Research),
                None,
                Some(McpToolName::ProjectGetState),
                20,
            ),
            Err(CoreError::Unauthorized(_))
        ));
    }
}

#[test]
fn revocation_cross_project_scope_tool_and_size_limits_fail_closed() {
    let (_directory, store, _) = project();
    let created = store
        .create_mcp_client_grant(&user(), grant_input(McpClientFamily::Codex, 10))
        .unwrap();
    let session = store
        .open_mcp_session(
            &created.bearer_token,
            McpTransport::Stdio,
            continuum_core::MCP_PROTOCOL_VERSION,
            McpClientInfo {
                name: "Codex".into(),
                version: "test".into(),
                capabilities: json!({}),
            },
        )
        .unwrap();
    store
        .activate_mcp_session(&session.id, &created.bearer_token)
        .unwrap();
    assert!(matches!(
        store.authorize_mcp_request(
            &session.id,
            &created.bearer_token,
            "tools/call",
            "continuum.context.build",
            Some(CheckpointScope::Development),
            None,
            Some(McpToolName::ContextBuild),
            20,
        ),
        Err(CoreError::Unauthorized(_))
    ));
    assert!(matches!(
        store.authorize_mcp_request(
            &session.id,
            &created.bearer_token,
            "tools/call",
            "unknown",
            Some(CheckpointScope::Research),
            None,
            Some(McpToolName::ProjectGetState),
            100_000,
        ),
        Err(CoreError::Validation(_))
    ));

    let (_other_directory, other, _) = project();
    assert!(matches!(
        other.open_mcp_session(
            &created.bearer_token,
            McpTransport::Stdio,
            continuum_core::MCP_PROTOCOL_VERSION,
            McpClientInfo {
                name: "Codex".into(),
                version: "test".into(),
                capabilities: json!({}),
            },
        ),
        Err(CoreError::Unauthorized(_))
    ));

    store
        .revoke_mcp_client_grant(&user(), &created.grant.id)
        .unwrap();
    assert!(matches!(
        store.authorize_mcp_request(
            &session.id,
            &created.bearer_token,
            "tools/list",
            "catalog",
            None,
            None,
            None,
            20,
        ),
        Err(CoreError::Unauthorized(_))
    ));
}

#[test]
fn durable_rate_limit_and_sanitized_audit_are_enforced() {
    let (_directory, store, _) = project();
    let created = store
        .create_mcp_client_grant(&user(), grant_input(McpClientFamily::Generic, 1))
        .unwrap();
    let session = store
        .open_mcp_session(
            &created.bearer_token,
            McpTransport::Stdio,
            continuum_core::MCP_PROTOCOL_VERSION,
            McpClientInfo {
                name: "Harness".into(),
                version: "test".into(),
                capabilities: json!({}),
            },
        )
        .unwrap();
    store
        .activate_mcp_session(&session.id, &created.bearer_token)
        .unwrap();
    store
        .authorize_mcp_request(
            &session.id,
            &created.bearer_token,
            "tools/list",
            "catalog",
            None,
            None,
            None,
            20,
        )
        .unwrap();
    assert!(matches!(
        store.authorize_mcp_request(
            &session.id,
            &created.bearer_token,
            "tools/list",
            "catalog",
            None,
            None,
            None,
            20,
        ),
        Err(CoreError::RateLimited(_))
    ));
    let audit = store
        .list_mcp_audit(PageRequest {
            limit: 20,
            offset: 0,
        })
        .unwrap();
    assert!(audit.items.iter().any(|item| item.outcome == "denied"));
    assert!(
        !serde_json::to_string(&audit)
            .unwrap()
            .contains(&created.bearer_token)
    );
}

#[test]
fn mcp_freeform_markdown_is_a_reviewable_draft_not_a_template_or_canonical_write() {
    let (_directory,store,source_id)=project();
    let (_token,_session,access)=open_access(&store,McpClientFamily::Generic,20,McpToolName::ProposalSubmit);
    let sequence=store.summary().unwrap().ledger_sequence;
    let markdown=format!("# Experiment findings\n\nA source-grounded explanation [Research](continuum://source/{source_id}).\n\n```mermaid\nflowchart LR\n A[Evidence] --> B[Open question]\n```\n");
    let proposal=store.submit_external_proposal(&access,NewExternalProposal {
        idempotency_key:"freeform-report".into(),kind:ExternalProposalKind::ResearchSynthesis,
        scope:CheckpointScope::Research,title:"Professional research report".into(),rationale:"Draft awaiting human review".into(),
        payload:json!({"format":"markdown","research_session_id":source_id,"markdown":markdown,"source_sequence":sequence}),
        source_refs:vec![source_id.clone()],expires_at:(Utc::now()+Duration::days(7)).to_rfc3339(),
    }).unwrap();
    assert_eq!(store.summary().unwrap().ledger_sequence,sequence);
    assert_eq!(proposal.status,"pending");
    let received=store.get_external_proposal(&proposal.id).unwrap();
    assert_eq!(received.payload["markdown"],markdown);
    let key=format!("report:research:{source_id}");
    assert!(store.workspace_document(&key).unwrap().is_none());
    let saved=store.save_workspace_document(&user(),&key,"report",0,&json!({"markdown":received.payload["markdown"],"origin":"mcp","proposalId":proposal.id,"status":"draft"})).unwrap();
    assert_eq!(saved.payload["markdown"],markdown);
    assert_eq!(saved.payload["status"],"draft");
}

#[test]
fn proposal_is_idempotent_review_gated_and_cross_grant_private() {
    let (_directory, store, source_id) = project();
    let (_token, _session, access) = open_access(
        &store,
        McpClientFamily::Generic,
        20,
        McpToolName::ProposalSubmit,
    );
    let before = store.summary().unwrap().ledger_sequence;
    let input = NewExternalProposal {
        idempotency_key: "proposal-1".into(),
        kind: ExternalProposalKind::NextAction,
        scope: CheckpointScope::Research,
        title: "Validate the remaining research gap".into(),
        rationale: "The cited session is active and the next validation is still explicit.".into(),
        payload: json!({"action":"Run the bounded validation"}),
        source_refs: vec![source_id],
        expires_at: (Utc::now() + Duration::days(7)).to_rfc3339(),
    };
    let proposal = store
        .submit_external_proposal(&access, input.clone())
        .unwrap();
    let retry = store.submit_external_proposal(&access, input).unwrap();
    assert_eq!(proposal.id, retry.id);
    assert_eq!(before, store.summary().unwrap().ledger_sequence);
    assert_eq!(proposal.status, "pending");

    let (_token2, _session2, other_access) = open_access(
        &store,
        McpClientFamily::Generic,
        20,
        McpToolName::ProposalGetStatus,
    );
    assert!(matches!(
        store.get_external_proposal_for_access(&other_access, &proposal.id),
        Err(CoreError::Unauthorized(_))
    ));

    let reviewed = store
        .review_external_proposal(
            &user(),
            &proposal.id,
            proposal.version,
            ProposalReviewDecision::Accept,
            "Reviewed source and accepted as a proposal record; materialization remains a normal user command.",
        )
        .unwrap();
    assert_eq!(reviewed.status, "accepted");
    assert!(store.summary().unwrap().ledger_sequence > before);
}

#[test]
fn entity_projection_enforces_classification_scope_and_secret_scan() {
    let (_directory, store, source_id) = project();
    let (_token, _session, access) =
        open_access(&store, McpClientFamily::Generic, 20, McpToolName::EntityGet);
    assert_eq!(
        store
            .mcp_entity_summary(&access, &source_id, CheckpointScope::Research)
            .unwrap()
            .id,
        source_id
    );
    store
        .set_entity_ai_classification(
            &user(),
            &source_id,
            DataClassification::Secret,
            "Must never be exposed to this grant",
        )
        .unwrap();
    assert!(matches!(
        store.mcp_entity_summary(&access, &source_id, CheckpointScope::Research),
        Err(CoreError::Unauthorized(_))
    ));
    assert!(matches!(
        store.mcp_entity_summary(&access, &source_id, CheckpointScope::Development),
        Err(CoreError::Unauthorized(_))
    ));
}

#[test]
fn schema_v11_upgrades_atomically_without_rewriting_project_state() {
    let (_directory, store, source_id) = project();
    let project_root = store.root().to_path_buf();
    let before = store.get_entity(&source_id).unwrap();
    drop(store);
    let database = project_root.join("ledger.sqlite3");
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute_batch(
            "DROP TRIGGER mcp_grant_scope_immutable;
             DROP TRIGGER external_proposals_immutable_payload;
             DROP TRIGGER workspace_revision_no_update;
             DROP TRIGGER workspace_revision_no_delete;
             DROP TABLE workspace_document_revisions;
             DROP TABLE workspace_documents;
             DROP TABLE external_proposals;
             DROP TABLE mcp_audit_log;
             DROP TABLE mcp_rate_buckets;
             DROP TABLE mcp_sessions;
             DROP TABLE mcp_client_grants;
             DELETE FROM schema_migrations WHERE version>=12;",
        )
        .unwrap();
    drop(connection);

    let reopened = ContinuityStore::open(&project_root).unwrap();
    assert_eq!(reopened.get_entity(&source_id).unwrap(), before);
    let connection = reopened.debug_connection().unwrap();
    let version: u32 = connection
        .query_row("SELECT max(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(version, continuum_core::CORE_SCHEMA_VERSION);
    assert!(version >= 13);
}

#[test]
fn synthesis_and_diagram_proposals_are_persisted_for_human_review() {
    let (_directory, store, source_id) = project();
    let (_token, _session, access) = open_access(&store, McpClientFamily::Codex, 20, McpToolName::ProposalSubmit);
    for kind in [ExternalProposalKind::ResearchSynthesis, ExternalProposalKind::DiagramPlan] {
        let proposal = store.submit_external_proposal(&access, NewExternalProposal {
            idempotency_key: format!("presentation-{kind:?}"), kind,
            scope: CheckpointScope::Research, title:"Research presentation".into(),
            rationale:"Evidence-based draft for review".into(), payload:if kind == ExternalProposalKind::ResearchSynthesis {
                json!({"summary":"Review draft","session_id":source_id,"key_points":["Bounded evidence"],"limitations":[],"recommendations":["Verify"]})
            } else {
                json!({"title":"Reasoning","direction":"top_down","nodes":[{"id":"question","label":"Research goal","kind":"research_question","status":"supported","source_ids":[source_id]}],"edges":[],"textual_alternative":"The research goal"})
            },
            source_refs:vec![source_id.clone()], expires_at:(Utc::now()+Duration::days(7)).to_rfc3339(),
        }).unwrap();
        assert_eq!(proposal.status, "pending");
        assert_eq!(store.get_external_proposal(&proposal.id).unwrap().kind, kind);
        store.review_external_proposal(&user(), &proposal.id, proposal.version, ProposalReviewDecision::Accept, "Reviewed fixture").unwrap();
    }
    let document = store.compose_human_document(continuum_core::HumanDocumentRequest::integrated()).unwrap();
    assert_eq!(document.sources.iter().filter(|source| source.source_kind == "external_proposal").count(), 2);
    let saved = store.save_human_document(&user(), &document).unwrap();
    assert!(store.human_document_freshness(&saved.id).unwrap().fresh);
    let rendered = continuum_core::render_human_document(&document).unwrap();
    assert!(rendered.html.contains("Research Synthesis"));
    assert!(rendered.markdown.contains("Review draft"));
}

#[test]
fn integrity_scan_detects_proposal_payload_tampering() {
    let (_directory, store, source_id) = project();
    let (_token, _session, access) = open_access(
        &store,
        McpClientFamily::Generic,
        20,
        McpToolName::ProposalSubmit,
    );
    let proposal = store
        .submit_external_proposal(
            &access,
            NewExternalProposal {
                idempotency_key: "tamper-proposal".into(),
                kind: ExternalProposalKind::ResearchNote,
                scope: CheckpointScope::Research,
                title: "Immutable payload".into(),
                rationale: "Integrity fixture".into(),
                payload: json!({"note":"original"}),
                source_refs: vec![source_id],
                expires_at: (Utc::now() + Duration::days(1)).to_rfc3339(),
            },
        )
        .unwrap();
    let connection = store.debug_connection().unwrap();
    connection
        .execute_batch("DROP TRIGGER external_proposals_immutable_payload;")
        .unwrap();
    connection
        .execute(
            "UPDATE external_proposals SET payload_json='{}' WHERE id=?1",
            [&proposal.id],
        )
        .unwrap();
    let report = store.verify_integrity().unwrap();
    assert!(
        report
            .issues
            .iter()
            .any(|issue| issue.code == "mcp_proposal_fingerprint_mismatch")
    );
}
