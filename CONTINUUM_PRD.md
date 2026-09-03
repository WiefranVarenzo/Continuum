# Continuum - Product Requirements Document

> **Status:** Approved product baseline; implementation validated through CP3
> **Version:** 1.2.0
> **Last updated:** 2026-09-03
> **Primary source of truth:** This Markdown document  
> **Product stage:** CP3 PASS - Ready for CP4 Development Core
> **Change policy:** Material changes to product direction, domain boundaries, privacy posture, or checkpoint scope require an explicit decision record (ADR) and an update to this PRD.
> **Related decisions:** [ADR-001 - Modular Spaces and Optional R&D Bridge](docs/adr/ADR-001-MODULAR-SPACES-AND-OPTIONAL-RD-BRIDGE.md); [ADR-006 - Provider-Neutral AI and MCP Boundaries](docs/adr/ADR-006-PROVIDER-NEUTRAL-AI-AND-MCP-BOUNDARIES.md)

## 1. Executive Summary

Continuum is a local-first R&D workspace that preserves where work stopped, what is already known, why decisions were made, and what should happen next. It gives a user two purpose-built working environments-**Research Space** and **Development Space**-backed by a shared **Continuity Core**. Each Space is independently useful: a user may conduct research without development, document and continue development without repeating prior research, or connect both into a seamless R&D learning loop. The system records evidence, experiments, findings, decisions, requirements, repository changes, code structures, tests, and generated artifacts as traceable project history rather than disconnected notes and files.

The central product promise is continuity: a user, collaborator, or AI assistant should be able to answer *where work stopped*, *what changed*, *why it changed*, *which evidence justified it*, *what remains unresolved*, and *what to do next*. When Research and Development are connected, the same history should also reveal which Decision authorized a Requirement, where it was implemented, and how it was verified. Continuum therefore treats provenance, smart-bookmark Checkpoints, progressive retrieval, and reproducible project state as first-class product capabilities.

The initial product is optimized for a single technical researcher/developer working on a laptop with an AMD Ryzen 5 5600H and 16 GB RAM. Durable project facts are stored locally and deterministically. A provider-neutral AI layer can use Gemini, OpenAI, Anthropic, selected OpenAI-compatible services such as OpenRouter, DeepSeek, and BytePlus ModelArk, or future conforming providers for bounded semantic assistance. No model becomes the source of truth or may silently mutate canonical records. AI inputs pass through one privacy gateway, use versioned prompts and schemas, and produce reviewable outputs with provenance. Separately, a permissioned Continuum MCP Server lets compatible external clients such as Codex, Claude Code, and Gemini CLI consume bounded project context and submit proposals without raw database or filesystem access.

Delivery is organized into the previously agreed **12 checkpoints (CP1–CP12)**. CP1 completes the architecture, domain model, system contracts, privacy model, diagrams, performance budgets, and ADRs. CP2–CP12 progressively implement the continuity foundation, research and development workflows, code intelligence, provenance graph, provider-neutral semantic intelligence, visualization/reporting, research capture, context engine, MCP-based AI continuity interface, and production hardening.

## 2. Document Purpose and Authority

This PRD defines the product requirements for Continuum without changing the agreed architecture. It is intended to align product design, system architecture, implementation, testing, and release decisions.

When documents conflict, authority is:

1. Explicitly approved ADRs for their scoped architectural decision.
2. This PRD for product intent, scope, and acceptance.
3. Versioned system contracts and schemas for implementation interfaces.
4. Design documents and implementation plans for execution detail.
5. Generated reports, AI outputs, and informal notes as non-authoritative supporting material.

The Markdown file is version-controlled. Every material requirement should have a stable identifier so implementation issues, tests, and ADRs can reference it.

## 3. Product Vision

### 3.1 Vision statement

Continuum enables rigorous, inspectable, and resumable research and development-independently or together-by preserving current state, reasoning, provenance, and next actions.

### 3.2 Product principles

- **Continuity over snapshots:** preserve the evolution of understanding, not only the latest state.
- **Independently useful, seamlessly connected:** Research Space and Development Space must each deliver complete standalone value; their bridge is optional and frictionless.
- **Bookmark the work, not only the file:** a Checkpoint preserves meaningful work state, unresolved items, and next actions rather than merely a location or timestamp.
- **Provenance by default:** important claims, decisions, changes, and generated outputs remain traceable to their sources.
- **Local-first ownership:** the project remains useful without a network connection; local data is canonical.
- **Deterministic truth, semantic assistance:** machines enforce identity, storage, validation, ordering, and graph integrity; AI proposes interpretations.
- **Human authority:** users approve consequential AI suggestions and control external data transmission.
- **Progressive context:** retrieve the smallest useful context first, then expand on demand.
- **Inspectable state:** users can see what the system knows, how it knows it, and what remains uncertain.
- **Graceful degradation:** capture, browsing, visualization, and AI failures must not corrupt core project history.
- **Resource discipline:** the primary workflow must remain practical on the target laptop.

## 4. Problem Statement

Technical R&D is fragmented across browser tabs, recordings, documents, chat sessions, source repositories, issue trackers, terminal output, and human memory. Conventional note-taking preserves fragments but usually loses the causal chain between evidence and implementation. Git records code evolution but not necessarily research intent. AI assistants can accelerate analysis, yet their context is transient, their outputs may be unverifiable, and repeated sessions often reconstruct the same background at high token and time cost.

This fragmentation creates six core problems:

1. **Lost rationale:** teams know what the code does today but cannot reliably reconstruct why a design or implementation decision was made.
2. **Weak traceability:** evidence, findings, requirements, changes, and tests are stored separately and linked inconsistently when a project needs both research and development.
3. **Session discontinuity:** humans and AI assistants spend time rebuilding project context after every interruption or handoff.
4. **Unbounded semantic automation:** AI-generated summaries or conclusions can be mistaken for durable facts without review, evidence, or schema validation.
5. **Heavyweight workflows:** existing research, capture, knowledge-graph, and developer tools often require multiple products or exceed the resources available on a typical development laptop.
6. **Forced workflow mismatch:** tools often assume either a research-only process or a development-only process, while users need to start in either space and connect them only when useful.

## 5. Goals and Non-Goals

### 5.1 Goals

- G-01: Preserve a durable, queryable project ledger of research and development events.
- G-02: Model and connect Research Questions, Evidence, Experiments, Results, Findings, Decisions, Requirements, ChangeSets, code entities, and Tests.
- G-03: When both Spaces are connected, make the optional chain `Evidence → Finding → Decision → Requirement → ChangeSet → Code → Test` navigable in both directions.
- G-04: Make Research Space and Development Space independently useful over one shared Continuity Core, without requiring one as a prerequisite for the other.
- G-05: Create semantic checkpoints that allow users and AI tools to resume work from a compact, verifiable project state.
- G-06: Generate bounded Context Packs with explicit provenance, scope, freshness, and token budgets.
- G-07: Support replaceable AI providers for semantic tasks while keeping canonical data and safety-critical operations deterministic; Gemini remains a first-class provider rather than a domain dependency.
- G-08: Support structured capture of screens, system audio, microphone, screenshots, browser material, files, web evidence, and markers.
- G-09: Generate useful graph visualizations, architecture diagrams, research reports, and development documentation.
- G-10: Operate responsively on Ryzen 5 5600H / 16 GB RAM through incremental processing, caching, background jobs, and bounded concurrency.
- G-11: Protect private project data through local-first storage, explicit consent, data minimization, redaction, and auditable AI requests.
- G-12: Expose continuity safely through a permissioned Continuum MCP Server to compatible external AI clients in CP11.
- G-13: Let a project begin in Research-only or Development-only usage and enable the other Space later without project migration or loss of history.

### 5.2 Non-goals

- NG-01: Replacing Git as the source-control system.
- NG-02: Replacing a full IDE, browser, office suite, or general-purpose video editor.
- NG-03: Fully autonomous research, product decisions, code changes, or release approval.
- NG-04: Treating model-generated content as verified evidence without human review and provenance.
- NG-05: Uploading the complete repository, recording, or project database to an AI provider by default.
- NG-06: Building real-time multi-user collaboration in the initial MVP.
- NG-07: Supporting every programming language, repository host, capture backend, or model provider in the MVP.
- NG-08: Performing continuous whole-repository semantic re-analysis after every edit.
- NG-09: Requiring cloud availability for core project browsing, capture ingestion, linking, or checkpoint inspection.
- NG-10: Becoming a project-management suite, generic wiki, or issue tracker.
- NG-11: Forcing every Research project to produce software or every Development project to reconstruct prior research inside Continuum.

## 6. Target Users and Personas

### 6.1 Primary persona - Independent technical R&D builder

Works across literature, experiments, product decisions, and implementation. Needs to resume complex work after interruptions and wants AI help without losing ownership or auditability.

Key needs:

- capture evidence quickly without breaking focus;
- connect research conclusions to actual code changes;
- understand current state and next actions;
- keep private work local and control AI disclosure;
- run the product on a mid-range laptop.

### 6.2 Secondary persona - Software engineer joining or revisiting a project

Needs to understand design intent, relevant constraints, recent ChangeSets, tests, and unresolved contradictions without reading the entire history.

### 6.3 Secondary persona - Research lead or technical reviewer

Reviews whether findings are supported, decisions are justified, requirements are implemented, and reports reflect the underlying evidence.

### 6.4 System persona - Permissioned AI assistant

Consumes a bounded Context Pack through the MCP-based AI Continuity Interface, cites source entity IDs, proposes structured outputs, and operates within explicit project/capability permissions. It is a consumer and contributor of reviewable candidates, not the owner of canonical truth.

## 7. Core User Journeys

### 7.1 Start a project and establish a baseline

1. The user creates or opens a Continuum project.
2. Continuity Core initializes the Project Ledger and Artifact Store.
3. The user chooses an initial entry path: Research, Development, or Connected R&D.
4. For Research, the user defines an initial Research Question or Research Session. For Development, the user attaches a Git repository and may enter or import an existing Requirement, task, brief, or intent. Connected R&D may initialize both.
5. Continuum records the applicable research baseline and/or exact repository baseline plus project privacy policy.
6. The system creates a first scope-aware Checkpoint showing known state, unknowns, active work, and next actions.

Expected outcome: the project can be reopened and its baseline reconstructed deterministically.

The entry choice configures the initial experience; it is not an immutable project type. Either Space can be enabled later without migration.

### 7.2 Conduct and resume Research-only work

1. The user begins a Research Session.
2. The user captures a web page, file, screenshot, recording segment, note, or marker.
3. Continuum stores the original artifact or stable reference plus metadata and content hash.
4. The user associates the Evidence with a Research Question or Experiment.
5. An enabled AI provider may propose a summary, candidate Finding, or contradiction, with cited Evidence IDs.
6. The user edits, accepts, or rejects the candidate.
7. Accepted Findings become canonical records while retaining their derivation trail.
8. The user creates a Research Checkpoint containing active questions, Evidence state, ongoing Experiments, current Findings, uncertainties, and next research actions.

Expected outcome: research is valuable and resumable without a repository, ChangeSet, code, Test, or future development obligation. A reviewer can navigate from a Finding to its exact Evidence and distinguish source material, user assertions, and AI inference.

### 7.3 Conduct and resume Development-only work

1. The user attaches or opens an existing repository.
2. The user records or imports the existing Requirement, task, brief, issue, Decision, or implementation intent when available.
3. Continuum marks its origin honestly as manual, imported, external, legacy, or otherwise known; it does not fabricate prior Evidence.
4. Commit and diff ingestion identify ChangeSets and affected CodeEntities.
5. The user links Changes, Tests, TestRuns, validation results, and documentation to the available intent.
6. The user creates a Development Checkpoint containing repository state, active Requirement/task, working ChangeSet, verification status, blockers, and next development actions.

Expected outcome: development and its documentation are useful and resumable without requiring Research Space or recreating research that occurred elsewhere.

### 7.4 Connect research to implementation

1. The user converts a Finding into a Decision or links it to an existing Decision.
2. The user records alternatives, rationale, constraints, and status.
3. The Decision creates or updates one or more Requirements.
4. Development Space detects or imports repository commits and diffs.
5. The user links ChangeSets to Requirements; Continuum suggests links but does not finalize them silently.
6. Code intelligence derives affected files, symbols, dependencies, configurations, and tests.
7. Verification status indicates whether each Requirement has implementation and test coverage evidence.
8. Failed or surprising validation may create Learning Feedback as new Evidence, a new Result, a revised Finding, a revised Decision, or a revised Requirement.

Expected outcome: the complete rationale-to-verification chain and optional learning loop are inspectable in either direction. Neither Space loses its standalone behavior.

### 7.5 Resume after interruption

1. The user opens a project after time away.
2. The system selects the latest valid checkpoint and detects subsequent events.
3. The user sees a scope-appropriate Current Project State: research state, development state, or integrated R&D state.
4. The user or AI requests a task-scoped Context Pack.
5. The system composes the pack from deterministic retrieval, includes provenance and freshness, and only invokes AI if requested and permitted.

Expected outcome: useful work can resume without rereading the entire project.

### 7.6 Explain a change

1. The user selects a commit, ChangeSet, file, or symbol.
2. Continuum collects deterministic diff and graph context.
3. The selected AI provider produces a structured explanation grounded in linked Decisions, Requirements, and tests.
4. Unsupported statements are flagged as inference or unknown.
5. The user may save the explanation as a versioned generated artifact.

### 7.7 Generate a report or diagram

1. The user selects scope, audience, and checkpoint/time range.
2. The system builds a deterministic source bundle.
3. AI may plan narrative or diagram structure within a versioned schema.
4. The renderer produces a Research Report, development documentation, knowledge graph view, or architecture diagram.
5. Every generated claim retains source references; stale outputs are visibly marked.

## 8. Product Scope and System Boundaries

### 8.1 In scope

- local project lifecycle and settings;
- SQLite-backed Project Ledger;
- content-addressed or hash-verified Artifact Store;
- domain entity and relationship persistence;
- append-oriented event history and projections;
- Research Space workflows;
- Development Space workflows and Git ingestion;
- Research-only, Development-only, and Connected R&D usage without separate project formats;
- optional activation of either Space at any point in a project lifecycle;
- optional R&D Bridge linking research outcomes, implementation, validation, and learning feedback;
- structural code intelligence using Tree-sitter, ast-grep, dependency/configuration analyzers, and test parsers;
- provenance graph and bidirectional navigation;
- provider-neutral semantic services with Gemini, OpenAI, Anthropic, selected OpenAI-compatible providers, structured outputs, caching, privacy controls, and prompt/schema versioning;
- React Flow + ELK graph/diagram presentation;
- capture workflow for screen, system audio, microphone, screenshots, markers, browser, files, and web evidence;
- checkpoints, Context Packs, and progressive retrieval;
- permissioned MCP-based AI Continuity Interface for compatible external clients;
- observability, recovery, migration, packaging, and release hardening.

### 8.2 Outside the initial scope

- simultaneous collaborative editing;
- hosted SaaS synchronization as a requirement;
- mobile-native capture applications;
- arbitrary binary analysis;
- autonomous code execution based solely on AI output;
- broad enterprise administration and billing;
- generalized video post-production;
- guaranteed archival of third-party web content where legal or technical restrictions apply.

### 8.3 Logical architecture

Continuum has three product domains and one optional cross-domain bridge:

1. **Continuity Core** is always active and owns identity, project lifecycle, the ledger, artifacts, events, provenance relationships, checkpoints, context retrieval, privacy policy enforcement, and stable contracts.
2. **Research Space** is independently usable and manages inquiry, evidence, experiments, results, findings, decisions, research sessions, and research reporting. It does not require a repository or development outcome.
3. **Development Space** is independently usable and manages existing intent/Requirements, repository baselines, commits, diffs, ChangeSets, code structure, dependencies, configurations, Tests, TestRuns, validation, and development documentation. It does not require in-product research history.
4. **R&D Bridge** is optional and connects accepted research outcomes to implementation, then returns validation learning to the appropriate research or requirement state.

Cross-cutting services include Provider-Neutral Semantic Intelligence, Visual Intelligence & Reports, Research Capture System, and the MCP-based AI Continuity Interface. The outbound AI Provider Gateway and inbound Continuum MCP Server are separate adapters; both consume Continuity Core application contracts rather than bypassing them.

Research-only, Development-only, and Connected R&D are usage configurations, not incompatible project types or separate storage formats. Enabling another Space changes available capabilities and views, not the identity or history of the project. Space-specific modules must not manufacture placeholder entities merely to satisfy the connected chain.

### 8.4 Supported usage configurations

**Research-only** uses Continuity Core plus Research Space. Its valid terminal states include a Finding, Decision, unresolved investigation, Research Report, or Research Checkpoint. Development entities are optional.

**Development-only** uses Continuity Core plus Development Space. Work may originate from a manually entered or imported Requirement, task, brief, issue, Decision, existing repository state, or direct implementation intent. Missing in-product research provenance is represented honestly as external, legacy, manual, or unknown-not backfilled by AI.

**Connected R&D** uses both Spaces and the R&D Bridge. A Finding may inform a Decision, a Decision may create a Requirement, a ChangeSet may implement it, and validation may feed learning back into Research Space or revise the Requirement. Each relationship remains explicit and typed.

## 9. Domain Model and Provenance Contract

### 9.1 Canonical entity classes

- **Project:** top-level ownership and policy boundary.
- **ResearchSession:** bounded period of inquiry or capture.
- **ResearchQuestion:** question, hypothesis, or uncertainty being investigated.
- **Evidence:** sourced material with origin, capture metadata, integrity data, and relevance.
- **Experiment:** planned procedure intended to test a question or hypothesis.
- **Result:** observed output of an Experiment, distinct from interpretation.
- **Finding:** interpreted conclusion supported or challenged by Evidence and/or Results.
- **Decision:** selected course of action with rationale, alternatives, constraints, and status.
- **Requirement:** verifiable product or technical obligation derived from a need, Finding, or Decision.
- **Repository:** attached Git repository identity and configuration.
- **RepositoryBaseline:** a known repository state used for comparison.
- **Commit:** ingested Git commit metadata.
- **ChangeSet:** meaningful grouping of changes, potentially spanning one or more commits.
- **CodeEntity:** file, module, symbol, configuration item, dependency, or other addressable code structure.
- **Test:** known verification artifact or test case.
- **TestRun:** execution result tied to a repository state when available.
- **Artifact:** immutable or versioned binary/text payload managed by the Artifact Store.
- **Checkpoint:** immutable summary/index of project state at a known ledger position.
- **ContextPack:** purpose-bound, budgeted, provenance-bearing retrieval result.
- **GeneratedArtifact:** AI-assisted or deterministic report, diagram, explanation, or summary.
- **Event:** timestamped record of a state transition or observed external change.

### 9.2 Entity invariants

- Every canonical entity has a stable unique ID, project ID, type, lifecycle status, creation time, update/version information, and origin.
- Source content and user-authored interpretation are represented separately.
- AI candidates are distinguishable from user-approved canonical records.
- Mutable records use versioning or event references sufficient to reconstruct material history.
- Artifact payloads are referenced by stable IDs and integrity hashes; database rows do not silently point to missing payloads.
- Deletion uses explicit lifecycle semantics. Referential history is not silently destroyed.
- Relationships are typed, directed where meaningful, timestamped/versioned, and attributable to user, deterministic process, import, or AI proposal.

### 9.3 Conditional relationship chains

The most complete Connected R&D traceability chain is:

```text
Evidence → Finding → Decision → Requirement → ChangeSet → Code → Test
```

This is a composable chain, not a mandatory pipeline. Valid standalone chains include:

```text
ResearchQuestion → Evidence and/or Experiment → Result → Finding → Decision

External/Manual Origin → Requirement or ChangeSet → CodeEntity → Test → TestRun
```

Any unavailable upstream context is represented by an explicit origin and provenance state. The system must not create synthetic Evidence, Findings, Decisions, Requirements, code, or Tests merely to make the complete chain appear populated.

The graph must also represent:

- ResearchQuestion framed-by or answered-by Findings;
- Experiment addresses ResearchQuestion;
- Experiment produces Result;
- Result supports, challenges, or is inconclusive-for Finding;
- Evidence supports, challenges, contextualizes, or supersedes Finding;
- Finding informs Decision;
- Decision accepts, rejects, or supersedes alternatives;
- Decision creates, modifies, or retires Requirement;
- imported/manual/external intent originates Requirement or ChangeSet when Research Space is not used;
- ChangeSet implements, partially-implements, or reverts Requirement;
- ChangeSet modifies CodeEntity;
- Test verifies Requirement or CodeEntity;
- TestRun produces a validation outcome;
- validation outcome creates Learning Feedback that may support, challenge, or revise Evidence, Finding, Decision, or Requirement;
- Checkpoint summarizes entities and ledger position;
- ContextPack includes source entities under a declared retrieval policy.

### 9.4 Provenance requirements

- PRV-01: Every derived or generated claim must expose its direct source entity IDs.
- PRV-02: Multi-hop provenance queries must preserve relationship types and direction.
- PRV-03: A missing link must be shown as unknown/unlinked, never inferred as fact.
- PRV-04: Superseded entities remain traceable from current entities.
- PRV-05: Imported and captured Evidence records include origin URI/path when permissible, capture time, method, content hash, and availability state.
- PRV-06: AI output records include model/provider, prompt template version, output schema version, input entity IDs, generation time, and review status.
- PRV-07: Generated reports and diagrams declare their source checkpoint or ledger position and become visibly stale when relevant sources change.

## 10. Functional Requirements

### 10.1 Continuity Core

- FR-CORE-001: Users can create, open, archive, export, and recover a local project.
- FR-CORE-002: Each project has an isolated SQLite Project Ledger and project-level settings.
- FR-CORE-003: The ledger persists canonical entities, typed relationships, versions, and events transactionally.
- FR-CORE-004: The Artifact Store persists large or binary payloads outside ordinary entity fields and verifies them by hash.
- FR-CORE-005: Database references and artifact payloads remain consistent across success, cancellation, crash, and retry.
- FR-CORE-006: State-changing commands are validated, authorized, idempotent where retried, and recorded as events.
- FR-CORE-007: Read models/projections can be rebuilt from canonical records and event history where specified by the architecture.
- FR-CORE-008: Users can inspect entity history, origin, relationships, and current lifecycle status.
- FR-CORE-009: Import/export uses a versioned manifest and reports unsupported or corrupt content without damaging the active project.
- FR-CORE-010: Schema migrations are versioned, backed up, failure-safe, and testable against representative older projects.
- FR-CORE-011: Background jobs expose status, progress, cancellation, error details, and safe retry.
- FR-CORE-012: All cross-domain writes use declared Continuity Core contracts.
- FR-CORE-013: A project can activate Research Space, Development Space, or both without changing project identity, storage format, or existing history.
- FR-CORE-014: Initial usage configuration affects navigation and defaults only; it does not prevent later activation or deactivation of a Space.
- FR-CORE-015: Continuity Core supports partial provenance and explicitly records manual, imported, external, legacy, and unknown origins without fabricating missing upstream entities.

### 10.2 Research Space

- FR-RS-001: Users can create, edit, relate, status, and search Research Questions.
- FR-RS-002: Users can start and end Research Sessions and associate activities with a session.
- FR-RS-003: Users can create Evidence from notes, files, web sources, screenshots, capture markers, and recording segments.
- FR-RS-004: Evidence retains the original source or stable reference separately from annotations and summaries.
- FR-RS-005: Users can define Experiments with hypothesis/question, method, inputs, expected observations, and status.
- FR-RS-006: Users can record Results independently of Findings.
- FR-RS-007: Users can create Findings and mark Evidence/Results as supporting, challenging, contextual, or inconclusive.
- FR-RS-008: Users can record Decisions with selected option, alternatives, rationale, constraints, status, and supersession.
- FR-RS-009: A research timeline shows material events in deterministic chronological order with filters.
- FR-RS-010: An enabled AI provider may propose summaries, Finding candidates, and contradictions; users can accept, edit, reject, or defer each proposal.
- FR-RS-011: Accepted AI candidates preserve both the original proposal and the user-approved version.
- FR-RS-012: Search supports metadata/text filtering and, when enabled, semantic retrieval without hiding deterministic matches.
- FR-RS-013: Research workflows, reports, Checkpoints, and Context Packs operate without an attached repository or any Development Space entity.
- FR-RS-014: A Finding or Decision may remain a valid research outcome without producing a Requirement.

### 10.3 Development Space

- FR-DS-001: Users can attach a local Git repository without transferring ownership of source control to Continuum.
- FR-DS-002: Continuum records a RepositoryBaseline using an exact commit/ref and repository identity.
- FR-DS-003: Commit ingestion captures stable commit metadata and is incremental and repeatable.
- FR-DS-004: Diff ingestion records affected paths and change metadata while allowing source content retention policy to be configured.
- FR-DS-005: Users can group one or more changes into a ChangeSet and link it to Requirements and Decisions.
- FR-DS-006: Code intelligence extracts supported files, symbols, dependencies, configurations, and tests using deterministic analyzers.
- FR-DS-007: Tree-sitter, ast-grep, dependency/configuration analyzers, and test parsers run through versioned analyzer contracts.
- FR-DS-008: Unsupported languages or constructs degrade to file/diff-level representation rather than blocking ingestion.
- FR-DS-009: Users can navigate from ChangeSet to modified CodeEntities and known Tests.
- FR-DS-010: The system distinguishes detected test definitions, Requirement-to-Test links, and observed TestRuns.
- FR-DS-011: An enabled AI provider may explain changes and propose traceability links, but deterministic diffs and user-approved links remain authoritative.
- FR-DS-012: Repository rebase, amended commit, branch switch, rename, and deletion scenarios do not silently corrupt existing provenance.
- FR-DS-013: Development workflows, documentation, Checkpoints, and Context Packs operate without a Research Question, Evidence, Experiment, Result, or Finding.
- FR-DS-014: Users can enter or import an existing Requirement, task, brief, issue, Decision, or direct implementation intent and retain its declared origin.
- FR-DS-015: Development documentation distinguishes known rationale from external, legacy, manual, or unknown rationale.

### 10.4 R&D Bridge

- FR-BRIDGE-001: Users can enable the bridge at any point without migrating or duplicating the project.
- FR-BRIDGE-002: Users can promote or link an accepted Finding or Decision to a Requirement while preserving source provenance.
- FR-BRIDGE-003: Users can link a Requirement to one or more ChangeSets and navigate back to its research rationale when available.
- FR-BRIDGE-004: A validation outcome can create typed Learning Feedback as Evidence, Result, revised Finding, revised Decision, revised Requirement, or an explicitly unresolved item.
- FR-BRIDGE-005: Enabling the bridge does not require retroactive creation of research or development records that never existed.
- FR-BRIDGE-006: Disabling a Space from the active UI preserves its data, relationships, Checkpoints, and future reactivation path.

### 10.5 Provenance and Knowledge Graph

- FR-KG-001: Users can traverse the principal provenance chain in both directions.
- FR-KG-002: Graph queries support project, entity type, relationship, status, time range, checkpoint, and confidence/review-state filters.
- FR-KG-003: The system identifies broken, missing, stale, circular-invalid, or ambiguous links according to domain rules.
- FR-KG-004: Users can manually create, approve, reject, annotate, and retire permitted relationships.
- FR-KG-005: AI-proposed relationships remain pending until accepted where they affect canonical traceability.
- FR-KG-006: The UI distinguishes deterministic, user-authored, imported, and AI-proposed graph elements.

### 10.6 Provider-Neutral Semantic Intelligence

- FR-AI-001: Every model provider is isolated behind one application-owned AI Provider Gateway and privacy gateway.
- FR-AI-002: Every supported semantic task uses a versioned prompt template and output schema.
- FR-AI-003: Outputs are schema-validated before entering the application.
- FR-AI-004: Invalid, incomplete, timed-out, or refused outputs fail safely and remain retryable.
- FR-AI-005: Semantic tasks include research synthesis, Finding candidates, contradiction analysis, change explanation, and diagram planning.
- FR-AI-006: Cache keys include task, prompt/schema, permitted input state, exact provider/model, adapter profile, and relevant routing configuration.
- FR-AI-007: Cached outputs expose their generation time and become invalid/stale when relevant inputs or versions change.
- FR-AI-008: Users can inspect the categories and estimated size of data before transmission.
- FR-AI-009: The system logs AI request metadata without logging prohibited secret or source content.
- FR-AI-010: Loss of any or all provider connectivity does not block deterministic project operations.
- FR-AI-011: Provider/model profiles declare capabilities and known deviations; unknown capabilities fail closed rather than being inferred from API compatibility.
- FR-AI-012: Provider routing and failover are deterministic, policy-constrained, user-visible where consent is required, and fully provenance-recorded.
- FR-AI-013: Sensitive content never silently fails over to another provider; `secret/never-send` is never remotely transmitted.
- FR-AI-014: Gemini is supported as a first-class adapter while at least one non-Gemini path proves provider replacement without domain changes.
- FR-AI-015: Each provider attempt records exact destination/model, adapter/capability versions, sources, policy decision, usage, validation, errors, and review state.

### 10.7 Visual Intelligence and Reports

- FR-VIS-001: The knowledge graph is rendered with React Flow and ELK-based layout.
- FR-VIS-002: Large graphs default to scoped, progressive views rather than rendering the full project graph.
- FR-VIS-003: Users can expand, collapse, filter, focus, and navigate graph nodes.
- FR-VIS-004: AI may produce a schema-valid diagram plan; deterministic renderers own final node/edge construction and layout constraints.
- FR-VIS-005: Users can generate Research Reports, architecture diagrams, and Development Documentation from selected scope.
- FR-VIS-006: Reports cite underlying entity IDs and identify unresolved uncertainty or contradiction.
- FR-VIS-007: Generated artifacts are versioned, reproducible from declared inputs where practical, and visibly marked when stale.

### 10.8 Research Capture System

- FR-CAP-001: Users can create a capture session for screen recording, system audio, microphone, or a permitted combination.
- FR-CAP-002: Users can capture screenshots and timestamped markers during a session.
- FR-CAP-003: Users can attach browser, file, and web Evidence to the active Research Session.
- FR-CAP-004: Capture sources, permissions, encoding settings, timestamps, and failure state are recorded.
- FR-CAP-005: Partial capture output is finalized or recoverable after interruption where the underlying media format permits.
- FR-CAP-006: Capture uses bounded buffers and applies backpressure or quality reduction before exhausting system memory.
- FR-CAP-007: Users receive a clear indicator whenever microphone, screen, or system audio capture is active.
- FR-CAP-008: The capture system never enables a sensor silently and respects operating-system permission boundaries.
- FR-CAP-009: Recording segments and markers can become Evidence without duplicating the entire recording.
- FR-CAP-010: Transcription/OCR, if enabled in a phase, produces derived artifacts and never replaces the source capture.

### 10.9 Checkpoint and Context Engine

- FR-CTX-001: Users or system policy can create an immutable semantic Checkpoint at a known ledger position.
- FR-CTX-002: A Checkpoint records only the state applicable to its scope, including current goals/questions, accepted Findings and Decisions, active Requirements, repository state, recent ChangeSets, verification state, unresolved contradictions, risks, and next actions.
- FR-CTX-003: Checkpoint creation distinguishes deterministic snapshot data from AI-generated narrative.
- FR-CTX-004: Users can compare checkpoints and inspect intervening events.
- FR-CTX-005: The engine can detect when a checkpoint is stale relative to the current ledger/repository state.
- FR-CTX-006: Users and authorized clients can request a Context Pack for a declared task, scope, audience, and token/size budget.
- FR-CTX-007: Context construction uses progressive retrieval: project summary, task-relevant entities, supporting evidence, then deeper artifacts only as needed.
- FR-CTX-008: Every Context Pack includes project/checkpoint identity, retrieval time, source IDs, omitted-content summary, budget usage, and freshness.
- FR-CTX-009: Context Packs honor privacy policy, permissions, secret detection, and redaction rules.
- FR-CTX-010: Repeating the same request against unchanged state and configuration yields the same deterministic source selection, excluding explicitly semantic ranking ties.
- FR-CTX-011: Users can preview and remove Context Pack items before external transmission.
- FR-CTX-012: Context Packs are ephemeral by default unless explicitly saved as GeneratedArtifacts.
- FR-CTX-013: The engine supports Research, Development, and Integrated R&D Checkpoints with a shared envelope and scope-specific payloads.
- FR-CTX-014: Research Checkpoints do not require repository or development state; Development Checkpoints do not require research entities.
- FR-CTX-015: An Integrated R&D Checkpoint records bridge status, connected rationale, implementation/validation state, Learning Feedback, and cross-Space next actions where available.
- FR-CTX-016: A Checkpoint may be created at any meaningful point, including incomplete research, active implementation, failed validation, interruption, handoff, milestone, or successful validation.
- FR-CTX-017: A Checkpoint represents a resumable bookmark, not project termination; work may continue from or beyond it.

### 10.10 AI Continuity Interface

- FR-ACI-001: External AI clients authenticate and receive least-privilege, project-scoped capabilities.
- FR-ACI-002: A Continuum MCP Server exposes versioned resources and read tools for project state, graph traversal, checkpoints, Context Packs, and artifact metadata.
- FR-ACI-003: Write operations, if enabled, create proposals or commands subject to validation, policy, audit, and user approval.
- FR-ACI-004: The interface never exposes raw secrets, denied artifacts, or unrestricted filesystem access.
- FR-ACI-005: Responses include stable IDs, provenance, schema version, pagination/budget information, and structured errors.
- FR-ACI-006: Clients cannot bypass the privacy gateway or canonical domain invariants.
- FR-ACI-007: Local stdio is the MVP MCP transport; Streamable HTTP requires separate authentication, origin, binding, revocation, and threat-model acceptance.
- FR-ACI-008: Client-specific configuration for Codex, Claude Code, Gemini CLI, and future clients remains outside domain contracts and relies on negotiated MCP capabilities.
- FR-ACI-009: MCP resources use opaque application identifiers rather than filesystem paths, and tool access is filtered by project/capability grant.
- FR-ACI-010: MCP failure, disablement, or client incompatibility does not block local deterministic workflows or outbound provider use.

## 11. AI and Deterministic Processing Policy

### 11.1 Deterministic responsibilities

The following must remain deterministic and application-controlled:

- entity IDs, timestamps, versions, lifecycle state, and schema validation;
- SQLite transactions, migrations, indexes, backups, and integrity checks;
- artifact hashing, storage references, deduplication, and availability state;
- event ordering and idempotency;
- Git metadata, commit identity, diffs, and repository baseline;
- parsing and structural extraction using pinned analyzer versions;
- graph persistence, relationship constraints, and traversal;
- access control, privacy policy evaluation, redaction gates, and audit metadata;
- checkpoint ledger position and Context Pack budget enforcement;
- report/diagram rendering from an approved schema;
- retry, timeout, cache, cancellation, and error-state behavior.

### 11.2 AI-appropriate responsibilities

An enabled provider may assist with:

- summarization and synthesis;
- candidate Findings and candidate relationships;
- contradiction and gap detection;
- natural-language change explanations;
- relevance ranking within a deterministic candidate set;
- diagram planning within a validated schema;
- draft report narrative and labels;
- compression of context while retaining source references.

### 11.3 Prohibited AI authority

AI must not:

- invent or alter canonical source content;
- silently accept Findings, Decisions, Requirements, or provenance links;
- execute repository changes or tests solely because generated text requests it;
- determine access permissions or override a privacy decision;
- manufacture citations or present inference as observed Result/Evidence;
- delete project history;
- transmit data outside the privacy gateway;
- become the only representation of current project state.

### 11.4 Output status model

AI output progresses through explicit states such as `generated`, `invalid`, `pending_review`, `accepted`, `edited_and_accepted`, `rejected`, `stale`, and `superseded`. Only approved output may become part of canonical decision or traceability state, and its AI origin remains visible.

### 11.5 Failure and uncertainty behavior

- Schema validation failure produces no canonical mutation.
- Missing citations cause the affected claim to be marked unsupported.
- Provider errors surface a retryable task state without blocking local work.
- Contradictory AI suggestions are shown as candidates, not automatically resolved.
- Model/provider or prompt changes invalidate affected caches according to policy.
- AI summaries display the source checkpoint/ledger position and freshness.

## 12. Non-Functional Requirements

### 12.1 Reliability and data integrity

- NFR-REL-001: Canonical entity writes are transactional.
- NFR-REL-002: A crash during artifact ingestion must leave either a committed, verifiable artifact reference or a detectable recoverable orphan-not a silently broken record.
- NFR-REL-003: Import, migration, indexing, capture, and AI tasks are restartable or safely retryable.
- NFR-REL-004: Project open performs lightweight integrity checks; full verification is available on demand.
- NFR-REL-005: Users can create and restore local backups using a documented, versioned format.

### 12.2 Offline and degradation

- NFR-OFF-001: Project navigation, entity CRUD, graph traversal, checkpoint inspection, Git history already ingested, and deterministic search work offline.
- NFR-OFF-002: Network-dependent actions clearly state unavailable status and do not block unrelated workflows.
- NFR-OFF-003: Cached AI output is labeled with provider/model, age, source state, and freshness.

### 12.3 Maintainability and evolvability

- NFR-MNT-001: Domain modules communicate through versioned contracts.
- NFR-MNT-002: Analyzer, AI provider, MCP transport/client profile, capture backend, renderer, and export formats have replaceable adapters where defined in CP1.
- NFR-MNT-003: Schema, prompt, parser, and contract versions are observable in diagnostics.
- NFR-MNT-004: Architectural decisions are recorded as ADRs.

### 12.4 Accessibility and usability

- NFR-UX-001: Core workflows are keyboard accessible.
- NFR-UX-002: Status is not conveyed by color alone.
- NFR-UX-003: Long-running operations show progress and can be cancelled where safe.
- NFR-UX-004: AI, deterministic, imported, and user-authored content are visually distinguishable.
- NFR-UX-005: Destructive actions require clear scope and recovery expectations.

### 12.5 Observability

- NFR-OBS-001: Structured local logs use correlation IDs for commands and background jobs.
- NFR-OBS-002: Diagnostics include duration, queue time, cache status, analyzer/provider/MCP contract versions, and bounded error context.
- NFR-OBS-003: Secret and private payload content is excluded or redacted from logs.
- NFR-OBS-004: Users can export a privacy-scrubbed diagnostic bundle.

### 12.6 Portability

- NFR-PORT-001: Project export includes a versioned manifest, database, required artifacts, integrity hashes, and compatibility information.
- NFR-PORT-002: Platform-specific capture capabilities are isolated behind explicit adapters and capability detection.

## 13. Privacy and Security Requirements

### 13.1 Privacy posture

Continuum is local-first. Local canonical data remains usable without cloud services. Outbound provider transmission and inbound MCP disclosure are separate policy decisions; both are purpose-bound, minimized, previewable where required, and controlled by project/user policy.

### 13.2 Data classification

Project data must support at least these policy classes:

- public;
- internal/project-private;
- sensitive;
- secret/never-send.

Defaults should be conservative. Exact default classification and inheritance behavior are a CP1 decision to be documented in an ADR.

### 13.3 Privacy gateway

- SEC-001: All AI-bound payloads pass through one enforceable privacy gateway.
- SEC-002: The gateway evaluates project policy, item classification, task purpose, destination/provider, and user consent.
- SEC-003: The gateway supports inclusion previews, exclusions, redaction, and estimated payload size.
- SEC-004: Secret detection applies to repository content, configuration, environment-like data, credentials, keys, and user-defined patterns.
- SEC-005: Denied content cannot be reintroduced through retrieved summaries or linked artifacts.
- SEC-006: AI request audit records store metadata and references without retaining prohibited raw content.
- SEC-007: The user can disable all external AI requests per project.
- SEC-018: Enabled provider/model profiles are project-allowlisted and include verified destination, capability, retention/training, region, and compatibility metadata.
- SEC-019: Sensitive data cannot silently fail over to another provider/destination; secret/never-send has no remote route.

### 13.4 Storage and access

- SEC-008: The OS user account is the baseline local trust boundary for MVP.
- SEC-009: Sensitive tokens and provider credentials use platform-appropriate secure credential storage, not the project database or source tree.
- SEC-010: Temporary files use restrictive permissions and are cleaned or recoverably quarantined after failure.
- SEC-011: Exports warn when they include sensitive or secret-classified content.
- SEC-012: Path traversal, symlink escape, malformed archive, and untrusted file-name risks are addressed in import/export and artifact handling.
- SEC-013: Rendering and parsing untrusted captured content does not grant it command execution authority.
- SEC-014: External AI clients use explicit authentication, scoped authorization, rate limits, and audit records.
- SEC-020: MCP authorization is checked on every request against project, Space, resource/tool, privacy audience, and budget scope.
- SEC-021: MCP exposes no raw database, SQL, unrestricted filesystem, shell, environment, credential, or direct canonical-write capability.
- SEC-022: Streamable HTTP remains disabled until origin validation, local-binding defaults, authentication, session, revocation, and deployment threat controls pass.

### 13.5 Capture consent

- SEC-015: Screen, microphone, and system-audio capture require explicit initiation and persistent visible indication.
- SEC-016: The user can pause or stop capture immediately.
- SEC-017: The product communicates that the user is responsible for lawful capture and consent of participants.

### 13.6 Threats in scope

CP1 threat modeling must cover malicious repository content, prompt injection inside Evidence, secret exfiltration, unsafe provider routing/failover, compatibility drift, unsafe file parsing, artifact tampering, database corruption, over-broad Context Packs, unauthorized or cross-project MCP access, confused-deputy behavior, capture without awareness, and denial of service through oversized graphs/media/repos/AI responses.

## 14. Performance and Resource Targets

### 14.1 Reference hardware

- CPU: AMD Ryzen 5 5600H class, 6 cores / 12 threads.
- Memory: 16 GB system RAM.
- Storage: SSD assumed for target measurements.
- GPU: no discrete GPU required for core workflows.
- Network: optional for core; required only for configured external AI/web operations.

### 14.2 Test fixture profiles

Performance claims must name the fixture. CP1 defines at least:

- **Small:** up to 5,000 domain entities, 10,000 relationships, 10,000 repository files.
- **Standard:** up to 50,000 domain entities, 150,000 relationships, 50,000 repository files, and 10,000 commits.
- **Large/stress:** beyond Standard; graceful degradation is required but interactive SLOs are not guaranteed until benchmarked.

Media payload size is measured separately from entity counts.

### 14.3 Interactive targets on Standard fixture

- PERF-001: Warm application shell to usable project overview: p95 ≤ 3 seconds.
- PERF-002: Cold open with lightweight integrity and projection checks: p95 ≤ 8 seconds, excluding explicit migration/rebuild.
- PERF-003: Entity create/update transaction: p95 ≤ 150 ms.
- PERF-004: Metadata/text search returning first page: p95 ≤ 500 ms.
- PERF-005: One-hop provenance traversal: p95 ≤ 300 ms.
- PERF-006: Scoped graph view up to 500 visible nodes / 1,500 edges: first useful render p95 ≤ 2 seconds.
- PERF-007: Checkpoint overview load from an existing valid checkpoint: p95 ≤ 1 second.
- PERF-008: Deterministic Context Pack source selection excluding AI/network latency: p95 ≤ 2 seconds for a normal task scope.
- PERF-009: Incremental Git ingestion of 100 new commits: completes within 60 seconds under the Standard fixture, with progress and cancellation.
- PERF-010: Incremental code analysis should prioritize changed files and expose first useful results within 5 seconds; full completion is a background task.
- PERF-017: Local stdio MCP Current Project State or existing Checkpoint first-page response adds p95 ≤ 500 ms server overhead beyond the equivalent application query, excluding external-client/model latency.

### 14.4 Resource budgets

- PERF-011: Idle project memory target ≤ 500 MB RSS.
- PERF-012: Typical interactive use target ≤ 1.5 GB RSS excluding media encoding and OS/browser processes.
- PERF-013: Heavy indexing/capture soft target ≤ 4 GB RSS for the Continuum process group; the system must throttle before memory exhaustion.
- PERF-014: Default CPU concurrency reserves capacity for UI responsiveness; background worker concurrency is bounded and configurable.
- PERF-015: No unbounded in-memory media, graph, diff, embedding, or model-context accumulation.
- PERF-016: Background indexing and AI preparation are cancellable and yield to interactive commands.

These targets are product budgets, not yet validated measurements. CP1 must define reproducible benchmarks; CP2 onward records actual p50/p95 results and revises implementation-not the user-visible target-unless an explicit product decision changes it.

## 15. Success Metrics

### 15.1 North-star outcome

**Traceable continuity rate:** percentage of active implementation Requirements that can be traversed to at least one approved rationale source and one implementation or verification artifact.

### 15.2 Product metrics

- SM-01: ≥ 80% of active MVP Requirements have an approved link to a Decision or Finding.
- SM-02: ≥ 80% of completed MVP Requirements link to a ChangeSet; test linkage is reported separately and never fabricated.
- SM-03: A returning pilot user can identify current state, top unresolved question, and next action within 2 minutes of opening a valid checkpoint.
- SM-04: ≥ 90% of sampled generated claims in accepted reports link to relevant source entities.
- SM-05: ≥ 95% of canonical AI-assisted records are explicitly reviewed before acceptance.
- SM-06: Median time to capture and link simple Evidence is ≤ 30 seconds, excluding source acquisition.
- SM-07: ≥ 99.9% of successful canonical writes pass post-write integrity checks in automated reliability testing.
- SM-08: Zero known transmissions of `secret/never-send` content in privacy-gateway test suites and pilot incident reports.
- SM-09: ≥ 90% of normal interactive operations meet their Standard-fixture p95 performance target.
- SM-10: A fresh AI session using a Context Pack can correctly identify the active goal, accepted constraints, and relevant source IDs in ≥ 80% of curated continuity evaluations.
- SM-11: A Research-only pilot can stop and resume an investigation from a Research Checkpoint without creating any Development entity.
- SM-12: A Development-only pilot can document and resume repository work from a Development Checkpoint without creating synthetic research history.
- SM-13: A pilot project can activate the second Space and establish its first R&D Bridge link without project export/import, duplicate project creation, or loss of prior history.
- SM-14: Every enabled production provider profile passes the common privacy, schema, provenance, failure, and capability conformance suite before use.
- SM-15: Supported MCP client profiles complete the scoped read/resume workflow with zero cross-project or denied-content disclosure in acceptance tests.

### 15.3 Guardrail metrics

- AI candidate acceptance rate is diagnostic, not a target to maximize.
- Unsupported-claim rate, stale-output usage, redaction misses, unsafe provider failover, compatibility drift, MCP authorization failures, failed migrations, broken artifact references, and UI-blocking background tasks must trend toward zero.
- Capture duration and data volume do not count as success without useful Evidence linkage.

## 16. MVP Definition

### 16.1 MVP objective

Prove that a single user can use Research Space or Development Space independently, pause and resume either workflow through a trustworthy Checkpoint and Context Pack, and-when useful-connect an evidence-backed Decision to implementation and validation without migrating projects or reconstructing context.

### 16.2 MVP must include

- one local user and local-first project lifecycle;
- Research-only, Development-only, and Connected R&D entry paths within one project format;
- later activation of either Space without migration or loss of history;
- SQLite Project Ledger and Artifact Store;
- Research Questions, Evidence, Experiments, Results, Findings, Decisions, and Requirements;
- repository baseline, incremental commit/diff ingestion, ChangeSets, and file-level code entities;
- basic symbol/test extraction for the initially supported language set defined in CP1;
- standalone Research and Development provenance plus optional canonical linking across the full Connected R&D chain;
- provider-neutral AI-proposed research synthesis, candidate Findings, contradiction analysis, and change explanation through the privacy gateway, proven with Gemini and at least one non-Gemini provider path;
- semantic Checkpoints and task-scoped Context Packs;
- a local stdio Continuum MCP Server supporting the scoped read/resume workflow and reviewable proposal submission for supported external clients;
- scoped knowledge graph visualization and a basic cited Research Report;
- basic screenshot, file, web evidence, marker, and one supported recording path; advanced cross-platform capture may follow;
- export/backup, migrations, diagnostics, and recovery needed for pilot safety;
- measured performance on the reference hardware.

### 16.3 MVP may defer

- simultaneous collaboration and hosted sync;
- broad language/analyzer coverage;
- advanced report theming and complex diagram editing;
- every OS-specific system-audio backend;
- full transcription/OCR pipeline if it jeopardizes resource targets;
- unrestricted external write APIs;
- support for every model provider and every optional provider-native feature;
- remote/public MCP deployment and Streamable HTTP if its CP11 security gate is not yet satisfied;
- enterprise identity, administration, and policy distribution;
- large/stress fixture interactive SLOs.

### 16.4 MVP exit criteria

MVP is complete only when all three usage-configuration acceptance scenarios in Section 21 pass, no critical privacy/data-loss defect remains open, backup/restore has been exercised, and performance results on the reference hardware are published against the named fixture.

## 17. Phased Roadmap - CP1 to CP12

The checkpoint order is architectural and remains unchanged. A checkpoint exits only when its required contracts, tests, documentation, and relevant acceptance criteria are complete.

### CP1 - Architecture, Domain Model & System Contracts

Purpose: complete the blueprint before implementation.

Deliverables:

- product and logical architecture;
- Research/Development/Continuity boundaries and optional R&D Bridge;
- standalone usage, late Space activation, and scope-aware Checkpoint contracts;
- domain ontology, identifiers, lifecycles, and relationship rules;
- storage, event, artifact, query, job, import/export, and error contracts;
- AI-versus-deterministic responsibility matrix;
- provider-neutral AI Provider Gateway, capability/routing/failover, and provider-attempt provenance contracts;
- inbound Continuum MCP Server resources/prompts/tools, grant, transport, and proposal contracts;
- privacy classification, threat model, and outbound/inbound AI boundary contracts;
- performance fixtures, budgets, and benchmark protocol;
- architecture diagrams, sequence flows, data-flow diagrams, and ADR set;
- acceptance test strategy and traceability from PRD requirements.

Exit: ambiguous cross-domain ownership is resolved; Research-only, Development-only, and Connected R&D flows are defined end to end; no entity is mandatory solely to complete an unused Space; versioned contracts are reviewable; critical architectural risks have explicit ADRs or experiments.

### CP2 - Continuity Core

Status: **PASS (contract-aligned 2026-09-02).** The CP2.1 audit closure, migration evidence, and validation are recorded in `docs/cp2/`.

Purpose: implement durable local foundation.

Deliverables:

- SQLite schema/migrations;
- Project Ledger and lifecycle;
- Artifact Store and integrity handling;
- entity/relation persistence;
- event system, command validation, job state, backup/export baseline.

Exit: canonical CRUD, events, artifacts, recovery, and migrations pass integration and corruption tests.

### CP3 - Research Core

Status: **PASS (validated 2026-09-02).** Architecture, implementation, traceability, acceptance, benchmark, and validation evidence are recorded in `docs/cp3/`.

Purpose: implement deterministic research workflow.

Deliverables:

- Research Question, Evidence, Experiment, Result, Finding, Decision, and Requirement flows;
- research timeline and basic Research Sessions;
- manual provenance links and review states.

Exit: a user can complete, pause, resume, and report a research cycle without AI, repository, or Development Space dependency and audit every material transition.

### CP4 - Development Core

Purpose: connect repository history to Continuum.

Deliverables:

- repository attachment and baseline;
- Git commit and diff ingestion;
- ChangeSet model and Requirement linkage;
- branch/rewrite/rename handling rules.

Exit: a user can start from existing/manual/external development intent, ingest changes incrementally and reproducibly, document known and unknown rationale honestly, and resume without Research Space.

### CP5 - Code Intelligence

Purpose: understand code structure deterministically.

Deliverables:

- Tree-sitter and ast-grep adapters;
- dependency/configuration analyzers;
- test parsers;
- symbol and structural change extraction;
- incremental analysis and unsupported-language fallback.

Exit: changed code structures and tests are addressable, versioned, and linked to exact repository state.

### CP6 - Provenance & Knowledge Graph

Purpose: make research-to-code traceability operational.

Deliverables:

- full typed chain from Evidence through Test;
- graph validation, bidirectional traversal, gap detection, and history;
- provenance query/read models.

Exit: standalone partial chains and the optional end-to-end Connected R&D chain can be created, queried, validated, and audited without AI; Learning Feedback can cross the bridge without forcing a loop.

### CP7 - Provider-Neutral Semantic Intelligence

Purpose: add bounded semantic assistance without provider lock-in.

Deliverables:

- versioned semantic-task and normalized-result contracts;
- provider registry, capability descriptors, deterministic routing, and explicit failover policy;
- native Gemini adapter and at least one independently configured non-Gemini provider path;
- OpenAI-compatible provider profiles for selected services without assuming feature parity;
- structured output validation;
- research synthesis, Finding candidates, contradiction analysis, and change explanation;
- AI privacy gateway;
- semantic cache and prompt/schema versioning;
- review workflow, provider-attempt provenance, usage metadata, and conformance tests.

Exit: enabled providers pass the common conformance gates; routing and failover are inspectable; all AI tasks fail safely, respect privacy policy, cite inputs, and cannot silently mutate canonical truth.

### CP8 - Visual Intelligence & Reports

Purpose: make project reasoning understandable and shareable.

Deliverables:

- React Flow + ELK scoped knowledge graph;
- AI diagram planner with deterministic renderer;
- Research Report, architecture diagram, and Development Documentation generation;
- freshness and citations for generated artifacts.

Exit: users can navigate a scoped graph and generate a source-backed, versioned report within resource budgets.

### CP9 - Research Capture System

Purpose: reduce friction between observation and Evidence.

Deliverables:

- screen recording, system audio, microphone, screenshot, and marker workflows;
- browser, file, and web Evidence capture;
- explicit capture indicators, permissions, bounded buffers, recovery, and segment linkage.

Exit: supported capture paths produce recoverable artifacts and Evidence without destabilizing core workflows.

### CP10 - Checkpoint & Context Engine

Purpose: make work resumable and context-efficient.

Deliverables:

- semantic Checkpoints and current project state;
- checkpoint comparison and staleness;
- progressive retrieval;
- provider-neutral, budgeted, privacy-filtered Context Packs with declared token-estimate method and uncertainty;
- context quality and token-efficiency evaluations.

Exit: a user and AI can resume curated pilot tasks from a Context Pack with traceable sources and within declared budgets.

### CP11 - AI Continuity Interface

Purpose: allow MCP-compatible external AI systems to consume Continuum safely.

Deliverables:

- Continuum MCP Server with local stdio as the MVP transport;
- versioned, permissioned resources, prompts, and tools for checkpoints, context, graph, and artifact metadata;
- authentication/grants, scoped authorization, pagination, audit, revocation, and rate limits;
- proposal-based write contracts where enabled.
- compatibility fixtures and setup profiles for Codex, Claude Code, and Gemini CLI;
- separately gated Streamable HTTP support when remote access is approved.

Exit: supported MCP clients can perform the approved read/resume and proposal workflows without database/filesystem access, cross-project leakage, direct canonical writes, or privacy bypass.

### CP12 - Hardening & Release

Purpose: reach pilot/release quality.

Deliverables:

- performance optimization and benchmark report;
- threat-model closure and security testing;
- migration, backup/restore, crash recovery, and corrupted-project testing;
- packaging, onboarding, diagnostics, accessibility, and release documentation;
- end-to-end acceptance, regression, and upgrade suites.

Exit: MVP exit criteria are met on the reference hardware, release blockers are closed, and known limitations are documented.

## 18. Dependencies

### 18.1 Confirmed architectural dependencies

- SQLite for local canonical persistence.
- Git for repository history and baseline identity.
- Tree-sitter for supported-language parsing.
- ast-grep for structural matching/transformation-oriented analysis use cases.
- language/package-specific dependency and configuration analyzers.
- test parsers for supported frameworks.
- pluggable model-provider APIs behind the AI Provider Gateway; initial candidates include Gemini, OpenAI, Anthropic, OpenRouter, DeepSeek, and BytePlus ModelArk.
- MCP protocol implementation for the CP11 Continuum MCP Server.
- React Flow for interactive graph presentation.
- ELK for graph/diagram layout.
- operating-system capture APIs and media codecs/backends for screen/audio capture.

### 18.2 Dependency policies

- Versions are pinned or constrained and recorded in diagnostics/build metadata.
- External APIs are wrapped by application-owned adapters.
- License, redistribution, platform support, maintenance health, and security posture are evaluated before adoption.
- Parser/analyzer upgrades trigger compatibility and output-diff tests.
- The loss of an optional dependency degrades only its bounded capability.
- Every enabled provider/model profile must document capabilities, deviations, data retention/training posture, region, rate limits, and pricing/usage behavior with a verification date before pilot use.
- API-format compatibility does not waive provider-specific conformance, privacy, or failure testing.

### 18.3 Sequencing dependencies

- CP3–CP11 depend on stable CP2 identity and persistence contracts.
- CP5 depends on repository contracts from CP4.
- CP6 depends on stable domain models from CP3–CP5.
- CP7 depends on privacy and AI contracts defined in CP1 and persisted by CP2.
- CP8 depends on graph contracts from CP6 and semantic planning contracts from CP7.
- CP9 depends on Artifact Store reliability from CP2 and Evidence contracts from CP3.
- CP10 depends on stable provenance and project-state contracts from CP6.
- CP11 depends on Context Pack contracts from CP10 and security boundaries from CP1.
- CP12 validates the integrated system rather than introducing a new core product domain.

## 19. Risks and Mitigations

### R-01: Domain model becomes too complex

Impact: high. Likelihood: medium.

Mitigation: keep a small canonical vocabulary, define invariants in CP1, allow typed metadata extensions, and require real end-to-end examples before adding entities.

### R-02: Provenance capture creates excessive user friction

Impact: high. Likelihood: high.

Mitigation: automatic deterministic links where certainty is exact, optional AI suggestions, batch review, sensible defaults, and metrics for time-to-capture/link.

### R-03: AI hallucination contaminates project truth

Impact: high. Likelihood: medium.

Mitigation: candidate states, source-ID requirements, schema validation, visible origin, human approval, and no silent canonical writes.

### R-04: Private code or secrets leave the device

Impact: critical. Likelihood: medium.

Mitigation: centralized privacy gateway, conservative classifications, secret scanning, preview/redaction, never-send policy, audit metadata, and adversarial tests.

### R-05: Performance exceeds 16 GB RAM

Impact: high. Likelihood: medium.

Mitigation: incremental analysis, bounded queues/concurrency, pagination, graph scoping, streamed artifacts, cache limits, and benchmarks from CP1 onward.

### R-06: Cross-platform capture is unreliable

Impact: medium. Likelihood: high.

Mitigation: capability-detected adapters, phased OS support, recoverable media formats, explicit degraded modes, and a limited MVP capture path.

### R-07: Repository history rewriting breaks links

Impact: high. Likelihood: medium.

Mitigation: stable Continuum IDs, immutable observations, explicit supersession/reconciliation, repository identity rules, and test fixtures for rebase/amend/rename.

### R-08: Generated reports become stale or misleading

Impact: medium. Likelihood: high.

Mitigation: bind outputs to checkpoint/ledger position, show freshness, preserve citations, and require regeneration or explicit acknowledgment.

### R-09: External dependency/provider changes

Impact: medium. Likelihood: medium.

Mitigation: adapters, version pinning, contract tests, graceful degradation, exportability, and provider metadata in generated outputs.

### R-10: Scope expansion prevents MVP completion

Impact: high. Likelihood: high.

Mitigation: preserve CP order, enforce MVP scenario, defer breadth, require acceptance evidence for checkpoint exit, and use ADRs for material additions.

### R-11: Event history and mutable projections diverge

Impact: high. Likelihood: medium.

Mitigation: define canonical ownership in CP1, transactional write rules, rebuild/verification tooling, and invariant/property testing.

### R-12: Prompt injection inside captured Evidence

Impact: high. Likelihood: high.

Mitigation: treat Evidence as untrusted data, separate system instructions from retrieved content, label source boundaries, minimize tool permissions, and test hostile artifacts.

### R-13: Standalone Spaces become accidentally coupled

Impact: high. Likelihood: medium.

Mitigation: contract tests for Research-only and Development-only projects, nullable/optional cross-Space references by design, no synthetic placeholder entities, scope-aware UI and Checkpoints, and checkpoint exit criteria for independent operation.

### R-14: Superficially compatible providers behave differently

Impact: high. Likelihood: high.

Mitigation: capability descriptors, provider-specific profiles, known-deviation records, common conformance fixtures, exact model/adapter provenance, fail-closed unsupported features, and compatibility revalidation before profile upgrades.

### R-15: MCP expands the project attack surface

Impact: critical. Likelihood: medium.

Mitigation: stdio-first deployment, explicit project/capability grants, authorization on every request, opaque resource IDs, read-first tools, proposal-only consequential writes, no raw database/filesystem/shell access, bounded responses, audit/revocation, and adversarial cross-project tests.

## 20. CP1 Decisions and Deferred Choices

All former CP1 open questions have been resolved or explicitly assigned as non-blocking work to their owning later checkpoint. The authoritative record is [CP1 Decision Register](docs/cp1/CP1-DECISION-REGISTER.md).

Decisions required by CP2 are locked: Windows-first Tauri/React/Rust desktop architecture, SQLite current state plus audit/outbox, UUIDv7 identity, content-addressed Artifact Store, modular Spaces, explicit provenance/origin, scope-aware Checkpoints, deterministic canonical authority, local-first privacy, and reference-hardware performance budgets.

Exact provider/model support and routing defaults belong to CP7; capture backend validation to CP9; optional embeddings to CP10; and the final MCP resource/prompt/tool catalog plus remote-transport decision to CP11. These choices are constrained by CP1 invariants and ADR-006 and do not reopen CP2 or CP3.

## 21. Acceptance Criteria

### 21.1 MVP usage-configuration acceptance scenarios

#### Scenario A - Research-only

Given a new project without a repository, a user can create a Research Question, capture/import Evidence, optionally define an Experiment and Result, create a supported Finding or Decision, create a Research Checkpoint, close the project, and later resume with the correct unresolved items and next action. The user can generate a cited Research Report and Context Pack without creating a Requirement, ChangeSet, CodeEntity, Test, or other Development entity.

#### Scenario B - Development-only

Given a new project with a supported existing Git repository and no in-product research history, a user can declare or import an existing Requirement, task, brief, issue, Decision, or direct implementation intent; ingest commits/diffs; create a ChangeSet; associate CodeEntities, Tests, TestRuns, and validation; generate development documentation; create a Development Checkpoint; and later resume from the exact repository/work state. The system records the declared origin and never fabricates Evidence, Findings, or research rationale.

#### Scenario C - Connected R&D

Given a Research-only project with an accepted Finding or Decision, the user can activate Development Space without export/import or duplicate project creation; create or link a Requirement; ingest and link a ChangeSet, CodeEntities, Tests, and TestRuns; inspect the complete chain in both directions; and turn a failed or surprising validation outcome into typed Learning Feedback. The user can create an Integrated R&D Checkpoint and resume with cross-Space next actions.

#### Shared completion conditions

Each scenario must support Checkpoint and budgeted Context Pack creation, close/reopen reconstruction, export/restore verification, and correct provenance. AI is optional for canonical workflow completion; AI-derived content remains identifiable; denied data is not transmitted; and no canonical or artifact integrity error occurs.

### 21.2 CP1 acceptance criteria

- AC-CP1-01: Every canonical entity and relationship has a definition, owner, identity rule, lifecycle, validation rules, and example.
- AC-CP1-02: System context, container/component, core sequence, data-flow/privacy, and deployment diagrams agree with this PRD.
- AC-CP1-03: Contracts cover commands, queries, events, artifacts, jobs, errors, AI requests/outputs, checkpoints, and Context Packs.
- AC-CP1-04: The AI/deterministic responsibility matrix has no ambiguous canonical-write authority.
- AC-CP1-05: Threat model includes all threats named in Section 13.6 with owners and planned controls.
- AC-CP1-06: Benchmark fixtures and measurement scripts/protocol can test Section 14 targets on the reference hardware.
- AC-CP1-07: Every unresolved architectural choice is recorded as an open question, experiment, or ADR; none is hidden in an implementation assumption.
- AC-CP1-08: Requirements are mapped to checkpoint ownership and planned verification level.
- AC-CP1-09: Architecture and contracts demonstrate that Research Space and Development Space can each operate without entities or services owned exclusively by the other.
- AC-CP1-10: The R&D Bridge is optional, typed, auditable, and can be enabled after project creation without migration or identity change.
- AC-CP1-11: Research, Development, and Integrated R&D Checkpoint schemas share a common envelope while permitting scope-specific state.
- AC-CP1-12: No happy-path, schema constraint, navigation rule, or acceptance fixture requires synthetic placeholder entities for an unused Space.

### 21.3 Core data acceptance criteria

- AC-DATA-01: Transaction rollback leaves no visible partial canonical state.
- AC-DATA-02: Artifact hash mismatch or missing payload is detected and reported with recovery guidance.
- AC-DATA-03: Retried idempotent commands do not duplicate entities, relations, events, or artifacts.
- AC-DATA-04: Migration failure restores or preserves the prior usable project.
- AC-DATA-05: Export/import preserves IDs, relationships, hashes, versions, and supported history.

### 21.4 Research acceptance criteria

- AC-RS-01: Result can be recorded without forcing a Finding.
- AC-RS-02: A Finding can show supporting, challenging, and inconclusive sources simultaneously.
- AC-RS-03: Superseding a Decision preserves the earlier rationale and dependent links.
- AC-RS-04: Users can distinguish source Evidence, their annotations, and AI-generated interpretation.
- AC-RS-05: A Research-only project can complete its workflow, checkpoint, report, export, restore, and resume with no repository or Development entity.
- AC-RS-06: A Finding or Decision can remain complete without producing a Requirement.

### 21.5 Development acceptance criteria

- AC-DS-01: Re-ingesting an unchanged repository creates no duplicate canonical changes.
- AC-DS-02: Unsupported code still appears at file/diff level with an explicit analyzer limitation.
- AC-DS-03: Commit amendment/rebase does not silently redirect old provenance to different content.
- AC-DS-04: Requirement implementation and test status are reported independently.
- AC-DS-05: A Development-only project can checkpoint, document, export, restore, and resume with no Research entity.
- AC-DS-06: External/manual/legacy/unknown rationale remains visibly distinguished from in-product evidence-backed rationale.

### 21.6 R&D Bridge acceptance criteria

- AC-BRIDGE-01: Activating the second Space preserves project ID, entity IDs, prior Checkpoints, artifacts, and event history.
- AC-BRIDGE-02: Users can traverse an accepted research outcome through Requirement, ChangeSet, code, and verification when those links exist.
- AC-BRIDGE-03: Validation Learning Feedback can target the appropriate research or development entity without automatically changing its canonical status.
- AC-BRIDGE-04: Removing a Space from active navigation does not delete or orphan its canonical records.

### 21.7 AI acceptance criteria

- AC-AI-01: Malformed or schema-invalid model output produces no canonical write.
- AC-AI-02: Every accepted AI-derived claim retains source IDs and generation metadata.
- AC-AI-03: `secret/never-send` fixtures never appear in outbound requests, logs, caches, or error payloads.
- AC-AI-04: Offline/provider-failure mode preserves all deterministic workflows.
- AC-AI-05: Prompt injection in Evidence cannot change permissions, tool scope, privacy rules, or canonical acceptance state.
- AC-AI-06: Users can inspect and reject a candidate without losing the underlying sources.
- AC-AI-07: The same golden task contract runs against Gemini and at least one non-Gemini path without changing domain commands or canonical schemas.
- AC-AI-08: Unsupported provider capabilities, provider changes, and denied failover produce explicit safe states rather than silent degradation or data transmission.

### 21.8 MCP continuity acceptance criteria

- AC-MCP-01: A project-scoped read grant can retrieve Current Project State, a Checkpoint, and a bounded Context Pack through versioned MCP contracts.
- AC-MCP-02: Cross-project, revoked, expired, capability-denied, and `secret/never-send` requests fail closed and are safely audited.
- AC-MCP-03: A proposal tool creates no canonical mutation until normal validation and explicit human acceptance complete.
- AC-MCP-04: The server exposes no raw SQLite, unrestricted filesystem, shell, credential, or provider-token access.
- AC-MCP-05: Codex, Claude Code, and Gemini CLI client profiles pass the supported read/resume workflow where their negotiated MCP capabilities permit it.
- AC-MCP-06: MCP server failure or disablement leaves desktop and deterministic project workflows operational.

### 21.9 Checkpoint and context acceptance criteria

- AC-CTX-01: Checkpoint identity resolves to an exact project and ledger position.
- AC-CTX-02: Repeated deterministic Context Pack selection over unchanged state is stable.
- AC-CTX-03: Context Pack output stays within its declared budget or returns an explicit budget error/omission summary.
- AC-CTX-04: Every included claim or entity is traceable to a source ID; unavailable artifacts are declared.
- AC-CTX-05: Changed relevant state marks prior checkpoints/generated contexts stale according to policy.
- AC-CTX-06: Privacy exclusions apply transitively to derived summaries and linked payloads.
- AC-CTX-07: Research and Development Checkpoints omit irrelevant mandatory fields rather than filling them with placeholders.
- AC-CTX-08: Checkpoints can be created before completion and never imply that the project or workflow has ended.

### 21.10 Performance acceptance criteria

- AC-PERF-01: Performance is measured on the declared reference hardware with fixture version, cold/warm state, and p50/p95 results recorded.
- AC-PERF-02: Standard interactive targets in Section 14.3 pass or have an approved release exception with mitigation.
- AC-PERF-03: Heavy indexing and capture remain cancellable and do not cause out-of-memory termination under the defined stress procedure.
- AC-PERF-04: Graph views beyond the visible-node budget paginate, cluster, or scope rather than freezing the UI.

### 21.11 Security and recovery acceptance criteria

- AC-SEC-01: Credentials are absent from project exports, logs, and the SQLite database.
- AC-SEC-02: Malformed archives and traversal/symlink fixtures cannot write outside the intended project/import area.
- AC-SEC-03: Capture always presents an active indicator and stops when the user invokes stop.
- AC-SEC-04: Backup/restore and interrupted-write recovery pass automated and manual drills.
- AC-SEC-05: External clients cannot access another project or capability outside their granted scope.

## 22. Verification Strategy

- Unit tests validate entity invariants, lifecycle transitions, policy rules, budget calculations, and schema validators.
- Contract tests validate adapters for Git, analyzers, every enabled AI provider profile, capture backends, renderers, and the MCP-based AI Continuity Interface.
- Integration tests validate SQLite transactions, migrations, Artifact Store consistency, event/projection behavior, and import/export.
- Golden-fixture tests validate parsers, provenance chains, checkpoint summaries, Context Pack selection, and report citations.
- Property/fuzz tests target graph constraints, import formats, paths/archives, schema migrations, and idempotent ingestion.
- Security tests cover prompt injection, secret leakage, path traversal, authorization, untrusted content, and oversized inputs.
- Performance tests use named fixtures and reference hardware, reporting p50/p95 and peak memory.
- Recovery tests simulate process termination during writes, indexing, capture, migration, and export.
- End-to-end tests exercise the complete scenario in Section 21.1 with AI enabled and disabled.

## 23. Release Readiness

A release candidate requires:

- all MVP exit criteria satisfied;
- no unresolved critical/high data-loss or privacy defect;
- migrations tested from every supported project schema version;
- backup and restore verified;
- dependency licenses and notices reviewed;
- privacy behavior and external AI disclosure documented for users;
- performance benchmark report published;
- capture support matrix and known limitations published;
- diagnostic/export behavior verified not to leak credentials;
- user onboarding demonstrates standalone Research, standalone Development, optional R&D connection, Checkpoint resume, and the AI review model.

## 24. Glossary

**ADR (Architecture Decision Record):** A versioned record of an important architectural choice, its context, alternatives, and consequences.

**AI candidate:** Structured model output awaiting validation and, where consequential, user review before canonical acceptance.

**AI Continuity Interface:** Permissioned external interface, implemented as the Continuum MCP Server in CP11, through which compatible AI clients obtain project state or submit bounded proposals.

**AI Provider Gateway:** The sole outbound application boundary that privacy-checks, routes, invokes, normalizes, validates, and audits semantic model requests.

**Artifact:** A stored payload such as a source snapshot, screenshot, recording, report, or derived file, identified independently from domain metadata.

**Artifact Store:** Local subsystem that stores, retrieves, verifies, and manages artifact payloads.

**Canonical:** Authoritative project state governed by deterministic domain rules and human-approved transitions.

**ChangeSet:** A meaningful collection of repository changes linked to intent, Requirements, and affected code.

**Checkpoint:** Immutable reference to project state at a known ledger position, with deterministic state and optional reviewed semantic summary.

**CodeEntity:** Addressable code structure such as repository, file, module, symbol, dependency, configuration item, or test definition.

**Context Pack:** A purpose-bound, size/token-budgeted, privacy-filtered set of current project information with provenance and freshness metadata.

**Continuity:** The ability to preserve, inspect, transfer, and resume the reasoning and state of an R&D project over time.

**Continuity Core:** Shared foundation responsible for project identity, ledger, artifacts, events, relationships, policy, checkpoints, and context contracts.

**Connected R&D:** Usage configuration in which Research Space and Development Space are both active and explicitly linked through the optional R&D Bridge.

**Current Project State:** Scope-aware representation of where work currently stands, including completed work, active items, unresolved questions/blockers, relevant provenance, and next actions.

**Decision:** A selected course of action with rationale, alternatives, constraints, status, and provenance.

**Deterministic process:** Application logic whose output is governed by explicit rules and inputs rather than probabilistic model generation.

**Development Space:** Continuum workspace for repository history, ChangeSets, code structure, configurations, dependencies, tests, and implementation traceability.

**Development-only:** Usage configuration that combines Continuity Core and Development Space without requiring Research Space or reconstructing prior research.

**Evidence:** Sourced material used to support, challenge, or contextualize a Finding; kept separate from interpretation.

**Experiment:** A planned procedure used to investigate a Research Question or hypothesis.

**Finding:** An interpreted conclusion grounded in Evidence and/or Results.

**GeneratedArtifact:** Versioned report, diagram, explanation, summary, or other output produced deterministically or with AI assistance.

**Ledger position:** Stable marker identifying the included boundary of project history for a checkpoint or generated output.

**MCP (Model Context Protocol):** The negotiated protocol used by compatible external AI hosts to consume Continuum resources, prompts, and tools.

**Continuum MCP Server:** The inbound, project-scoped CP11 adapter that exposes bounded continuity reads and proposal operations without raw storage or filesystem access.

**Learning Feedback:** A typed outcome from development or validation that may create new Evidence or Result, or propose revision of a Finding, Decision, Requirement, or unresolved item.

**Project Ledger:** Local canonical persistence and history mechanism for project entities, relationships, and events.

**Provenance:** Traceable origin and derivation of an entity, relationship, claim, decision, change, or generated output.

**Provider adapter/profile:** Replaceable provider-specific transport plus declared capabilities, deviations, policy metadata, and conformance evidence behind the stable AI Provider Gateway contract.

**Research Question:** A defined question, hypothesis, or uncertainty that organizes research activity.

**Research Session:** A bounded interval of research or capture activities.

**Research Space:** Continuum workspace for questions, evidence, experiments, results, findings, decisions, and research timelines.

**Research-only:** Usage configuration that combines Continuity Core and Research Space without requiring a repository or development outcome.

**R&D Bridge:** Optional cross-domain capability that connects research outcomes to Requirements and implementation, and returns validation learning to the appropriate project state.

**Requirement:** A verifiable product or technical obligation derived from research, a decision, or an explicit need.

**Result:** An observed outcome of an Experiment, recorded independently from its interpretation.

**Semantic checkpoint:** A Checkpoint enriched with a source-backed representation of goals, decisions, risks, contradictions, and next actions.

**Source of truth:** The authoritative representation used to resolve conflicts; for product requirements, this Markdown PRD; for runtime facts, the canonical project records defined by system contracts.

**Stale:** A state indicating that relevant source data changed after an output or checkpoint was produced.

**Test:** A verification artifact linked to a Requirement or CodeEntity; distinct from a specific TestRun result.

## Appendix A - Requirement Priority Convention

- **Must:** Required for the checkpoint or MVP exit criteria stated in this PRD.
- **Should:** Important for usability, resilience, or completeness but may receive a documented release exception.
- **Could:** Valuable extension that must not block the agreed architecture or MVP path.
- **Won't now:** Explicitly deferred without rejecting future consideration.

Unless otherwise stated, all numbered `FR`, `NFR`, `SEC`, `PERF`, `PRV`, and `AC` items in this document are **Must** for the checkpoint that owns them or for CP12 release verification. MVP deferrals in Section 16.3 override this default only where explicitly applicable.

## Appendix B - Change-Control Checklist

A proposed change is material if it alters any of the following:

- the 12-checkpoint sequence or ownership;
- Research Space, Development Space, or Continuity Core boundaries;
- the conditional provenance chains or optional R&D Bridge;
- local-first canonical ownership;
- the AI/deterministic boundary;
- privacy classification or external transmission policy;
- reference hardware or release performance targets;
- MVP end-to-end acceptance scenario.

Material changes require an ADR, affected requirement IDs, migration/compatibility impact, privacy and performance impact, acceptance updates, and explicit approval before this PRD is revised.
