-- CP10 Checkpoint & Context Engine. Existing CP2/CP3/CP5 checkpoints and the
-- original context_packs table remain intact; versioned envelopes extend them
-- without changing stable IDs or rewriting historical rows.

CREATE TABLE checkpoint_envelopes (
    checkpoint_id TEXT PRIMARY KEY REFERENCES checkpoints(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL CHECK(schema_version > 0),
    trigger_type TEXT NOT NULL CHECK(trigger_type IN (
        'manual','interruption','handoff','milestone','policy'
    )),
    deterministic_json TEXT NOT NULL,
    semantic_candidate_id TEXT REFERENCES ai_candidates(id) ON DELETE RESTRICT,
    semantic_snapshot_json TEXT,
    privacy_policy_version TEXT NOT NULL,
    privacy_snapshot_json TEXT NOT NULL,
    repository_snapshot_json TEXT NOT NULL,
    capture_snapshot_json TEXT NOT NULL,
    material_fingerprint TEXT NOT NULL CHECK(length(material_fingerprint)=64),
    supersedes_checkpoint_id TEXT REFERENCES checkpoints(id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL
);
CREATE INDEX idx_checkpoint_envelopes_project
    ON checkpoint_envelopes(project_id,created_at,checkpoint_id);

CREATE TABLE checkpoint_artifact_sources (
    checkpoint_id TEXT NOT NULL REFERENCES checkpoints(id) ON DELETE RESTRICT,
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    sha256 TEXT NOT NULL CHECK(length(sha256)=64),
    availability TEXT NOT NULL CHECK(availability IN ('available','unavailable','purged_payload')),
    classification TEXT NOT NULL CHECK(classification IN (
        'public','internal','confidential','sensitive','secret','never_send'
    )),
    PRIMARY KEY(checkpoint_id,artifact_id)
);

CREATE TABLE context_pack_records (
    context_pack_id TEXT PRIMARY KEY REFERENCES context_packs(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    checkpoint_id TEXT REFERENCES checkpoints(id) ON DELETE RESTRICT,
    schema_version INTEGER NOT NULL CHECK(schema_version > 0),
    request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint)=64),
    task TEXT NOT NULL,
    audience TEXT NOT NULL CHECK(audience IN ('local_user','external_ai','public_portable')),
    consumer_target TEXT NOT NULL,
    privacy_policy_version TEXT NOT NULL,
    freshness_requirement TEXT NOT NULL CHECK(freshness_requirement IN (
        'current','allow_stale_with_warning'
    )),
    retrieval_profile TEXT NOT NULL CHECK(retrieval_profile IN (
        'resume','research','development','integrated','custom'
    )),
    estimator_id TEXT NOT NULL,
    estimator_uncertainty TEXT NOT NULL CHECK(estimator_uncertainty IN ('low','medium','high')),
    included_bytes INTEGER NOT NULL CHECK(included_bytes >= 0),
    estimated_tokens INTEGER NOT NULL CHECK(estimated_tokens >= 0),
    content_fingerprint TEXT NOT NULL CHECK(length(content_fingerprint)=64),
    omissions_json TEXT NOT NULL,
    unavailable_sources_json TEXT NOT NULL,
    source_snapshot_json TEXT NOT NULL,
    created_by TEXT NOT NULL
);
CREATE INDEX idx_context_pack_records_project
    ON context_pack_records(project_id,checkpoint_id,context_pack_id);

CREATE TABLE context_pack_sources (
    context_pack_id TEXT NOT NULL REFERENCES context_packs(id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    tier INTEGER NOT NULL CHECK(tier BETWEEN 1 AND 4),
    source_kind TEXT NOT NULL CHECK(source_kind IN (
        'project_state','checkpoint','entity','relationship','artifact','event','capture'
    )),
    source_id TEXT NOT NULL,
    source_version TEXT NOT NULL,
    classification TEXT NOT NULL CHECK(classification IN (
        'public','internal','sensitive','secret','never_send'
    )),
    content_fingerprint TEXT NOT NULL CHECK(length(content_fingerprint)=64),
    PRIMARY KEY(context_pack_id,ordinal),
    UNIQUE(context_pack_id,source_kind,source_id)
);

CREATE TABLE context_pack_generated_artifacts (
    context_pack_id TEXT PRIMARY KEY REFERENCES context_packs(id) ON DELETE RESTRICT,
    generated_artifact_id TEXT NOT NULL UNIQUE REFERENCES generated_artifacts(id) ON DELETE RESTRICT
);

CREATE TRIGGER checkpoint_sources_immutable_update
BEFORE UPDATE ON checkpoint_sources
BEGIN SELECT RAISE(ABORT, 'checkpoint sources are immutable'); END;
CREATE TRIGGER checkpoint_sources_immutable_delete
BEFORE DELETE ON checkpoint_sources
BEGIN SELECT RAISE(ABORT, 'checkpoint sources are immutable'); END;
CREATE TRIGGER checkpoint_envelopes_immutable_update
BEFORE UPDATE ON checkpoint_envelopes
BEGIN SELECT RAISE(ABORT, 'checkpoint envelopes are immutable'); END;
CREATE TRIGGER checkpoint_envelopes_immutable_delete
BEFORE DELETE ON checkpoint_envelopes
BEGIN SELECT RAISE(ABORT, 'checkpoint envelopes are immutable'); END;
CREATE TRIGGER checkpoint_artifact_sources_immutable_update
BEFORE UPDATE ON checkpoint_artifact_sources
BEGIN SELECT RAISE(ABORT, 'checkpoint artifact sources are immutable'); END;
CREATE TRIGGER checkpoint_artifact_sources_immutable_delete
BEFORE DELETE ON checkpoint_artifact_sources
BEGIN SELECT RAISE(ABORT, 'checkpoint artifact sources are immutable'); END;
CREATE TRIGGER context_packs_immutable_update
BEFORE UPDATE ON context_packs
BEGIN SELECT RAISE(ABORT, 'saved context packs are immutable'); END;
CREATE TRIGGER context_packs_immutable_delete
BEFORE DELETE ON context_packs
BEGIN SELECT RAISE(ABORT, 'saved context packs are immutable'); END;
CREATE TRIGGER context_pack_records_immutable_update
BEFORE UPDATE ON context_pack_records
BEGIN SELECT RAISE(ABORT, 'context pack records are immutable'); END;
CREATE TRIGGER context_pack_records_immutable_delete
BEFORE DELETE ON context_pack_records
BEGIN SELECT RAISE(ABORT, 'context pack records are immutable'); END;
CREATE TRIGGER context_pack_sources_immutable_update
BEFORE UPDATE ON context_pack_sources
BEGIN SELECT RAISE(ABORT, 'context pack sources are immutable'); END;
CREATE TRIGGER context_pack_sources_immutable_delete
BEFORE DELETE ON context_pack_sources
BEGIN SELECT RAISE(ABORT, 'context pack sources are immutable'); END;
CREATE TRIGGER context_pack_generated_artifacts_immutable_update
BEFORE UPDATE ON context_pack_generated_artifacts
BEGIN SELECT RAISE(ABORT, 'context pack artifact links are immutable'); END;
CREATE TRIGGER context_pack_generated_artifacts_immutable_delete
BEFORE DELETE ON context_pack_generated_artifacts
BEGIN SELECT RAISE(ABORT, 'context pack artifact links are immutable'); END;
