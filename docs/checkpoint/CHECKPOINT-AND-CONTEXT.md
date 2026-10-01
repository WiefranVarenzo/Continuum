# Checkpoint and Context Architecture

> **Status:** Implemented and validated in CP10; consumed through the permissioned CP11 MCP boundary; schema v12 with `context` contract v1 unchanged.

## 1. Product Contract

Checkpoint is Continuum's smart bookmark: it answers where work stopped, what is complete, what is active, what remains uncertain or blocked, why the current direction exists, and what should happen next. It is valid at any meaningful interruption or milestone and never means the project has ended.

## 2. Current Project State

Current Project State is a derived, scope-aware view computed from the latest valid Checkpoint plus subsequent canonical events and live repository observations. It is not a single mutable AI summary.

Common fields:

```text
project_id, scope, as_of_ledger_sequence, computed_at
active_goal, completed_items, active_items
unresolved_items, blockers, risks, next_actions
source_ids, repository_state_refs, freshness
deterministic_state, reviewed_semantic_summary
```

## 3. Checkpoint Envelope

```text
checkpoint_id        UUIDv7
project_id           UUIDv7
scope                research | development | integrated | core
ledger_sequence      exact committed boundary
schema_version
created_at/by
trigger              manual | interruption | handoff | milestone | policy
source_entity_ids
artifact/repository references
deterministic_payload
semantic_payload     optional, reviewed status and generation metadata
unresolved_items
next_actions
privacy_snapshot
```

Checkpoints are immutable. A correction creates a superseding Checkpoint. Staleness is derived from relevant subsequent events, repository divergence, unavailable artifacts, or policy/schema changes.

## 4. Scope Payloads

### Research

Active Questions, Research Session state, Evidence reviewed/unreviewed, Experiments and Results, accepted/challenged Findings, Decisions, contradictions, gaps, and next research actions. When relevant, include active/paused/interrupted capture session IDs, latest persisted segment/marker boundary, recoverability/failure status, and selected capture-backed Evidence IDs. Raw complete media is never embedded by default. No development field is mandatory.

### Development

Declared intent/Requirement, repository/baseline/worktree, active ChangeSets, affected CodeEntities, Tests/TestRuns, validation, blockers, and next development actions. No research field is mandatory.

### Integrated R&D

Relevant fields from both scopes plus bridge relationships, rationale-to-implementation coverage, validation LearningFeedback, and cross-Space next actions.

## 5. Creation Algorithm

1. Validate project, scope, permission, and capability state.
2. Start a consistent read transaction and capture ledger boundary.
3. Select deterministic scope state and source IDs.
4. Optionally request a privacy-filtered semantic summary.
5. Require review if semantic narrative is saved as approved.
6. Commit immutable Checkpoint and source edges.
7. Emit `CheckpointCreated` and invalidate affected current-state cache.

Failure in semantic generation may still produce a deterministic Checkpoint.

## 6. Resume Algorithm

1. Load latest applicable non-invalid Checkpoint.
2. Validate referenced artifacts/repository state.
3. Apply or summarize relevant events after its ledger sequence.
4. calculate freshness and divergence.
5. show `then`, `since`, `now`, and `next` separately.
6. allow user to resume, create a new Checkpoint, or inspect sources.

No AI is required to reconstruct deterministic changes since the Checkpoint.

## 7. Context Pack Request

```text
request_id, project_id, task, audience, consumer_target
scope, checkpoint_id optional, freshness requirement
include/exclude filters, privacy audience
soft_budget_tokens, hard_budget_tokens, byte_budget
retrieval_profile, token_estimator, requested schema version
```

Defaults are a 16k-token soft budget and 32k-token hard ceiling for external semantic tasks, configurable downward by task, provider, or MCP client grant. Larger local-only packs may use byte budgets. Budgets are estimates; output declares actual measured bytes plus estimator identity, target provider/client when known, token estimate, and uncertainty/fallback when an exact tokenizer is unavailable.

## 8. Progressive Retrieval

Retrieval tiers:

1. project identity, active goal, scope, latest Checkpoint, and next actions;
2. directly task-matched entities and one-hop relationships;
3. supporting/challenging Evidence, Results, diffs, Tests, and Decisions;
4. deeper artifacts or graph expansion explicitly requested or needed within budget.

Capture-backed sources follow the same tiers: tier 1 may state active/interrupted capture status, tiers 2–3 may include markers and selected Evidence metadata or bounded excerpts, and raw media bytes require an explicit authorized request. Checkpoint creation records only stable IDs and the latest committed fragment boundary, never an in-memory recorder buffer.

Candidate generation is deterministic using type, status, graph distance, explicit links, FTS, recency, checkpoint relevance, repository coordinates, and user filters. AI may rerank/compress only the permitted candidate set.

## 9. Context Pack Output

Every pack includes schema version, purpose, audience, consumer target, scope, generated time, checkpoint/ledger position, privacy policy version, ordered items, source IDs/versions, provenance edges, freshness, included bytes/token estimate and method, omission summary, unresolved contradictions, and unavailable sources. It never implies omitted content does not exist.

Packs are ephemeral by default. Saving creates a GeneratedArtifact with source state and staleness tracking.

Context Packs are machine-facing and renderer-neutral. They do not include HTML/CSS/JavaScript presentation markup by default. Human Documentation may cite the same checkpoint and source entities, but its Human Document Model and rendered HTML are separate projections; an explicitly requested saved report can be referenced as an Artifact without replacing the structured context.

## 10. Privacy and Determinism

Transitive exclusion removes denied payloads and derived summaries whose sources are denied. Users can preview/remove items before transmission. Repeating deterministic selection against identical canonical state, request, policy, and indexes returns the same ordered candidate set; documented semantic tie/reranking is separate.

The same provider-neutral pack contract serves CP7 outbound semantic tasks and CP11 MCP reads. CP7 still applies destination-specific provider privacy policy before transmission; CP11 still applies project/client-grant authorization before disclosure. A pack created for one audience or destination is not automatically reusable for another.

## 11. Acceptance

- Research, Development, Integrated, and Core scope variants validate without placeholders;
- interrupted incomplete work resumes correctly;
- active, paused, failed, or interrupted capture resumes from the latest persisted CP9 boundary without claiming uncommitted media;
- changed relevant state marks older output stale;
- pack respects hard budget or returns explicit budget failure/omissions;
- every included claim resolves to authorized source IDs;
- offline deterministic creation/resume works;
- secret/never-send content is excluded transitively.
