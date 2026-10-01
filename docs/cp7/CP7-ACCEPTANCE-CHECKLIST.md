# CP7 Acceptance Checklist

> **Result:** PASS — 2026-09-08

- [x] Semantic task, provider, result, usage, error, and review contracts are versioned.
- [x] Gemini and OpenAI-compatible envelopes are independently encoded and normalized.
- [x] OpenAI-compatible profiles do not infer provider/model capability parity.
- [x] Profiles are disabled from routing until explicitly allowlisted.
- [x] Routing is deterministic and capability-, privacy-, consent-, and budget-bound.
- [x] A bounded preview exposes source categories/freshness, conservative size, exact destinations/models, and route decisions before transmission.
- [x] Failover requires opt-in at both task and project levels.
- [x] Worst-case units across all planned failover attempts must fit the project budget before transport.
- [x] Sensitive consent binds task, destination, and source fingerprint.
- [x] Revoked/expired consent blocks transport.
- [x] Secret/never-send and detected credential material have no remote route.
- [x] Credential values are absent from project schema; credential-shaped references are rejected.
- [x] Explicit sources and transitive provenance are bounded and fingerprinted.
- [x] Changed sources mark a task stale before transport.
- [x] Output schema definitions and values use a strict bounded subset.
- [x] Citations resolve only to permitted source IDs.
- [x] Factual claim objects require direct sources.
- [x] Executable report/diagram content fails closed.
- [x] Invalid output creates no candidate and no canonical mutation.
- [x] Candidates remain separate from canonical entities/relationships.
- [x] Human edits are revalidated and stale candidates cannot be accepted.
- [x] Cache keys include source/provider/model/adapter/policy/prompt/schema/budget inputs.
- [x] Cache hits receive new task-specific attempt and candidate provenance.
- [x] Rejected/edited candidates cannot be silently replayed from cache.
- [x] Concurrent completion cannot create duplicate candidates.
- [x] Provider, task, and review-queue queries are bounded and paginated; pending tasks can be explicitly cancelled.
- [x] Provider errors are sanitized and terminal attempts are immutable.
- [x] Project integrity scan covers provider configuration and candidate-attempt provenance.
- [x] Migration is additive and export/restore retains CP7 tables through the existing database backup path.
- [x] Formatter, strict linter, all tests, and release benchmark pass.

## CP8 Entry Gate

- [x] Report narrative and diagram plan task types exist.
- [x] Presentation proposals are structured data, never raw executable HTML/Mermaid/JavaScript.
- [x] Source IDs, authorship, provider attempt, validation, and freshness are available to CP8.
- [x] Deterministic data remains usable when semantic providers are absent.
