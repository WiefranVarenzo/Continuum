-- Reviewed external proposals are first-class report sources, not AI candidates.
CREATE TABLE human_document_sources_next (
    document_id TEXT NOT NULL REFERENCES human_documents(id) ON DELETE RESTRICT,
    source_kind TEXT NOT NULL CHECK(source_kind IN ('entity','artifact','ai_candidate','external_proposal')),
    source_id TEXT NOT NULL,
    source_version TEXT NOT NULL,
    classification TEXT NOT NULL CHECK(classification IN ('public','internal','sensitive','secret','never_send')),
    PRIMARY KEY(document_id,source_kind,source_id)
);
INSERT INTO human_document_sources_next SELECT * FROM human_document_sources;
DROP TRIGGER human_document_source_immutable;
DROP TRIGGER human_document_source_no_delete;
DROP TABLE human_document_sources;
ALTER TABLE human_document_sources_next RENAME TO human_document_sources;
CREATE INDEX idx_human_document_sources_lookup ON human_document_sources(source_kind,source_id,document_id);
CREATE TRIGGER human_document_source_immutable BEFORE UPDATE ON human_document_sources
BEGIN SELECT RAISE(ABORT, 'Human Document source boundaries are immutable'); END;
CREATE TRIGGER human_document_source_no_delete BEFORE DELETE ON human_document_sources
BEGIN SELECT RAISE(ABORT, 'Human Document source boundaries are audit records'); END;
