-- Extend proposal kinds without modifying the checksummed CP11 migration.
-- This runs transactionally after the normal pre-migration database backup.
CREATE TABLE external_proposals_next (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    grant_id TEXT NOT NULL REFERENCES mcp_client_grants(id) ON DELETE RESTRICT,
    session_id TEXT REFERENCES mcp_sessions(id) ON DELETE SET NULL,
    idempotency_key TEXT NOT NULL,
    proposal_kind TEXT NOT NULL CHECK(proposal_kind IN (
        'research_note','finding_candidate','decision_candidate',
        'requirement_candidate','relationship_candidate','next_action',
        'research_synthesis','diagram_plan'
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
INSERT INTO external_proposals_next SELECT * FROM external_proposals;
DROP TABLE external_proposals;
ALTER TABLE external_proposals_next RENAME TO external_proposals;
CREATE INDEX idx_external_proposals_project
    ON external_proposals(project_id,status,created_at,id);
CREATE TRIGGER external_proposals_immutable_payload
BEFORE UPDATE OF project_id,grant_id,session_id,idempotency_key,proposal_kind,scope,
                 title,rationale,payload_json,source_refs_json,payload_fingerprint,
                 created_at,expires_at
ON external_proposals
BEGIN SELECT RAISE(ABORT, 'external proposal payload is immutable'); END;
