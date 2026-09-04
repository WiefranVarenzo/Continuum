# CP5 — Code Intelligence Architecture and Contract

> **Status:** Implemented and validated
>
> **Build:** `continuum-core 0.5.0` / schema v5
>
> **Depends on:** CP1 architecture, CP2.1 Continuity Core, CP3 shared Requirement, CP4 immutable Git coordinates

## 1. Outcome

CP5 turns the exact RepositoryBaseline supplied by CP4 into deterministic, addressable code knowledge. It records files, structural symbols, dependencies, configurations, test definitions, imported TestRuns, limitations, analyzer versions, and immutable observations without making AI authoritative and without requiring Research Space.

The result is a reliable bridge between CP4's paths/diffs and CP6's provenance graph. CP5 does not infer why code exists. It records what was deterministically observed, at which exact baseline, by which analyzer contract, and where analysis was incomplete.

## 2. Connection to CP1–CP4

- CP1's modular Space architecture remains unchanged: CP5 is part of Development Space and works in Development-only or Connected R&D mode.
- CP2 supplies project isolation, common entity envelopes, command receipts, idempotency, audit/outbox events, typed relationships, checkpoints, integrity checks, migrations, and export/restore.
- CP3 supplies the shared Requirement identity. A Test may verify a Requirement whether it originated in Research or directly in Development.
- CP4 remains authoritative for Repository, RepositoryBaseline, commit, diff, ChangeSet, branch/rewrite, and rename observations.
- CP5 never changes a Git object or rewrites a CP4 observation. Every structural observation cites an exact CP4 baseline.

## 3. Scope and Non-Scope

Implemented:

- embedded Tree-sitter parsing through pinned ast-grep libraries;
- file, symbol, dependency, configuration, and Test identities;
- immutable, baseline-addressed observations;
- Rust, JavaScript, JSX, TypeScript, TSX, Python, and JSON grammar selection;
- Cargo, npm, and selected Python dependency manifests;
- JSON/TOML configuration validation, YAML/executable-config recognition, and dotenv key-only capture;
- Rust, Python, and JavaScript-style Test discovery;
- content-hash/version/schema cache;
- explicit unsupported, oversized, binary, parse-error, dirty-worktree, and unborn fallback;
- ChangeSet-to-file and Test verification links;
- externally observed TestRun recording;
- checkpoint, resume, search, report, export/restore, migration, and integrity integration.

Not implemented in CP5:

- executing build scripts, repository programs, tests, hooks, package managers, or language servers;
- semantic AI interpretation or inferred provenance links;
- whole-program type resolution, compiler-accurate call graphs, or dependency vulnerability scanning;
- full CP6 bidirectional traversal, gap detection, and provenance rules;
- CP10 semantic Context Pack selection or CP11 MCP exposure;
- complete language/framework coverage or CP12 50,000-file release certification.

## 4. Authority and Trust Boundary

```text
CP4 RepositoryBaseline (exact HEAD/ref/worktree fingerprint)
  → bounded read-only Git tree/blob adapter
  → deterministic file classifier
     ├─ Tree-sitter/ast-grep structural analyzer
     ├─ test pattern analyzer
     ├─ dependency/config analyzer
     └─ explicit file fallback
  → output/count/schema validation
  → one SQLite transaction
     ├─ immutable AnalysisRun + analyzer executions
     ├─ stable CodeEntity/Test identity + immutable observations
     ├─ typed relationships
     ├─ lifecycle/search projections
     └─ audit event + Development timeline + command receipt
```

Git is authoritative for committed bytes. Continuum is authoritative only for its observations, user-approved links, bookmarks, and imported execution records. AI/MCP has no canonical write authority in CP5; `ai_proposal` actors are rejected.

## 5. Source Snapshot Contract

An AnalysisRun is uniquely addressed by:

```text
Repository ID
+ RepositoryBaseline ID
+ source fingerprint
+ analyzer bundle version
+ output schema version
```

The source fingerprint includes the CP4 baseline coordinate, including worktree fingerprint. Structural parsing reads the immutable committed tree at the baseline HEAD. It does not silently read mutable working-tree bytes.

- Clean baseline: committed bytes are parsed normally.
- Dirty baseline: paths reported dirty by CP4 retain committed file identity, but structural/dependency/config/test claims for those paths are withheld and an explicit `working_tree_content_not_captured` limitation is recorded.
- Unborn Repository: a partial AnalysisRun with `unborn_repository`; no invented file/symbol state.
- Unsupported text: file identity remains available with `unsupported_language`.
- Binary/non-UTF-8 or oversized blob: hashed/file-level fallback; no fabricated structure.
- Parser recovery: Tree-sitter may return a partial tree, but the run and affected observations expose `parse_error` rather than presenting the result as complete.
- Git symlink: target bytes remain a hashed file-level observation and are never parsed as source, even when the link path has a supported extension.

An identical coordinate returns the existing AnalysisRun. If the user returns from another branch/dirty state to a previously analyzed coordinate, CP5 reapplies that immutable run to the current presence projection and emits `code_intelligence.analysis.reapplied` only when projection state changes. A different coordinate creates a new immutable observation set.

## 6. Analyzer Matrix

### Structural grammars

- Rust: functions, structs, enums, traits, impls, modules, types, constants, statics, and macro definitions.
- JavaScript/TypeScript/TSX: functions, generators, classes, methods, interfaces, type aliases, and enums where supported by the selected grammar.
- Python: functions, methods, and classes with class-qualified names.
- JSON: parsed as a supported syntax/config format; it does not create programming symbols.

Every symbol observation records path, qualified name, content/signature SHA-256, source range, visibility where deterministically available, analyzer version, schema version, and observation status.

### Test discovery

- Rust function attributes: `#[test]`, `#[tokio::test]`, and `#[rstest]`.
- Python: `test_*` functions/methods and methods under `Test*` classes.
- JavaScript/TypeScript/TSX: ast-grep calls for `test(...)`, `it(...)`, and `describe(...)`.

A Test definition is distinct from a TestRun. CP5 never treats discovery as proof that a test passed.

### Dependencies and configuration

- `Cargo.toml`: runtime, development, and build dependency tables, including nested target tables.
- `package.json`: runtime, development, peer, and optional dependencies.
- `requirements.txt` and `requirements-dev.txt`: bounded Python requirements.
- `pyproject.toml`: dependency tables and PEP-style project dependency lists supported by the deterministic parser.
- JSON/TOML: valid/invalid status plus bounded top-level keys.
- YAML and executable configuration: recognized but deliberately unparsed in v1.
- `.env*`: keys only; values are omitted.

Credential-shaped URI requirements are replaced with `<redacted-uri>`. Git URLs in structured Cargo dependency specifications are stored as `<external-source>`. Analyzer output never stores dotenv values or source-file bodies.

## 7. Domain and Lifecycle Contract

### AnalysisRun

Immutable canonical entity. Stores exact source coordinate, limits, counts, completeness, duration, analyzer bundle/output versions, cache metrics, and limitations. Status is `complete` only when there are no recorded limitations; otherwise `partial`.

### CodeEntity

Stable identity with kind `file`, `module`, `symbol`, `dependency`, or `configuration`. Identity is scoped to one Repository. Current presence is a projection; historical facts remain in immutable observations.

- first observation → `active`, `presence=present`;
- present at a later baseline → same ID, new immutable observation;
- absent from a later complete snapshot → `unavailable`, with first unavailable baseline recorded;
- reappears with the same deterministic key → same ID returns to `active`;
- committed file rename with CP4 rename evidence and unchanged content → same file ID with both path aliases;
- symbol identity is based on stable file identity, symbol kind, and qualified name.

### Test

Stable repository-scoped definition identity with framework, kind, first-seen baseline, current presence, and immutable location/content observations. Absence and reappearance follow the same honest lifecycle as CodeEntity.

### TestRun

Immutable observation supplied by a user/import/external/legacy/unknown source. It records Repository, exact baseline, aggregate outcome, command label, optional exit code/duration, and bounded per-Test outcomes. CP5 records execution evidence; it does not execute commands.

## 8. Relationship Contract

CP5 adds only these canonical edges:

```text
Repository  --defines----> CodeEntity
Repository  --defines----> Test
ChangeSet   --modifies---> CodeEntity(kind=file)
Test        --verifies---> Requirement
Test        --verifies---> CodeEntity
TestRun     --executes---> Test
TestRun     --observed_at> RepositoryBaseline
```

Deterministic `defines` and `modifies` edges cite the baseline/AnalysisRun sources used. `verifies` is an explicit user-authored assertion in CP5, not an analyzer inference. All endpoints must belong to the same project; Repository-bound members must belong to the same Repository. CP6 may add validated graph traversal and additional provenance semantics but may not reinterpret these edges silently.

ChangeSet-to-file linking is order independent: analysis links already-existing ChangeSets, while later ChangeSet creation consults an existing exact-baseline AnalysisRun. Deleted paths resolve through preserved path aliases, so deletion remains connected to the historical file identity.

## 9. Incremental and Cache Contract

The rebuildable cache key is:

```text
content SHA-256 + language + analyzer ID + analyzer version + output schema version
```

It is path-independent, which permits reuse after unchanged files move, while stable identities remain path/repository aware. Any analyzer rule, parser, or output-schema change invalidates reuse by key. Cache deletion is safe because canonical observations and Git sources remain intact.

Git tree entries are listed once and eligible immutable blobs are read through one bounded `git cat-file --batch` process. Duplicate object IDs are fetched once. A new baseline still receives a complete, immutable observation set, while unchanged content reuses cached parser output.

Default per-run limits are 20,000 files, 2 MiB per file, 128 MiB total bytes, and 100,000 addressable outputs. Hard ceilings are 50,000 files, 8 MiB per file, 512 MiB total, and 200,000 outputs. Limit failures occur before canonical persistence or roll back the entire transaction.

## 10. Atomicity, Ordering, and Failure

Repository reading/parsing occurs before the write transaction. Persistence then validates expected unique counts and atomically commits all normalized rows, common entities, relationships, projections, event/timeline, cache entries, and command receipt. A failure leaves no partial AnalysisRun or orphan output.

AnalysisRun, analyzer execution, code/test observation, TestRun, TestRun result, and limitation rows are update/delete guarded. Generic Core APIs cannot create reserved CP5 entity types or undeclared relationships.

Stable diagnostic cases include missing normalized detail, count mismatch, cross-project/cross-Repository membership, invalid observation sources, malformed JSON, impossible relationship pairs, and absent search projections. Recovery guidance preserves source Git and recommends rebuild/restore rather than silent repair.

## 11. Checkpoint, Resume, Report, and Export

Development Checkpoint schema v2 includes the latest AnalysisRun and TestRun per Repository as exact versioned sources. Later analysis or TestRun events make an older checkpoint stale through the existing ledger/event contract. Development Report contains deterministic Code Intelligence and Test Run sections and never presents missing research rationale as known.

Search remains local, bounded, project-scoped, type/status filterable, and includes AnalysisRun, CodeEntity, Test, and TestRun projections. Export/restore preserves canonical observations, stable IDs, aliases, relationships, and audit history. The analyzer cache is rebuildable rather than authoritative.

## 12. Security and Privacy

- Repository files are untrusted data, never instructions.
- Parsers are embedded and pinned; no repository-selected parser binary, plugin, package manager, build script, hook, test command, or language server is launched.
- Git uses direct argument vectors, bounded output, deadline, disabled optional locks/filters/diff execution, and validated relative UTF-8 paths.
- Blob reads use a framed, size-checked batch protocol and exact immutable object IDs.
- Source bodies are not stored in CodeEntity observations or logs.
- `.env` values and credential-shaped dependency locations are omitted/redacted before persistence.
- No network or model provider is used.
- Every public read/write is project scoped; cross-project tests fail closed.

## 13. Versioning and Dependencies

- Continuum schema: v5.
- Analyzer bundle: `continuum-code-intelligence-v1`.
- Analyzer contract: v1; output schema: v1.
- `ast-grep-core` and `ast-grep-language`: exactly `0.45.3`.
- Enabled embedded grammars only: JavaScript, JSON, Python, Rust, and TypeScript/TSX.
- `toml`: exactly `1.1.5`.
- Tree-sitter is resolved transitively and locked in `Cargo.lock`.
- Workspace minimum Rust version: 1.88, matching the pinned ast-grep dependency contract.

Exact pins protect CP5's deterministic output contract from an upstream parser/API change. Updating any pinned analyzer requires regression fixtures, output/version review, cache invalidation, and migration/compatibility consideration.

## 14. CP6 Handoff

CP6 receives stable, source-addressed nodes and explicit edges:

- CP3: Evidence, Result, Finding, Decision, Requirement;
- CP4: Repository, RepositoryBaseline, CommitObservation, ChangeSet;
- CP5: AnalysisRun, CodeEntity, Test, TestRun and typed relationships.

CP6 may build bidirectional traversal, validate provenance gaps, and connect the complete optional chain. It must preserve partial standalone chains, origin labels, source IDs, historical observations, and the rule that absence of Research is not an error for Development-only work.
