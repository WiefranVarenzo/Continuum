# CP1 Architecture Master Document

> **Status:** Approved baseline; provider-neutral amendment accepted 2026-09-03; implemented through CP3
> **Authority:** [Continuum PRD](../../CONTINUUM_PRD.md)  
> **Decision baseline:** [CP1 Decision Register](../cp1/CP1-DECISION-REGISTER.md)

## 1. Architectural Objective

Continuum is a local-first desktop system that preserves resumable research and development state. It supports Research-only, Development-only, and Connected R&D without separate project formats. The architecture prioritizes durable canonical state, explicit provenance, bounded resource use, offline core operation, and human authority over semantic output.

## 2. Technology Baseline

- Desktop shell: Tauri v2.
- UI: React + TypeScript.
- native/application core: Rust.
- canonical store: SQLite in WAL mode with foreign keys enabled.
- artifact payloads: project-local content-addressed filesystem store using SHA-256.
- repository integration: Git CLI/libgit adapter behind a stable Rust port.
- code analysis: Tree-sitter, ast-grep, and format-specific adapters.
- semantic providers: replaceable adapters behind an application-owned AI Provider Gateway; Gemini remains first-class, with OpenAI, Anthropic, and selected OpenAI-compatible services supported by capability-tested profiles.
- graph UI: React Flow with ELK layout.
- local external-AI protocol in CP11: Continuum MCP Server over stdio, read-first with project-scoped proposal-based writes; Streamable HTTP is separately gated.
- first release target: Windows 11; Linux follows through platform adapters. Core architecture remains cross-platform.

The desktop stack is selected because it combines a web UI ecosystem with a resource-bounded native core and OS capability adapters. Provider- and platform-specific behavior may not leak into domain contracts.

## 3. Architectural Style

Continuum uses a modular monolith with ports and adapters. One desktop application and one project-local canonical database are preferred over distributed services for the MVP. Modules communicate through typed application commands, queries, domain events, and read models—not direct cross-module table mutation.

```text
React UI
  ↓ typed IPC facade
Application Services / Command Bus / Query Bus
  ↓
Domain Modules
  ↓ ports
SQLite • Artifact Store • Git • Analyzers • AI Provider Gateway • Capture • Export • MCP Server
```

This is not full event sourcing. Current canonical state is stored directly, while an append-only audit event stream records material transitions. Derived projections can be rebuilt from canonical state plus source observations where declared.

## 4. System Boundaries

### Continuity Core — always active

Owns Project, capability state, identity, origin/authorship, Project Ledger, relationships, audit events, Artifact metadata, background jobs, Checkpoints, Context Packs, privacy policy, import/export, diagnostics, and schema version.

### Research Space — optional capability

Owns ResearchSession, ResearchQuestion, Evidence, Experiment, Result, Finding, Decision, research timeline, and research reports.

### Development Space — optional capability

Owns Repository, RepositoryBaseline, Commit observation, draft/accepted ChangeSet, CodeEntity, Test, TestRun, validation state, dependency/configuration observations, and development documentation.

### R&D Bridge — optional relationship/application capability

Owns no duplicate business entities. It validates typed cross-Space links, supports promotion from Decision to Requirement, relates Requirements to ChangeSets and validation, and creates reviewable LearningFeedback.

### Cross-cutting adapters

AI providers, capture, graph/reporting, Git, parser/analyzer, import/export, and MCP adapters depend on application ports. They cannot bypass domain validation, privacy policy, authorization, provenance, review, or transaction boundaries. Outbound model invocation and inbound MCP access are separate ports.

## 5. Component Responsibilities

### Desktop UI

Renders scope-aware navigation, forms, timelines, graphs, job progress, provenance, checkpoint resume, privacy previews, and AI review. It holds no canonical authority.

### Application layer

Authenticates local intent, validates command envelopes, starts transactions, calls domain policies, appends audit events, schedules post-commit jobs, and returns structured errors. Queries use read repositories and never mutate state.

### Domain layer

Defines entities, value objects, lifecycle transitions, origin, provenance, relationship rules, Space optionality, and invariants. It is independent of UI, model provider, MCP client, Git, and OS APIs.

### Persistence adapter

Implements SQLite repositories, unit-of-work transactions, migrations, indexes, outbox/job claims, integrity checks, backup, and restore.

### Artifact adapter

Stages streams, hashes content, atomically finalizes payloads, stores metadata transactionally, verifies availability, and quarantines recoverable orphans.

### Git and code-intelligence adapters

Observe repository state without modifying Git history. They emit versioned deterministic observations tied to repository identity and exact content/commit coordinates.

### AI Provider Gateway

Receives a typed semantic task, builds a deterministic candidate source set, applies privacy policy and redaction, selects an eligible provider/model through explicit deterministic routing, invokes a replaceable provider adapter, normalizes and validates structured output, records attempt metadata, and returns a pending-review candidate. Provider capabilities and deviations are declared and tested; API-format compatibility alone never grants support status.

### Continuum MCP Server

Acts as an inbound CP11 adapter for compatible AI clients. It exposes versioned, bounded, project-scoped resources, prompts, and read/proposal tools through application services. It never exposes raw SQLite, arbitrary files or shell execution, provider credentials, or direct canonical mutation. Local stdio is the MVP transport; remote Streamable HTTP requires its own authentication, binding, origin, revocation, rate-limit, and threat-model gate.

### Checkpoint and Context Engine

Creates immutable scope-aware bookmarks at a ledger position, computes Current Project State, detects staleness, selects Context Pack sources progressively, and enforces size/privacy budgets.

### Background job runner

Runs bounded, cancellable ingestion, analysis, capture finalization, AI, report, verification, and maintenance work. A durable job record supports leases, retry policy, progress, cancellation, and crash recovery.

## 6. Command and Query Contract

Every state command uses this conceptual envelope:

```text
command_id       UUIDv7
project_id       UUIDv7
command_type     versioned name
actor            user | system | import | ai_proposal
expected_version optional optimistic concurrency value
idempotency_key  required for retryable external actions
payload          schema-versioned body
issued_at        UTC timestamp
```

Successful commands commit canonical changes and audit/outbox events in one SQLite transaction. File payloads use a staged-artifact protocol. Failed validation produces no canonical mutation.

Queries declare project, scope, filters, pagination, consistency/freshness, and privacy audience. Query results return stable IDs, versions, origin, and availability/staleness where relevant.

Representative CP2 commands are `CreateProject`, `SetSpaceCapability`, `CreateEntity`, `UpdateEntity`, `CreateRelationship`, `StageArtifact`, `FinalizeArtifact`, `CreateCheckpoint`, `BuildContextPack`, `ArchiveEntity`, `ExportProject`, and `VerifyProjectIntegrity`.

## 7. Domain Event Contract

Audit events contain event ID, project ID, aggregate/entity ID, event type/version, actor/origin, occurred time, causation/correlation IDs, sanitized payload, and ledger sequence. Events are append-only. Sensitive payload content remains in protected canonical/artifact storage and is referenced rather than copied into logs.

Events are used for audit, staleness, background-job triggers, and timeline projections. They are not the sole reconstruction source for all canonical entities.

## 8. Runtime and Concurrency

- UI interaction executes through asynchronous typed IPC.
- SQLite writes are short and serialized through a unit-of-work boundary.
- reads may use pooled read connections with WAL semantics.
- background workers use bounded concurrency and durable job leases.
- artifact and capture streams never accumulate without bounds in memory.
- external processes receive explicit paths/arguments and timeouts.
- cancellation is cooperative; unsafe transaction interruption rolls back.
- interactive work has higher scheduling priority than indexing and AI preparation.

## 9. Deployment View

The MVP deploys as one signed desktop application plus project directories and OS-secured credentials. Each project directory contains a manifest, SQLite database, Artifact Store, recovery/quarantine area, and optional export cache. Provider credentials are stored in the OS credential facility, never inside a project.

Windows 11 is the first certified platform. Git, media/capture, credential, and filesystem differences are isolated by platform ports. No discrete GPU or local model is required.

## 10. Failure Domains and Recovery

- SQLite failure: roll back transaction; preserve last known database; create recovery diagnostic.
- artifact finalization failure: keep staged/quarantined payload with no canonical live reference.
- background crash: lease expires and retry resumes idempotently.
- Git rewrite: preserve old observation and reconcile through explicit supersession.
- analyzer failure: retain file/diff-level observation and expose limitation.
- provider/network/capability failure: isolate the affected profile, preserve deterministic workflow, and return a normalized retryable or non-retryable state; sensitive data never silently changes destination.
- MCP client/server failure: terminate or revoke the scoped session without affecting local workflows or outbound provider operation.
- capture interruption: finalize recoverable segment where format permits.
- projection corruption: rebuild derived projection without rewriting canonical data.
- migration failure: restore pre-migration backup and keep old project usable.

## 11. Versioning and Compatibility

Database schema, domain contracts, event types, analyzer outputs, prompt templates, AI task/output schemas, provider capability profiles, Context Pack schemas, project exports, MCP resources/prompts/tools, and client grants are independently versioned. Readers must reject unsupported major versions and tolerate documented additive minor fields. Provider and MCP protocol versions are negotiated outside domain identity. Migrations are forward-only with automatic pre-migration backup.

## 12. Architectural Invariants

1. A project always has Continuity Core and may have either or both Spaces enabled.
2. Enabling a Space never changes project identity or rewrites history.
3. No unused Space requires placeholder records.
4. Canonical writes pass deterministic validation and transaction boundaries.
5. AI output is never canonical merely because it is schema-valid.
6. Every derived claim exposes direct source IDs and origin.
7. External transmission passes the privacy gateway.
8. Checkpoints identify an exact ledger position and never imply completion.
9. Context Packs declare budget, scope, omission, provenance, and freshness.
10. Artifact references never silently resolve outside the project policy boundary.
11. Git and analyzers observe repositories; they do not become source control.
12. Optional adapter failure cannot corrupt or disable unrelated core workflows.
13. No provider or MCP client is a canonical dependency or gains direct canonical-write authority.
14. Provider routing is explicit and policy-bound; external-client access is project- and capability-scoped.

## 13. CP2 Handoff Contract

CP2 implements only Continuity Core foundations needed by later checkpoints:

- project manifest and lifecycle;
- SQLite migration framework and core schema;
- UUIDv7 identity and version/origin metadata;
- generic typed relationships and audit events;
- Artifact Store staging/finalization/integrity;
- bounded durable job state;
- capability state for optional Spaces;
- backup/export skeleton and integrity diagnostics;
- application command/query transaction boundary.

CP2 must not prematurely implement full Research, Git intelligence, AI provider, capture, or MCP behavior. It provides stable ports and test doubles for those later checkpoints. ADR-006 introduces no CP2/CP3 migration because provider profiles, AI attempts/candidates, and external-client grants are forward additions owned by CP7 and CP11.
