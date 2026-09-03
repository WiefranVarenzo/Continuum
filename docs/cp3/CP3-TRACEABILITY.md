# CP3 — Requirements and Acceptance Traceability

> **Status:** Complete

## PRD Research Requirements

- FR-RS-001 → typed Question create/update/lifecycle/search plus optimistic version test.
- FR-RS-002 → session start/end, creation-time membership, later idempotent association, and closed-session rejection.
- FR-RS-003 → Evidence kinds for notes/files/web/screenshots/markers/recording segments plus source/Artifact metadata. Actual capture adapters remain CP9.
- FR-RS-004 → normalized original source fields separated from versioned annotations and summaries.
- FR-RS-005 → Experiment hypothesis/question, method, inputs, expected observation, and lifecycle.
- FR-RS-006 → independent Result aggregate and `produces` provenance.
- FR-RS-007 → Finding source assessments for support, challenge, context, and inconclusive evidence.
- FR-RS-008 → Decision option, alternatives, rationale, constraints, lifecycle, Finding provenance, and supersession.
- FR-RS-009 → immutable ledger-ordered filtered research timeline.
- FR-RS-010/011 → correctly deferred to CP7; CP3 rejects AI actors/origins from canonical state.
- FR-RS-012 → deterministic metadata/text filtering implemented; semantic retrieval remains CP10.
- FR-RS-013 → end-to-end Research fixture passes with Development disabled and zero repository dependency.
- FR-RS-014 → Finding and Decision operate and report without Requirement creation.

## Research Acceptance Criteria

- AC-RS-01 → `result_can_exist_without_a_finding_and_finding_can_mix_source_assessments`.
- AC-RS-02 → the same test verifies simultaneous supports/challenges/inconclusive links.
- AC-RS-03 → `decision_supersession_preserves_prior_rationale_and_requirement_is_optional`.
- AC-RS-04 → `evidence_original_source_is_immutable_while_annotations_are_versioned`; AI-origin canonical rejection test.
- AC-RS-05 → `research_only_cycle_is_auditable_resumable_and_requires_no_development` plus export/restore fixture.
- AC-RS-06 → optional-Requirement assertions in Decision supersession and full-cycle fixtures.

## Core and Context Regression

- AC-DATA-01 → invalid source/session/lifecycle/stale-version commands prove atomic rollback.
- AC-DATA-03 → Research create and session association retry with a different command ID but stable idempotency key.
- AC-DATA-04 → v1 and v2 migration fixtures automatically back up and migrate to v3.
- AC-DATA-05 → Research export/restore preserves IDs, source fields, origins, capability state, and integrity.
- AC-CTX-01 → Research Checkpoint records exact ledger position and source versions.
- AC-CTX-07/08 → `development_required: false`, no placeholders, `complete: false`, and mid-work creation.
- AC-AI-01 foundation → AI actor/origin cannot execute canonical Research commands.

## Architecture Invariants

- optional Space and stable project identity → capability test;
- typed atomic module writes → end-to-end and failure fixtures;
- source separated from interpretation → Evidence annotation fixture;
- Result separated from Finding → independent Result fixture;
- no synthetic provenance → relationship matrix and standalone fixtures;
- canonical links registered at Core entry points → generic bypass rejection;
- bounded resources → page, source-list, text/JSON, checkpoint, report, and benchmark limits;
- repairability → Research-aware integrity diagnostic fixture.

## Verification Files

- Unit/migration tests: `src/research.rs`, `src/store.rs`.
- CP2 regression: `tests/cp2_acceptance.rs`.
- CP3 acceptance: `tests/cp3_research_acceptance.rs`.
- Performance harness: `examples/cp3_benchmark.rs`.
- Performance evidence: `docs/cp3/CP3-BENCHMARK-2026-09-02.json`.
