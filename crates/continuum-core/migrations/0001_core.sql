CREATE TABLE projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('active', 'archived')),
    lifecycle_version INTEGER NOT NULL DEFAULT 1,
    ledger_sequence INTEGER NOT NULL DEFAULT 0 CHECK (ledger_sequence >= 0),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE space_capabilities (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    space TEXT NOT NULL CHECK (space IN ('research', 'development')),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    updated_at TEXT NOT NULL,
    PRIMARY KEY (project_id, space)
);

CREATE TABLE entities (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    title TEXT NOT NULL,
    status TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1 CHECK (version > 0),
    origin TEXT NOT NULL CHECK (origin IN ('manual','imported','external','legacy','system','unknown')),
    data_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_entities_project_type ON entities(project_id, entity_type, status);

CREATE TABLE relationships (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    relation_type TEXT NOT NULL,
    source_entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    target_entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    origin TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(project_id, relation_type, source_entity_id, target_entity_id)
);
CREATE INDEX idx_relationships_source ON relationships(project_id, source_entity_id, relation_type);
CREATE INDEX idx_relationships_target ON relationships(project_id, target_entity_id, relation_type);

CREATE TABLE artifacts (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    sha256 TEXT NOT NULL CHECK (length(sha256) = 64),
    byte_size INTEGER NOT NULL CHECK (byte_size >= 0),
    media_type TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(project_id, sha256)
);

CREATE TABLE entity_artifacts (
    entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    role TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(entity_id, artifact_id, role)
);

CREATE TABLE commands (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    operation TEXT NOT NULL,
    result_id TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE audit_events (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ledger_sequence INTEGER NOT NULL CHECK (ledger_sequence > 0),
    command_id TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    UNIQUE(project_id, ledger_sequence),
    UNIQUE(project_id, command_id, event_type)
);

CREATE TABLE outbox (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    audit_event_id TEXT NOT NULL REFERENCES audit_events(id) ON DELETE CASCADE,
    topic TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    processed_at TEXT,
    UNIQUE(audit_event_id, topic)
);
CREATE INDEX idx_outbox_pending ON outbox(processed_at, created_at);

CREATE TABLE jobs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    job_type TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('queued','running','succeeded','failed','cancelled')),
    attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
    max_attempts INTEGER NOT NULL CHECK (max_attempts BETWEEN 1 AND 20),
    payload_json TEXT NOT NULL,
    idempotency_key TEXT NOT NULL,
    available_at TEXT NOT NULL,
    lease_until TEXT,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(project_id, job_type, idempotency_key)
);
CREATE INDEX idx_jobs_claim ON jobs(project_id, state, available_at, created_at);

CREATE TABLE checkpoints (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    scope TEXT NOT NULL CHECK (scope IN ('research','development','integrated','core')),
    ledger_sequence INTEGER NOT NULL CHECK (ledger_sequence >= 0),
    summary_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(project_id, id)
);

CREATE TABLE checkpoint_sources (
    checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE RESTRICT,
    source_entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    source_version INTEGER NOT NULL CHECK (source_version > 0),
    PRIMARY KEY(checkpoint_id, source_entity_id)
);

CREATE TABLE context_packs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    scope TEXT NOT NULL,
    source_ledger_sequence INTEGER NOT NULL,
    budget_json TEXT NOT NULL,
    content_json TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE generated_artifacts (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    lifecycle TEXT NOT NULL,
    source_json TEXT NOT NULL,
    artifact_id TEXT REFERENCES artifacts(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TRIGGER checkpoints_immutable_update
BEFORE UPDATE ON checkpoints BEGIN SELECT RAISE(ABORT, 'checkpoints are immutable'); END;
CREATE TRIGGER checkpoints_immutable_delete
BEFORE DELETE ON checkpoints BEGIN SELECT RAISE(ABORT, 'checkpoints are immutable'); END;
CREATE TRIGGER audit_events_append_only_update
BEFORE UPDATE ON audit_events BEGIN SELECT RAISE(ABORT, 'audit events are append-only'); END;
CREATE TRIGGER audit_events_append_only_delete
BEFORE DELETE ON audit_events BEGIN SELECT RAISE(ABORT, 'audit events are append-only'); END;

