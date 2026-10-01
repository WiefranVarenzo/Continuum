# CP6 — Requirements and Acceptance Traceability

## R&D Bridge

- FR-BRIDGE-001/005/006 → one project graph and shared IDs; capability toggles never migrate/delete graph records; standalone acceptance fixture.
- FR-BRIDGE-002/003 → CP3 Decision→Requirement and CP4 ChangeSet→Requirement links participate in the same bounded traversal.
- FR-BRIDGE-004 → typed Learning Feedback creation, source/target links, lifecycle, and non-mutating target test.
- AC-BRIDGE-01/04 → earlier IDs/history survive second-Space activation/deactivation through unchanged Core contracts and regression suite.
- AC-BRIDGE-02 → complete Evidence/Finding/Decision/Requirement/ChangeSet/Code/Test/TestRun traversal fixture.
- AC-BRIDGE-03 → Learning Feedback targets an accepted Decision while its status remains unchanged.

## Provenance and Knowledge Graph

- FR-KG-001 → `traverse_provenance` inbound/outbound/both contract and complete connected fixture.
- FR-KG-002 → project isolation plus entity/relation/status/review/origin/confidence/time/Checkpoint filters and bounded pagination/history.
- FR-KG-003 → invalid pair, stale edge, missing source, history mismatch, cycle, research-claim gap, and unresolved feedback rules.
- FR-KG-004 → existing permitted relationship creation plus CP6 review/annotate/reject/retire state machine and immutable history.
- FR-KG-005 → Core continues to reject AI canonical commands/origin; CP7 owns candidate acceptance.
- FR-KG-006 → origin/review/status/actor fields are returned on every edge; CP8 owns visual badges.

## Core, Data, and Performance

- FR-CORE-003/006/008/009/010/012/015 → schema v7, transactions, idempotency, audit, history inspection, migration, export/restore, declared partial origins.
- AC-DATA-01/03/05 → failed cycle/cross-project/stale mutations leave no event/state; retries remain idempotent; export/restore preserves CP6 records.
- NFR-OFF-001 → all graph operations are local SQLite operations.
- PERF-005 → one-hop p95 38.877 ms (target ≤300 ms).
- PERF-006 precursor → 500-node/1,497-edge scoped query p95 232.195 ms; CP8 still owns layout/render timing.
- PERF-015 → query root/depth/node/edge/page limits prevent unbounded graph accumulation.

## Downstream Contracts

- CP7 → validated serializable graph and gap report, no provider coupling.
- CP8 → bounded projection, current state, provenance fields, truncation/frontier/omission metadata.
- CP10 → deterministic roots, filters, direction, distance, source IDs.
- CP11 → permission-ready query output without raw SQLite/filesystem access.
