# CP4 — Development Core Architecture and Contract

> **Status:** Implemented and validated
>
> **Build:** `continuum-core 0.4.0` / schema v4
>
> **Depends on:** CP1 architecture, CP2.1 Continuity Core, CP3 shared Requirement handoff

## 1. Outcome

CP4 makes Development Space independently useful. A project can attach a local non-bare Git repository, preserve exact repository baselines, ingest immutable commit and file-diff observations, group work into ChangeSets, connect changes to honestly sourced Requirements, create a Development Checkpoint, and resume later without enabling or fabricating Research Space.

Git remains authoritative for repository objects and history. Continuum is a read-only observer in CP4: it does not stage files, commit, checkout, merge, rebase, reset, amend, run hooks, or execute repository content.

## 2. Connection to CP1–CP3

- CP1's optional-Space decision remains intact: Development-only, Research-only, and Connected R&D share one project identity.
- CP2 supplies the common entity envelope, command idempotency, optimistic versions, audit sequence, outbox, checkpoints, export/restore, and integrity scanner.
- CP3's `Requirement` identity is reused. CP4 adds `created_in_space` so Development-only Requirements can exist without a Research Session or synthetic Decision.
- A research-origin Requirement created by CP3 can be linked to a CP4 ChangeSet without copying or rewriting it.
- CP4 adds no CodeEntity, symbol, dependency, configuration, Test, or TestRun claim; those remain CP5.

## 3. Authority Boundary

```text
User command
  → Development application service
    → read-only Git adapter
    → validation and project/Space checks
    → one SQLite transaction
       canonical rows + typed links + audit event + timeline + outbox + command receipt

Git repository → authoritative objects and working tree
Continuum      → immutable observations, intent, links, bookmarks, and projections
AI/MCP         → no canonical authority in CP4
```

All canonical Development commands reject an `ai_proposal` actor. Future AI and MCP paths must submit reviewable proposals through CP7/CP11.

## 4. Implemented Domain Slice

### Repository

A stable Continuum entity identifies an attachment. The normalized record stores canonical local root path, SHA-256 root fingerprint, Git common-directory fingerprint, object format, adapter version, attachment time, and last observation time. Remote URLs and credentials are not collected.

The initial attachment requires the exact canonical Git top-level directory and rejects bare repositories, non-UTF-8 paths, missing repositories, and unsupported object formats. An explicit, expected-version-protected relocation command preserves the Continuum Repository ID after a directory move. If commits were previously observed, the new path must contain a known commit as continuity proof.

### RepositoryBaseline

An immutable baseline records exact HEAD OID or an explicit unborn state, symbolic ref, branch, deterministic worktree fingerprint, NUL-safe structured status entries, observation time, prior baseline, adapter version, and a reconciliation classification. The fingerprint covers staged and unstaged binary diffs plus streamed content hashes for untracked files; a two-pass capture rejects a worktree that changes during observation.

```text
initial | unchanged | worktree_changed | fast_forward |
branch_switch | history_rewrite | detached_head | unborn
```

Immediately re-observing an unchanged HEAD/ref/worktree coordinate returns the same baseline. Returning to a previously known coordinate reuses its baseline identity but appends a new reconciliation/audit observation, so branch chronology is not lost. Amend, rebase, force-update, and divergent history create a new baseline observation; older provenance is never redirected.

### CommitObservation and file diff

Commit observations are unique by Repository plus exact OID. They preserve tree OID, parent OIDs, raw author identity, authored/committed times, subject, first observed baseline, adapter version, and ordered file changes. File changes distinguish add, copy, delete, modify, rename, type change, unmerged, and unknown fallback. Rename/copy similarity and old/new paths remain explicit.

Ingestion is deterministic, bounded, repeatable, and baseline-addressed. It only accepts commits reachable from the selected baseline. Re-ingestion creates no duplicate commit or file-change records.

### Requirement

Development-only Requirements use the same canonical and normalized identity as CP3 Requirements. Their rationale origin must be `user`, `import`, `external`, `legacy`, or `unknown`; `research` is accepted only through CP3's Decision-backed command. Requirement progression remains optimistic and explicit:

```text
draft → accepted → in_progress → implemented → verified
                    ↕ blocked
retired | superseded | archived are explicit branches
```

### ChangeSet

A ChangeSet is either:

- `working_tree`: a draft tied to an exact baseline worktree fingerprint and structured status; or
- `committed`: an observation grouping one or more ingested commits reachable from its baseline.

It stores summary, honest intent origin, Repository, baseline, ordered commits, file observations, optional Requirement links, and optional explicit supersession. A committed ChangeSet may supersede a draft, but similarity never silently merges their identities.

Canonical Requirement links are only:

```text
ChangeSet --implements------------> Requirement
ChangeSet --partially_implements--> Requirement
ChangeSet --reverts---------------> Requirement
ChangeSet --supersedes------------> ChangeSet
```

The ChangeSet lifecycle follows the CP1 model:

```text
draft → abandoned|superseded|archived
observed → linked → validated → closed
    each material state may branch explicitly to abandoned, superseded, reverted, or archived where valid
```

## 5. Git Adapter Safety Contract

- Git is invoked directly with an argument vector, never through a shell.
- Commands are read-only and run with optional locks disabled.
- external diff environment variables are removed; diff reads use `--no-ext-diff` and `--no-textconv`.
- repository-local executable clean/smudge/process filters, diff commands, and text-conversion drivers are rejected at attachment and before live observation.
- filesystem monitoring and untracked cache side effects are disabled for adapter calls.
- stdin is closed; stdout/stderr are separately bounded and drained.
- every Git process has a 60-second deadline and is killed on timeout.
- output, commit count, path count, JSON, text, ChangeSet membership, and checkpoint sources are bounded; worktree diff/content identity is streamed and capped at 512 MiB per observation.
- malformed, over-limit, non-UTF-8, unreachable, cross-project, or wrong-type data fails before canonical commit.

Repository content is untrusted data. CP4 never executes it and never interprets commit text as instructions.

## 6. Transactions, Ordering, and Idempotency

Git inspection occurs before a write transaction. A successful mutation then atomically commits normalized state, the common entity, relationship changes, one material audit/outbox event, Development timeline entry, deterministic search projection where applicable, and command receipt.

The command ID and idempotency key prevent retry duplication. Immutable observations have database update/delete guards. Repository relocation, Requirement transition, and ChangeSet transition use expected versions. Cross-project and cross-Repository membership is rejected.

## 7. Development Bookmark and Resume

A Development Checkpoint captures the pre-event ledger sequence, latest baseline for every attached Repository, active Requirements and ChangeSets with versions, user note, blockers, and next actions. It explicitly records `research_required: false` and `complete: false`.

Resume combines the immutable bookmark with later Development events and live read-only repository inspection. It marks a checkpoint stale when:

- a material Development event occurred afterward;
- live HEAD or worktree fingerprint differs from the saved baseline; or
- repository state is unavailable.

The saved bookmark remains readable even when stale or the Repository moved.

## 8. Deterministic Development Report

The local report lists Repository IDs and fingerprints, latest exact baseline, Requirement status and rationale origin, ChangeSet status/kind/intent origin, commit and file-observation counts, source ledger sequence, and the fact that Git remains authoritative. It escapes and bounds user/repository-controlled text and never invents research rationale. It remains a deterministic compatibility projection under ADR-007; CP8 now composes the same facts into HTML-first Development Documentation through the Human Document Model without changing CP4 observations.

## 9. Failure and Recovery

- unavailable/moved path: resume reports unavailable; explicit relocation can preserve identity;
- rewritten history: create a new baseline and `history_rewrite` reconciliation; preserve old commits and links;
- identical re-observation/re-ingestion: return existing identity or zero new commits;
- transaction failure: no partial entity, link, event, timeline, projection, or command receipt;
- migration: backup schema v1–v3 projects before transactional v4 migration;
- corrupted normalized state or invalid Development link: integrity scan reports stable issue code and repair guidance;
- oversized or slow Git output: fail closed without changing canonical state.

## 10. Deliberate Deferrals

- file/symbol CodeEntity extraction, dependency/configuration parsing, Tests and TestRuns: CP5;
- full bidirectional provenance traversal and gap detection: CP6;
- change explanation by configured model providers: CP7;
- HTML-first Human Documentation, Human Document schema, offline HTML export, styled documentation, and diagrams: CP8;
- semantic checkpoint/context selection: CP10;
- external client access: CP11 MCP boundary;
- Windows certification, stress/cancellation/background-worker release evidence: CP12.

## 11. CP5 Handoff

CP5 receives stable Repository, baseline, commit, ChangeSet, and ordered changed-path coordinates. Its analyzers must create versioned deterministic CodeEntity/Test observations tied to these exact sources, degrade unsupported languages to the existing file/diff layer, and must not rewrite CP4 observations.
