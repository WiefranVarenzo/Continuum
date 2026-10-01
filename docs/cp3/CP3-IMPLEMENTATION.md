# CP3 — Research Core Implementation

> **Status:** PASS
> **Completed:** 2026-09-02
> **Build:** `continuum-core 0.3.0` / schema v3

## Outcome

CP3 implements a deterministic, local-first Research Space on the validated CP2.1 foundation. Research can be completed, paused, resumed, searched, reported, exported, restored, and audited with Development disabled and without any repository or AI service.

## Implemented Surface

- typed create APIs for ResearchSession, ResearchQuestion, Evidence, Experiment, Result, Finding, Decision, and optional Requirement;
- optimistic ResearchQuestion editing and Evidence annotation updates;
- explicit lifecycle transitions for every Research aggregate;
- active-session membership at creation or later association;
- original Evidence versus annotation/summary separation;
- Artifact-backed Evidence and Result validation;
- typed Finding source assessments;
- Decision supersession with expected-version protection;
- exact Research relationship policy registered at the Core boundary;
- deterministic bounded search and append-only timeline;
- immutable Research Checkpoint and live resume projection;
- deterministic cited Markdown Research Report retained as the ADR-007 compatibility projection and future Human Document content baseline;
- Research-aware integrity diagnostics;
- schema v3 migration with v1/v2 backup and preservation tests.

## Code Map

```text
crates/continuum-core/
├── migrations/0003_research_core.sql
├── src/research.rs
├── src/store.rs
├── examples/cp3_benchmark.rs
└── tests/cp3_research_acceptance.rs
```

## Preserved CP2 Guarantees

- one project identity and optional Space capabilities;
- short immediate write transactions;
- idempotent command receipts;
- actor/correlation/causation audit envelopes;
- monotonic ledger sequence;
- transactional outbox;
- project-scoped foreign keys and relationships;
- content-addressed Artifact storage;
- forward migrations with verified backup;
- export/restore identity and hash preservation;
- no canonical AI write authority.

## Explicit Non-Claims

CP3 does not claim provider-neutral AI integration, semantic search, repository ingestion, ChangeSets, code analysis, capture acquisition, HTML-first Human Documentation, full Context Pack selection, MCP, Windows certification, or release hardening. Those remain assigned to CP4–CP12 under the ADR-006 and ADR-007 amendments. The existing Markdown report remains supported and requires no CP3 data migration.
