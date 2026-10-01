# CP5 — Code Intelligence Implementation

> **Status:** PASS
>
> **Completed:** 2026-09-04
>
> **Build:** `continuum-core 0.5.0` / schema v6 (CP5.1 hardened)

## Outcome

CP5 implements deterministic Code Intelligence on exact CP4 RepositoryBaselines. A Development-only project can analyze committed code, discover supported structure and tests, inspect dependency/config declarations, connect ChangeSets and verification, record externally executed test outcomes, bookmark/resume/report, export/restore, and run integrity checks without Research or AI.

## Implemented Surface

- immutable AnalysisRun with source/analyzer/schema coordinate, counts, timing, limits, cache metrics, completeness, and limitations;
- stable CodeEntity and Test identity with baseline-specific immutable observations;
- content-preserving file rename identity and temporal path aliases that distinguish later path reuse;
- explicit unavailable/reactivation lifecycle for deleted and reappearing files, symbols, and tests;
- projection-safe historical analysis: only the latest reconciled baseline may update current presence;
- embedded Tree-sitter/ast-grep analysis for Rust, JavaScript/JSX, TypeScript/TSX, Python, and JSON;
- Cargo/npm/Python dependency parsing and secret-safe configuration metadata;
- Rust/Python/JavaScript-style Test discovery;
- versioned content-hash analyzer cache with byte accounting and a 256 MiB LRU fail-safe;
- one-process bounded Git blob batching;
- file-level fallback for unsupported/binary/oversized content and honest dirty/unborn/parse-error limitations;
- automatic Repository `defines` and ChangeSet `modifies` links;
- order-independent and deletion-aware ChangeSet-to-file linking;
- user-authored Test `verifies` links;
- immutable imported TestRun and per-Test results;
- schema-v2 Development Checkpoint and deterministic report/search/timeline integration, with the report retained as the ADR-007 compatibility/content baseline;
- CP5-aware integrity diagnostics, migration backup, and export/restore.
- artifact retry/orphan protection, archived-project write guards, atomic project/export staging, and no-follow local file opens across the hardened core.

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

ADR-007 changes no CP5 analyzer, identity, canonical schema, or validated report behavior. CP8 consumes these existing application facts through the renderer-neutral Human Document composition layer; it does not parse compatibility Markdown back into canonical data.

## Code Map

```text
crates/continuum-core/
├── migrations/0005_code_intelligence.sql
├── migrations/0006_cp5_1_hardening.sql
├── src/code_intelligence.rs
├── src/development.rs
├── src/store.rs
├── examples/cp5_benchmark.rs
└── tests/cp5_code_intelligence_acceptance.rs
```

The corrective design and regression evidence are recorded in `CP5.1-HARDENING.md`.

## Preserved Guarantees

- CP2 atomicity, identity, idempotency, audit/outbox, typed relation, export/restore, and migration behavior;
- CP3 Research-only behavior and Decision-backed research Requirement origin;
- CP4 read-only Git authority, exact baselines, immutable commit/diff history, and rewrite/rename handling;
- independent Development-only use with no placeholder Research data;
- deterministic truth remains canonical; AI remains proposal-only and is not invoked;
- limitations and absence remain explicit rather than converted into confident structural claims.

## Explicit Non-Claims

CP5 does not run tests, compile/type-check source, resolve dynamic imports or call graphs, scan dependency vulnerabilities, infer requirements/decisions, construct the full CP6 provenance graph, call AI providers, capture media, build semantic Context Packs, expose MCP, or certify the CP12 Standard/stress corpus.
