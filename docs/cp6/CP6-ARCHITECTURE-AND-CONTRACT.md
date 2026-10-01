# CP6 — Provenance & Knowledge Graph Architecture and Contract

> **Status:** Implemented and validated 2026-09-08  
> **Schema:** Continuum Core v7  
> **Depends on:** CP1 contracts, CP2.1 ledger/relationship envelope, CP3 Research identities, CP4 Git/ChangeSet identities, CP5.1 history-safe Code/Test identities

## 1. Purpose

CP6 makes research-to-code traceability operational. It does not introduce a second graph database: canonical nodes remain rows in `entities`, canonical edges remain rows in `relationships`, and SQLite remains the sole structured authority. CP6 adds the missing application contracts for bounded graph traversal, relationship review/history, deterministic graph validation, and Learning Feedback.

The graph is useful in three valid modes:

1. Research-only: a partial chain may stop at a Question, Evidence, Result, Finding, or Decision.
2. Development-only: a partial chain may begin with an external/manual/legacy Requirement or direct intent.
3. Connected R&D: available links connect Research rationale to implementation and verification without copying entities.

Absence of an unused Space is never treated as corruption. A gap is reported only when an existing record makes a stronger provenance claim that its links do not support.

## 2. Non-negotiable Invariants

- project IDs, entity IDs, and earlier relationships are never rewritten to fabricate continuity;
- all endpoints and direct sources belong to the same project;
- source/type/relation/target combinations pass the registered CP3–CP6 matrices;
- `supersedes` is acyclic and self-links are rejected;
- graph writes are atomic, idempotent, audited, and rejected when the project is archived;
- AI actors and `ai_proposal` origins cannot write canonical graph state;
- deterministic observation edges may be acknowledged or annotated, but cannot be rejected or retired as though they were opinions;
- relationship state changes use optimistic `state_version` checks;
- every CP6 state change appends immutable, ledger-addressed relationship history;
- query depth, roots, nodes, edges, and pages are bounded;
- Learning Feedback never changes its target's status automatically;
- rejected, superseded, archived, missing, or ambiguous information remains visible when explicitly requested.

## 3. Canonical Graph

### Nodes

Every canonical domain entity is graph-addressable through its stable `entities.id`. CP6 adds only `learning_feedback`; it does not duplicate CP3–CP5 entities.

### Edges

`relationships` retains the CP2 envelope and now includes:

- `state_version` for optimistic state transitions;
- human `annotation`;
- reviewer identity/time;
- retirement time.

`relationship_history` is append-only. Existing pre-CP6 relationships receive a transparent `migration_baseline` at ledger sequence `0`, because CP6 cannot truthfully invent their original event sequence. New relationships receive a `created` history head, and later accept/reject/annotate/retire/supersede actions append new versions.

## 4. Principal Relationship Matrix

```text
Experiment       --addresses----------> ResearchQuestion
Experiment       --produces-----------> Result
Evidence/Result  --supports|challenges|inconclusive_for--> Finding
Finding          --answers------------> ResearchQuestion
Finding          --informs------------> Decision
Decision         --creates|modifies|retires-------------> Requirement
ChangeSet        --implements|partially_implements|reverts--> Requirement
ChangeSet        --modifies------------> CodeEntity
Repository       --defines-------------> CodeEntity/Test
Test             --verifies------------> Requirement/CodeEntity
TestRun          --executes------------> Test
TestRun          --observed_at---------> RepositoryBaseline
TestRun/Result/Evidence/ChangeSet --produces-------------> LearningFeedback
LearningFeedback --supports|challenges-> Evidence/Finding/Decision/Requirement
LearningFeedback --requests_revision_of-> Finding/Decision/Requirement
```

The conceptual chain is navigated in both directions because several canonical relationships intentionally point from the observing/implementing record back to what it verifies or implements.

## 5. Learning Feedback Contract

Learning Feedback is an explicit promotion of an observed validation outcome. Creation requires one or more real source entities. It may target zero or more permitted entities; zero targets means the feedback is intentionally unresolved, not lost.

Kinds: `validation_failure`, `validation_success`, `observation`, `contradiction`, `revision_request`, `other`.

Severity: `info`, `warning`, `error`.

Lifecycle:

```text
open --> acknowledged --> resolved
  \---------------------> resolved
```

Creation and status transition preserve source links, actor, event, command receipt, and details. Resolving feedback records a note but does not revise a Finding, Decision, Requirement, Evidence, or code automatically.

## 6. Relationship Review Contract

Permitted actions are `accept`, `reject`, `annotate`, and `retire`.

- Accept sets review state to `accepted` while keeping the relationship active.
- Reject sets relationship status and review state to `rejected`.
- Annotate preserves status/review and adds review context.
- Retire archives the relationship without deleting it.
- Archived and superseded states are terminal.
- A stale `expected_state_version` fails with no partial event/history/write.
- Deterministic observations may be accepted as reviewed or annotated; they cannot be rejected or retired because their observed truth cannot be voted away.

CP7 may persist AI relationship candidates separately. CP6 deliberately rejects canonical AI proposal writes.

## 7. Graph Query Contract

`traverse_provenance(GraphQuery)` supports:

- one or more roots, or source roots from a Checkpoint;
- inbound, outbound, or both-direction traversal;
- depth up to 8;
- at most 50 explicit roots, 500 nodes, and 1,500 edges;
- entity-type, relationship-type, status, review-state, origin, confidence, and RFC3339 creation-time filters;
- deterministic ordering, truncation state, frontier IDs, and omission explanations.

Checkpoint scoping selects the immutable Checkpoint's source IDs as graph roots. Returned nodes/edges expose current state; CP6 explicitly reports that this is not an as-of reconstruction. CP10 may use the same bounded contract for Context Pack candidate selection without changing this truthfulness rule.

## 8. Validation and Gap Rules

Structural errors:

- endpoint type mismatch or forbidden relationship pair;
- missing/cross-project direct source;
- relationship current state/history-head mismatch;
- `supersedes` cycle;
- research-origin Requirement without Decision provenance;
- research-origin ChangeSet without Requirement link;
- Learning Feedback without a source.

Reviewable warnings/information:

- active edge to archived endpoint;
- accepted Finding without supporting/challenging/inconclusive sources;
- duplicate direct source;
- unresolved Learning Feedback without a target.

Only error-severity provenance issues fail `verify_integrity`. Partial but honest standalone work remains healthy.

## 9. Transactions, Failure, and Recovery

Every canonical CP6 command uses an immediate SQLite transaction. Entity/edge rows, relationship-history entries, command receipts, audit events, and outbox messages commit together or roll back together. Export/import includes schema v7 unchanged, and integrity verification runs before export and after staged import. Migration from v6 is backed up by the existing CP2 migration protocol.

## 10. Privacy and Security

CP6 is offline and performs no provider or MCP calls. Graph outputs contain structured project data and stable IDs; later outbound consumers must still apply the CP1 privacy gateway. Queries are project-scoped, use fixed SQL with bound parameters, reject cross-project IDs, and cap resource consumption. CP8 receives data, never trusted HTML. CP11 receives bounded read models, never raw SQLite access.

## 11. Performance Contract

On fixture `graph-500-v1` (500 nodes, 1,497 relationships, 30 release iterations):

- one-hop p95: 38.877 ms against ≤300 ms;
- 500-node scoped query p95: 232.195 ms against the ≤2 s query/render precursor budget;
- full graph validation: 111.594 ms.

CP8 must separately measure layout/browser rendering. CP12 repeats certification on declared Ryzen 5 5600H/16 GB reference hardware.

## 12. Downstream Handoff

- CP7 receives validated nodes/edges and gap reports as grounding; AI output remains candidate state.
- CP8 receives renderer-neutral bounded graph projections, frontier/omission metadata, and provenance badges.
- CP10 receives deterministic traversal roots and filters for progressive Context Pack selection.
- CP11 exposes the same bounded queries through permissioned versioned MCP contracts.

CP6 does not implement model providers, embeddings, vector retrieval, graph UI/layout, HTML reports, semantic Checkpoints, capture, or MCP.
