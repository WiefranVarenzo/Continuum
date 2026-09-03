# CP4 — Validation and Exit Decision

> **Decision:** PASS
>
> **Validated:** 2026-09-03
>
> **Reference machine:** AMD Ryzen 5 5600H, approximately 16 GB RAM, Linux x86_64

## Scope Reviewed

CP4 was checked against the Development-only and connected handoff journeys, FR-DS-001–015, AC-DS-01–06, CP1 Architecture, Domain Model, Data Architecture, Spaces contract, ADR-001/003/004/006, the CP3 Requirement handoff, and all CP2.1 invariants.

## Automated Evidence

- `cargo test --workspace --locked`: PASS, 55 tests, 0 failures.
- unit and migration tests: 11 PASS.
- CP2 regression suite: 17 PASS.
- CP3 regression suite: 15 PASS.
- CP4 integration/acceptance suite: 12 PASS.
- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- real Git repository fixtures: PASS.
- SQLite integrity and foreign-key checks: PASS on valid fixtures.
- v1→v4, v2→v4, and v3→v4 pre-migration backup: PASS.
- CP4 export/import restoration: PASS.

## Failure and Abuse Cases Covered

- disabled Development capability;
- subdirectory instead of exact Git top-level;
- bare/missing/different-object-store attachment constraints;
- repository-local executable Git filter/diff-driver configuration;
- AI actor canonical-write attempt;
- generic typed-entity bypass;
- immutable baseline direct mutation;
- empty working-tree or commit-less committed ChangeSet;
- stale optimistic Repository/Requirement/ChangeSet transition;
- duplicate commit/Requirement membership;
- cross-Repository baseline/commit/supersession;
- commit not reachable from selected baseline;
- malformed, oversized, timed-out, or non-UTF-8 Git output/path contract;
- duplicate baseline and commit re-ingestion;
- branch switch, amend/history rewrite, rename, worktree drift, and repository relocation;
- invalid lifecycle skip and undeclared Development relationship;
- oversized commit/path/member/text/JSON/checkpoint/report inputs;
- unavailable or silently changed repository after a bookmark.

## Reference Benchmark

Fixture `incremental-git-100-v1` begins from one ingested commit, adds 100 commits changing one tracked file each, observes a new baseline, ingests all new commits and diffs, and creates one 100-commit ChangeSet. Local network use is zero.

- incremental Git ingestion of 100 commits: 1,108.570 ms, target 60,000 ms;
- creation of 100-commit ChangeSet: 1,522.342 ms, informational;
- Development resume p95: 93.990 ms, target 1,000 ms;
- deterministic Development Report p95: 9.036 ms, target 3,000 ms;
- existing Checkpoint load p95: 2.862 ms, target 1,000 ms.

The CP2 50,000-entity/150,000-relationship fixture remains the scale regression. CP12 still owns 10,000-commit/50,000-file release stress, peak RSS/CPU/disk, active cancellation/background scheduling, Windows certification, and full release evidence.

## Exit Evaluation

1. Development-only works without Research, AI, or fabricated rationale: PASS.
2. Repository attachment, relocation, immutable baseline, commit, and diff observations are implemented: PASS.
3. Re-ingestion is repeatable and rewrite/branch/rename history remains explicit: PASS.
4. Working-tree and committed ChangeSets preserve exact material identity and Requirement links: PASS.
5. Every material canonical mutation is validated, transactional, actor-attributed, idempotent where retried, evented, and timeline-visible: PASS.
6. Pause, Checkpoint, live-drift-aware resume, report, export, restore, and integrity verification work: PASS.
7. CP2 and CP3 regressions remain green: PASS.
8. No CP5+ structural analyzer, AI, capture, context, or MCP claim is made: PASS.

## Final Decision

**CP4 PASS — Development Core is complete for its declared deterministic scope and provides exact repository coordinates to CP5 Code Intelligence.**

This decision does not pre-approve CP5–CP12 behavior. Code structure, dependency/configuration analysis, Tests/TestRuns, full provenance traversal, AI explanation, visual reporting, semantic context, MCP, and release certification remain with their owning checkpoints.
