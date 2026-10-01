-- CP6 makes provenance operational without replacing the canonical entity and
-- relationship tables introduced by CP2. The migration is additive so projects
-- created by CP2-CP5 remain valid partial graphs.

ALTER TABLE relationships ADD COLUMN state_version INTEGER NOT NULL DEFAULT 1
    CHECK(state_version > 0);
ALTER TABLE relationships ADD COLUMN annotation TEXT NOT NULL DEFAULT '';
ALTER TABLE relationships ADD COLUMN reviewed_at TEXT;
ALTER TABLE relationships ADD COLUMN reviewed_by TEXT;
ALTER TABLE relationships ADD COLUMN retired_at TEXT;

CREATE INDEX idx_relationships_graph_source
    ON relationships(project_id,status,review_state,source_entity_id,relation_type,created_at,id);
CREATE INDEX idx_relationships_graph_target
    ON relationships(project_id,status,review_state,target_entity_id,relation_type,created_at,id);

-- Every CP6 relationship state change has an immutable, ledger-addressed record.
-- Existing relationships receive an explicit migration baseline at sequence 0;
-- Continuum never fabricates an original event sequence for pre-CP6 links.
CREATE TABLE relationship_history (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    relationship_id TEXT NOT NULL REFERENCES relationships(id) ON DELETE RESTRICT,
    ledger_sequence INTEGER NOT NULL CHECK(ledger_sequence >= 0),
    action TEXT NOT NULL CHECK(action IN (
        'migration_baseline','created','accepted','rejected','annotated','retired','superseded'
    )),
    state_version INTEGER NOT NULL CHECK(state_version > 0),
    from_status TEXT,
    to_status TEXT NOT NULL,
    from_review_state TEXT,
    to_review_state TEXT NOT NULL,
    annotation TEXT NOT NULL DEFAULT '',
    actor_id TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    UNIQUE(relationship_id,state_version)
);

INSERT INTO relationship_history(
    id,project_id,relationship_id,ledger_sequence,action,state_version,
    from_status,to_status,from_review_state,to_review_state,annotation,actor_id,occurred_at
)
SELECT 'cp6-baseline-' || id,project_id,id,0,'migration_baseline',1,
       NULL,status,NULL,review_state,annotation,'continuum-migration',created_at
FROM relationships;

CREATE TRIGGER relationships_record_creation
AFTER INSERT ON relationships
BEGIN
    INSERT INTO relationship_history(
        id,project_id,relationship_id,ledger_sequence,action,state_version,
        from_status,to_status,from_review_state,to_review_state,annotation,actor_id,occurred_at
    ) VALUES(
        'cp6-created-' || NEW.id,NEW.project_id,NEW.id,
        COALESCE((SELECT ledger_sequence + 1 FROM projects WHERE id=NEW.project_id),0),
        'created',NEW.state_version,NULL,NEW.status,NULL,NEW.review_state,
        NEW.annotation,NEW.actor_id,NEW.created_at
    );
END;

CREATE INDEX idx_relationship_history_relationship
    ON relationship_history(project_id,relationship_id,state_version);
CREATE INDEX idx_relationship_history_sequence
    ON relationship_history(project_id,ledger_sequence,id);

CREATE TRIGGER relationship_history_immutable_update
BEFORE UPDATE ON relationship_history
BEGIN SELECT RAISE(ABORT, 'relationship history is append-only'); END;
CREATE TRIGGER relationship_history_immutable_delete
BEFORE DELETE ON relationship_history
BEGIN SELECT RAISE(ABORT, 'relationship history is append-only'); END;

-- Learning Feedback is the only new CP6 aggregate. Validation observations stay
-- separate; explicit typed relationships connect their source and targets.
CREATE TABLE learning_feedback (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    feedback_kind TEXT NOT NULL CHECK(feedback_kind IN (
        'validation_failure','validation_success','observation','contradiction',
        'revision_request','other'
    )),
    summary TEXT NOT NULL,
    details_json TEXT NOT NULL DEFAULT '{}',
    severity TEXT NOT NULL CHECK(severity IN ('info','warning','error')),
    resolution_note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    resolved_at TEXT,
    CHECK(length(summary) BETWEEN 1 AND 30000)
);

CREATE TRIGGER learning_feedback_type_guard
BEFORE INSERT ON learning_feedback
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'learning_feedback'
BEGIN SELECT RAISE(ABORT, 'learning feedback entity type mismatch'); END;

CREATE INDEX idx_learning_feedback_kind
    ON learning_feedback(feedback_kind,severity,created_at,entity_id);
