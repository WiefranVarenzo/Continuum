# CP6 — Validation Report

> **Decision:** CP6 PASS  
> **Validated:** 2026-09-08  
> **Core:** 0.6.0 / schema v7

## Scope Reviewed

CP6 was checked against the approved CP1 architecture, domain/data/Spaces/checkpoint/privacy/performance contracts, ADR-001/003/004/005/006/007, CP2.1 transaction and migration invariants, CP3 Research relationship matrix, CP4 Git/ChangeSet authority, and CP5.1 stable CodeEntity/Test identities.

## Verification

The full workspace validation includes:

- compiler check and formatting check;
- Clippy with warnings denied;
- 83 unit/acceptance/regression tests across CP2–CP6;
- connected R&D traversal from Evidence through Finding, Decision, Requirement, ChangeSet, CodeEntity, Test, and TestRun;
- Learning Feedback creation, target non-mutation, review history, resolution, export, and restore;
- Research-only and Development-only partial graph fixtures;
- cycle, cross-project, stale-version, oversized-query, corrupt-history/source, and claimed-provenance-gap failure paths;
- v1–v6 forward migration regression coverage through current schema;
- release-mode 500-node/1,497-edge benchmark.

## Performance Evidence

Fixture `graph-500-v1`, 30 iterations:

- one-hop p50 33.260 ms; p95 38.877 ms; target p95 ≤300 ms;
- scoped 500-node graph query p50 205.937 ms; p95 232.195 ms;
- full validation 111.594 ms;
- graph health: true.

The CP8 visual render target remains unclaimed because no browser layout or renderer is part of CP6. CP12 must repeat formal certification on Ryzen 5 5600H/16 GB and supported operating systems.

## Exit Decision

Standalone partial chains and the optional Connected R&D chain can be created, queried, validated, reviewed, audited, exported, restored, and traversed without AI. Learning Feedback crosses the bridge without forcing a loop or mutating its target. Queries are bounded and provide an honest truncation/omission contract.

**CP6 PASS — the deterministic provenance foundation is ready for CP7 Provider-Neutral Semantic Intelligence.**
