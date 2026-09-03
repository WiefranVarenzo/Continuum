# CP3 — Research Core Architecture and Contract

> **Status:** Implemented and validated
> **Build:** `continuum-core 0.3.0`
> **Schema:** 3
> **Authority:** Continuum PRD, CP1 Architecture, Domain Model, Data Architecture, ADR-001, ADR-003, ADR-004, ADR-005

## 1. Purpose

CP3 turns the approved Research Space model into a deterministic application/domain module. It lets a project conduct, pause, resume, search, audit, export, restore, and report research without Gemini, Git, a repository, or any Development Space entity.

CP3 does not alter the CP1 direction. It consumes the identity, command, transaction, relationship, artifact, event, capability, integrity, checkpoint, backup, and export contracts delivered by CP2.1.

## 2. Boundary

Research Space owns:

- `ResearchSession`;
- `ResearchQuestion`;
- `Evidence`;
- `Experiment`;
- `Result`;
- `Finding`;
- `Decision`;
- deterministic research timeline, search projection, resume state, and basic cited report.

CP3 also introduces the normalized `Requirement` record as an optional, provenance-bearing handoff from an accepted research Decision, while Requirement remains the Development-side intent contract defined by CP1. A Requirement is never required to complete Research. CP4 may create Development-only Requirements through its own typed command or link this same stable Requirement identity to ChangeSets without rewriting its origin.

Continuity Core continues to own common envelopes, commands, events, relationships, Artifacts, Checkpoints, capability state, ledger ordering, integrity, backup, and export/restore.

## 3. Storage Contract

Schema migration `0003_research_core.sql` adds one normalized table per aggregate plus:

- `research_session_items` for explicit activity/session membership;
- `research_timeline` as an append-only, ledger-sequenced research projection;
- `research_search_documents` as a deterministic, rebuildable local text projection.

Every normalized record references one common `entities` row. Database triggers reject a normalized row whose common-envelope `entity_type` is wrong. Typed APIs reserve Research entity names so generic Core CRUD cannot create or mutate incomplete Research records.

Migration is forward-only, checksummed, transactional, and preceded by the CP2 automatic database backup. v1 and v2 fixtures both migrate to schema v3 while preserving existing Core state.

## 4. Command and Atomicity Contract

Every Research mutation accepts CP2's `CommandContext` and therefore carries:

- stable command and idempotency identity;
- actor;
- command/payload version;
- issued time;
- correlation and optional causation;
- optional optimistic expected entity version.

A successful Research command commits the common entity, normalized state, typed relationships, session membership, search projection, audit event, timeline entry, command receipt, and transactional outbox in one SQLite transaction. A validation, relationship, lifecycle, stale-version, or persistence failure leaves none of those effects visible.

An `ai_proposal` actor or origin cannot execute these canonical commands. CP7 must store model output as a reviewable candidate and submit a normal human-approved command when accepted.

## 5. Aggregate Rules

### ResearchSession

An active session defines a bounded period of inquiry. Items may be attached during creation or associated later. Completed or cancelled sessions reject new membership. Ending a session records the completion time and optional note; it does not imply that its Questions or project are complete.

### ResearchQuestion

A Question stores its exact wording separately from context and desired outcome. It supports question, hypothesis, and uncertainty kinds, bounded priority, due time, optimistic editing, search, and explicit status transitions. Closed Questions must be superseded rather than silently rewritten.

### Evidence

Evidence requires at least one original source representation: Artifact, URI, stable reference, or source content. Original source fields are separate from annotation, summary, and relevance. Annotation updates increase the entity version without mutating the original source. Artifact references must resolve inside the same project and be available when Evidence is created.

Evidence origin is explicit (`user`, `import`, `external`, `legacy`, `deterministic`, or `unknown`). AI proposal origin is rejected. Evidence and Artifact availability may not claim `available` or `purged_payload` inconsistently.

### Experiment and Result

An Experiment requires a Question or a non-empty hypothesis plus method and expected observations. It must enter `running` before normal Results can be recorded. Result is an observation, not an interpretation: it stores outcome, measurements, time, optional Artifact, and exact Experiment identity. A Result can exist indefinitely without a Finding.

### Finding

A Finding stores claim, interpretation, uncertainty, and optional bounded confidence. Sources are typed independently as supporting, challenging, contextual, or inconclusive. Acceptance requires at least one active supporting Evidence or Result. Conflicting source assessments can coexist and remain visible.

### Decision

A Decision stores selected option, rationale, alternatives, constraints, source Findings, decision time, lifecycle, and supersession. Supersession preserves the earlier Decision and rationale, creates an acyclic typed edge, and uses expected-version checking to prevent lost updates.

### Requirement

A Requirement stores a verifiable statement, acceptance criteria, priority, verification method, and honest rationale origin. `research` origin requires a Decision link. Other origins remain explicit. Creating or completing a Requirement is optional in Research-only use.

## 6. Relationship Policy

The CP3 policy permits only declared pairs:

```text
Experiment       --addresses----------> ResearchQuestion
Experiment       --produces-----------> Result
Evidence/Result  --supports-----------> Finding
Evidence/Result  --challenges---------> Finding
Evidence         --contextualizes-----> ResearchQuestion/Experiment/Finding
Evidence/Result  --inconclusive_for---> Finding
Finding          --answers------------> ResearchQuestion
Finding          --informs------------> Decision
Decision         --creates/modifies/retires--> Requirement
same semantic type --supersedes-------> earlier record
```

Unknown pairs, self-links, wrong endpoint types, missing endpoints, cross-project endpoints, duplicate direct sources, and cyclic supersession are rejected. Core's generic relationship entry point also applies this registered policy whenever a Research-owned endpoint is present.

## 7. Lifecycle Policy

Lifecycle transitions are explicit allowlists. Representative flows are:

```text
ResearchSession: active → completed|cancelled → archived
ResearchQuestion: active → answered|deferred|superseded|archived
Experiment: planned → running → completed|failed|cancelled
Result: recorded → superseded|archived
Finding: candidate → accepted|challenged|rejected → superseded|archived
Decision: proposed → accepted|rejected → superseded|retired|archived
Requirement: draft → accepted → in_progress → implemented → verified
```

Invalid reopening, skipped material states, stale expected versions, and edits to closed Questions are rejected without a partial event or projection update.

## 8. Timeline, Search, Checkpoint, and Resume

Research timeline entries use the same ledger sequence as their audit event and are immutable. Filtering supports session, entity, event type, and bounded pagination. Session filters also include later activity on items associated with that session.

Search is local and deterministic. It supports bounded text, type, status, session, and page filters and never hides deterministic matches behind semantic ranking. Semantic retrieval remains CP10.

A Research Checkpoint is an immutable bookmark at an exact pre-event ledger position. Its schema records counts, unresolved items, next actions, note, source entity versions, `development_required: false`, and `complete: false`. It is valid during unfinished work. Resume returns the latest bookmark, live unresolved state, whether relevant Research events occurred afterward, the delta count, saved next actions, and an explicit truncation indicator.

## 9. Deterministic Research Report

The CP3 report is generated locally without AI. It includes Questions, Evidence summaries and stable references, Experiments, Results, Finding claims with typed source IDs, Decisions with rationale, optional Requirements, exact project ID, and source ledger sequence. User-controlled text is bounded, flattened, HTML-escaped, and truncated in the projection. Original Evidence payload is not copied into the report by default.

CP8 may add styled, saved, and diagram-aware report projections without changing CP3 canonical records.

## 10. Privacy and Resource Controls

- No network or provider dependency exists in CP3.
- Artifact classification and availability remain enforced by CP2.
- Evidence source, annotation, and interpretation remain distinct.
- Audit payloads carry identifiers and bounded metadata rather than copying full source content.
- Search and timeline are project-local and paginated to at most 100 results.
- Direct source sets, lists, metadata JSON, text, checkpoint sources, and reports have explicit size/count ceilings.
- Report rendering neutralizes source-controlled line/HTML injection.
- Integrity scans diagnose missing normalized rows, missing search projections, invalid Research links, foreign keys, and Artifact faults with recovery guidance.

## 11. CP4 Handoff

CP4 may attach a repository and link ChangeSets to the stable Requirement records created here. It must not require a Research record, rewrite Research origins, mutate Research normalized tables directly, or invent missing Evidence. The R&D bridge remains optional and relationship-based.

## 12. Deliberate Deferrals

- Gemini proposals, synthesis, and contradiction analysis: CP7.
- captured screenshot/audio/recording acquisition adapters: CP9; CP3 already accepts their stable Artifact references and capture metadata.
- semantic retrieval and full Context Pack selection: CP10.
- Git and ChangeSet behavior: CP4.
- polished visual reports/diagrams: CP8.

These are deferred services, not missing CP3 canonical behavior.
