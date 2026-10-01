-- Presentation state and authored documents belong to the project, not browser storage.
CREATE TABLE workspace_documents (
    project_id TEXT NOT NULL REFERENCES projects(id),
    document_key TEXT NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('board','report','messages','preferences')),
    revision INTEGER NOT NULL CHECK(revision > 0),
    payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
    updated_at TEXT NOT NULL,
    PRIMARY KEY(project_id,document_key)
);
CREATE TABLE workspace_document_revisions (
    project_id TEXT NOT NULL REFERENCES projects(id),
    document_key TEXT NOT NULL,
    revision INTEGER NOT NULL,
    payload_json TEXT NOT NULL CHECK(json_valid(payload_json)),
    created_at TEXT NOT NULL,
    PRIMARY KEY(project_id,document_key,revision)
);
CREATE TRIGGER workspace_revision_no_update BEFORE UPDATE ON workspace_document_revisions
BEGIN SELECT RAISE(ABORT,'workspace revisions are immutable'); END;
CREATE TRIGGER workspace_revision_no_delete BEFORE DELETE ON workspace_document_revisions
BEGIN SELECT RAISE(ABORT,'workspace revisions are immutable'); END;
