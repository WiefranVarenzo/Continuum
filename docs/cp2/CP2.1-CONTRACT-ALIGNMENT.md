# CP2.1 — Contract Alignment Closure

> **Result:** PASS  
> **Closed:** 2026-09-02  
> **Architecture impact:** none; this work implements already-approved CP1 contracts.

## Why this closure existed

The post-CP2 audit found that the original implementation was functionally sound but simplified several CP1 contracts. CP2.1 closes those differences before CP3 can depend on them. No Research, Development, AI, capture, or MCP behavior was pulled forward.

## Closed gaps

1. Entity envelope now stores schema version, canonical origin, bounded metadata, `created_by`, `updated_by`, and `archived_at`.
2. `OriginKind` now matches CP1 exactly: `user`, `deterministic`, `import`, `external`, `legacy`, `ai_proposal`, and `unknown`.
3. Command envelope stores command/payload versions, actor, expected version, idempotency key, issued time, correlation, causation, and sanitized payload.
4. Audit events store aggregate ID, event version, actor, correlation, and causation while remaining append-only.
5. Relationship records store endpoint types, relation version, lifecycle, actor, confidence, review state, direct sources, and supersession. Multiple versions of the same typed edge are supported.
6. Artifact metadata now includes privacy classification, availability, origin, actor, metadata, unavailable reason, and explicit auditable purge.
7. Durable jobs now support progress, cooperative cancellation, terminal cancellation, and bounded retry.
8. Transactional outbox now supports leases, attempts, failure release, retry, and completion.
9. Queries expose bounded pagination; integrity checks include `foreign_key_check` and lifecycle-aware artifact verification.
10. Schema v1 opens through an automatic verified pre-migration backup and lossless forward migration to v2.

## Compatibility policy

Legacy public helpers remain as deterministic-system compatibility wrappers. New CP3 code must use the `CommandContext` APIs whenever a real user/import/AI-proposal actor exists. Schema v1 origin values remain only in a retained compatibility column on entities; canonical reads and all new writes use the CP1 vocabulary.

## Evidence

- schema version: 2;
- crate version: 0.2.0;
- formatting: PASS;
- Clippy with warnings denied: PASS;
- 19 automated tests: PASS, 0 failures;
- v1 backup/migration fixture: PASS;
- export/import and corruption regression: PASS;
- reference-hardware benchmark: PASS for named CP2 interactive budgets.

## Remaining work by design

- Tauri/React application shell and user-facing Research workflow begin with CP3 integration; the core application boundary is now ready.
- Windows certification remains a CP12 release gate, while platform adapters must be respected from CP3 onward.
- Git history ingestion, semantic graph policy, privacy gateway transmission, and Context Pack selection remain in CP4, CP6, CP7, and CP10 respectively.
