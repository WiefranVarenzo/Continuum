-- CP5.1 hardening: make path aliases temporal so rename, deletion, restore,
-- and later path reuse cannot collapse different files into one identity.

ALTER TABLE code_entity_aliases RENAME TO code_entity_aliases_legacy;

CREATE TABLE code_entity_aliases (
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    alias_kind TEXT NOT NULL CHECK(alias_kind IN ('path','qualified_name')),
    alias_value TEXT NOT NULL,
    code_entity_id TEXT NOT NULL REFERENCES code_entities(entity_id) ON DELETE RESTRICT,
    first_seen_baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    last_seen_baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    retired_at_baseline_id TEXT REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    PRIMARY KEY(repository_id, alias_kind, alias_value, code_entity_id)
);

INSERT INTO code_entity_aliases(
    repository_id,alias_kind,alias_value,code_entity_id,
    first_seen_baseline_id,last_seen_baseline_id,retired_at_baseline_id
)
SELECT repository_id,alias_kind,alias_value,code_entity_id,
       first_seen_baseline_id,first_seen_baseline_id,NULL
FROM code_entity_aliases_legacy;

DROP TABLE code_entity_aliases_legacy;

CREATE UNIQUE INDEX idx_code_entity_aliases_active
ON code_entity_aliases(repository_id,alias_kind,alias_value)
WHERE retired_at_baseline_id IS NULL;

CREATE INDEX idx_code_entity_aliases_entity
ON code_entity_aliases(code_entity_id,alias_kind,retired_at_baseline_id);

-- The analyzer cache is rebuildable, but must be bounded. Access metadata
-- enables deterministic least-recently-used eviction.
ALTER TABLE analyzer_cache RENAME TO analyzer_cache_legacy;

CREATE TABLE analyzer_cache (
    content_sha256 TEXT NOT NULL CHECK(length(content_sha256) = 64),
    language TEXT NOT NULL,
    analyzer_id TEXT NOT NULL,
    analyzer_version TEXT NOT NULL,
    output_schema_version INTEGER NOT NULL CHECK(output_schema_version > 0),
    output_json TEXT NOT NULL,
    byte_size INTEGER NOT NULL CHECK(byte_size >= 0),
    created_at TEXT NOT NULL,
    last_accessed_at TEXT NOT NULL,
    PRIMARY KEY(content_sha256, language, analyzer_id, analyzer_version, output_schema_version)
);

INSERT INTO analyzer_cache(
    content_sha256,language,analyzer_id,analyzer_version,output_schema_version,
    output_json,byte_size,created_at,last_accessed_at
)
SELECT content_sha256,language,analyzer_id,analyzer_version,output_schema_version,
       output_json,length(CAST(output_json AS BLOB)),created_at,created_at
FROM analyzer_cache_legacy;

DROP TABLE analyzer_cache_legacy;

CREATE INDEX idx_analyzer_cache_lru
ON analyzer_cache(last_accessed_at,created_at,content_sha256);

-- Keep the newest entries that fit the CP5.1 fail-safe budget. Canonical
-- observations are not stored in this table and are never evicted here.
DELETE FROM analyzer_cache
WHERE rowid IN (
    SELECT rowid FROM (
        SELECT rowid,
               sum(byte_size) OVER (
                   ORDER BY last_accessed_at DESC,created_at DESC,content_sha256 DESC
               ) AS newest_first_bytes
        FROM analyzer_cache
    )
    WHERE newest_first_bytes > 268435456
);
