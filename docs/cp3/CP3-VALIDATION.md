# CP3 — Validation and Exit Decision

> **Decision:** PASS
> **Validated:** 2026-09-02
> **Reference machine:** AMD Ryzen 5 5600H, approximately 16 GB RAM, Linux x86_64

## Scope Reviewed

CP3 was checked against the Continuum PRD Research-only journey, FR-RS-001 through FR-RS-014, AC-RS-01 through AC-RS-06, CP1 Architecture, Domain Model, Data Architecture, Spaces contract, ADR-001/003/004/005, and every CP2.1 invariant.

## Automated Evidence

- `cargo test --workspace --locked`: PASS, 39 tests, 0 failures.
- Unit and migration tests: 7 PASS.
- CP2 regression suite: 17 PASS.
- CP3 integration/acceptance suite: 15 PASS.
- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- SQLite integrity and foreign-key checks: PASS on valid fixtures.
- Intentional normalized-row corruption: detected with recovery guidance.
- v1→v3 and v2→v3 automatic pre-migration backup: PASS.
- Research export/import restoration: PASS.

## Failure and Abuse Cases Covered

- disabled Research capability;
- archived/closed session membership;
- missing Evidence source;
- missing/unavailable Artifact;
- generic CRUD and generic relationship policy bypass;
- wrong endpoint type and unsupported relationship;
- duplicate direct source;
- self/cyclic supersession;
- invalid lifecycle jump;
- stale optimistic version;
- unsupported Finding acceptance;
- Decision supersession with stale expected version;
- AI actor/origin canonical write attempt;
- duplicate retry with a new command ID;
- incomplete normalized state corruption;
- unbounded pages, source sets, text, JSON, checkpoint, and report inputs.

## Reference Benchmark

Fixture `standard-research-v1` contains one session, 999 active Questions, 999 Evidence records, 999 typed Question/Evidence links, and 1,999 Checkpoint sources. Five warmups and thirty samples were measured without network activity.

- deterministic metadata/text search p95: 2.040 ms, target 500 ms;
- timeline first page p95: 0.891 ms, informational;
- Research resume state p95: 6.156 ms, CP3 budget 1,000 ms;
- deterministic Research Report p95: 21.554 ms, CP3 budget 3,000 ms;
- Research Checkpoint creation p95: 15.567 ms, CP3 budget 1,000 ms.

The existing CP2 fixture of 50,000 entities and 150,000 relationships remains the scale regression for Continuity Core. CP12 still owns full 5 GB payload, peak RSS/CPU/disk, Windows certification, background-load responsiveness, and release stress evidence.

## Exit Evaluation

1. Research-only works without AI, Git, repository, or Development state: PASS.
2. ResearchSession, Question, Evidence, Experiment, Result, Finding, Decision, and optional Requirement flows are implemented: PASS.
3. Result remains independent from Finding and Requirement remains optional: PASS.
4. Original Evidence and interpretation remain separable and traceable: PASS.
5. Every material mutation is transactional, actor-attributed, idempotent where retried, evented, and timeline-visible: PASS.
6. Pause, Checkpoint, reopen/resume state, report, export, restore, and integrity verification work: PASS.
7. CP2 regressions remain green: PASS.
8. No CP4+ dependency is pulled into canonical Research behavior: PASS.

## Final Decision

**CP3 PASS — Research Core is complete for its declared deterministic scope and is ready to provide optional research rationale to CP4 Development Core.**

This decision does not pre-approve CP4–CP12 behavior. AI proposals, Git ingestion, capture acquisition, semantic Context Packs, HTML-first Human Documentation/styled visual reporting, and release certification retain their existing checkpoint ownership. ADR-007 preserves this validated Markdown report as compatibility evidence and introduces no CP3 canonical or schema change.
