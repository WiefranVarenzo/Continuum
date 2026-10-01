# Continuum Data Architecture

> **Status:** Approved baseline; provider/MCP extension plan accepted 2026-09-03; implemented through schema v12 (CP11)
> **Storage model:** SQLite canonical metadata + project-local content-addressed Artifact Store.

## 1. Data Authority

SQLite owns canonical structured state, relationships, audit sequence, jobs, policy metadata, and artifact references. The Artifact Store owns payload bytes. Git remains authoritative for repository objects; Continuum stores immutable observations and optional bounded snapshots, not a replacement repository.

AI caches, search indexes, graph layouts, Human Document composition caches, and report render caches are derived and rebuildable unless explicitly saved as GeneratedArtifacts. HTML and Markdown projections never replace canonical entities, relationships, events, or artifacts.

Saved Human Documentation records its Human Document schema, source checkpoint/ledger position, source entity IDs, freshness, privacy/omission metadata, renderer/template/asset versions, and content/asset hashes. CP8 may add normalized metadata or versioned GeneratedArtifact data required by that contract; this additive presentation work does not require CP2–CP5 canonical entities to migrate.

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

Space-specific normalized tables reference `entities.id` and are added in CP3–CP6. Frequently queried lifecycle, type, version, time, origin, source/target, hash, sequence, and job-state fields remain normalized and indexed; JSON is reserved for bounded versioned extensions, not opaque replacement of the domain model.

CP3 adds normalized `research_sessions`, `research_questions`, `evidence`, `experiments`, `results`, `findings`, `decisions`, `requirements`, `research_session_items`, append-only `research_timeline`, and rebuildable `research_search_documents`. Database type guards and the typed application API prevent partial Research aggregates.

CP4 extends shared `requirements` with an explicit creation Space and adds `repositories`, immutable `repository_baselines`, immutable `git_commit_observations` and `git_commit_file_changes`, append-only `repository_reconciliations`, `change_sets`, `change_set_commits`, `change_set_file_changes`, append-only `development_timeline`, and rebuildable `development_search_documents`. Git objects remain external authority; normalized rows preserve exact observations and typed intent without storing repository credentials or replacing `.git`.

CP5 adds immutable `analysis_runs`, `analyzer_executions`, `code_entity_observations`, `test_observations`, `test_runs`, `test_run_results`, and `analysis_limitations`; stable `code_entities`, temporal `code_entity_aliases`, and `tests`; plus rebuildable `analyzer_cache`. Every structural observation cites an exact CP4 baseline. Schema v6 adds alias first/last/retired coordinates and bounded-cache byte/access metadata. The cache key includes content, language, analyzer, analyzer version, and output schema, so it can be discarded without losing canonical truth.

CP6 adds `learning_feedback` and append-only `relationship_history`, while extending `relationships` with optimistic state version, annotation, reviewer, and retirement metadata. Schema v7 keeps SQLite as the graph authority and uses indexed inbound/outbound adjacency; it does not add Neo4j or another canonical store. Pre-CP6 relationships receive an explicit migration baseline at ledger sequence 0 rather than fabricated historical ordering.

CP7 adds `ai_provider_profiles`, `ai_entity_classification`, `ai_project_policy`, `ai_consents`, `ai_semantic_tasks`, `ai_attempts`, `ai_candidates`, and rebuildable `ai_cache_entries`. Schema v8 stores non-secret destination/capability/policy metadata, bounded source IDs and fingerprints, sanitized attempt metadata, validated candidate JSON, and human-review state. It never stores API keys, OAuth secrets, bearer tokens, authorization headers, or raw credential material.

CP8 adds immutable `human_documents`, `human_document_sources`, and `human_document_exports`. Schema v9 stores the bounded renderer-neutral document JSON, source version/hash snapshots, audience and aggregate classification, material fingerprint, Artifact link, renderer/template versions, output hash, and zero-network asset manifest. HTML, Markdown, SVG, graph coordinates, and UI state remain derived.

CP9 adds `capture_sessions`, `capture_session_sources`, append-only `capture_permission_events`, immutable `capture_segments`, immutable `capture_markers`, `capture_evidence_links`, `capture_external_items`, and immutable `capture_derivations`. Schema v10 stores capability/backend/encoding/buffer snapshots, versioned lifecycle and failure/recovery state, ordered media-fragment coordinates and hashes, source-preserving Evidence references, and derivation provenance. Capture bytes remain in the Artifact Store.

CP10 adds immutable `checkpoint_envelopes`, `checkpoint_artifact_sources`, `context_pack_records`, `context_pack_sources`, and `context_pack_generated_artifacts`. Schema v11 extends, rather than replaces, the stable CP2 `checkpoints`, `checkpoint_sources`, `context_packs`, and `generated_artifacts` tables. It stores source versions/hashes, repository/capture/privacy snapshots, request/content fingerprints, audience, freshness/profile, estimator/budget usage, explicit omissions, and GeneratedArtifact linkage. Ephemeral previews create no rows.

CP11 adds `mcp_client_grants`, `mcp_sessions`, `mcp_rate_buckets`, `mcp_audit_log`, and `external_proposals`. Schema v12 stores only SHA-256 token digests, immutable grant authority, negotiated session metadata, durable rate counters, sanitized audit facts, and immutable source-referenced proposal payloads/fingerprints. Proposal review is versioned; raw tokens, provider credentials, and denied response content never enter these tables.

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

SQLite FTS supports local text search. Graph adjacency uses indexed relationships. Current-state/project-overview projections may be materialized when benchmarks justify them. CP10 benchmarks select deterministic Context Packs well inside target without embeddings, so embeddings remain disabled. Any future embedding store must be optional, derived, versioned, privacy-classified, rebuildable, and approved through a separate ADR and benchmark.

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

Checkpoints and canonical audit history are retained by default. Saved generated artifacts follow user retention. AI cache defaults to bounded LRU and 30 days; provider-attempt and request audit metadata defaults to 90 days without prohibited raw content. Ephemeral Context Packs disappear after task/session unless explicitly saved. External client session/audit retention is bounded by release policy; reviewed proposals remain separate provenance records, and any later materialized entity cites them through the normal command path. Artifact GC requires zero live references plus a 30-day recovery window; secret purge may bypass the window with explicit confirmation.

## 12. Encryption and Credentials

MVP relies on OS account and filesystem/disk encryption for project-at-rest protection. Application-level database encryption is deferred and documented as a limitation. All model-provider credentials and remote MCP authentication secrets use OS credential storage or an equivalent approved external secret facility. Credentials, environment secrets, OAuth material, and raw provider/client tokens never enter SQLite, exports, logs, Context Packs, or generated artifacts.

## 13. Integrity SLOs

- foreign keys enabled for every connection;
- `PRAGMA integrity_check` available on demand and lightweight checks on open;
- no committed live artifact reference without verified payload availability;
- command idempotency prevents duplicate retry effects;
- backup/restore preserves IDs, versions, sequence, relationships, and hashes;
- derived indexes can be discarded and rebuilt without canonical loss.
