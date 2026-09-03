# CP1 Decision Register

> **Status:** Locked for CP2; amended by ADR-006 on 2026-09-03
> **Rule:** `Deferred` means intentionally assigned to a later owning checkpoint and not required to implement CP2.

## Accepted Decisions

- D-001: Research and Development are independently useful; optional R&D Bridge. See ADR-001.
- D-002: Tauri v2 + React/TypeScript + Rust modular-monolith desktop stack. See ADR-002.
- D-003: Windows 11 first certification; Linux follows behind adapters.
- D-004: MVP is single-user; one active project context per window. Multi-user collaboration is out of MVP.
- D-005: SQLite current state + append-only audit events/outbox; not full event sourcing. See ADR-003.
- D-006: UUIDv7 entity/event identity and per-project ledger sequence.
- D-007: Project-local SHA-256 content-addressed Artifact Store. See ADR-003.
- D-008: soft-delete/tombstone by default; explicit policy-controlled payload purge and delayed GC.
- D-009: MVP code intelligence targets TypeScript/JavaScript and Rust; npm/Cargo plus common JS/Rust tests.
- D-010: working-tree changes are draft ChangeSets tied to exact baseline and fingerprint.
- D-011: Git rewrites create new observations and explicit reconciliation; old provenance is preserved.
- D-012: default classification is internal/project-private; derived data inherits most restrictive source.
- D-013: internal AI requests require project consent; sensitive requires per-request preview; secret/never-send is denied.
- D-014: AI claims require resolvable source IDs; no numeric score creates canonical truth. See ADR-004.
- D-015: Markdown and HTML are first report formats; PDF is deferred to reporting implementation if validated.
- D-016: Checkpoints are immutable, non-terminal, and scope-aware. See ADR-005.
- D-017: Context Pack default external budget is 16k soft / 32k hard tokens with explicit omission metadata.
- D-018: Checkpoints retained; AI cache bounded LRU/30 days; sanitized AI audit metadata 90 days; Context Packs ephemeral unless saved.
- D-019: React Flow + ELK render scoped graphs.
- D-020: CP11 baseline is a Continuum MCP Server over local stdio, read-first, project/capability-scoped, and proposal-based for consequential writes. Streamable HTTP is separately gated. See ADR-006.
- D-021: performance reference is Ryzen 5 5600H / 16 GB / SSD; target correctness and budgets are defined in Performance Budget.
- D-022: context-resumption pilot target is ≥80% correct active goal/constraints/source IDs.
- D-028: CP7 is provider-neutral. Gemini remains first-class; all providers use capability-tested adapters behind one AI Provider Gateway. See ADR-006.
- D-029: Outbound model-provider APIs and inbound MCP access are separate application ports and cannot bypass each other's privacy, authorization, validation, provenance, or review boundaries. See ADR-006.
- D-030: Provider routing is deterministic and project-allowlisted; sensitive requests cannot silently fail over and secret/never-send has no remote route. See ADR-006.
- D-031: CP7 proves at least Gemini plus one non-Gemini provider path; supporting every named provider is not an MVP requirement.
- D-032: CP11 targets compatibility fixtures for Codex, Claude Code, and Gemini CLI against one negotiated MCP contract; client-specific configuration never enters domain logic.

## Deferred Non-Blocking Decisions

- D-023 (CP7): exact supported provider/model profiles, regional endpoints, quota/rate limits, retention/training references, cost metadata, and routing defaults, selected against then-current availability and CP7 conformance evidence.
- D-024 (CP9): final Windows capture implementation and codec/backend after capability spike; architecture baseline is OS-native screen/WASAPI-class adapters with segmented media.
- D-025 (CP9/after MVP): transcription and OCR are off by default and not required for MVP.
- D-026 (CP10): embeddings are off by default; local versus remote requires benchmark/privacy ADR if introduced.
- D-027 (CP11): exact MCP resource/prompt/tool catalog, grant/authentication handshake, protocol support matrix, and any Streamable HTTP deployment, constrained by the read-first/project-scoped ADR-006 contract.

## Supersession Policy

Changing an accepted decision requires a new ADR that names the superseded decision, compatibility/migration impact, affected PRD requirements, and CP2+ consequences. Deferred choices cannot violate accepted invariants.
