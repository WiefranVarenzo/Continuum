-- CP3 adds normalized Research Space state while preserving the CP2 common envelope.
-- Every domain row is anchored to exactly one canonical entities row.

CREATE TABLE research_sessions (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    objective TEXT NOT NULL,
    started_at TEXT NOT NULL,
    ended_at TEXT,
    completion_note TEXT,
    CHECK(length(objective) BETWEEN 1 AND 10000),
    CHECK(ended_at IS NULL OR ended_at >= started_at)
);

CREATE TABLE research_questions (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    question_kind TEXT NOT NULL CHECK(question_kind IN ('question','hypothesis','uncertainty')),
    question_text TEXT NOT NULL,
    context TEXT NOT NULL DEFAULT '',
    desired_outcome TEXT NOT NULL DEFAULT '',
    priority INTEGER NOT NULL DEFAULT 2 CHECK(priority BETWEEN 0 AND 4),
    due_at TEXT,
    CHECK(length(question_text) BETWEEN 1 AND 20000)
);

CREATE TABLE evidence (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    evidence_kind TEXT NOT NULL CHECK(evidence_kind IN (
        'note','web','file','screenshot','recording_segment','capture_marker','observation','other'
    )),
    source_uri TEXT,
    source_title TEXT,
    source_author TEXT,
    captured_at TEXT NOT NULL,
    capture_method TEXT NOT NULL,
    stable_reference TEXT,
    source_content TEXT,
    annotation_text TEXT NOT NULL DEFAULT '',
    summary_text TEXT NOT NULL DEFAULT '',
    relevance TEXT NOT NULL DEFAULT '',
    original_artifact_id TEXT REFERENCES artifacts(id) ON DELETE RESTRICT,
    CHECK(
        original_artifact_id IS NOT NULL OR
        NULLIF(trim(COALESCE(source_uri,'')), '') IS NOT NULL OR
        NULLIF(trim(COALESCE(stable_reference,'')), '') IS NOT NULL OR
        NULLIF(trim(COALESCE(source_content,'')), '') IS NOT NULL
    )
);

CREATE TABLE experiments (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    hypothesis TEXT NOT NULL,
    method TEXT NOT NULL,
    inputs_json TEXT NOT NULL DEFAULT '{}',
    expected_observations TEXT NOT NULL,
    started_at TEXT,
    ended_at TEXT,
    CHECK(length(method) BETWEEN 1 AND 30000),
    CHECK(ended_at IS NULL OR started_at IS NOT NULL)
);

CREATE TABLE results (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    experiment_id TEXT NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    observation_text TEXT NOT NULL,
    outcome TEXT NOT NULL CHECK(outcome IN ('positive','negative','mixed','inconclusive','error','observed')),
    measurements_json TEXT NOT NULL DEFAULT '{}',
    observed_at TEXT NOT NULL,
    artifact_id TEXT REFERENCES artifacts(id) ON DELETE RESTRICT,
    CHECK(length(observation_text) BETWEEN 1 AND 30000)
);

CREATE TABLE findings (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    claim TEXT NOT NULL,
    interpretation TEXT NOT NULL,
    uncertainty TEXT NOT NULL DEFAULT '',
    confidence REAL CHECK(confidence IS NULL OR (confidence >= 0.0 AND confidence <= 1.0)),
    accepted_at TEXT,
    CHECK(length(claim) BETWEEN 1 AND 20000)
);

CREATE TABLE decisions (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    selected_option TEXT NOT NULL,
    rationale TEXT NOT NULL,
    alternatives_json TEXT NOT NULL DEFAULT '[]',
    constraints_json TEXT NOT NULL DEFAULT '[]',
    decided_at TEXT,
    CHECK(length(selected_option) BETWEEN 1 AND 10000),
    CHECK(length(rationale) BETWEEN 1 AND 30000)
);

-- Requirement is introduced as the optional CP3 handoff record. CP4 extends its
-- Development relationships without changing this identity or provenance.
CREATE TABLE requirements (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    statement TEXT NOT NULL,
    acceptance_criteria_json TEXT NOT NULL DEFAULT '[]',
    priority INTEGER NOT NULL DEFAULT 2 CHECK(priority BETWEEN 0 AND 4),
    rationale_origin TEXT NOT NULL CHECK(rationale_origin IN (
        'research','user','import','external','legacy','unknown'
    )),
    verification_method TEXT NOT NULL DEFAULT '',
    CHECK(length(statement) BETWEEN 1 AND 20000)
);

CREATE TABLE research_session_items (
    session_entity_id TEXT NOT NULL REFERENCES research_sessions(entity_id) ON DELETE RESTRICT,
    entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE RESTRICT,
    added_at TEXT NOT NULL,
    added_by TEXT NOT NULL,
    PRIMARY KEY(session_entity_id, entity_id),
    CHECK(session_entity_id <> entity_id)
);

CREATE TABLE research_timeline (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ledger_sequence INTEGER NOT NULL,
    session_entity_id TEXT REFERENCES research_sessions(entity_id) ON DELETE RESTRICT,
    entity_id TEXT REFERENCES entities(id) ON DELETE RESTRICT,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    UNIQUE(project_id, ledger_sequence)
);

CREATE TABLE research_search_documents (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_research_session_items_entity
    ON research_session_items(entity_id, session_entity_id);
CREATE INDEX idx_research_timeline_project_sequence
    ON research_timeline(project_id, ledger_sequence, id);
CREATE INDEX idx_research_timeline_session
    ON research_timeline(project_id, session_entity_id, ledger_sequence);
CREATE INDEX idx_research_search_type
    ON research_search_documents(project_id, entity_type, updated_at);
CREATE INDEX idx_research_questions_priority
    ON research_questions(priority, due_at);
CREATE INDEX idx_results_experiment ON results(experiment_id, observed_at);

-- Persistence-level type guards prevent normalized rows from being attached to
-- the wrong common-envelope type, even if an adapter bypasses application code.
CREATE TRIGGER research_sessions_type_guard
BEFORE INSERT ON research_sessions
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'research_session'
BEGIN SELECT RAISE(ABORT, 'research_sessions entity type mismatch'); END;

CREATE TRIGGER research_questions_type_guard
BEFORE INSERT ON research_questions
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'research_question'
BEGIN SELECT RAISE(ABORT, 'research_questions entity type mismatch'); END;

CREATE TRIGGER evidence_type_guard
BEFORE INSERT ON evidence
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'evidence'
BEGIN SELECT RAISE(ABORT, 'evidence entity type mismatch'); END;

CREATE TRIGGER experiments_type_guard
BEFORE INSERT ON experiments
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'experiment'
BEGIN SELECT RAISE(ABORT, 'experiments entity type mismatch'); END;

CREATE TRIGGER results_type_guard
BEFORE INSERT ON results
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'result'
BEGIN SELECT RAISE(ABORT, 'results entity type mismatch'); END;

CREATE TRIGGER findings_type_guard
BEFORE INSERT ON findings
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'finding'
BEGIN SELECT RAISE(ABORT, 'findings entity type mismatch'); END;

CREATE TRIGGER decisions_type_guard
BEFORE INSERT ON decisions
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'decision'
BEGIN SELECT RAISE(ABORT, 'decisions entity type mismatch'); END;

CREATE TRIGGER requirements_type_guard
BEFORE INSERT ON requirements
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'requirement'
BEGIN SELECT RAISE(ABORT, 'requirements entity type mismatch'); END;

CREATE TRIGGER results_experiment_type_guard
BEFORE INSERT ON results
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.experiment_id),'') <> 'experiment'
BEGIN SELECT RAISE(ABORT, 'result experiment type mismatch'); END;

CREATE TRIGGER research_timeline_immutable_update
BEFORE UPDATE ON research_timeline
BEGIN SELECT RAISE(ABORT, 'research timeline is append-only'); END;

CREATE TRIGGER research_timeline_immutable_delete
BEFORE DELETE ON research_timeline
BEGIN SELECT RAISE(ABORT, 'research timeline is append-only'); END;
