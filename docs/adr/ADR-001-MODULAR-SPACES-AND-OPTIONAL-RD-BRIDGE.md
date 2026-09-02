# ADR-001 — Modular Spaces and Optional R&D Bridge

> **Status:** Accepted  
> **Date:** 2026-08-31  
> **Checkpoint:** CP1 — Architecture, Domain Model & System Contracts  
> **Decision owners:** Continuum product and architecture  

## Context

Continuum serves users whose work does not always follow a complete Research-to-Development pipeline.

Some users need only to investigate questions, collect Evidence, run Experiments, produce Findings, document Decisions, and resume the research later. They may never implement software.

Other users already have prior research, an external brief, an existing Requirement, a task, or a repository. They need to continue development, preserve implementation rationale where known, document ChangeSets and validation, and resume later. Requiring them to recreate Research Questions or Evidence would add friction and create misleading provenance.

A third group uses Continuum across the complete R&D lifecycle. For them, accepted research outcomes should connect seamlessly to Requirements and implementation, while validation should be able to produce new learning and feed it back into research or requirement revision.

The architecture must support all three cases without separate products, incompatible project formats, fabricated history, or migration when usage expands.

## Decision

Continuum adopts the principle:

> **Research Space and Development Space are independently useful and seamlessly connected.**

The system uses the following structure:

```text
                 Continuity Core
        ┌──────────────┴──────────────┐
        │                             │
 Research Space              Development Space
    optional                      optional
        │                             │
        └──── optional R&D Bridge ────┘
```

### Continuity Core

Continuity Core is always active. It owns project identity, the Project Ledger, Artifact Store, events, provenance primitives, privacy policy, Checkpoints, Current Project State, and Context Packs.

### Research Space

Research Space is independently usable. It owns Research Questions, Research Sessions, Evidence, Experiments, Results, Findings, Decisions, research timelines, and research reporting. It does not require a repository, Requirement, ChangeSet, CodeEntity, Test, or development outcome.

### Development Space

Development Space is independently usable. It owns repository baselines, commits, diffs, ChangeSets, CodeEntities, dependency/configuration intelligence, Tests, TestRuns, validation, and development documentation. Work may begin from an existing Requirement, task, brief, issue, Decision, repository state, or direct implementation intent. It does not require a Research Question, Evidence, Experiment, Result, or Finding.

### R&D Bridge

The R&D Bridge is optional. It owns no duplicate canonical entities. It provides typed relationships and commands that:

- connect a Finding or Decision to a Requirement;
- connect a Requirement to its ChangeSets, CodeEntities, Tests, and TestRuns;
- turn validation outcomes into Learning Feedback;
- route Learning Feedback to new Evidence/Result or a proposed revision of a Finding, Decision, or Requirement.

The bridge never automatically accepts a semantic revision or fabricates missing upstream provenance.

### Usage configurations

Continuum supports three first-class usage configurations:

1. **Research-only:** Continuity Core plus Research Space.
2. **Development-only:** Continuity Core plus Development Space.
3. **Connected R&D:** Continuity Core plus both Spaces and the optional bridge.

These are UI and capability configurations, not immutable project types. A user may enable the other Space at any time without changing the project ID, exporting/importing, duplicating data, or losing history. Hiding or disabling a Space in the active UI does not delete its canonical data.

### Provenance policy

The full chain is valid when its records genuinely exist:

```text
Evidence → Finding → Decision → Requirement → ChangeSet → CodeEntity → Test → TestRun
```

It is not mandatory. Standalone partial chains are valid. When earlier work occurred outside Continuum, the system records its origin as manual, imported, external, legacy, or unknown. AI must not generate placeholder Evidence or rationale to make a chain look complete.

### Checkpoint policy

Checkpoint is a cross-cutting smart bookmark, not a terminal workflow step. It can be created during incomplete or completed work and has three scope variants:

- **Research Checkpoint:** active questions, Evidence/Experiment state, Findings, uncertainties, and next research actions.
- **Development Checkpoint:** repository state, active intent/Requirement, ChangeSet, verification state, blockers, and next development actions.
- **Integrated R&D Checkpoint:** relevant state from both Spaces, bridge links, validation learning, and cross-Space next actions.

All variants share project identity, ledger position, creation metadata, provenance, freshness, and optional reviewed semantic summary.

## Consequences

### Positive

- Continuum is valuable to researchers who never develop software.
- Continuum is valuable to developers who do not need to repeat or reconstruct research.
- Users can expand from one Space to Connected R&D without migration.
- Provenance reflects reality instead of rewarding artificial completeness.
- Checkpoints support interruption and resumption throughout the lifecycle.

### Costs

- UI navigation and onboarding must adapt to enabled capabilities.
- Schemas and contracts must tolerate intentionally partial provenance.
- Tests and release fixtures must cover three configurations rather than one linear happy path.
- Reports and Context Packs require scope-aware composition.
- Authorization and privacy rules must work consistently across late Space activation.

### Risks

- Optional relationships may complicate queries and UI empty states.
- A poorly implemented bridge could create duplicate entities or ambiguous ownership.
- Development-only documentation may be mistaken for evidence-backed rationale unless origin is prominent.
- Hiding a Space could be confused with deleting its data.

These risks are controlled through explicit origin metadata, typed optional relationships, common Continuity Core identity, scope-aware schemas, and independent-mode contract tests.

## Rejected Alternatives

### Mandatory Research → Development pipeline

Rejected because it blocks valid Research-only and Development-only workflows and encourages fabricated placeholder records.

### Separate Research and Development products or databases

Rejected because it breaks seamless activation, complicates provenance, duplicates infrastructure, and introduces migration/synchronization problems.

### Permanent project type selected at creation

Rejected because users cannot always predict whether research will lead to development or whether development will later require investigation.

### Automatic AI backfilling of missing rationale

Rejected because inferred history is not Evidence and would undermine Continuum's provenance promise.

## Contract Implications for CP1

- Project capability state must be explicit and changeable without migration.
- Cross-Space foreign references must be optional unless required by a bridge command.
- Space activation/deactivation must be evented and idempotent.
- Checkpoint envelopes must support scope-specific payloads.
- Context Pack retrieval must accept Research, Development, or Integrated scope.
- Report generators must declare their scope and missing/unavailable provenance.
- Graph validation must accept valid partial chains and reject fabricated or structurally invalid links.
- Import/export must preserve enabled capabilities and dormant Space data.
- Permission and privacy policy must remain project-scoped across all configurations.

## Acceptance Evidence Required

CP1 is conformant with this decision when architecture diagrams, schemas, contracts, and fixtures demonstrate:

1. a Research-only project with no Development entities;
2. a Development-only project with no Research entities;
3. activation of the second Space without project migration;
4. creation and traversal of an optional Connected R&D chain;
5. validation Learning Feedback without automatic canonical revision;
6. Research, Development, and Integrated R&D Checkpoints;
7. export/restore with dormant Space data preserved;
8. no synthetic placeholder records in any configuration.
