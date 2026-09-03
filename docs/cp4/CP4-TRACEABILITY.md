# CP4 — Requirements and Acceptance Traceability

> **Status:** Complete

## PRD Development Requirements

- FR-DS-001 → exact local Git top-level attachment, stable Continuum identity, read-only adapter, and explicit relocation.
- FR-DS-002 → immutable RepositoryBaseline with HEAD/ref/branch/worktree identity and adapter version.
- FR-DS-003 → unique Repository/OID CommitObservation, bounded batch ingestion, idempotent retry/re-ingestion.
- FR-DS-004 → ordered file-level diff metadata with explicit old/new paths, rename similarity, and no required source-content retention.
- FR-DS-005 → working-tree/committed ChangeSets and canonical implements/partial/reverts Requirement links.
- FR-DS-006–010 → file/diff handoff foundation implemented; structural CodeEntity, analyzer, Test, and TestRun ownership remains CP5.
- FR-DS-011 → correctly deferred to CP7; CP4 rejects AI actors from canonical Development commands.
- FR-DS-012 → baseline reconciliation fixtures for branch switch, amend/history rewrite, and rename preservation.
- FR-DS-013 → complete Development-only checkpoint/report/export/restore fixture with Research disabled.
- FR-DS-014 → typed Development Requirement command accepts user/import/external/legacy/unknown origin.
- FR-DS-015 → deterministic report exposes Requirement rationale and ChangeSet intent origin without fabricated research.

## Development Acceptance Criteria

- AC-DS-01 → `unchanged_baseline_and_commit_reingestion_are_idempotent`.
- AC-DS-02 → CP4 preserves file/diff fallback; CP5 must prove unsupported structural analyzer fallback.
- AC-DS-03 → `rename_branch_switch_and_amend_preserve_distinct_observations` and immutable baseline guards.
- AC-DS-04 → Requirement state is independent now; Test status remains a CP5 acceptance extension.
- AC-DS-05 → `development_only_cycle_is_reproducible_resumable_and_has_honest_origin` including export/restore.
- AC-DS-06 → the same end-to-end fixture checks external origin and Research report exclusion.

## Core, Context, and Security Regression

- AC-DATA-01 → invalid/disabled/empty/wrong-type commands leave no partial Development record.
- AC-DATA-03 → identical baseline and commit re-ingestion do not duplicate canonical state.
- AC-DATA-04 → v1, v2, and v3 fixtures back up and migrate to schema v4.
- AC-DATA-05 → CP4 export/restore preserves Repository, baseline, commit, ChangeSet, links, and origins.
- AC-CTX-01/07/08 → immutable mid-work Development Checkpoint at an exact pre-event ledger sequence.
- FR-CTX-005 → live unobserved HEAD/worktree divergence marks a saved checkpoint stale.
- AC-AI-01 foundation → AI actor cannot execute canonical Development commands.
- SEC-003/004 foundation → direct argument-vector Git invocation, bounded output/deadline, external diff disabled, repository-local executable filter/diff configuration rejected, canonical paths, and no repository-content execution.

## Architecture Invariants

- Git authority versus Continuum observation → real Git fixtures plus immutable SQL guards;
- stable identity across path move → `explicit_repository_relocation_preserves_continuum_identity_and_history`;
- no synthetic provenance → shared Requirement origin rules and Development report assertions;
- seamless optional bridge → accepted Research Decision produces one shared Requirement identity consumed by CP4;
- exact baseline/worktree identity → streamed content fingerprint, two-pass race check, and committed ChangeSet ancestry validation;
- draft versus committed identity → explicit fingerprint and supersession fixture;
- rename preservation → NUL-delimited parser unit test and real `git mv` fixture;
- closed relationship matrix → Development relationship-policy unit test;
- bounded resources → process timeout/output, commit/path/member/text/JSON/checkpoint/report limits;
- repairability → Development-aware integrity diagnostics.

## Verification Files

- unit/migration tests: `src/development.rs`, `src/store.rs`;
- CP2 regression: `tests/cp2_acceptance.rs`;
- CP3 regression: `tests/cp3_research_acceptance.rs`;
- CP4 acceptance: `tests/cp4_development_acceptance.rs`;
- performance harness: `examples/cp4_benchmark.rs`;
- performance evidence: `docs/cp4/CP4-BENCHMARK-2026-09-03.json`.
