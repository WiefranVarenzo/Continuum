# CP4 — Acceptance Checklist

> **Result:** PASS

- [x] Development Space can be enabled without changing project identity or enabling Research.
- [x] Development writes fail atomically while the capability is disabled.
- [x] Only an exact canonical non-bare Git top-level can be attached.
- [x] Git adapter uses direct bounded read-only processes with timeout and no shell/external diff.
- [x] Repository-local executable Git filters and diff/text-conversion drivers fail closed.
- [x] Repository relocation is explicit, versioned, audited, and continuity-checked.
- [x] Baselines preserve exact HEAD/ref/branch/worktree content identity and adapter version.
- [x] Same-status but different staged/unstaged/untracked content produces a different fingerprint.
- [x] Clean, dirty, detached, unborn, fast-forward, branch-switch, and rewrite states are modeled.
- [x] Immediate identical baseline observation is idempotent; returning to a known state appends reconciliation history without duplicating its identity.
- [x] Commit ingestion is baseline-addressed, bounded, incremental, and repeatable.
- [x] Re-ingestion produces no duplicate CommitObservation or file change.
- [x] Commit parent/tree/author/time/subject metadata remains tied to exact OID.
- [x] Add/copy/delete/modify/rename/type/unmerged paths have explicit deterministic representation.
- [x] Merge diffs use a documented first-parent policy.
- [x] Working-tree ChangeSet is tied to exact baseline fingerprint.
- [x] Committed ChangeSet accepts only ingested same-Repository commits reachable from its baseline.
- [x] Draft realization/supersession is explicit and preserves the earlier record.
- [x] Requirement linkage supports implements, partially-implements, and reverts only.
- [x] Development-only Requirement uses shared identity and honest non-research rationale origin.
- [x] Research-origin Requirement remains Decision-backed and reusable by Development.
- [x] Generic Core APIs cannot bypass typed Development entity or relationship rules.
- [x] AI actors cannot mutate canonical Development state.
- [x] Development timeline and deterministic search projections are bounded and local.
- [x] Development Checkpoint is immutable, incomplete-safe, and records exact source versions.
- [x] Resume detects later events, unobserved Git divergence, and unavailable repository state.
- [x] Deterministic report distinguishes known rationale from external/legacy/manual/unknown origin.
- [x] Research report excludes Development-created Requirements.
- [x] Export/restore preserves CP4 identity, observations, links, origins, and integrity.
- [x] Integrity scanner diagnoses missing detail/search state, invalid links, and impossible ChangeSet material.
- [x] v1, v2, and v3 projects back up and migrate safely to schema v4.
- [x] All CP2 and CP3 regression tests remain green.
- [x] Reference-hardware 100-commit ingestion budget passes.
- [x] Formatting and zero-warning lint gates pass.
