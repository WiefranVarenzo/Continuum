# CP7 Traceability

> **Status:** Complete  
> **Validated:** 2026-09-08

## Product Requirements

- FR-AI-001 → one `SemanticTransport` gateway boundary; no alternate provider call path.
- FR-AI-002–003 → versioned task/prompt/schema plus strict schema validator.
- FR-AI-004 → normalized terminal attempt states and zero-candidate failure.
- FR-AI-005 → stable nine-value semantic task catalog.
- FR-AI-006–007 → versioned fingerprinted cache and explicit candidate freshness.
- FR-AI-008 → task snapshot, audience, classification, provider, and conservative input budget are inspectable before execution.
- FR-AI-009 → sanitized attempt/policy metadata; prompt/response/credential bodies are not persisted.
- FR-AI-010 → CP2–CP6 regression suite passes without providers.
- FR-AI-011 → explicit capability descriptors and fail-closed eligibility.
- FR-AI-012–013 → deterministic routing, two-sided failover opt-in, destination consent, never-send denial.
- FR-AI-014 → Gemini native and OpenAI-compatible adapter fixtures.
- FR-AI-015 → immutable provider attempts, source fingerprint, privacy decision, exact model, usage, latency, error, validation, candidate review.

## Acceptance Criteria

- AC-AI-01 → invalid schema/citation/presentation fixtures create zero candidates and zero canonical writes.
- AC-AI-02 → candidates retain source IDs, fingerprint, attempt, provider, model, and validation metadata.
- AC-AI-03 → secret/never-send produces zero transport calls; credential-shaped configuration is rejected.
- AC-AI-04 → all 91 deterministic/core tests pass with no provider dependency.
- AC-AI-05 → hostile source instructions remain bounded data; provider output cannot change permissions or canonical state.
- AC-AI-06 → user review is optimistic, source-preserving, and separate from canonical mutation.
- AC-AI-07 → one stable task contract executes through both adapter families.
- AC-AI-08 → capability mismatch, revocation, source drift, malformed output, and denied failover fail closed.

## Upstream / Downstream

- CP1: preserves ADR-004 and ADR-006 authority split.
- CP2: uses ledger, command idempotency, audit/outbox, migration, backup/export, and integrity boundaries.
- CP3–CP5: consumes stable entity versions, typed source data, artifact hashes, Git baselines, code/test identities.
- CP6: derives bounded source closure and grounding IDs from provenance without modifying the graph.
- CP8: receives validated renderer-neutral semantic proposals.
- CP10: may reuse task/cache/provenance contracts for Context Pack compression.
- CP11: must submit inbound proposals through a separate permissioned boundary.
