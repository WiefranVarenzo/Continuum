-- CP9 Research Capture System. Media bytes remain in the content-addressed
-- Artifact Store; this schema persists consent, lifecycle, segmentation,
-- recovery, and Evidence linkage metadata only.

CREATE TABLE capture_sessions (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    research_session_entity_id TEXT REFERENCES research_sessions(entity_id) ON DELETE RESTRICT,
    state TEXT NOT NULL CHECK(state IN (
        'awaiting_permission','ready','blocked','capturing','paused',
        'finalizing','completed','interrupted','failed','cancelled'
    )),
    state_version INTEGER NOT NULL DEFAULT 1 CHECK(state_version > 0),
    backend_id TEXT NOT NULL,
    backend_version TEXT NOT NULL,
    backend_platform TEXT NOT NULL,
    capability_snapshot_json TEXT NOT NULL,
    encoding_settings_json TEXT NOT NULL,
    buffer_policy_json TEXT NOT NULL,
    legal_consent_acknowledged INTEGER NOT NULL CHECK(legal_consent_acknowledged IN (0,1)),
    indicator_id TEXT,
    recoverable INTEGER NOT NULL DEFAULT 0 CHECK(recoverable IN (0,1)),
    failure_code TEXT,
    failure_message TEXT,
    started_at TEXT,
    paused_at TEXT,
    ended_at TEXT,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    updated_by TEXT NOT NULL,
    CHECK((state IN ('capturing','paused','finalizing')) = (indicator_id IS NOT NULL)),
    CHECK(ended_at IS NULL OR started_at IS NULL OR ended_at >= started_at)
);
CREATE INDEX idx_capture_sessions_project_state
    ON capture_sessions(project_id,state,updated_at,id);
CREATE INDEX idx_capture_sessions_research
    ON capture_sessions(project_id,research_session_entity_id,created_at,id);

CREATE TABLE capture_session_sources (
    capture_session_id TEXT NOT NULL REFERENCES capture_sessions(id) ON DELETE RESTRICT,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('screen','system_audio','microphone')),
    capability_id TEXT NOT NULL,
    permission_status TEXT NOT NULL CHECK(permission_status IN (
        'unknown','granted','denied','unavailable','revoked'
    )),
    permission_checked_at TEXT,
    permission_reference TEXT,
    PRIMARY KEY(capture_session_id,source_kind)
);

CREATE TABLE capture_permission_events (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    capture_session_id TEXT NOT NULL REFERENCES capture_sessions(id) ON DELETE RESTRICT,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('screen','system_audio','microphone')),
    permission_status TEXT NOT NULL CHECK(permission_status IN ('granted','denied','unavailable','revoked')),
    permission_reference TEXT,
    occurred_at TEXT NOT NULL,
    actor_id TEXT NOT NULL
);
CREATE INDEX idx_capture_permissions_session
    ON capture_permission_events(project_id,capture_session_id,occurred_at,id);

CREATE TABLE capture_segments (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    capture_session_id TEXT NOT NULL REFERENCES capture_sessions(id) ON DELETE RESTRICT,
    sequence INTEGER NOT NULL CHECK(sequence >= 0),
    state TEXT NOT NULL CHECK(state IN ('finalized','recoverable')),
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    media_type TEXT NOT NULL,
    source_kinds_json TEXT NOT NULL,
    start_offset_ms INTEGER NOT NULL CHECK(start_offset_ms >= 0),
    end_offset_ms INTEGER NOT NULL CHECK(end_offset_ms > start_offset_ms),
    byte_size INTEGER NOT NULL CHECK(byte_size > 0),
    sha256 TEXT NOT NULL CHECK(length(sha256) = 64),
    recovery_note TEXT,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    UNIQUE(capture_session_id,sequence)
);
CREATE INDEX idx_capture_segments_session
    ON capture_segments(project_id,capture_session_id,sequence,id);

CREATE TABLE capture_markers (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    capture_session_id TEXT NOT NULL REFERENCES capture_sessions(id) ON DELETE RESTRICT,
    segment_id TEXT REFERENCES capture_segments(id) ON DELETE RESTRICT,
    offset_ms INTEGER NOT NULL CHECK(offset_ms >= 0),
    label TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL
);
CREATE INDEX idx_capture_markers_session
    ON capture_markers(project_id,capture_session_id,offset_ms,id);

CREATE TABLE capture_evidence_links (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    capture_session_id TEXT NOT NULL REFERENCES capture_sessions(id) ON DELETE RESTRICT,
    evidence_entity_id TEXT NOT NULL REFERENCES evidence(entity_id) ON DELETE RESTRICT,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('segment','marker','external')),
    segment_id TEXT REFERENCES capture_segments(id) ON DELETE RESTRICT,
    marker_id TEXT REFERENCES capture_markers(id) ON DELETE RESTRICT,
    start_offset_ms INTEGER,
    end_offset_ms INTEGER,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    UNIQUE(evidence_entity_id),
    CHECK((source_kind='segment') = (segment_id IS NOT NULL)),
    CHECK((source_kind='marker') = (marker_id IS NOT NULL)),
    CHECK(start_offset_ms IS NULL OR start_offset_ms >= 0),
    CHECK(end_offset_ms IS NULL OR (start_offset_ms IS NOT NULL AND end_offset_ms >= start_offset_ms))
);
CREATE INDEX idx_capture_evidence_session
    ON capture_evidence_links(project_id,capture_session_id,evidence_entity_id);
CREATE UNIQUE INDEX idx_capture_evidence_segment_unique
    ON capture_evidence_links(evidence_entity_id,segment_id) WHERE source_kind='segment';
CREATE UNIQUE INDEX idx_capture_evidence_marker_unique
    ON capture_evidence_links(evidence_entity_id,marker_id) WHERE source_kind='marker';
CREATE UNIQUE INDEX idx_capture_evidence_external_unique
    ON capture_evidence_links(evidence_entity_id) WHERE source_kind='external';

CREATE TABLE capture_external_items (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    capture_kind TEXT NOT NULL CHECK(capture_kind IN ('browser','file','web','screenshot')),
    evidence_entity_id TEXT NOT NULL REFERENCES evidence(entity_id) ON DELETE RESTRICT,
    artifact_id TEXT REFERENCES artifacts(id) ON DELETE RESTRICT,
    source_uri TEXT,
    request_fingerprint TEXT NOT NULL CHECK(length(request_fingerprint) = 64),
    captured_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    UNIQUE(evidence_entity_id)
);

CREATE TABLE capture_derivations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    source_artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    derived_artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    derivation_kind TEXT NOT NULL CHECK(derivation_kind IN ('transcription','ocr','thumbnail','waveform')),
    engine_id TEXT NOT NULL,
    engine_version TEXT NOT NULL,
    settings_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    CHECK(source_artifact_id <> derived_artifact_id),
    UNIQUE(source_artifact_id,derived_artifact_id,derivation_kind)
);

CREATE TRIGGER capture_permission_events_immutable_update
BEFORE UPDATE ON capture_permission_events
BEGIN SELECT RAISE(ABORT, 'capture permission events are append-only'); END;
CREATE TRIGGER capture_permission_events_immutable_delete
BEFORE DELETE ON capture_permission_events
BEGIN SELECT RAISE(ABORT, 'capture permission events are append-only'); END;
CREATE TRIGGER capture_segments_immutable_update
BEFORE UPDATE ON capture_segments
BEGIN SELECT RAISE(ABORT, 'capture segments are immutable'); END;
CREATE TRIGGER capture_segments_immutable_delete
BEFORE DELETE ON capture_segments
BEGIN SELECT RAISE(ABORT, 'capture segments are audit records'); END;
CREATE TRIGGER capture_markers_immutable_update
BEFORE UPDATE ON capture_markers
BEGIN SELECT RAISE(ABORT, 'capture markers are immutable'); END;
CREATE TRIGGER capture_markers_immutable_delete
BEFORE DELETE ON capture_markers
BEGIN SELECT RAISE(ABORT, 'capture markers are audit records'); END;
CREATE TRIGGER capture_evidence_links_immutable_update
BEFORE UPDATE ON capture_evidence_links
BEGIN SELECT RAISE(ABORT, 'capture evidence links are immutable'); END;
CREATE TRIGGER capture_evidence_links_immutable_delete
BEFORE DELETE ON capture_evidence_links
BEGIN SELECT RAISE(ABORT, 'capture evidence links are audit records'); END;
CREATE TRIGGER capture_external_items_immutable_update
BEFORE UPDATE ON capture_external_items
BEGIN SELECT RAISE(ABORT, 'external capture records are immutable'); END;
CREATE TRIGGER capture_external_items_immutable_delete
BEFORE DELETE ON capture_external_items
BEGIN SELECT RAISE(ABORT, 'external capture records are audit records'); END;
CREATE TRIGGER capture_derivations_immutable_update
BEFORE UPDATE ON capture_derivations
BEGIN SELECT RAISE(ABORT, 'capture derivations are immutable'); END;
CREATE TRIGGER capture_derivations_immutable_delete
BEFORE DELETE ON capture_derivations
BEGIN SELECT RAISE(ABORT, 'capture derivations are audit records'); END;
