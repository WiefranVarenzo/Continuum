# CP10 Traceability

> **Status:** Complete  
> **Validated:** 2026-09-10

## Functional Requirements

- FR-CTX-001 → semantic Checkpoint command at interruption/milestone with exact scope boundary.
- FR-CTX-002 → Research, Development, Integrated, and Core scope validation without placeholders.
- FR-CTX-003 → deterministic state separated from optional reviewed semantic snapshot.
- FR-CTX-004 → latest valid scope Checkpoint plus explicit selection/listing.
- FR-CTX-005 → Then/Since/Now/Next Current Project State and visible freshness reasons.
- FR-CTX-006 → purpose/audience/consumer/scope/budget Context Pack request.
- FR-CTX-007 → stable four-tier deterministic progressive retrieval.
- FR-CTX-008 → identity, source versions, omissions, budgets, estimator, and freshness in every pack.
- FR-CTX-009 → classification gate, external secret scan, explicit-root denial, and Artifact payload opt-in.
- FR-CTX-010 → provider-neutral structured JSON with no presentation markup dependency.
- FR-CTX-011 → desktop preview and per-source removal before save/transmission.
- FR-CTX-012 → ephemeral preview; explicit save creates immutable GeneratedArtifact linkage.
- FR-CTX-013 → stale-current rejection and explicit stale-with-warning mode.
- FR-CTX-014 → exact entity/Artifact/repository/capture source snapshot and fingerprint.
- FR-CTX-015 → deterministic result independent of AI/network availability.
- FR-CTX-016 → accepted/fresh CP7 candidate only; AI actor cannot commit canonical Checkpoint.
- FR-CTX-017 → source/pack integrity diagnostics and pre-save optimistic validation.

## Acceptance Criteria

- AC-CTX-01 → independent-space/integrated acceptance scenario.
- AC-CTX-02 → repeated pack request/content fingerprints and order are stable.
- AC-CTX-03 → token/byte/item bounds and explicit omissions/errors.
- AC-CTX-04 → stale source versions, later events, and repository divergence are detected.
- AC-CTX-05 → Then/Since/Now/Next reconstructs interrupted state without AI.
- AC-CTX-06 → Public/Internal/Sensitive/Secret/NeverSend audience policy fails closed.
- AC-CTX-07 → saved pack links exact source snapshots and GeneratedArtifact atomically/idempotently.
- AC-CTX-08 → legacy Checkpoints and v10→v11 migration preserve identity/data.

## Upstream / Downstream

- CP1 invariants: local-first, modular Spaces, deterministic authority, explicit privacy, bounded work.
- CP2: transactions, migration backup, ledger, command receipts, Artifact Store, integrity.
- CP3–CP5: typed Research and Development entities plus legacy bookmarks.
- CP6: relationship/provenance candidates and stable entity IDs.
- CP7: classifications and optional reviewed semantic candidates; no provider is invoked automatically.
- CP8: desktop shell/HTML-first human projection remains separate from machine Context Packs.
- CP9: only durable capture boundaries and selected Evidence metadata flow into context.
- CP11: consumes versioned read contracts and must add client grants/auditing.
- CP12: release-platform, long-duration, packaging, and security qualification.
