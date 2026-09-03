# CP1 Traceability Matrix

> **Status:** Complete  
> **Method:** Requirement families map to authoritative architecture and planned verification. Individual IDs inherit their family mapping unless explicitly listed.

## Product Direction

- G-01–G-13, NG-01–NG-11 → PRD Sections 3–8; Architecture Sections 1–4; Spaces Sections 1–7 → document consistency review and three usage fixtures.
- Research-only/Development-only/Connected R&D → ADR-001; Spaces; Domain Sections 2–4; Diagrams 3–5 → Fixtures A–C.

## Provenance and Domain

- PRV-01–PRV-07 → Domain Sections 1, 4–9; AI Sections 4–6; Checkpoint Sections 2–9 → provenance graph fixture, invalid-source and staleness review.
- FR-RS-001–014 → Domain Research ownership/lifecycles; Spaces Section 2 → Fixture A; CP3 implementation tests.
- FR-DS-001–015 → Domain Development ownership/lifecycles; Spaces Section 3; Data Section 7 → Fixture B; CP4–CP5 tests.
- FR-BRIDGE-001–006 → ADR-001; Spaces Sections 4–6; Domain relationship model → Fixture C; CP6 tests.
- FR-KG-001–006 → Domain relationship/provenance; Architecture adapter boundary; Diagram 5 → CP6 graph validation tests.

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

- FR-VIS-001–007 → Architecture adapters; Performance graph budget; Diagram sources → CP8 render/citation/staleness tests.
- FR-CAP-001–010 → Security capture consent; Architecture adapter/failure boundary; Performance streaming/backpressure → CP9 platform tests.
- FR-ACI-001–010 → ADR-006; MCP Continuity Interface Sections 1–13; Architecture ports; Security MCP boundary; Decisions D-020/D-027/D-029/D-032 → CP11 protocol, client-compatibility, cross-project, revocation, authorization, pagination, and proposal tests.

## Checkpoint and Context

- FR-CTX-001–017 → ADR-005; Checkpoint and Context Sections 1–11; Diagrams 10–11 → Fixtures A–D; CP2 envelope tests and CP10 engine tests.
- AC-CTX-01–08 → Checkpoint immutable envelope, resume/staleness, budget/privacy rules → golden checkpoint/context fixtures.

## Security and Privacy

- SEC-001–022 → Security Sections 1–8; AI Architecture Sections 7–11; MCP Continuity Interface Sections 3–11; Data Sections 10–12 → secret canary, routing/failover, traversal, consent, credential, diagnostic, cross-project, revocation, and authorization tests.
- NFR-OBS-001–004 → Security logging; Architecture event/job correlation → diagnostic scrub fixture.

## Performance

- PERF-001–017 → Performance Sections 1–7 and PRD local MCP overhead target → versioned benchmark harness and reference-device reports.
- NFR-UX-001–005 → Architecture UI responsibility; Spaces UX rules → accessibility and long-job interaction tests in owning checkpoints.

## Acceptance Ownership

- AC-CP1-01–12 → CP1 Validation and Acceptance Checklist.
- AC-DATA-01–05 → CP2.
- AC-RS-01–06 → CP3.
- AC-DS-01–06 → CP4–CP5.
- AC-BRIDGE-01–04 → CP6.
- AC-AI-01–08 → CP7.
- AC-MCP-01–06 → CP11.
- AC-CTX-01–08 → CP10, with envelope foundations in CP2.
- AC-PERF-01–04 and AC-SEC-01–05 → continuous validation, final release gate CP12.

No PRD requirement family is architecturally ownerless. Detailed test cases are added by the checkpoint that implements the behavior; they must reference the stable requirement IDs above.
