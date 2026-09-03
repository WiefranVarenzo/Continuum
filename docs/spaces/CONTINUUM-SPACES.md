# Continuum Spaces and R&D Bridge

> **Status:** Approved baseline; Research Space implemented through CP3
> **Governing decision:** [ADR-001](../adr/ADR-001-MODULAR-SPACES-AND-OPTIONAL-RD-BRIDGE.md)

## 1. Capability Model

A project has one identity and one Continuity Core. `research_enabled` and `development_enabled` are capability state, not project types. At least one Space is normally enabled for user value, but Core can exist briefly during creation, import, repair, or configuration.

Transitions are idempotent:

```text
Core → Research-only
Core → Development-only
Research-only ⇄ Connected R&D
Development-only ⇄ Connected R&D
```

Disabling a Space hides active UI/routes and stops new Space-specific jobs; it does not delete data. Destructive purge is a separate explicit privacy operation.

## 2. Research Space Contract

Inputs include questions, notes, captured/imported sources, research plans, observations, and user/AI proposals. Outputs include Evidence, Results, Findings, Decisions, research reports, Research Checkpoints, and Research Context Packs.

Valid outcomes do not require development. A Finding may remain unresolved; a Decision may produce no Requirement; a Research Checkpoint may be created mid-investigation.

Research Space depends only on Continuity Core ports for identity, persistence, artifacts, provenance, policy, jobs, checkpoints, and context.

## 3. Development Space Contract

Inputs include repository state and manual/imported/external/legacy intent such as a Requirement, issue, brief, task, or Decision. Outputs include ChangeSets, CodeEntities, dependency/configuration observations, Tests, TestRuns, validation, development documentation, Development Checkpoints, and Development Context Packs.

Development Space never claims evidence-backed rationale unless linked sources exist. Unknown rationale remains explicit. Repository observation is read-only by default; Continuum does not rewrite Git history.

## 4. R&D Bridge Contract

The bridge validates these relationship families:

- Finding `informs` Decision.
- Decision `creates|modifies|retires` Requirement.
- Requirement `implemented_by|partially_implemented_by|reverted_by` ChangeSet.
- ChangeSet `modifies` CodeEntity.
- Test `verifies` Requirement or CodeEntity.
- TestRun `observes` Test at repository state.
- validation `produces` LearningFeedback.
- LearningFeedback `supports|challenges|requests_revision_of` Evidence, Finding, Decision, or Requirement.

All relationship creation records actor, origin, time, confidence/review state, and source IDs. AI can propose links; deterministic exact links and accepted user links become canonical.

## 5. Space Activation Sequence

1. User requests activation.
2. Core validates project health and permissions.
3. Core commits capability state and `SpaceCapabilityChanged` audit event.
4. Space initializes empty projections/settings without placeholder domain entities.
5. UI exposes relevant navigation.
6. Optional discovery jobs require explicit user action or declared policy.

Activation failure rolls back capability state. Repeated activation returns success without duplicate state.

## 6. Cross-Space Learning

Validation failure is not automatically Evidence and success is not an end state. A validation observation may be promoted to LearningFeedback. The user selects or approves its target and meaning. Consequential revisions remain pending until accepted. Checkpoints can be created before or after feedback.

## 7. UX Rules

- onboarding offers Research, Development, or Connected R&D entry paths;
- the selection is described as “start with,” not permanent project type;
- absent Space navigation stays quiet rather than showing broken empty states;
- provenance badges distinguish in-product, external, manual, legacy, unknown, deterministic, and AI-proposed origins;
- enabling a second Space explains what becomes available and confirms no migration is required;
- hiding and deleting are never presented as the same action.

## 8. Acceptance Rules

- Research fixture passes with zero Development records.
- Development fixture passes with zero Research records.
- activation preserves project ID, records, artifacts, ledger sequence, and prior Checkpoints.
- bridge queries tolerate missing optional links.
- no AI or import path creates synthetic provenance to fill a chain.
