-- CP11 AI Continuity Interface. Authentication secrets are never persisted;
-- only SHA-256 token digests are stored. Every session and request remains
-- project-scoped, revocable, bounded, and auditable.

CREATE TABLE mcp_client_grants (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    client_label TEXT NOT NULL,
    client_family TEXT NOT NULL CHECK(client_family IN (
        'codex','claude_code','gemini_cli','generic'
    )),
    transport TEXT NOT NULL CHECK(transport IN ('stdio','streamable_http')),
    token_sha256 TEXT NOT NULL UNIQUE CHECK(length(token_sha256)=64),
    allowed_scopes_json TEXT NOT NULL,
    allowed_resources_json TEXT NOT NULL,
    allowed_tools_json TEXT NOT NULL,
    allow_proposals INTEGER NOT NULL CHECK(allow_proposals IN (0,1)),
    classification_ceiling TEXT NOT NULL CHECK(classification_ceiling IN (
        'public','internal','sensitive'
    )),
    max_artifact_bytes INTEGER NOT NULL CHECK(max_artifact_bytes BETWEEN 0 AND 1048576),
    max_request_bytes INTEGER NOT NULL CHECK(max_request_bytes BETWEEN 1024 AND 1048576),
    max_response_bytes INTEGER NOT NULL CHECK(max_response_bytes BETWEEN 4096 AND 4194304),
    max_context_tokens INTEGER NOT NULL CHECK(max_context_tokens BETWEEN 256 AND 65536),
    max_calls_per_minute INTEGER NOT NULL CHECK(max_calls_per_minute BETWEEN 1 AND 600),
    tool_timeout_ms INTEGER NOT NULL CHECK(tool_timeout_ms BETWEEN 100 AND 60000),
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    last_used_at TEXT,
    revoked_at TEXT,
    created_by TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK(version > 0)
);
CREATE INDEX idx_mcp_grants_project
    ON mcp_client_grants(project_id,revoked_at,expires_at,created_at,id);

CREATE TABLE mcp_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    grant_id TEXT NOT NULL REFERENCES mcp_client_grants(id) ON DELETE RESTRICT,
    transport TEXT NOT NULL CHECK(transport IN ('stdio','streamable_http')),
    protocol_version TEXT NOT NULL,
    client_name TEXT NOT NULL,
    client_version TEXT NOT NULL,
    client_capabilities_json TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('initializing','active','closed','revoked')),
    opened_at TEXT NOT NULL,
    initialized_at TEXT,
    last_seen_at TEXT NOT NULL,
    closed_at TEXT,
    close_reason TEXT
);
CREATE INDEX idx_mcp_sessions_project
    ON mcp_sessions(project_id,grant_id,state,last_seen_at,id);

CREATE TABLE mcp_rate_buckets (
    grant_id TEXT NOT NULL REFERENCES mcp_client_grants(id) ON DELETE CASCADE,
    minute_epoch INTEGER NOT NULL CHECK(minute_epoch >= 0),
    call_count INTEGER NOT NULL CHECK(call_count >= 0),
    PRIMARY KEY(grant_id,minute_epoch)
);

CREATE TABLE mcp_audit_log (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    grant_id TEXT REFERENCES mcp_client_grants(id) ON DELETE SET NULL,
    session_id TEXT REFERENCES mcp_sessions(id) ON DELETE SET NULL,
    correlation_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    method TEXT NOT NULL,
    target TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK(outcome IN (
        'allowed','succeeded','denied','failed','cancelled','revoked'
    )),
    request_bytes INTEGER NOT NULL CHECK(request_bytes >= 0),
    response_bytes INTEGER NOT NULL CHECK(response_bytes >= 0),
    duration_ms INTEGER NOT NULL CHECK(duration_ms >= 0),
    safe_detail_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL
);
CREATE INDEX idx_mcp_audit_project
    ON mcp_audit_log(project_id,occurred_at,id);
CREATE INDEX idx_mcp_audit_grant
    ON mcp_audit_log(grant_id,occurred_at,id);

CREATE TABLE external_proposals (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    grant_id TEXT NOT NULL REFERENCES mcp_client_grants(id) ON DELETE RESTRICT,
    session_id TEXT REFERENCES mcp_sessions(id) ON DELETE SET NULL,
    idempotency_key TEXT NOT NULL,
    proposal_kind TEXT NOT NULL CHECK(proposal_kind IN (
        'research_note','finding_candidate','decision_candidate',
        'requirement_candidate','relationship_candidate','next_action'
    )),
    scope TEXT NOT NULL CHECK(scope IN ('research','development','integrated','core')),
    title TEXT NOT NULL,
    rationale TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    source_refs_json TEXT NOT NULL,
    payload_fingerprint TEXT NOT NULL CHECK(length(payload_fingerprint)=64),
    status TEXT NOT NULL CHECK(status IN ('pending','accepted','rejected','expired')),
    review_note TEXT,
    reviewed_by TEXT,
    created_at TEXT NOT NULL,
    reviewed_at TEXT,
    expires_at TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK(version > 0),
    UNIQUE(grant_id,idempotency_key)
);
CREATE INDEX idx_external_proposals_project
    ON external_proposals(project_id,status,created_at,id);

CREATE TRIGGER external_proposals_immutable_payload
BEFORE UPDATE OF project_id,grant_id,session_id,idempotency_key,proposal_kind,scope,
                 title,rationale,payload_json,source_refs_json,payload_fingerprint,
                 created_at,expires_at
ON external_proposals
BEGIN SELECT RAISE(ABORT, 'external proposal payload is immutable'); END;

CREATE TRIGGER mcp_grant_scope_immutable
BEFORE UPDATE OF project_id,client_label,client_family,transport,token_sha256,
                 allowed_scopes_json,allowed_resources_json,allowed_tools_json,
                 allow_proposals,classification_ceiling,max_artifact_bytes,
                 max_request_bytes,max_response_bytes,max_context_tokens,
                 max_calls_per_minute,tool_timeout_ms,created_at,expires_at,created_by
ON mcp_client_grants
BEGIN SELECT RAISE(ABORT, 'MCP grant authority is immutable; revoke and replace it'); END;
