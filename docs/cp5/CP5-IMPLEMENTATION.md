# CP5 — Code Intelligence Implementation

> **Status:** PASS
>
> **Completed:** 2026-09-04
>
> **Build:** `continuum-core 0.5.0` / schema v5

## Outcome

CP5 implements deterministic Code Intelligence on exact CP4 RepositoryBaselines. A Development-only project can analyze committed code, discover supported structure and tests, inspect dependency/config declarations, connect ChangeSets and verification, record externally executed test outcomes, bookmark/resume/report, export/restore, and run integrity checks without Research or AI.

## Implemented Surface

- immutable AnalysisRun with source/analyzer/schema coordinate, counts, timing, limits, cache metrics, completeness, and limitations;
- stable CodeEntity and Test identity with baseline-specific immutable observations;
- content-preserving file rename identity and path aliases;
- explicit unavailable/reactivation lifecycle for deleted and reappearing files, symbols, and tests;
- safe reapplication when returning to an already-analyzed baseline;
- embedded Tree-sitter/ast-grep analysis for Rust, JavaScript/JSX, TypeScript/TSX, Python, and JSON;
- Cargo/npm/Python dependency parsing and secret-safe configuration metadata;
- Rust/Python/JavaScript-style Test discovery;
- versioned content-hash analyzer cache;
- one-process bounded Git blob batching;
- file-level fallback for unsupported/binary/oversized content and honest dirty/unborn/parse-error limitations;
- automatic Repository `defines` and ChangeSet `modifies` links;
- order-independent and deletion-aware ChangeSet-to-file linking;
- user-authored Test `verifies` links;
- immutable imported TestRun and per-Test results;
- schema-v2 Development Checkpoint and deterministic report/search/timeline integration;
- CP5-aware integrity diagnostics, migration backup, and export/restore.

## Public Application Surface

```text
analyze_repository_baseline
get_analysis_run
get_code_entity
get_test
record_test_run
get_test_run
link_test_verification
```

Existing Development search, timeline, checkpoint, resume, report, export/import, and integrity APIs now understand the CP5 entity types.

## Code Map

```text
crates/continuum-core/
├── migrations/0005_code_intelligence.sql
├── src/code_intelligence.rs
├── src/development.rs
├── src/store.rs
├── examples/cp5_benchmark.rs
└── tests/cp5_code_intelligence_acceptance.rs
```

## Preserved Guarantees

- CP2 atomicity, identity, idempotency, audit/outbox, typed relation, export/restore, and migration behavior;
- CP3 Research-only behavior and Decision-backed research Requirement origin;
- CP4 read-only Git authority, exact baselines, immutable commit/diff history, and rewrite/rename handling;
- independent Development-only use with no placeholder Research data;
- deterministic truth remains canonical; AI remains proposal-only and is not invoked;
- limitations and absence remain explicit rather than converted into confident structural claims.

## Explicit Non-Claims

CP5 does not run tests, compile/type-check source, resolve dynamic imports or call graphs, scan dependency vulnerabilities, infer requirements/decisions, construct the full CP6 provenance graph, call AI providers, capture media, build semantic Context Packs, expose MCP, or certify the CP12 Standard/stress corpus.
