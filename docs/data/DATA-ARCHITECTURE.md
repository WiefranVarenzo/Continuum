# Continuum Data Architecture

> **Status:** Approved baseline; provider/MCP extension plan accepted 2026-09-03; implemented through schema v3
> **Storage model:** SQLite canonical metadata + project-local content-addressed Artifact Store.

## 1. Data Authority

SQLite owns canonical structured state, relationships, audit sequence, jobs, policy metadata, and artifact references. The Artifact Store owns payload bytes. Git remains authoritative for repository objects; Continuum stores immutable observations and optional bounded snapshots, not a replacement repository.

AI caches, search indexes, graph layouts, and report render caches are derived and rebuildable unless explicitly saved as GeneratedArtifacts.

Provider/model identities and MCP client labels are provenance, not canonical business authority. Provider credentials and MCP authentication secrets are never project data.

## 2. Project Layout

```text
project-root/
├── continuum.project.json
├── ledger.sqlite3
├── artifacts/
│   └── sha256/aa/bb/<full-hash>
├── staging/
├── quarantine/
├── backups/
└── derived/
```

The manifest contains project ID, format version, database relative path, capability flags, creation metadata, and compatibility information. Paths are validated as project-relative; symlink/traversal escape is rejected.

## 3. Conceptual SQLite Schema

Core tables:

- `projects`
- `space_capabilities`
- `entities` — common envelope and bounded JSON extension.
- `relationships`
- `artifacts`
- `entity_artifacts`
- `audit_events`
- `jobs`
- `checkpoints`
- `checkpoint_sources`
- `context_packs` for saved packs only.
- `generated_artifacts`
- `schema_migrations`
- `outbox`

Space-specific normalized tables reference `entities.id` and are added in CP3–CP5. Frequently queried lifecycle, type, version, time, origin, source/target, hash, sequence, and job-state fields remain normalized and indexed; JSON is reserved for bounded versioned extensions, not opaque replacement of the domain model.

CP3 adds normalized `research_sessions`, `research_questions`, `evidence`, `experiments`, `results`, `findings`, `decisions`, `requirements`, `research_session_items`, append-only `research_timeline`, and rebuildable `research_search_documents`. Database type guards and the typed application API prevent partial Research aggregates.

CP7 may add normalized `ai_provider_profiles` (non-secret configuration), `ai_tasks`, `ai_attempts`, `ai_candidates`, and `ai_candidate_sources`. CP11 may add `external_client_grants`, `external_client_sessions`, and `external_proposals`. Exact schemas belong to forward-only migrations in their owning checkpoints. No such table stores API keys, OAuth secrets, bearer tokens, or raw credential material.

## 4. Identity and Ordering

- UUIDv7 identifies projects, entities, relationships, commands, events, jobs, checkpoints, and artifacts metadata records.
- SHA-256 identifies payload content.
- integer `ledger_sequence` assigned transactionally orders committed material events within a project.
- UTC RFC3339 timestamps support display/audit but do not replace ledger ordering.
- optimistic entity version prevents lost updates.

## 5. Transaction Boundaries

A normal command validates input, opens a transaction, checks expected version/idempotency, changes canonical rows, appends audit/outbox events, and commits. Post-commit adapters consume outbox records.

Artifact ingestion uses:

1. stream to restrictive staging file while computing SHA-256 and size;
2. validate media/policy limits;
3. atomically move/deduplicate into content-addressed location;
4. commit Artifact metadata and entity link;
5. on failure, quarantine or remove staging data under recovery policy.

Integrity scans detect payload-without-row, row-without-payload, size/hash mismatch, invalid path, and dangling entity link.

## 6. Audit Events and Outbox

Continuum uses state plus append-only audit events, not full event sourcing. Events capture material business transitions with sanitized payloads. The transactional outbox guarantees that committed work can schedule analysis/index/cache invalidation after restart. Consumers are idempotent and record last processed event/unique key.

## 7. Working-Tree Changes

An uncommitted ChangeSet is a draft tied to a RepositoryBaseline plus deterministic working-tree fingerprint and captured diff metadata. Optional diff payload storage follows privacy/size policy. Later commits may explicitly supersede or realize the draft; similarity never silently rewrites identity.

## 8. Index and Projection Strategy

SQLite FTS supports local text search. Graph adjacency uses indexed relationships. Current-state/project-overview projections may be materialized when benchmarks justify them. Semantic embeddings are deferred to CP10 and disabled by default; their store must be derived, versioned, privacy-classified, and rebuildable.

## 9. Migration

- forward-only numbered migrations;
- application refuses unsupported future major schema;
- automatic consistent backup before migration;
- migration transaction where SQLite permits;
- external artifact transformations are staged with resumable journal;
- failure restores/preserves the prior usable project;
- migration tests cover every supported prior version.

## 10. Backup, Export, and Restore

Backup uses SQLite online backup plus immutable/hard-linked or copied referenced artifacts and a versioned manifest. Export verifies hashes and lists intentionally omitted payloads. Restore extracts to a new validated directory, blocks traversal/symlink escape, verifies manifest/database/artifacts, then atomically exposes the project.

## 11. Retention and Garbage Collection

Checkpoints and canonical audit history are retained by default. Saved generated artifacts follow user retention. AI cache defaults to bounded LRU and 30 days; provider-attempt and request audit metadata defaults to 90 days without prohibited raw content. Ephemeral Context Packs disappear after task/session unless explicitly saved. External client session/audit retention is bounded by the CP11 policy while immutable accepted proposal provenance follows the canonical entity it produced. Artifact GC requires zero live references plus a 30-day recovery window; secret purge may bypass the window with explicit confirmation.

## 12. Encryption and Credentials

MVP relies on OS account and filesystem/disk encryption for project-at-rest protection. Application-level database encryption is deferred and documented as a limitation. All model-provider credentials and remote MCP authentication secrets use OS credential storage or an equivalent approved external secret facility. Credentials, environment secrets, OAuth material, and raw provider/client tokens never enter SQLite, exports, logs, Context Packs, or generated artifacts.

## 13. Integrity SLOs

- foreign keys enabled for every connection;
- `PRAGMA integrity_check` available on demand and lightweight checks on open;
- no committed live artifact reference without verified payload availability;
- command idempotency prevents duplicate retry effects;
- backup/restore preserves IDs, versions, sequence, relationships, and hashes;
- derived indexes can be discarded and rebuilt without canonical loss.
