# CP4 — Development Core Implementation

> **Status:** PASS
>
> **Completed:** 2026-09-03
>
> **Build:** `continuum-core 0.4.0` / schema v4

## Outcome

CP4 implements a deterministic, local-first Development Space on CP2/CP3. Development-only work can attach and observe Git, ingest history, describe intent, link Requirements and ChangeSets, bookmark progress, resume after repository drift, report, export, restore, and pass integrity verification without Research Space or AI.

## Implemented Surface

- exact, read-only local repository attachment;
- explicit Repository relocation with identity-continuity proof;
- immutable clean, dirty, detached, and unborn RepositoryBaselines;
- fast-forward, worktree-change, branch-switch, and rewrite reconciliation;
- bounded incremental commit metadata and first-parent merge diff ingestion;
- exact add/copy/delete/modify/rename/type/unmerged file observations;
- Development-only Requirement creation with honest rationale origin;
- shared Requirement progression without Research capability dependency;
- working-tree and committed ChangeSets;
- reachable-commit and same-Repository validation;
- explicit ChangeSet supersession and typed Requirement implementation links;
- immutable Development timeline and deterministic search projection;
- Development Checkpoint, live divergence-aware resume, and deterministic local report retained as the ADR-007 compatibility/content baseline;
- Development-aware integrity diagnostics;
- schema v4 migration with v1/v2/v3 backup and preservation tests.

## Code Map

```text
crates/continuum-core/
├── migrations/0004_development_core.sql
├── src/development.rs
├── src/store.rs
├── examples/cp4_benchmark.rs
└── tests/cp4_development_acceptance.rs
```

## Preserved Guarantees

- CP2 atomicity, idempotency, audit/outbox, identity, artifact, checkpoint, export/restore, and migration behavior;
- CP3 Research-only behavior and Research-owned Requirement provenance;
- one project with optional Spaces and no placeholder records;
- Git remains authoritative and unmodified;
- no AI canonical write authority;
- no fabricated upstream research history;
- no CP5 structural-analysis claim.

## Explicit Non-Claims

CP4 does not parse ASTs, discover symbols/dependencies/configurations/tests, run tests, calculate semantic similarity, invoke model providers, capture media, produce HTML-first Human Documentation/styled visual documentation, build semantic Context Packs, expose MCP, or certify Windows/release stress. These remain CP5–CP12 responsibilities. ADR-007 introduces no CP4 data migration and does not invalidate the tested local report.
