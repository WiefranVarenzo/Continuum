# CP6 — Acceptance Checklist

- [x] Schema v7 migration is additive, checksum-versioned, backup-protected, and preserves v1–v6 identities.
- [x] Research-only and Development-only graphs remain valid without placeholder entities.
- [x] Complete connected R&D nodes and edges are traversable in both directions.
- [x] Graph queries are project-scoped and bounded to 50 roots, depth 8, 500 nodes, and 1,500 edges.
- [x] Filters cover type, relationship, status, review state, origin, confidence, time, and Checkpoint source roots.
- [x] Truncated results expose frontier IDs and omission reasons.
- [x] Relationship review uses optimistic state versions and immutable ledger-addressed history.
- [x] Deterministic links may be acknowledged/annotated but cannot be rejected or retired as subjective claims.
- [x] `supersedes` cycles, cross-project endpoints, and stale writes fail atomically.
- [x] Learning Feedback requires real source provenance and can remain explicitly unresolved.
- [x] Learning Feedback never changes target entity status automatically.
- [x] Gap validation distinguishes dishonest connected claims from honest partial chains.
- [x] Integrity verification includes error-severity provenance faults.
- [x] Export/restore preserves Learning Feedback, relationships, review history, IDs, and health.
- [x] CP1–CP5.1 regression tests remain green.
- [x] One-hop graph p95 meets ≤300 ms budget on the recorded fixture.
- [x] CP6 does not claim CP7 AI, CP8 rendering, CP10 semantic context, or CP11 MCP behavior.

**Decision: CP6 PASS.**
