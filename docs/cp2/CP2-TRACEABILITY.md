# CP2 Traceability

> Status: Complete

## CP1 handoff to implementation

- Project manifest and lifecycle → `manifest.rs`, `ContinuityStore::create/open/set_project_archived`.
- SQLite migrations and core schema → `migrations/0001_core.sql`, checksummed transactional migration runner, pre-migration backup.
- UUIDv7 and origin/version/authorship metadata → `id.rs`, canonical CP1 origin vocabulary, complete entity envelope, command actor, and optimistic updates.
- Typed relationships and audit events → versioned relationship envelope, endpoint types, review/provenance/supersession, same-project enforcement, correlated actor-attributed events, monotonic ledger sequence, append-only triggers.
- Artifact Store → streaming stage/finalize, SHA-256 layout, privacy classification, availability/purge lifecycle, deduplication, entity links, quarantine/integrity diagnostics.
- Durable job state → bounded queue, idempotency key, claim lease, progress, cancellation, retry, attempts, terminal completion.
- Optional Spaces → Research/Development capability rows with no placeholder entities.
- Checkpoint envelope → scope-aware immutable snapshot at an exact pre-event ledger sequence.
- Backup/export baseline → SQLite online backup, verified export/import, symlink rejection.
- Command/query transaction boundary → versioned `CommandContext`, actor/correlation/causation, idempotency, atomic canonical state + audit + leased outbox, bounded pagination.
- Migration compatibility → schema v1 automatic backup and lossless forward migration to schema v2.

## PRD acceptance ownership

- AC-DATA-01 → `failed_multi_row_command_leaves_no_command_event_or_relationship`, `failed_migration_rolls_back_every_statement`.
- AC-DATA-02 → `artifact_store_deduplicates_and_detects_corruption_with_guidance`.
- AC-DATA-03 → capability/entity/relationship/artifact/link/job idempotency assertions.
- AC-DATA-04 → migration rollback, checksum tampering, and pre-migration backup implementation.
- AC-DATA-05 → `export_and_import_preserve_ids_history_and_hashes`.
- CP2.1 contract audit → authorship/correlation, relationship supersession, artifact policy/purge, job control, outbox retry, pagination, foreign-key integrity, and v1→v2 migration fixtures.
- AC-CTX-01/07/08 foundation → immutable scope and ledger-position Checkpoint test; semantic state computation remains CP10.
- AC-SEC-03/04 foundation → project-local safe paths, symlink rejection, content hash verification, transaction recovery.

## Deferred contract ports

The generic entity, relationship, artifact, checkpoint, job, and query APIs are the extension seam for CP3–CP11. Deferred domains must add their normalized schemas through new forward-only migrations and preserve every CP2 invariant.
