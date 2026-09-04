# CP5 — Requirements and Acceptance Traceability

> **Status:** Complete

## PRD Development Requirements

- FR-DS-006 → CodeEntity/Test extraction, dependency/config analyzers, exact baseline observations, and acceptance fixture.
- FR-DS-007 → pinned analyzer bundle, per-analyzer execution/version/contract/output records, embedded Tree-sitter/ast-grep and TOML adapters.
- FR-DS-008 → unsupported, binary, oversized, dirty, unborn, and parse-error limitations preserve file identity without false structure.
- FR-DS-009 → ChangeSet `modifies` file CodeEntity, Repository `defines`, local Development search, and Test navigation.
- FR-DS-010 → distinct Test definitions, explicit `verifies` links, and immutable observed TestRuns/results.
- FR-DS-011 → correctly remains CP7; CP5 rejects AI actors and creates no semantic explanation.
- FR-DS-012 → CP4 rewrite/branch guarantees regress; CP5 real rename plus disappear/reappear fixtures preserve history/identity.
- FR-DS-013–015 → Development-only E2E includes checkpoint/report/export/restore with Research disabled and no invented rationale.

## Development Acceptance Criteria

- AC-DS-01 → identical coordinate returns one AnalysisRun; unchanged files reuse the combined analyzer-version cache; returning to an older coordinate reapplies its current projection safely.
- AC-DS-02 → `dirty_worktree_and_unborn_repository_degrade_without_stale_structural_claims`, unsupported and binary assertions.
- AC-DS-03 → CP4 regression plus `committed_rename_preserves_file_and_symbol_identity_when_content_is_unchanged`.
- AC-DS-04 → E2E separately records ChangeSet implementation, Test verification, and TestRun outcome; report has separate sections.
- AC-DS-05 → `development_only_code_intelligence_is_resumable_auditable_and_exportable`.
- AC-DS-06 → CP3/CP4 rationale-origin regressions remain green; CP5 adds no synthetic research entity.

## Core, Context, Security, and Performance

- AC-DATA-01 → invalid limits, inconsistent TestRun, wrong types, AI actor, and cross-Repository operations commit nothing.
- AC-DATA-03 → AnalysisRun idempotency and cache reuse test.
- AC-DATA-04 → v1/v2/v3/v4 backup-and-migrate tests; v4 rows remain unchanged while schema v5 tables start empty.
- AC-DATA-05 → E2E export/restore preserves AnalysisRun, CodeEntity, Test, TestRun, and relationships.
- AC-CTX-01/07/08 → Development Checkpoint schema v2 cites AnalysisRun/TestRun and becomes stale after later material events.
- SEC-003/004 → embedded pinned parsers, no source execution/network, direct bounded Git, exact batch framing, safe paths, hard resource ceilings, secret-safe manifest/config handling.
- PERF changed-file target → 500-file fixture with one new file completes incremental analysis in 200.318 ms against ≤5,000 ms target.
- PERF search/report → p95 CodeEntity search 2.543 ms against ≤500 ms; report 4.430 ms against ≤3,000 ms.

## Architecture Invariants

- Git is authoritative → only exact committed tree objects are parsed; dirty worktree claims are withheld.
- source addressability → every observation names RepositoryBaseline and AnalysisRun; ChangeSet linking is order independent and deletion-aware.
- deterministic identity → repository-scoped stable keys, rename evidence, and immutable observations.
- no fabricated confidence → `partial`, `parse_error`, `fallback`, and limitation codes are preserved.
- test definition ≠ execution result → separate Test, TestObservation, TestRun, and result tables.
- no AI authority → analyzer outputs are deterministic; canonical APIs reject AI proposal actors.
- partial/standalone continuity → Development capability alone supports the full CP5 journey.
- repairability → CP5 integrity checks emit stable issue codes and guidance.
- forward compatibility → analyzer/output versions and CP6-owned graph semantics are explicit.

## Verification Files

- unit/analyzer and migration tests: `src/code_intelligence.rs`, `src/development.rs`, `src/store.rs`;
- CP2 regression: `tests/cp2_acceptance.rs`;
- CP3 regression: `tests/cp3_research_acceptance.rs`;
- CP4 regression: `tests/cp4_development_acceptance.rs`;
- CP5 acceptance: `tests/cp5_code_intelligence_acceptance.rs`;
- performance harness: `examples/cp5_benchmark.rs`;
- performance evidence: `docs/cp5/CP5-BENCHMARK-2026-09-04.json`.
