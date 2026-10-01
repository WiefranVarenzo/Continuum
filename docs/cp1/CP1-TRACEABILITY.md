# CP1 Traceability Matrix

> **Status:** Complete  
> **Method:** Requirement families map to authoritative architecture and planned verification. Individual IDs inherit their family mapping unless explicitly listed.

## Product Direction

- G-01–G-13, NG-01–NG-11 → PRD Sections 3–8; Architecture Sections 1–4; Spaces Sections 1–7 → document consistency review and three usage fixtures.
- Research-only/Development-only/Connected R&D → ADR-001; Spaces; Domain Sections 2–4; Diagrams 3–5 → Fixtures A–C.

## Provenance and Domain

- PRV-01–PRV-07 → Domain Sections 1, 4–9; AI Sections 4–6; Checkpoint Sections 2–9 → provenance graph fixture, invalid-source and staleness review.
- FR-RS-001–014 → Domain Research ownership/lifecycles; Spaces Section 2 → Fixture A; CP3 implementation tests.
- FR-DS-001–015 → Domain Development ownership/lifecycles; Spaces Section 3; Data Section 7 → CP4 Repository/ChangeSet evidence and CP5 structural-analysis extensions.
- FR-BRIDGE-001–006 → ADR-001; Spaces Sections 4–6; Domain relationship model → Fixture C; implemented and validated by CP6 tests.
- FR-KG-001–006 → Domain relationship/provenance; Architecture adapter boundary; Diagram 5 → implemented by schema v7 traversal, history, Learning Feedback, and CP6 graph validation tests.

## Continuity Core and Data

- FR-CORE-001–015 → Architecture Sections 3–13; Data Sections 1–13; Domain common envelope/invariants → CP2 integration, migration, artifact, idempotency, and recovery tests.
- NFR-REL-001–005 → Data transactions/migration/backup; Architecture failure domains → crash-injection and restore fixtures.
- NFR-OFF-001–003 → Architecture local deployment; AI failure/degradation → offline acceptance fixture.
- NFR-MNT-001–004 → Architecture ports/versioning; ADR set → contract/version review.
- NFR-PORT-001–002 → Data export/restore; platform ports → CP2 export contract and later adapter tests.

## AI

- FR-AI-001–015 → ADR-004 and ADR-006; AI Architecture Sections 1–15; Security privacy gateway → CP7 schema, grounding, routing/failover, capability, provider-isolation, compatibility-drift, injection, cache, and offline tests.
- AI/deterministic policy → Architecture invariant 5–7; AI pipeline; Diagram 7–8 → adversarial fixture.

## Visualization, Capture, and External Interface

- FR-VIS-001–015 → CP8 PASS: Human Document schema, deterministic composition, HTML/Markdown parity, offline export, bounded interactive graph, render/citation/staleness, accessibility, and injection tests; see `docs/cp8/CP8-TRACEABILITY.md` and `docs/cp8/CP8-VALIDATION.md`.
- FR-CAP-001–010 → CP9 PASS: schema v10 capture lifecycle, OS/WebView-mediated adapter, permissions/indicator, segmented recovery, Evidence/derivation linkage, bounds, migration, UI, and benchmark; see `docs/cp9/CP9-TRACEABILITY.md` and `docs/cp9/CP9-VALIDATION.md`. Live release-platform qualification remains CP12.
- FR-ACI-001–010 → ADR-006; MCP Continuity Interface Sections 1–13; Architecture ports; Security MCP boundary; Decisions D-020/D-027/D-029/D-032 → CP11 protocol, client-compatibility, cross-project, revocation, authorization, pagination, and proposal tests.

## Checkpoint and Context

- FR-CTX-001–017 → CP10 PASS: schema v11 immutable semantic envelope, exact source boundary, Then/Since/Now/Next, freshness/comparison, deterministic progressive retrieval, privacy/budget/omission controls, preview/removal, atomic saved pack and GeneratedArtifact; see `docs/cp10/CP10-TRACEABILITY.md` and `docs/cp10/CP10-VALIDATION.md`.
- AC-CTX-01–08 → CP10 PASS with 11 core acceptance scenarios, three desktop interaction tests, strict bridge validation, migration/integrity fixtures, and target-hardware benchmark.

## Security and Privacy

- SEC-001–025 → Security Sections 1–8; Human Documentation Architecture Sections 5–7 and 12; AI Architecture Sections 7–11; MCP Continuity Interface Sections 3–11; Data Sections 10–12 → secret canary, routing/failover, traversal, consent, credential, diagnostic, HTML/diagram injection, offline report, cross-project, revocation, and authorization tests.
- NFR-OBS-001–004 → Security logging; Architecture event/job correlation → diagnostic scrub fixture.

## Performance

- PERF-001–019 → Performance Sections 1–7 and PRD local MCP/report targets → versioned benchmark harness and reference-device reports.
- NFR-UX-001–009 → Architecture UI responsibility; Human Documentation Architecture; Spaces UX rules → accessibility, progressive-disclosure, readable-theme, zoom/reflow, and long-job interaction tests in owning checkpoints.

## Acceptance Ownership

- AC-CP1-01–12 → CP1 Validation and Acceptance Checklist.
- AC-DATA-01–05 → CP2.
- AC-RS-01–06 → CP3.
- AC-DS-01/03/05/06 → CP4; AC-DS-02/04 completed with CP5 analyzer/Test extensions.
- AC-BRIDGE-01–04 → CP6 PASS.
- AC-AI-01–08 → CP7 PASS; see `docs/cp7/CP7-TRACEABILITY.md` and `CP7-VALIDATION.md`.
- AC-MCP-01–06 → CP11.
- AC-CTX-01–08 → CP10 PASS, with legacy envelope foundations retained from CP2.
- AC-VIS-01–08 → CP8 PASS, backed by CP2–CP7 source/provenance/AI contracts; installer-level cross-platform, extended accessibility, and release security certification remain CP12 responsibilities.
- AC-PERF-01–04 and AC-SEC-01–05 → continuous validation, final release gate CP12.

No PRD requirement family is architecturally ownerless. Detailed test cases are added by the checkpoint that implements the behavior; they must reference the stable requirement IDs above.
