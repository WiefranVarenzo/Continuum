-- CP4 adds deterministic, read-only Git observations and Development Space state.
-- Git remains authoritative for repository objects. Continuum stores immutable
-- observations tied to common-envelope entities and never rewrites Git history.

ALTER TABLE requirements ADD COLUMN created_in_space TEXT NOT NULL DEFAULT 'research'
    CHECK(created_in_space IN ('research','development'));

CREATE TABLE repositories (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    root_path TEXT NOT NULL,
    root_fingerprint TEXT NOT NULL CHECK(length(root_fingerprint) = 64),
    git_common_dir_fingerprint TEXT NOT NULL CHECK(length(git_common_dir_fingerprint) = 64),
    object_format TEXT NOT NULL,
    adapter_version TEXT NOT NULL,
    attached_at TEXT NOT NULL,
    last_observed_at TEXT,
    UNIQUE(project_id, root_path)
);

CREATE TABLE repository_baselines (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    head_oid TEXT NOT NULL,
    head_ref TEXT NOT NULL DEFAULT '',
    branch_name TEXT NOT NULL DEFAULT '',
    worktree_fingerprint TEXT NOT NULL CHECK(length(worktree_fingerprint) = 64),
    worktree_status_json TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    previous_baseline_id TEXT REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    relation_to_previous TEXT NOT NULL CHECK(relation_to_previous IN (
        'initial','unchanged','worktree_changed','fast_forward','branch_switch',
        'history_rewrite','detached_head','unborn'
    )),
    adapter_version TEXT NOT NULL,
    UNIQUE(repository_id, head_oid, head_ref, worktree_fingerprint)
);

CREATE TABLE git_commit_observations (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    first_observed_baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    commit_oid TEXT NOT NULL,
    tree_oid TEXT NOT NULL,
    parent_oids_json TEXT NOT NULL,
    author_name TEXT NOT NULL,
    author_email TEXT NOT NULL,
    authored_at TEXT NOT NULL,
    committed_at TEXT NOT NULL,
    subject TEXT NOT NULL,
    observed_at TEXT NOT NULL,
    adapter_version TEXT NOT NULL,
    UNIQUE(repository_id, commit_oid)
);

CREATE TABLE git_commit_file_changes (
    commit_entity_id TEXT NOT NULL REFERENCES git_commit_observations(entity_id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    change_kind TEXT NOT NULL CHECK(change_kind IN (
        'added','copied','deleted','modified','renamed','type_changed','unmerged','unknown'
    )),
    old_path TEXT,
    new_path TEXT NOT NULL,
    similarity INTEGER CHECK(similarity IS NULL OR similarity BETWEEN 0 AND 100),
    PRIMARY KEY(commit_entity_id, ordinal)
);

CREATE TABLE repository_reconciliations (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    previous_baseline_id TEXT REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    current_baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    relation_kind TEXT NOT NULL,
    details_json TEXT NOT NULL,
    observed_at TEXT NOT NULL
);

CREATE TABLE change_sets (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE RESTRICT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    repository_id TEXT NOT NULL REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    baseline_id TEXT NOT NULL REFERENCES repository_baselines(entity_id) ON DELETE RESTRICT,
    change_set_kind TEXT NOT NULL CHECK(change_set_kind IN ('working_tree','committed')),
    summary TEXT NOT NULL,
    intent_origin TEXT NOT NULL CHECK(intent_origin IN (
        'user','import','external','legacy','research','unknown'
    )),
    worktree_fingerprint TEXT,
    supersedes_change_set_id TEXT REFERENCES change_sets(entity_id) ON DELETE RESTRICT,
    created_at TEXT NOT NULL,
    CHECK(
        (change_set_kind='working_tree' AND worktree_fingerprint IS NOT NULL) OR
        (change_set_kind='committed' AND worktree_fingerprint IS NULL)
    )
);

CREATE TABLE change_set_commits (
    change_set_id TEXT NOT NULL REFERENCES change_sets(entity_id) ON DELETE RESTRICT,
    commit_entity_id TEXT NOT NULL REFERENCES git_commit_observations(entity_id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    PRIMARY KEY(change_set_id, commit_entity_id),
    UNIQUE(change_set_id, ordinal)
);

CREATE TABLE change_set_file_changes (
    change_set_id TEXT NOT NULL REFERENCES change_sets(entity_id) ON DELETE RESTRICT,
    ordinal INTEGER NOT NULL CHECK(ordinal >= 0),
    source_commit_entity_id TEXT REFERENCES git_commit_observations(entity_id) ON DELETE RESTRICT,
    change_kind TEXT NOT NULL,
    old_path TEXT,
    new_path TEXT NOT NULL,
    similarity INTEGER CHECK(similarity IS NULL OR similarity BETWEEN 0 AND 100),
    PRIMARY KEY(change_set_id, ordinal)
);

CREATE TABLE development_timeline (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    ledger_sequence INTEGER NOT NULL,
    repository_id TEXT REFERENCES repositories(entity_id) ON DELETE RESTRICT,
    entity_id TEXT REFERENCES entities(id) ON DELETE RESTRICT,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    actor_id TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    UNIQUE(project_id, ledger_sequence)
);

CREATE TABLE development_search_documents (
    entity_id TEXT PRIMARY KEY REFERENCES entities(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    entity_type TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_repositories_project ON repositories(project_id, entity_id);
CREATE INDEX idx_repository_baselines_latest
    ON repository_baselines(project_id, repository_id, observed_at, entity_id);
CREATE INDEX idx_git_commits_repository
    ON git_commit_observations(project_id, repository_id, committed_at, commit_oid);
CREATE INDEX idx_git_file_changes_path
    ON git_commit_file_changes(new_path, commit_entity_id);
CREATE INDEX idx_reconciliations_repository
    ON repository_reconciliations(project_id, repository_id, observed_at);
CREATE INDEX idx_change_sets_repository
    ON change_sets(project_id, repository_id, created_at, entity_id);
CREATE INDEX idx_change_set_commits_commit
    ON change_set_commits(commit_entity_id, change_set_id);
CREATE INDEX idx_development_timeline_sequence
    ON development_timeline(project_id, ledger_sequence, id);
CREATE INDEX idx_development_timeline_repository
    ON development_timeline(project_id, repository_id, ledger_sequence);
CREATE INDEX idx_development_search_type
    ON development_search_documents(project_id, entity_type, updated_at);

CREATE TRIGGER repositories_type_guard
BEFORE INSERT ON repositories
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'repository'
  OR COALESCE((SELECT project_id FROM entities WHERE id=NEW.entity_id),'') <> NEW.project_id
BEGIN SELECT RAISE(ABORT, 'repository entity type or project mismatch'); END;

CREATE TRIGGER repository_baselines_type_guard
BEFORE INSERT ON repository_baselines
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'repository_baseline'
  OR COALESCE((SELECT project_id FROM entities WHERE id=NEW.entity_id),'') <> NEW.project_id
  OR COALESCE((SELECT project_id FROM repositories WHERE entity_id=NEW.repository_id),'') <> NEW.project_id
BEGIN SELECT RAISE(ABORT, 'repository baseline type or project mismatch'); END;

CREATE TRIGGER git_commit_observations_type_guard
BEFORE INSERT ON git_commit_observations
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'commit_observation'
  OR COALESCE((SELECT project_id FROM entities WHERE id=NEW.entity_id),'') <> NEW.project_id
  OR COALESCE((SELECT project_id FROM repositories WHERE entity_id=NEW.repository_id),'') <> NEW.project_id
BEGIN SELECT RAISE(ABORT, 'commit observation type or project mismatch'); END;

CREATE TRIGGER change_sets_type_guard
BEFORE INSERT ON change_sets
WHEN COALESCE((SELECT entity_type FROM entities WHERE id=NEW.entity_id),'') <> 'change_set'
  OR COALESCE((SELECT project_id FROM entities WHERE id=NEW.entity_id),'') <> NEW.project_id
  OR COALESCE((SELECT project_id FROM repositories WHERE entity_id=NEW.repository_id),'') <> NEW.project_id
BEGIN SELECT RAISE(ABORT, 'change set type or project mismatch'); END;

CREATE TRIGGER repository_baselines_immutable_update
BEFORE UPDATE ON repository_baselines BEGIN SELECT RAISE(ABORT, 'repository baselines are immutable'); END;
CREATE TRIGGER repository_baselines_immutable_delete
BEFORE DELETE ON repository_baselines BEGIN SELECT RAISE(ABORT, 'repository baselines are immutable'); END;
CREATE TRIGGER git_commit_observations_immutable_update
BEFORE UPDATE ON git_commit_observations BEGIN SELECT RAISE(ABORT, 'commit observations are immutable'); END;
CREATE TRIGGER git_commit_observations_immutable_delete
BEFORE DELETE ON git_commit_observations BEGIN SELECT RAISE(ABORT, 'commit observations are immutable'); END;
CREATE TRIGGER git_commit_file_changes_immutable_update
BEFORE UPDATE ON git_commit_file_changes BEGIN SELECT RAISE(ABORT, 'commit file changes are immutable'); END;
CREATE TRIGGER git_commit_file_changes_immutable_delete
BEFORE DELETE ON git_commit_file_changes BEGIN SELECT RAISE(ABORT, 'commit file changes are immutable'); END;
CREATE TRIGGER repository_reconciliations_immutable_update
BEFORE UPDATE ON repository_reconciliations BEGIN SELECT RAISE(ABORT, 'repository reconciliation is immutable'); END;
CREATE TRIGGER repository_reconciliations_immutable_delete
BEFORE DELETE ON repository_reconciliations BEGIN SELECT RAISE(ABORT, 'repository reconciliation is immutable'); END;
CREATE TRIGGER development_timeline_immutable_update
BEFORE UPDATE ON development_timeline BEGIN SELECT RAISE(ABORT, 'development timeline is append-only'); END;
CREATE TRIGGER development_timeline_immutable_delete
BEFORE DELETE ON development_timeline BEGIN SELECT RAISE(ABORT, 'development timeline is append-only'); END;
