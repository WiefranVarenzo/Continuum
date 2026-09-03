# CP3 — Acceptance Checklist

> **Result:** PASS

- [x] Research Space can be enabled without changing project identity.
- [x] Research writes fail atomically while the capability is disabled.
- [x] A ResearchSession can start, contain activity, complete/cancel, and reject later membership.
- [x] ResearchQuestion supports create, optimistic edit, lifecycle, search, and timeline.
- [x] Evidence requires an honest source representation.
- [x] Original Evidence is separate from annotation, summary, and interpretation.
- [x] Artifact-backed Evidence/Result validates project and availability.
- [x] Experiment cannot skip required lifecycle states.
- [x] Result can exist without Finding.
- [x] Finding can simultaneously expose supporting, challenging, contextual, and inconclusive sources.
- [x] Finding acceptance requires support.
- [x] Decision preserves alternatives, rationale, constraints, provenance, and supersession history.
- [x] Decision supersession is acyclic and expected-version protected.
- [x] Requirement is optional and its rationale origin is explicit.
- [x] Generic Core APIs cannot bypass typed Research invariants.
- [x] AI actor/origin cannot mutate canonical Research state.
- [x] Timeline is append-only, ledger ordered, filterable, and paginated.
- [x] Search is deterministic, local, filterable, and bounded.
- [x] Research Checkpoint is immutable and does not imply completion.
- [x] Resume exposes bookmark, relevant deltas, unresolved live state, next actions, and truncation.
- [x] Deterministic report contains source ledger position and cited stable IDs.
- [x] Export/restore preserves Research identity, origin, source data, relationships, and capability state.
- [x] Integrity scan diagnoses missing normalized state and invalid links.
- [x] v1 and v2 projects back up and migrate safely to schema v3.
- [x] All CP2 regression tests remain green.
- [x] Reference-hardware performance budgets pass.
- [x] Formatting and zero-warning lint gates pass.
