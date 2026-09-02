-- CP2.1 closes the contract gaps identified by the post-CP2 architecture audit.
-- Legacy columns are retained only to make the forward migration lossless.

ALTER TABLE entities RENAME COLUMN origin TO legacy_origin;
ALTER TABLE entities ADD COLUMN entity_schema_version INTEGER NOT NULL DEFAULT 1 CHECK(entity_schema_version > 0);
ALTER TABLE entities ADD COLUMN origin_type TEXT NOT NULL DEFAULT 'unknown'
    CHECK(origin_type IN ('user','deterministic','import','external','legacy','ai_proposal','unknown'));
ALTER TABLE entities ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE entities ADD COLUMN created_by TEXT NOT NULL DEFAULT 'continuum-core';
ALTER TABLE entities ADD COLUMN updated_by TEXT NOT NULL DEFAULT 'continuum-core';
ALTER TABLE entities ADD COLUMN archived_at TEXT;
UPDATE entities SET origin_type = CASE legacy_origin
    WHEN 'manual' THEN 'user'
    WHEN 'imported' THEN 'import'
    WHEN 'system' THEN 'deterministic'
    WHEN 'external' THEN 'external'
    WHEN 'legacy' THEN 'legacy'
    ELSE 'unknown'
END;

ALTER TABLE relationships RENAME COLUMN origin TO legacy_origin;
ALTER TABLE relationships ADD COLUMN relation_version INTEGER NOT NULL DEFAULT 1 CHECK(relation_version > 0);
ALTER TABLE relationships ADD COLUMN source_entity_type TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE relationships ADD COLUMN target_entity_type TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE relationships ADD COLUMN status TEXT NOT NULL DEFAULT 'active'
    CHECK(status IN ('active','rejected','superseded','archived'));
ALTER TABLE relationships ADD COLUMN origin_type TEXT NOT NULL DEFAULT 'unknown'
    CHECK(origin_type IN ('user','deterministic','import','external','legacy','ai_proposal','unknown'));
ALTER TABLE relationships ADD COLUMN actor_id TEXT NOT NULL DEFAULT 'continuum-core';
ALTER TABLE relationships ADD COLUMN confidence REAL CHECK(confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0));
ALTER TABLE relationships ADD COLUMN review_state TEXT NOT NULL DEFAULT 'unreviewed'
    CHECK(review_state IN ('unreviewed','accepted','rejected'));
ALTER TABLE relationships ADD COLUMN direct_source_ids_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE relationships ADD COLUMN supersedes_id TEXT REFERENCES relationships(id) ON DELETE RESTRICT;
ALTER TABLE relationships ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
UPDATE relationships SET
    origin_type = CASE legacy_origin
        WHEN 'manual' THEN 'user'
        WHEN 'imported' THEN 'import'
        WHEN 'system' THEN 'deterministic'
        WHEN 'external' THEN 'external'
        WHEN 'legacy' THEN 'legacy'
        ELSE 'unknown'
    END,
    source_entity_type = COALESCE((SELECT entity_type FROM entities WHERE id=source_entity_id), 'unknown'),
    target_entity_type = COALESCE((SELECT entity_type FROM entities WHERE id=target_entity_id), 'unknown'),
    updated_at = created_at;
CREATE TABLE relationships_aligned (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,
    relation_version INTEGER NOT NULL CHECK(relation_version > 0),
    source_entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    source_entity_type TEXT NOT NULL,
    target_entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    target_entity_type TEXT NOT NULL,
    status TEXT NOT NULL CHECK(status IN ('active','rejected','superseded','archived')),
    origin_type TEXT NOT NULL CHECK(origin_type IN ('user','deterministic','import','external','legacy','ai_proposal','unknown')),
    actor_id TEXT NOT NULL,
    confidence REAL CHECK(confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
    review_state TEXT NOT NULL CHECK(review_state IN ('unreviewed','accepted','rejected')),
    direct_source_ids_json TEXT NOT NULL,
    supersedes_id TEXT REFERENCES relationships_aligned(id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
INSERT INTO relationships_aligned(
    id,project_id,relation_type,relation_version,source_entity_id,source_entity_type,
    target_entity_id,target_entity_type,status,origin_type,actor_id,confidence,review_state,
    direct_source_ids_json,supersedes_id,created_at,updated_at
)
SELECT id,project_id,relation_type,relation_version,source_entity_id,source_entity_type,
       target_entity_id,target_entity_type,status,origin_type,actor_id,confidence,review_state,
       direct_source_ids_json,supersedes_id,created_at,updated_at
FROM relationships;
DROP TABLE relationships;
ALTER TABLE relationships_aligned RENAME TO relationships;
CREATE INDEX idx_relationships_source ON relationships(project_id, source_entity_id, relation_type);
CREATE INDEX idx_relationships_target ON relationships(project_id, target_entity_id, relation_type);
CREATE INDEX idx_relationships_supersedes ON relationships(project_id, supersedes_id);

ALTER TABLE artifacts ADD COLUMN classification TEXT NOT NULL DEFAULT 'internal'
    CHECK(classification IN ('public','internal','confidential','secret','never_send'));
ALTER TABLE artifacts ADD COLUMN availability TEXT NOT NULL DEFAULT 'available'
    CHECK(availability IN ('available','unavailable','purged_payload'));
ALTER TABLE artifacts ADD COLUMN origin_type TEXT NOT NULL DEFAULT 'deterministic'
    CHECK(origin_type IN ('user','deterministic','import','external','legacy','ai_proposal','unknown'));
ALTER TABLE artifacts ADD COLUMN metadata_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE artifacts ADD COLUMN unavailable_reason TEXT;
ALTER TABLE artifacts ADD COLUMN created_by TEXT NOT NULL DEFAULT 'continuum-core';
ALTER TABLE artifacts ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
UPDATE artifacts SET updated_at=created_at;
CREATE INDEX idx_artifacts_policy ON artifacts(project_id, classification, availability);

ALTER TABLE commands ADD COLUMN command_type_version INTEGER NOT NULL DEFAULT 1 CHECK(command_type_version > 0);
ALTER TABLE commands ADD COLUMN actor_kind TEXT NOT NULL DEFAULT 'system'
    CHECK(actor_kind IN ('user','system','import','ai_proposal'));
ALTER TABLE commands ADD COLUMN actor_id TEXT NOT NULL DEFAULT 'continuum-core';
ALTER TABLE commands ADD COLUMN expected_version INTEGER;
ALTER TABLE commands ADD COLUMN idempotency_key TEXT;
ALTER TABLE commands ADD COLUMN payload_schema_version INTEGER NOT NULL DEFAULT 1 CHECK(payload_schema_version > 0);
ALTER TABLE commands ADD COLUMN payload_json TEXT NOT NULL DEFAULT '{}';
ALTER TABLE commands ADD COLUMN issued_at TEXT NOT NULL DEFAULT '';
ALTER TABLE commands ADD COLUMN correlation_id TEXT NOT NULL DEFAULT '';
ALTER TABLE commands ADD COLUMN causation_id TEXT;
UPDATE commands SET issued_at=created_at, correlation_id=id WHERE issued_at='' OR correlation_id='';
CREATE UNIQUE INDEX idx_commands_idempotency
    ON commands(project_id, operation, idempotency_key)
    WHERE idempotency_key IS NOT NULL;

DROP TRIGGER audit_events_append_only_update;
DROP TRIGGER audit_events_append_only_delete;
ALTER TABLE audit_events ADD COLUMN aggregate_id TEXT;
ALTER TABLE audit_events ADD COLUMN event_version INTEGER NOT NULL DEFAULT 1 CHECK(event_version > 0);
ALTER TABLE audit_events ADD COLUMN actor_kind TEXT NOT NULL DEFAULT 'system'
    CHECK(actor_kind IN ('user','system','import','ai_proposal'));
ALTER TABLE audit_events ADD COLUMN actor_id TEXT NOT NULL DEFAULT 'continuum-core';
ALTER TABLE audit_events ADD COLUMN correlation_id TEXT NOT NULL DEFAULT '';
ALTER TABLE audit_events ADD COLUMN causation_id TEXT;
UPDATE audit_events SET correlation_id=command_id WHERE correlation_id='';
CREATE TRIGGER audit_events_append_only_update
BEFORE UPDATE ON audit_events BEGIN SELECT RAISE(ABORT, 'audit events are append-only'); END;
CREATE TRIGGER audit_events_append_only_delete
BEFORE DELETE ON audit_events BEGIN SELECT RAISE(ABORT, 'audit events are append-only'); END;

ALTER TABLE outbox ADD COLUMN attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts >= 0);
ALTER TABLE outbox ADD COLUMN lease_until TEXT;
ALTER TABLE outbox ADD COLUMN last_error TEXT;
CREATE INDEX idx_outbox_claim ON outbox(project_id, processed_at, lease_until, created_at);

ALTER TABLE jobs ADD COLUMN progress_current INTEGER NOT NULL DEFAULT 0 CHECK(progress_current >= 0);
ALTER TABLE jobs ADD COLUMN progress_total INTEGER CHECK(progress_total IS NULL OR progress_total >= 0);
ALTER TABLE jobs ADD COLUMN progress_message TEXT;
ALTER TABLE jobs ADD COLUMN cancellation_requested INTEGER NOT NULL DEFAULT 0 CHECK(cancellation_requested IN (0,1));
ALTER TABLE jobs ADD COLUMN created_by TEXT NOT NULL DEFAULT 'continuum-core';
ALTER TABLE jobs ADD COLUMN updated_by TEXT NOT NULL DEFAULT 'continuum-core';

ALTER TABLE checkpoints ADD COLUMN created_by TEXT NOT NULL DEFAULT 'continuum-core';
