# CP1 — Architecture, Domain Model & System Contracts

> **Status:** PASS — Ready for CP2; provider/MCP amendment accepted 2026-09-03
> **PRD:** [Continuum PRD](../../CONTINUUM_PRD.md)  
> **Current architecture decisions:** [ADR-001 — Modular Spaces and Optional R&D Bridge](../adr/ADR-001-MODULAR-SPACES-AND-OPTIONAL-RD-BRIDGE.md); [ADR-006 — Provider-Neutral AI and MCP Boundaries](../adr/ADR-006-PROVIDER-NEUTRAL-AI-AND-MCP-BOUNDARIES.md)
> **Purpose:** Produce an implementation-ready architectural blueprint for CP2 without over-specifying later checkpoints.

## 1. CP1 Outcome

CP1 is complete when Continuum's product requirements have been translated into a coherent, reviewable, and testable architecture. The resulting documentation must let CP2 implement Continuity Core without guessing product boundaries, canonical ownership, identity, persistence responsibilities, privacy constraints, performance budgets, or how future Spaces connect.

CP1 designs the system. It does not implement the production database, desktop runtime, capture pipeline, model integration, or code analyzers.

## 2. Locked Product Direction

- Continuum is local-first.
- Continuity Core is always active.
- Research Space is independently useful.
- Development Space is independently useful.
- Connected R&D uses an optional, explicit R&D Bridge.
- Initial usage configuration is not an immutable project type.
- A Space can be enabled later without project migration or loss of history.
- Checkpoint is a cross-cutting smart bookmark, not a terminal workflow step.
- Research, Development, and Integrated R&D Checkpoints are supported.
- Context Packs are purpose-bound, budgeted, provenance-bearing, privacy-filtered, and progressively retrieved.
- Canonical truth and safety controls are deterministic.
- Replaceable AI providers provide bounded semantic assistance through reviewable structured outputs; Gemini remains first-class but is not a domain dependency.
- The outbound AI Provider Gateway and inbound Continuum MCP Server are separate, policy-enforced application adapters.
- The complete research-to-code chain is optional and composable, never fabricated.
- The product must remain practical on Ryzen 5 5600H / 16 GB RAM.
- Delivery remains organized as CP1 through CP12 in the approved order.

## 3. Lean CP1 Documentation Pack

### Existing

- [Continuum PRD](../../CONTINUUM_PRD.md) — product WHAT, WHY, scope, requirements, roadmap, and acceptance criteria.
- [ADR-001](../adr/ADR-001-MODULAR-SPACES-AND-OPTIONAL-RD-BRIDGE.md) — modular Spaces and optional R&D Bridge.
- `CP1-README.md` — CP1 scope, artifact index, dependency order, and status.

### Required before CP1 closure

- `docs/architecture/CP1-ARCHITECTURE.md` — system context, components, boundaries, runtime view, deployment, failure domains, and architectural contracts.
- `docs/domain/DOMAIN-MODEL.md` — ontology, entity ownership, relationships, lifecycle, provenance, and authorship.
- `docs/spaces/CONTINUUM-SPACES.md` — Continuity Core, Research Space, Development Space, R&D Bridge, and capability activation.
- `docs/data/DATA-ARCHITECTURE.md` — conceptual SQLite model, Artifact Store, identity/versioning, event persistence, transaction boundaries, migration, and recovery posture.
- `docs/ai/AI-ARCHITECTURE.md` — provider-neutral AI boundary, capability/routing policy, task catalog, structured outputs, privacy integration, caching, and provenance.
- `docs/ai/MCP-CONTINUITY-INTERFACE.md` — CP11 MCP resources, prompts, tools, authorization, proposal flow, transport, and client-compatibility contract.
- `docs/ai/PROVIDER-AND-MCP-DELIVERY-PLAN.md` — impact and delivery gates across CP1–CP12.
- `docs/checkpoint/CHECKPOINT-AND-CONTEXT.md` — Current Project State, Checkpoint variants, Context Pack, staleness, and progressive retrieval.
- `docs/security/PRIVACY-AND-SECURITY.md` — data classification, privacy gateway, trust boundaries, threats, controls, and capture consent.
- `docs/performance/PERFORMANCE-BUDGET.md` — reference hardware, workloads, budgets, degradation, and benchmark protocol.
- `docs/testing/CP1-VALIDATION.md` — architecture conformance, acceptance scenarios, fixtures, traceability, and final audit result.
- `docs/cp1/CP1-DECISION-REGISTER.md` — locked, deferred, and open decisions with ADR links.
- `docs/cp1/CP1-TRACEABILITY.md` — PRD requirement groups mapped to owning architecture sections and validation evidence.
- `docs/cp1/CP1-ACCEPTANCE-CHECKLIST.md` — executable CP1 exit checklist and PASS/BLOCKED status.
- `docs/diagrams/` — source-controlled core architecture diagrams.

### ADRs

ADRs are created only for consequential decisions. Accepted ADR-006 generalizes the original Gemini binding into a provider-neutral gateway and separates it from the MCP continuity boundary. Other candidate subjects include code-intelligence adapters, diagram layout, capture backend, remote MCP deployment, and optional embedding strategy. A candidate becomes an ADR only when the decision is evaluated and locked.

## 4. Core Diagram Set

The minimum diagram set is:

1. Continuum big picture.
2. System context.
3. Research-only flow.
4. Development-only flow.
5. Connected R&D continuity loop.
6. Research-to-code traceability graph.
7. component and module architecture.
8. deterministic and semantic processing boundaries.
9. AI privacy-gateway data flow.
10. outbound AI Provider Gateway versus inbound Continuum MCP Server boundary.
11. Checkpoint creation, resume, and Context Pack flow.

Diagram sources must be version-controlled and reviewable as text where practical. Mermaid is the baseline source format for CP1. A `.drawio` representation is added only when manual diagram editing materially improves communication; it does not replace the textual source or architectural explanation.

## 5. Work Order and Dependencies

The documentation is produced in this order:

1. **Architecture Master and Spaces** establish system boundaries and ownership.
2. **Domain Model** defines the language and invariants used by every subsequent document.
3. **Core Diagrams** verify that the architecture and user modes are visually coherent.
4. **Data Architecture** maps canonical domain responsibilities to persistence boundaries.
5. **AI Architecture** maps semantic tasks onto deterministic source selection, privacy controls, review, and provenance.
6. **Checkpoint and Context Architecture** defines resumable state and retrieval contracts across all usage configurations.
7. **Privacy/Security and Performance** constrain all components and deployment choices.
8. **Decision Register and Traceability** prove that product requirements have owners and unresolved choices are visible.
9. **CP1 Validation** evaluates the complete package and records PASS or blockers.

Later documents may reveal contradictions in earlier ones. Corrections must update all affected documents and traceability references rather than being documented as isolated exceptions.

## 6. Scope Boundary

### CP1 must decide

- domain and module ownership;
- stable conceptual entities, relationships, and invariants;
- canonical versus derived state;
- optionality rules for standalone Spaces and the bridge;
- conceptual persistence and Artifact Store boundaries;
- identity, versioning, origin, and provenance model;
- checkpoint and context envelopes;
- AI/deterministic responsibility boundary;
- privacy trust boundaries and mandatory controls;
- performance budgets and benchmark fixtures;
- public architectural contracts between checkpoints;
- major technology choices required before CP2.

### CP1 does not need to finalize

- every UI screen or interaction detail;
- every SQL statement or production index;
- exhaustive command/query definitions for features implemented in later checkpoints;
- production migration scripts;
- every parser, capture, and operating-system adapter;
- final prompt wording;
- final visual styling;
- operational runbooks for components that do not yet exist.

Those details are designed and verified in their owning implementation checkpoint while conforming to CP1 contracts.

## 7. Validation Fixtures

CP1 validation must include at least these conceptual fixtures:

### Fixture A — Research-only

A project with Research Questions, Evidence, an Experiment/Result, Findings, a Decision, Research Checkpoints, and a Research Context Pack. It has no repository or Development entities.

### Fixture B — Development-only

A project with external/manual intent, a repository baseline, ChangeSets, CodeEntities, Tests/TestRuns, validation, Development Checkpoints, and development documentation. It has no Research entities and no fabricated rationale.

### Fixture C — Connected R&D

A project that begins as Research-only, activates Development Space without migration, links a Decision to a Requirement and ChangeSet, records validation Learning Feedback, and produces an Integrated R&D Checkpoint and Context Pack.

### Fixture D — Interruption and resumption

Each usage configuration is stopped during incomplete work, reopened, checked against post-checkpoint events, and resumed with correct active work, unresolved items, provenance, and next action.

### Fixture E — Privacy and constrained resources

A project containing secret/never-send material, hostile prompt-like Evidence, large artifacts, and bounded hardware resources demonstrates redaction, permission enforcement, graceful degradation, cancellation, and no canonical corruption.

## 8. CP1 Exit Gate

CP1 receives **PASS** only when:

- all required documents exist and agree with the PRD;
- the three usage configurations are independently coherent;
- all canonical entities and cross-Space relationships have defined owners and invariants;
- Checkpoint and Context Pack contracts cover interruption and resumption;
- AI cannot bypass privacy or canonical approval;
- data loss, secret exfiltration, prompt injection, and resource exhaustion have planned controls;
- performance targets have reproducible measurement definitions;
- core diagrams agree with the written architecture;
- critical implementation-blocking choices are locked through the decision register and ADRs;
- PRD requirement groups map to architecture and planned validation evidence;
- every CP1 acceptance criterion in the PRD is PASS or has an explicit blocking record.

If any critical condition remains unresolved, the result is **BLOCKED**, not a provisional PASS.

## 9. Status

- [x] PRD baseline created.
- [x] Modular usage strategy incorporated into PRD.
- [x] ADR-001 accepted.
- [x] Lean CP1 documentation pack and exit gate defined.
- [x] Architecture Master completed.
- [x] Domain Model completed.
- [x] Continuum Spaces specification completed.
- [x] Core diagrams completed and reconciled.
- [x] Data Architecture completed.
- [x] AI Architecture completed.
- [x] Checkpoint and Context Architecture completed.
- [x] Privacy and Security completed.
- [x] Performance Budget completed.
- [x] Decision Register completed.
- [x] Traceability completed.
- [x] CP1 Validation completed.
- [x] CP1 acceptance audit passed.

**Current result: CP1 PASS — Ready for CP2 Continuity Core.**

The 2026-09-03 provider/MCP amendment does not reopen CP2 or CP3. It preserves every CP1 invariant and assigns new provider records to CP7 and external-client records to CP11 through forward-only extensions.
