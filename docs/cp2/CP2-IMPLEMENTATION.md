# CP2 — Continuity Core Implementation

> **Status:** CP2.1 contract-aligned and validated  
> Source architecture: approved CP1 pack  
> Scope boundary: CP2 foundations only

## Outcome

CP2 converts the CP1 system contracts into a deterministic Rust library and project-local storage layout. Continuity Core is always active. Research and Development capabilities are optional flags over the same project identity and do not create placeholder domain entities.

## Implemented boundaries

- Project manifest, lifecycle identity, and isolated project directory.
- SQLite WAL ledger with foreign keys, forward-only checksummed migrations, indexes, optimistic entity versions, command idempotency, and transaction boundaries.
- Complete entity envelopes with schema version, canonical origin vocabulary, metadata, actor attribution, archive timestamp, and optimistic versions.
- Versioned typed relationships with endpoint types, lifecycle, actor, confidence/review state, direct sources, and explicit supersession.
- Versioned command and event envelopes with actor, correlation/causation, payload version, aggregate ID, monotonic ledger sequence, append-only audit, and leased transactional outbox.
- SHA-256 content-addressed Artifact Store with restrictive staging, atomic finalization, privacy classification, availability/purge lifecycle, deduplication, quarantine, and integrity diagnostics.
- Immutable, scope-aware Checkpoint envelope at an exact ledger position.
- Bounded durable jobs with idempotency, leases, progress, cooperative cancellation, explicit retry, and terminal states.
- Verified database backup plus project export/import baseline that preserves IDs, history, schema, and artifact hashes.
- Stable Rust API ports for CP3 and later checkpoints.
- Forward-only schema v2 migration with automatic v1 backup and lossless origin/authorship mapping.

## Explicitly deferred

- Research Question, Evidence, Experiment, Result, Finding, Decision, and Requirement aggregates (CP3).
- Git ingestion, ChangeSets, and repository baseline behavior (CP4).
- Code intelligence (CP5), cross-Space knowledge graph behavior (CP6), provider-neutral AI (CP7), visual reports (CP8), capture (CP9), semantic Context Pack selection (CP10), Continuum MCP Server/external AI clients (CP11), and release hardening (CP12).

## Project layout

```text
<project>/
├── continuum.project.json
├── ledger.sqlite3
├── artifacts/sha256/aa/bb/<sha256>
├── staging/
├── quarantine/
├── backups/
└── derived/
```

## Locked invariants carried from CP1

1. Canonical truth is deterministic; AI is never required for a canonical write.
2. Every material canonical command, state mutation, audit event, and outbox record commits atomically.
3. UUIDv7 identifies canonical records; SHA-256 identifies artifact payload bytes.
4. Research-only, Development-only, and connected R&D all retain one project identity.
5. No unused Space receives fabricated entities or provenance.
6. Checkpoints are immutable bookmarks, can be created during incomplete work, and never imply completion.
7. File paths in manifests and artifacts remain project-local and exports/imports reject symlinks.
