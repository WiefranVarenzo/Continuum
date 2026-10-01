-- CP7 adds an optional semantic-assistance boundary. Canonical Research,
-- Development, and Provenance records remain unchanged and usable offline.

CREATE TABLE ai_provider_profiles (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    display_name TEXT NOT NULL,
    provider_kind TEXT NOT NULL CHECK(provider_kind IN ('gemini','openai_compatible')),
    endpoint TEXT NOT NULL,
    model_id TEXT NOT NULL,
    credential_ref TEXT NOT NULL,
    enabled INTEGER NOT NULL CHECK(enabled IN (0,1)),
    priority INTEGER NOT NULL CHECK(priority BETWEEN 0 AND 10000),
    adapter_version INTEGER NOT NULL CHECK(adapter_version > 0),
    capabilities_json TEXT NOT NULL,
    data_policy_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(project_id,display_name)
);
CREATE INDEX idx_ai_profiles_route
    ON ai_provider_profiles(project_id,enabled,priority,id);
CREATE TRIGGER ai_profile_contract_immutable
BEFORE UPDATE ON ai_provider_profiles
WHEN OLD.project_id IS NOT NEW.project_id
  OR OLD.display_name IS NOT NEW.display_name
  OR OLD.provider_kind IS NOT NEW.provider_kind
  OR OLD.endpoint IS NOT NEW.endpoint
  OR OLD.model_id IS NOT NEW.model_id
  OR OLD.credential_ref IS NOT NEW.credential_ref
  OR OLD.priority IS NOT NEW.priority
  OR OLD.adapter_version IS NOT NEW.adapter_version
  OR OLD.capabilities_json IS NOT NEW.capabilities_json
  OR OLD.data_policy_json IS NOT NEW.data_policy_json
  OR OLD.created_at IS NOT NEW.created_at
BEGIN SELECT RAISE(ABORT, 'provider contract is immutable; register a new profile'); END;

CREATE TABLE ai_entity_classification (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    classification TEXT NOT NULL CHECK(classification IN ('public','internal','sensitive','secret','never_send')),
    reason TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    updated_by TEXT NOT NULL
);
CREATE INDEX idx_ai_entity_classification
    ON ai_entity_classification(project_id,classification,entity_id);

CREATE TABLE ai_project_policy (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    policy_version INTEGER NOT NULL CHECK(policy_version > 0),
    allowed_profile_ids_json TEXT NOT NULL DEFAULT '[]',
    internal_remote_enabled INTEGER NOT NULL DEFAULT 0 CHECK(internal_remote_enabled IN (0,1)),
    automatic_failover INTEGER NOT NULL DEFAULT 0 CHECK(automatic_failover IN (0,1)),
    max_attempts INTEGER NOT NULL DEFAULT 1 CHECK(max_attempts BETWEEN 1 AND 5),
    max_total_units INTEGER NOT NULL DEFAULT 131072 CHECK(max_total_units BETWEEN 1 AND 10000000),
    updated_at TEXT NOT NULL,
    updated_by TEXT NOT NULL
);
INSERT INTO ai_project_policy(project_id,policy_version,updated_at,updated_by)
SELECT id,1,updated_at,'continuum-migration' FROM projects;
CREATE TRIGGER ai_project_policy_after_project_insert
AFTER INSERT ON projects
BEGIN
    INSERT INTO ai_project_policy(project_id,policy_version,updated_at,updated_by)
    VALUES(NEW.id,1,NEW.created_at,'continuum-core');
END;

-- A consent stores only scope and fingerprints, never the disclosed content.
CREATE TABLE ai_consents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    provider_profile_id TEXT NOT NULL REFERENCES ai_provider_profiles(id) ON DELETE RESTRICT,
    scope_kind TEXT NOT NULL CHECK(scope_kind IN ('project_internal','request_sensitive')),
    scope_ref TEXT,
    source_fingerprint TEXT,
    expires_at TEXT,
    revoked_at TEXT,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    CHECK((scope_kind='project_internal' AND scope_ref IS NULL AND source_fingerprint IS NULL)
       OR (scope_kind='request_sensitive' AND scope_ref IS NOT NULL AND source_fingerprint IS NOT NULL))
);
CREATE INDEX idx_ai_consents_lookup
    ON ai_consents(project_id,provider_profile_id,scope_kind,revoked_at,expires_at);
CREATE TRIGGER ai_consent_scope_immutable
BEFORE UPDATE ON ai_consents
WHEN OLD.project_id IS NOT NEW.project_id
  OR OLD.provider_profile_id IS NOT NEW.provider_profile_id
  OR OLD.scope_kind IS NOT NEW.scope_kind
  OR OLD.scope_ref IS NOT NEW.scope_ref
  OR OLD.source_fingerprint IS NOT NEW.source_fingerprint
  OR OLD.expires_at IS NOT NEW.expires_at
  OR OLD.created_at IS NOT NEW.created_at
  OR OLD.created_by IS NOT NEW.created_by
BEGIN SELECT RAISE(ABORT, 'AI consent scope is immutable'); END;

CREATE TABLE ai_semantic_tasks (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_type TEXT NOT NULL,
    contract_version INTEGER NOT NULL CHECK(contract_version > 0),
    status TEXT NOT NULL CHECK(status IN ('pending','completed','failed','cancelled','stale')),
    source_entity_ids_json TEXT NOT NULL,
    source_artifact_ids_json TEXT NOT NULL,
    source_snapshot_json TEXT NOT NULL,
    source_fingerprint TEXT NOT NULL,
    output_schema_json TEXT NOT NULL,
    prompt_template_id TEXT NOT NULL,
    prompt_template_version INTEGER NOT NULL CHECK(prompt_template_version > 0),
    requirements_json TEXT NOT NULL,
    route_policy_json TEXT NOT NULL,
    privacy_audience TEXT NOT NULL,
    consent_id TEXT REFERENCES ai_consents(id) ON DELETE RESTRICT,
    max_input_units INTEGER NOT NULL CHECK(max_input_units BETWEEN 1 AND 10000000),
    max_output_units INTEGER NOT NULL CHECK(max_output_units BETWEEN 1 AND 10000000),
    timeout_ms INTEGER NOT NULL CHECK(timeout_ms BETWEEN 100 AND 600000),
    cache_ttl_seconds INTEGER NOT NULL CHECK(cache_ttl_seconds BETWEEN 0 AND 2592000),
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_ai_tasks_status
    ON ai_semantic_tasks(project_id,status,created_at,id);
CREATE TRIGGER ai_semantic_task_contract_immutable
BEFORE UPDATE ON ai_semantic_tasks
WHEN OLD.project_id IS NOT NEW.project_id
  OR OLD.task_type IS NOT NEW.task_type
  OR OLD.contract_version IS NOT NEW.contract_version
  OR OLD.source_entity_ids_json IS NOT NEW.source_entity_ids_json
  OR OLD.source_artifact_ids_json IS NOT NEW.source_artifact_ids_json
  OR OLD.source_snapshot_json IS NOT NEW.source_snapshot_json
  OR OLD.source_fingerprint IS NOT NEW.source_fingerprint
  OR OLD.output_schema_json IS NOT NEW.output_schema_json
  OR OLD.prompt_template_id IS NOT NEW.prompt_template_id
  OR OLD.prompt_template_version IS NOT NEW.prompt_template_version
  OR OLD.requirements_json IS NOT NEW.requirements_json
  OR OLD.route_policy_json IS NOT NEW.route_policy_json
  OR OLD.privacy_audience IS NOT NEW.privacy_audience
  OR OLD.max_input_units IS NOT NEW.max_input_units
  OR OLD.max_output_units IS NOT NEW.max_output_units
  OR OLD.timeout_ms IS NOT NEW.timeout_ms
  OR OLD.cache_ttl_seconds IS NOT NEW.cache_ttl_seconds
  OR OLD.created_at IS NOT NEW.created_at
  OR OLD.created_by IS NOT NEW.created_by
BEGIN SELECT RAISE(ABORT, 'semantic task contract is immutable'); END;

CREATE TABLE ai_attempts (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES ai_semantic_tasks(id) ON DELETE RESTRICT,
    provider_profile_id TEXT NOT NULL REFERENCES ai_provider_profiles(id) ON DELETE RESTRICT,
    attempt_number INTEGER NOT NULL CHECK(attempt_number BETWEEN 1 AND 5),
    status TEXT NOT NULL CHECK(status IN ('running','succeeded','failed','refused','invalid_output')),
    request_hash TEXT NOT NULL,
    privacy_decision_json TEXT NOT NULL,
    exact_model_id TEXT,
    input_units INTEGER CHECK(input_units IS NULL OR input_units >= 0),
    output_units INTEGER CHECK(output_units IS NULL OR output_units >= 0),
    latency_ms INTEGER CHECK(latency_ms IS NULL OR latency_ms >= 0),
    cache_hit INTEGER NOT NULL DEFAULT 0 CHECK(cache_hit IN (0,1)),
    error_code TEXT,
    error_message TEXT,
    response_hash TEXT,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    UNIQUE(task_id,attempt_number)
);
CREATE INDEX idx_ai_attempts_task ON ai_attempts(project_id,task_id,attempt_number);
CREATE TRIGGER ai_attempt_terminal_immutable
BEFORE UPDATE ON ai_attempts
WHEN OLD.status <> 'running'
BEGIN SELECT RAISE(ABORT, 'terminal AI attempts are immutable'); END;
CREATE TRIGGER ai_attempt_no_delete
BEFORE DELETE ON ai_attempts
BEGIN SELECT RAISE(ABORT, 'AI attempts are audit records'); END;

-- Candidates are deliberately outside canonical entities/relationships.
CREATE TABLE ai_candidates (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES ai_semantic_tasks(id) ON DELETE RESTRICT,
    attempt_id TEXT NOT NULL REFERENCES ai_attempts(id) ON DELETE RESTRICT,
    provider_profile_id TEXT NOT NULL REFERENCES ai_provider_profiles(id) ON DELETE RESTRICT,
    candidate_version INTEGER NOT NULL DEFAULT 1 CHECK(candidate_version > 0),
    review_state TEXT NOT NULL CHECK(review_state IN ('pending','accepted','rejected')),
    output_json TEXT NOT NULL,
    validation_json TEXT NOT NULL,
    source_fingerprint TEXT NOT NULL,
    edited_by_human INTEGER NOT NULL DEFAULT 0 CHECK(edited_by_human IN (0,1)),
    created_at TEXT NOT NULL,
    reviewed_at TEXT,
    reviewed_by TEXT,
    review_note TEXT NOT NULL DEFAULT ''
);
CREATE INDEX idx_ai_candidates_review
    ON ai_candidates(project_id,review_state,created_at,id);
CREATE TRIGGER ai_candidate_terminal_immutable
BEFORE UPDATE ON ai_candidates
WHEN OLD.review_state <> 'pending'
BEGIN SELECT RAISE(ABORT, 'reviewed AI candidates are immutable'); END;

CREATE TABLE ai_cache_entries (
    cache_key TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_type TEXT NOT NULL,
    provider_profile_id TEXT NOT NULL REFERENCES ai_provider_profiles(id) ON DELETE RESTRICT,
    candidate_id TEXT NOT NULL REFERENCES ai_candidates(id) ON DELETE RESTRICT,
    source_fingerprint TEXT NOT NULL,
    policy_version INTEGER NOT NULL,
    prompt_template_version INTEGER NOT NULL,
    output_schema_hash TEXT NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    last_accessed_at TEXT NOT NULL
);
CREATE INDEX idx_ai_cache_eviction
    ON ai_cache_entries(project_id,expires_at,last_accessed_at);

-- Provider credentials must never be persisted in these profiles.
CREATE TRIGGER ai_profile_credential_guard_insert
BEFORE INSERT ON ai_provider_profiles
WHEN lower(NEW.credential_ref) LIKE '%key=%'
  OR lower(NEW.credential_ref) LIKE '%bearer %'
  OR lower(NEW.credential_ref) GLOB 'sk-*'
  OR lower(NEW.credential_ref) GLOB 'aiza*'
  OR lower(NEW.credential_ref) GLOB 'gsk_*'
  OR lower(NEW.credential_ref) GLOB 'or-*'
  OR length(NEW.credential_ref) > 200
BEGIN SELECT RAISE(ABORT, 'credential_ref must be an OS credential-store lookup key, not a secret'); END;
CREATE TRIGGER ai_profile_credential_guard_update
BEFORE UPDATE OF credential_ref ON ai_provider_profiles
WHEN lower(NEW.credential_ref) LIKE '%key=%'
  OR lower(NEW.credential_ref) LIKE '%bearer %'
  OR lower(NEW.credential_ref) GLOB 'sk-*'
  OR lower(NEW.credential_ref) GLOB 'aiza*'
  OR lower(NEW.credential_ref) GLOB 'gsk_*'
  OR lower(NEW.credential_ref) GLOB 'or-*'
  OR length(NEW.credential_ref) > 200
BEGIN SELECT RAISE(ABORT, 'credential_ref must be an OS credential-store lookup key, not a secret'); END;
