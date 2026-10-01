-- CP8 persists renderer-neutral Human Documents. HTML, Markdown, and graph
-- layouts remain derived projections and never replace canonical project state.

CREATE TABLE human_documents (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    schema_version INTEGER NOT NULL CHECK(schema_version = 1),
    document_kind TEXT NOT NULL CHECK(document_kind IN (
        'research_report','development_report','integrated_report',
        'architecture_explanation','timeline','handover'
    )),
    title TEXT NOT NULL,
    audience TEXT NOT NULL CHECK(audience IN ('local_project','private_portable','public_portable')),
    source_checkpoint_id TEXT REFERENCES checkpoints(id) ON DELETE RESTRICT,
    source_ledger_sequence INTEGER NOT NULL CHECK(source_ledger_sequence >= 0),
    classification TEXT NOT NULL CHECK(classification IN (
        'public','internal','sensitive','secret','never_send'
    )),
    document_json TEXT NOT NULL,
    material_fingerprint TEXT NOT NULL,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL
);
CREATE INDEX idx_human_documents_project
    ON human_documents(project_id,document_kind,created_at,id);

CREATE TABLE human_document_sources (
    document_id TEXT NOT NULL REFERENCES human_documents(id) ON DELETE RESTRICT,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('entity','artifact','ai_candidate')),
    source_id TEXT NOT NULL,
    source_version TEXT NOT NULL,
    classification TEXT NOT NULL CHECK(classification IN (
        'public','internal','sensitive','secret','never_send'
    )),
    PRIMARY KEY(document_id,source_kind,source_id)
);
CREATE INDEX idx_human_document_sources_lookup
    ON human_document_sources(source_kind,source_id,document_id);

CREATE TABLE human_document_exports (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    document_id TEXT NOT NULL REFERENCES human_documents(id) ON DELETE RESTRICT,
    format TEXT NOT NULL CHECK(format IN ('html','markdown')),
    artifact_id TEXT NOT NULL REFERENCES artifacts(id) ON DELETE RESTRICT,
    renderer_version INTEGER NOT NULL CHECK(renderer_version > 0),
    template_version INTEGER NOT NULL CHECK(template_version > 0),
    content_sha256 TEXT NOT NULL,
    asset_manifest_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    created_by TEXT NOT NULL,
    UNIQUE(document_id,format,content_sha256)
);
CREATE INDEX idx_human_document_exports_document
    ON human_document_exports(project_id,document_id,format,created_at,id);

CREATE TRIGGER human_document_immutable
BEFORE UPDATE ON human_documents
BEGIN SELECT RAISE(ABORT, 'saved Human Documents are immutable projections'); END;
CREATE TRIGGER human_document_no_delete
BEFORE DELETE ON human_documents
BEGIN SELECT RAISE(ABORT, 'saved Human Documents are audit records'); END;
CREATE TRIGGER human_document_source_immutable
BEFORE UPDATE ON human_document_sources
BEGIN SELECT RAISE(ABORT, 'Human Document source boundaries are immutable'); END;
CREATE TRIGGER human_document_source_no_delete
BEFORE DELETE ON human_document_sources
BEGIN SELECT RAISE(ABORT, 'Human Document source boundaries are audit records'); END;
CREATE TRIGGER human_document_export_immutable
BEFORE UPDATE ON human_document_exports
BEGIN SELECT RAISE(ABORT, 'Human Document exports are immutable'); END;
CREATE TRIGGER human_document_export_no_delete
BEFORE DELETE ON human_document_exports
BEGIN SELECT RAISE(ABORT, 'Human Document exports are audit records'); END;
