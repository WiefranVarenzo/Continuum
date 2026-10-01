# CP5 — Validation and Exit Decision

> **Decision:** CP5.1 PASS
>
> **Validated:** 2026-09-08
>
> **Reference machine:** AMD Ryzen 5 5600H, approximately 16 GB RAM, Linux x86_64

## Scope Reviewed

CP5 was checked against CP1 Architecture, Domain Model, Data Architecture, Spaces, Checkpoint/Context, Privacy/Security, Performance Budget, ADR-001/003/004/005/006, CP2.1 invariants, CP3 Requirement identity, CP4 Git authority, FR-DS-006–010/012–015, and AC-DS-01–06.

## Automated Evidence

- `cargo test --workspace --all-targets --locked`: PASS, 78 tests, 0 failures on the Linux reference machine.
- unit and migration tests: 15 PASS.
- CP2 regression suite: 18 PASS.
- CP3 regression suite: 15 PASS.
- CP4 regression suite: 12 PASS.
- CP5 integration/acceptance suite: 18 PASS on Linux (including historical projection; rename, copy, same-commit path reuse; archive; and Unix Git-symlink safety fixtures).
- `cargo fmt --all --check`: PASS.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: PASS.
- SQLite integrity/foreign-key checks: PASS on valid fixtures.
- v1→v6, v2→v6, v3→v6, v4→v6, and populated v5→v6 pre-migration backup/migration: PASS.
- CP5 export/import restoration: PASS.

## Failure and Abuse Cases Covered

- disabled Development capability;
- AI actor and generic reserved-entity bypass;
- invalid or above-hard-ceiling analysis limits;
- Repository/baseline mismatch and cross-project/cross-Repository membership;
- unsupported, binary, oversized, non-UTF-8, dirty, unborn, and parser-recovery inputs;
- malformed Git batch framing/object identity/size and bounded output/deadline;
- repository-local executable Git configuration inherited from CP4 checks;
- duplicate analysis retry and unchanged-content reuse;
- cached parser-recovery limitation preservation and proof that old-baseline analysis cannot rewind current projection;
- source-shaped Git symlink target data;
- ChangeSet creation both before and after analysis, including deleted-path alias linkage;
- committed file rename, ordinary copy, deletion, byte-identical reappearance, different-file reuse of a retired path, and rename plus immediate old-path reuse in one commit;
- mismatched artifact idempotency retry without orphan content or staging residue;
- archived-project rejection for Code Intelligence writes;
- bounded analyzer-cache metadata/eviction contract and atomic project/export staging;
- dotenv and credential-shaped dependency data;
- TestRun aggregate/result inconsistency, duplicate Test result, and excessive result count;
- invalid Test verification target;
- normalized-row corruption and count/source/projection mismatch;
- transaction failure with no partial AnalysisRun/entity;
- export/restore and project-scope isolation.

## Reference Benchmark

Fixture `code-intelligence-500-v1` contains 200 Rust files, 200 TypeScript files, 98 JSON files, and two manifests. It produces 1,402 CodeEntities and 400 Tests on the cold snapshot. A second snapshot adds one TypeScript source/test file, producing exactly 500 cache hits and one cache miss. Network use is zero.

- cold analysis of 500 files: 630.771 ms, informational checkpoint fixture;
- one-changed-file incremental analysis over 501 files: 200.318 ms, target ≤5,000 ms;
- AnalysisRun load p95: 2.261 ms, target ≤500 ms;
- CodeEntity search p95: 2.543 ms, target ≤500 ms;
- deterministic Development Report p95: 4.430 ms, target ≤3,000 ms.

This result proves the declared CP5 fixture on the reference machine. CP12 retains the full 50,000-file/10,000-commit Standard fixture, peak RSS/CPU/disk, cancellation/background scheduling, multi-platform, and stress certification.

## Exit Evaluation

1. Exact CP4 state becomes versioned, addressable deterministic structure: PASS.
2. Unsupported or unsafe input degrades honestly without losing file identity: PASS.
3. Files/symbols/dependencies/configurations/tests have stable identities and immutable observations: PASS.
4. Test discovery, verification claims, and observed runs remain distinct: PASS.
5. Cache/version/limit/atomicity and source-authority contracts hold: PASS.
6. Development-only checkpoint, resume, report, search, export/restore, and integrity work: PASS.
7. CP2–CP4 regressions remain green: PASS.
8. No CP6 graph or CP7 AI claim is made: PASS.

## Final Decision

**CP5.1 PASS — Code Intelligence is hardened for its declared deterministic scope and provides stable, history-safe CodeEntity/Test/TestRun coordinates to CP6 Provenance & Knowledge Graph.**

This decision does not pre-approve CP6–CP12. Full provenance traversal/gap validation, semantic AI, HTML-first Human Documentation/styled reports, capture, semantic Context Packs, MCP access, and release certification remain with their owning checkpoints. ADR-007 preserves the validated report as compatibility evidence and introduces no CP5 analyzer, canonical, or schema change.
