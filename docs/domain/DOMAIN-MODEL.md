# Continuum Domain Model

> **Status:** Approved baseline; provider/MCP provenance amendment accepted 2026-09-03; implemented through CP4
> **Scope:** Canonical vocabulary, ownership, lifecycle, relationships, provenance, and invariants.

## 1. Common Envelope

Every canonical entity carries:

```text
id               UUIDv7
project_id       UUIDv7
entity_type      stable versioned discriminator
version          monotonically increasing integer
status           type-specific lifecycle state
origin_type      user | deterministic | import | external | legacy | ai_proposal
created_at       UTC
created_by       actor reference
updated_at       UTC
updated_by       actor reference
archived_at      optional UTC
metadata         schema-bounded extension object
```

AI proposals use separate candidate records until accepted. Their provenance distinguishes an outbound provider attempt from an inbound external-client proposal without adding provider or client brands to canonical business entity types. Immutable observations may be superseded but are not rewritten to match later interpretation.

## 2. Aggregate Ownership

### Continuity Core

- Project — policy and identity boundary.
- SpaceCapability — enabled/disabled UI capability state.
- Artifact — payload metadata, hash, media type, size, classification, and availability.
- Relationship — typed edge between stable entity IDs.
- AuditEvent — append-only material transition record.
- Job — durable background-work state.
- Checkpoint — immutable bookmark at a ledger sequence.
- ContextPack — ephemeral or explicitly saved retrieval artifact.
- GeneratedArtifact — report, diagram, summary, or explanation with source state.

### Research Space

- ResearchSession — bounded activity interval.
- ResearchQuestion — inquiry, hypothesis, or uncertainty.
- Evidence — sourced material and capture metadata, distinct from interpretation.
- Experiment — planned procedure addressing a question.
- Result — observed experiment output.
- Finding — interpreted conclusion supported/challenged by sources.
- Decision — selected option, alternatives, rationale, constraints, and supersession.

### Development Space

- Requirement — verifiable obligation; origin may be research, manual, external, imported, or legacy.
- Repository — stable local repository identity.
- RepositoryBaseline — exact observed Git state.
- CommitObservation — immutable commit metadata observation.
- ChangeSet — meaningful grouping of committed or working-tree changes.
- CodeEntity — addressable file/module/symbol/config/dependency.
- Test — detected or authored verification definition.
- TestRun — observed execution at repository state.

### R&D Bridge

- LearningFeedback — reviewable cross-Space learning record. The bridge otherwise owns relationships, not duplicate entities.

## 3. Lifecycle Models

### General authored knowledge

```text
draft → active → superseded → archived
  └────────────→ rejected (candidate/proposal only)
```

Finding adds `candidate`, `accepted`, `challenged`, and `superseded`. Decision adds `proposed`, `accepted`, `rejected`, `superseded`, and `retired`. Requirement adds `draft`, `accepted`, `in_progress`, `implemented`, `verified`, `blocked`, `retired`, and `superseded`.

Transitions are explicit commands, version-checked, actor-attributed, and evented. `implemented` and `verified` are distinct.

### Evidence and observations

Evidence and source observations use `available`, `unavailable`, `superseded`, `archived`, or `purged_payload`. Interpretation status lives on typed relationships, not by rewriting source content.

### ChangeSet

`draft → observed → linked → validated → closed`, with `abandoned`, `superseded`, and `reverted` branches. A draft may refer to an exact working-tree snapshot identity.

### Job

`queued → running → succeeded|failed|cancelled`, with bounded retry returning `failed → queued`. Lease expiry recovers abandoned `running` jobs.

### AI candidate

`generated → schema_valid → pending_review → accepted|edited_and_accepted|rejected|stale|superseded`. Only acceptance creates or updates a canonical domain entity through a normal command.

An outbound candidate records provider/model, adapter/capability, task/prompt/schema, sources, privacy decision, and attempt metadata. An inbound MCP proposal records client/grant, MCP contract/tool, sources, scope, and validation metadata. Both retain `ai_proposal` authority until human review; neither gains a provider-specific canonical entity type.

## 4. Relationship Model

A canonical relationship contains ID, project ID, source ID/type, relation type/version, target ID/type, status, origin, actor, confidence/review state, source IDs, timestamps, and optional supersedes ID.

Core relation types include:

- `addresses`: Experiment → ResearchQuestion.
- `produces`: Experiment → Result; validation → LearningFeedback.
- `supports|challenges|contextualizes|inconclusive_for`: Evidence/Result → Finding.
- `answers`: Finding → ResearchQuestion.
- `informs`: Finding → Decision.
- `creates|modifies|retires`: Decision → Requirement.
- `implements|partially_implements|reverts`: ChangeSet → Requirement.
- `modifies`: ChangeSet → CodeEntity.
- `defines`: Repository → CodeEntity/Test.
- `verifies`: Test → Requirement/CodeEntity.
- `executes`: TestRun → Test.
- `observed_at`: CommitObservation/TestRun/ChangeSet → RepositoryBaseline.
- `requests_revision_of`: LearningFeedback → Finding/Decision/Requirement.
- `includes`: Checkpoint/ContextPack/GeneratedArtifact → source entity.
- `supersedes`: versioned semantic record → earlier record.

Allowed source/target pairs are validated. Self-links and cycles are permitted only for explicitly safe relation families; `supersedes` must be acyclic.

## 5. Provenance and Authorship

Provenance answers: who/what created this, from which direct sources, under which tool/schema version, and whether a human approved it.

Origin types:

- `user`: authored directly in Continuum.
- `deterministic`: exact observation or derivation by versioned code.
- `import`: imported from identified material.
- `external`: referenced external system/item.
- `legacy`: predates Continuum with incomplete machine-verifiable history.
- `ai_proposal`: semantic candidate, never canonical without review.
- `unknown`: origin genuinely unavailable and visibly declared.

Direct source references are mandatory for derived claims. Missing provenance is represented as missing, never inferred. User editing an AI candidate creates human-approved content while retaining the original candidate and generation metadata.

## 6. Repository Identity

Repository identity is a Continuum UUID plus observed root fingerprint and Git object identity, not a mutable filesystem path alone. Baselines store exact HEAD commit, branch/ref when available, worktree hash/status, observation time, and repository adapter version.

Rebase/amend creates new observations. Existing provenance remains linked to old content and may be explicitly reconciled or superseded. Renames use deterministic similarity/parser observations without destroying previous CodeEntity identity history.

## 7. Checkpoint Domain Rules

A Checkpoint is immutable and contains a common envelope plus Research, Development, or Integrated payload. It records ledger sequence, repository state references if relevant, deterministic state summary, reviewed semantic narrative if any, unresolved items, next actions, source IDs, and freshness basis.

Checkpoint creation is valid during incomplete work, validation failure, handoff, or milestone. New events do not mutate it; they may make it stale.

## 8. Deletion and Retention

Normal deletion archives or tombstones canonical metadata to preserve referential truth. Payload purge is explicit, policy-checked, and leaves a tombstone/hash where lawful and useful. An artifact is physically garbage-collected only when unreferenced, outside the recovery window, and not retention-protected. Export can exclude payloads but must declare omissions.

## 9. Domain Invariants

1. IDs never change or get reused.
2. Entity versions increase on canonical mutation.
3. Source observations are separate from interpretation.
4. Result is separate from Finding; Test is separate from TestRun.
5. Requirement rationale origin is explicit.
6. Cross-Space links are optional and typed.
7. AI proposals never masquerade as user or deterministic origin.
8. An unavailable/purged Artifact is visible as such.
9. Supersession preserves the earlier record and inbound provenance.
10. Checkpoint source state is immutable.
11. Project-scoped entities cannot link across projects.
12. Capability disablement never deletes domain data.
13. Provider/model and MCP client identity are provenance attributes, never canonical authority or mandatory dependencies.

## 10. CP2 Minimum Domain Slice

CP2 implements Project, SpaceCapability, generic entity envelope, Relationship, AuditEvent, Artifact metadata, Job, Checkpoint envelope, origin/authorship primitives, and lifecycle/version utilities. Research and Development aggregates are introduced by their owning checkpoints against these contracts.

## 11. CP3 Implemented Research Slice

CP3 implements normalized ResearchSession, ResearchQuestion, Evidence, Experiment, Result, Finding, Decision, and optional Requirement state against the common envelope. It registers the Research relationship matrix at the Core boundary, preserves source/interpretation separation, and adds deterministic research timeline, search, report, Checkpoint, resume, migration, and integrity behavior. Development aggregates and cross-Space validation feedback remain owned by CP4–CP6.

## 12. CP4 Implemented Development Slice

CP4 implements Repository attachment/relocation, immutable RepositoryBaseline and CommitObservation state, ordered file-diff observations, rewrite reconciliation, Development-created shared Requirements, working-tree and committed ChangeSets, explicit Requirement implementation and ChangeSet supersession links, Development timeline/search, Checkpoint/resume/report, migration, export/restore, and integrity behavior. CodeEntity, dependency/configuration, Test, and TestRun observations remain CP5-owned.
