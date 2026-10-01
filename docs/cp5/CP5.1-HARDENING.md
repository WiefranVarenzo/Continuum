# CP5.1 — Code Intelligence Correctness Hardening

> **Status:** PASS
>
> **Validated:** 2026-09-08
>
> **Build:** `continuum-core 0.5.0` / schema v6

## Purpose

CP5.1 closes correctness and durability gaps found by the post-CP5 audit without changing Continuum's agreed architecture. Git remains authoritative, deterministic records remain canonical, historical observations remain immutable, Development-only mode remains valid, and AI remains proposal-only.

## Closed Gaps

1. **Historical projection rewind:** analyzing or reusing an older RepositoryBaseline records/returns its immutable AnalysisRun but cannot reactivate old entities or mark current entities unavailable. Only the latest reconciled baseline may update the current presence projection.
2. **Rename and path reuse collision:** file stable identity is ID-based, while path aliases now have `first_seen`, `last_seen`, and `retired_at` baseline coordinates. A rename with corroborating CP4 evidence and matching content keeps the file ID. Restoring identical content can reactivate it. Different content later using an old path receives a new ID.
3. **Historical ChangeSet linkage:** exact-baseline observations are preferred. A deletion can resolve the path alias retired at that same baseline, preserving its link to the deleted file without making that alias current again.
4. **Artifact retry mismatch/orphan:** a repeated artifact idempotency key with different bytes, media type, or classification fails closed before content finalization. A newly finalized content file is guarded and removed automatically if the ledger transaction fails.
5. **Archived Development writes:** typed Development and Code Intelligence commands now require both an enabled Development capability and an active project. Archived projects remain readable but reject canonical writes.
6. **Unbounded analyzer cache:** the rebuildable cache records byte size and last access, and applies deterministic LRU eviction with a conservative 256 MiB CP5 runtime cap.
7. **Partial project/export directories:** project creation and export build in a unique sibling staging directory and become visible only after a successful atomic rename. Failed staging directories are removed.
8. **Symlink race at local file open:** artifact files and untracked Git files are opened with no-follow/close-on-exec flags on Unix, in addition to type/path checks. Git symlink blobs continue to be represented but never parsed as code.
9. **Idempotency envelope collision:** retries must retain project, operation, actor identity, command schema version, and payload schema version. Cross-project or changed-envelope reuse fails closed.

## Schema v6

Migration `0006_cp5_1_hardening.sql`:

- replaces one-path/one-entity aliases with temporal aliases and one active alias per repository/path;
- preserves every v5 alias as an active v6 alias;
- adds analyzer-cache byte size and last-access timestamps;
- preserves existing cache output as rebuildable data;
- is transactional and covered by a v5→v6 backup-and-migration fixture.

No CP1 domain direction or CP2–CP5 canonical entity meaning changes. Schema v6 is a corrective representation upgrade for already-agreed identity, history, safety, and bounded-resource invariants.

## Regression Evidence

- historical reanalysis cannot rewind the current CodeEntity/Test projection;
- rename keeps identity while later reuse of the old path creates a distinct identity;
- rename plus immediate old-path reuse in one commit is resolved in the correct order, while an ordinary copy keeps two distinct identities;
- deletion ChangeSet links still resolve after alias retirement;
- identical restoration keeps identity;
- changed artifact retry creates neither a second record nor orphan payload/staging file;
- archived project rejects code analysis writes;
- v5 aliases/cache migrate losslessly to v6;
- CP2, CP3, CP4, and all prior CP5 tests remain green.

## Explicit Boundaries

CP5.1 does not add semantic inference, execute repository code/tests, implement the CP6 provenance graph, create AI provider calls, render CP8 HTML reports, expose CP11 MCP, or claim CP12 multi-platform/stress certification. The 256 MiB analyzer cap is the implemented CP5 safety ceiling; a later configurable shared derived-cache budget may be stricter and must never weaken this fail-safe default without new performance evidence.
