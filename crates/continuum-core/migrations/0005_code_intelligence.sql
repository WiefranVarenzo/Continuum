-- CP5 adds deterministic, versioned code-intelligence observations on top of
-- immutable CP4 RepositoryBaseline coordinates. Git remains authoritative.

CREATE TABLE analysis_runs (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    source_fingerprint TEXT NOT NULL CHECK(length(source_fingerprint) = 64),
    analyzer_bundle_version TEXT NOT NULL,
    output_schema_version INTEGER NOT NULL CHECK(output_schema_version > 0),
    completeness TEXT NOT NULL CHECK(completeness IN ('complete','partial')),
    file_count INTEGER NOT NULL CHECK(file_count >= 0),
    analyzed_file_count INTEGER NOT NULL CHECK(analyzed_file_count >= 0),
    code_entity_count INTEGER NOT NULL CHECK(code_entity_count >= 0),
    test_count INTEGER NOT NULL CHECK(test_count >= 0),
    limitation_count INTEGER NOT NULL CHECK(limitation_count >= 0),
    cache_hit_count INTEGER NOT NULL CHECK(cache_hit_count >= 0),
    cache_miss_count INTEGER NOT NULL CHECK(cache_miss_count >= 0),
    limits_json TEXT NOT NULL,
    started_at TEXT NOT NULL,
    completed_at TEXT NOT NULL,
    duration_ms INTEGER NOT NULL CHECK(duration_ms >= 0),
    UNIQUE(repository_id, baseline_id, source_fingerprint, analyzer_bundle_version, output_schema_version)
);

CREATE TABLE analyzer_executions (
    analysis_run_id TEXT NOT NULL REFERENCES analysis_runs(entity_id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    analyzer_id TEXT NOT NULL,
    analyzer_version TEXT NOT NULL,
    contract_version INTEGER NOT NULL CHECK(contract_version > 0),
    output_schema_version INTEGER NOT NULL CHECK(output_schema_version > 0),
    status TEXT NOT NULL CHECK(status IN ('succeeded','degraded','skipped')),
    files_seen INTEGER NOT NULL CHECK(files_seen >= 0),
    outputs_created INTEGER NOT NULL CHECK(outputs_created >= 0),
    cache_hits INTEGER NOT NULL CHECK(cache_hits >= 0),
    cache_misses INTEGER NOT NULL CHECK(cache_misses >= 0),
    duration_ms INTEGER NOT NULL CHECK(duration_ms >= 0),
    diagnostic_code TEXT,
    PRIMARY KEY(analysis_run_id, ordinal),
    UNIQUE(analysis_run_id, analyzer_id)
);

CREATE TABLE code_entities (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    entity_kind TEXT NOT NULL CHECK(entity_kind IN ('file','module','symbol','dependency','configuration')),
    stable_key TEXT NOT NULL,
    language TEXT,
    first_seen_baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    UNIQUE(repository_id, entity_kind, stable_key)
);

CREATE TABLE code_entity_aliases (
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    alias_kind TEXT NOT NULL CHECK(alias_kind IN ('path','qualified_name')),
    alias_value TEXT NOT NULL,
    code_entity_id TEXT NOT NULL REFERENCES code_entities(entity_id) ON DELETE RESTRICT,
    first_seen_baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    PRIMARY KEY(repository_id, alias_kind, alias_value)
);

CREATE TABLE code_entity_observations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    analysis_run_id TEXT NOT NULL REFERENCES analysis_runs(entity_id) ON DELETE RESTRICT,
    baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    code_entity_id TEXT NOT NULL REFERENCES code_entities(entity_id) ON DELETE RESTRICT,
    source_path TEXT NOT NULL,
    qualified_name TEXT,
    content_sha256 TEXT NOT NULL CHECK(length(content_sha256) = 64),
    signature_sha256 TEXT NOT NULL CHECK(length(signature_sha256) = 64),
    start_line INTEGER CHECK(start_line IS NULL OR start_line >= 1),
    start_column INTEGER CHECK(start_column IS NULL OR start_column >= 1),
    end_line INTEGER CHECK(end_line IS NULL OR end_line >= 1),
    end_column INTEGER CHECK(end_column IS NULL OR end_column >= 1),
    visibility TEXT,
    observation_status TEXT NOT NULL CHECK(observation_status IN ('present','fallback','parse_error','unavailable')),
    analyzer_id TEXT NOT NULL,
    analyzer_version TEXT NOT NULL,
    output_schema_version INTEGER NOT NULL CHECK(output_schema_version > 0),
    details_json TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    UNIQUE(analysis_run_id, code_entity_id, source_path)
);

CREATE TABLE tests (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    stable_key TEXT NOT NULL,
    framework TEXT NOT NULL,
    test_kind TEXT NOT NULL CHECK(test_kind IN ('test','suite','fixture','benchmark','unknown')),
    first_seen_baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    UNIQUE(repository_id, stable_key)
);

CREATE TABLE test_observations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    analysis_run_id TEXT NOT NULL REFERENCES analysis_runs(entity_id) ON DELETE RESTRICT,
    baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    test_id TEXT NOT NULL REFERENCES tests(entity_id) ON DELETE RESTRICT,
    code_entity_id TEXT NOT NULL REFERENCES code_entities(entity_id) ON DELETE RESTRICT,
    source_path TEXT NOT NULL,
    qualified_name TEXT NOT NULL,
    content_sha256 TEXT NOT NULL CHECK(length(content_sha256) = 64),
    start_line INTEGER NOT NULL CHECK(start_line >= 1),
    end_line INTEGER NOT NULL CHECK(end_line >= start_line),
    analyzer_id TEXT NOT NULL,
    analyzer_version TEXT NOT NULL,
    details_json TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    UNIQUE(analysis_run_id, test_id)
);

CREATE TABLE test_runs (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    outcome TEXT NOT NULL CHECK(outcome IN ('passed','failed','error','cancelled','skipped','unknown')),
    command_label TEXT NOT NULL,
    exit_code INTEGER,
    duration_ms INTEGER CHECK(duration_ms IS NULL OR duration_ms >= 0),
    observed_at TEXT NOT NULL,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('user','import','external','legacy','unknown')),
    details_json TEXT NOT NULL
);

CREATE TABLE test_run_results (
    test_run_id TEXT NOT NULL REFERENCES test_runs(entity_id) ON DELETE RESTRICT,
    test_id TEXT NOT NULL REFERENCES tests(entity_id) ON DELETE RESTRICT,
    outcome TEXT NOT NULL CHECK(outcome IN ('passed','failed','error','cancelled','skipped','unknown')),
    duration_ms INTEGER CHECK(duration_ms IS NULL OR duration_ms >= 0),
    message TEXT,
    PRIMARY KEY(test_run_id, test_id)
);

CREATE TABLE analysis_limitations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    analysis_run_id TEXT NOT NULL REFERENCES analysis_runs(entity_id) ON DELETE RESTRICT,
    source_path TEXT,
    analyzer_id TEXT NOT NULL,
    code TEXT NOT NULL,
    severity TEXT NOT NULL CHECK(severity IN ('info','warning','error')),
    message TEXT NOT NULL
);

-- Rebuildable deterministic cache. Cached output is path-independent and may
-- be discarded without losing canonical observations.
CREATE TABLE analyzer_cache (
    content_sha256 TEXT NOT NULL CHECK(length(content_sha256) = 64),
    language TEXT NOT NULL,
    analyzer_id TEXT NOT NULL,
    analyzer_version TEXT NOT NULL,
    output_schema_version INTEGER NOT NULL CHECK(output_schema_version > 0),
    output_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(content_sha256, language, analyzer_id, analyzer_version, output_schema_version)
);

CREATE INDEX idx_analysis_runs_repository ON analysis_runs(project_id, repository_id, completed_at, entity_id);
CREATE INDEX idx_code_entities_repository ON code_entities(project_id, repository_id, entity_kind, entity_id);
CREATE INDEX idx_code_observations_baseline ON code_entity_observations(project_id, baseline_id, source_path);
CREATE INDEX idx_code_observations_entity ON code_entity_observations(code_entity_id, observed_at);
CREATE INDEX idx_tests_repository ON tests(project_id, repository_id, framework, entity_id);
CREATE INDEX idx_test_observations_baseline ON test_observations(project_id, baseline_id, source_path);
CREATE INDEX idx_test_runs_repository ON test_runs(project_id, repository_id, observed_at, entity_id);
CREATE INDEX idx_analysis_limitations_run ON analysis_limitations(analysis_run_id, severity, code);

CREATE TRIGGER analysis_runs_type_guard
BEFORE INSERT ON analysis_runs
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'analysis_run'
  OR COALESCE((SELECT project_id FROM entities WHERE id=NEW.entity_id),'') <> NEW.project_id
BEGIN SELECT RAISE(ABORT, 'analysis run entity type or project mismatch'); END;

CREATE TRIGGER code_entities_type_guard
BEFORE INSERT ON code_entities
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'code_entity'
  OR COALESCE((SELECT project_id FROM entities WHERE id=NEW.entity_id),'') <> NEW.project_id
BEGIN SELECT RAISE(ABORT, 'code entity type or project mismatch'); END;

CREATE TRIGGER tests_type_guard
BEFORE INSERT ON tests
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'test'
  OR COALESCE((SELECT project_id FROM entities WHERE id=NEW.entity_id),'') <> NEW.project_id
BEGIN SELECT RAISE(ABORT, 'test entity type or project mismatch'); END;

CREATE TRIGGER test_runs_type_guard
BEFORE INSERT ON test_runs
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'test_run'
  OR COALESCE((SELECT project_id FROM entities WHERE id=NEW.entity_id),'') <> NEW.project_id
BEGIN SELECT RAISE(ABORT, 'test run entity type or project mismatch'); END;

CREATE TRIGGER analysis_runs_immutable_update BEFORE UPDATE ON analysis_runs
BEGIN SELECT RAISE(ABORT, 'analysis runs are immutable'); END;
CREATE TRIGGER analysis_runs_immutable_delete BEFORE DELETE ON analysis_runs
BEGIN SELECT RAISE(ABORT, 'analysis runs are immutable'); END;
CREATE TRIGGER analyzer_executions_immutable_update BEFORE UPDATE ON analyzer_executions
BEGIN SELECT RAISE(ABORT, 'analyzer executions are immutable'); END;
CREATE TRIGGER analyzer_executions_immutable_delete BEFORE DELETE ON analyzer_executions
BEGIN SELECT RAISE(ABORT, 'analyzer executions are immutable'); END;
CREATE TRIGGER code_entity_observations_immutable_update BEFORE UPDATE ON code_entity_observations
BEGIN SELECT RAISE(ABORT, 'code entity observations are immutable'); END;
CREATE TRIGGER code_entity_observations_immutable_delete BEFORE DELETE ON code_entity_observations
BEGIN SELECT RAISE(ABORT, 'code entity observations are immutable'); END;
CREATE TRIGGER test_observations_immutable_update BEFORE UPDATE ON test_observations
BEGIN SELECT RAISE(ABORT, 'test observations are immutable'); END;
CREATE TRIGGER test_observations_immutable_delete BEFORE DELETE ON test_observations
BEGIN SELECT RAISE(ABORT, 'test observations are immutable'); END;
CREATE TRIGGER test_runs_immutable_update BEFORE UPDATE ON test_runs
BEGIN SELECT RAISE(ABORT, 'test runs are immutable'); END;
CREATE TRIGGER test_runs_immutable_delete BEFORE DELETE ON test_runs
BEGIN SELECT RAISE(ABORT, 'test runs are immutable'); END;
CREATE TRIGGER test_run_results_immutable_update BEFORE UPDATE ON test_run_results
BEGIN SELECT RAISE(ABORT, 'test run results are immutable'); END;
CREATE TRIGGER test_run_results_immutable_delete BEFORE DELETE ON test_run_results
BEGIN SELECT RAISE(ABORT, 'test run results are immutable'); END;
CREATE TRIGGER analysis_limitations_immutable_update BEFORE UPDATE ON analysis_limitations
BEGIN SELECT RAISE(ABORT, 'analysis limitations are immutable'); END;
CREATE TRIGGER analysis_limitations_immutable_delete BEFORE DELETE ON analysis_limitations
BEGIN SELECT RAISE(ABORT, 'analysis limitations are immutable'); END;
