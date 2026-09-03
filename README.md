# Continuum

This workspace contains the CP2 Continuity Core, CP3 Research Core, and CP4 Development Core implementations derived from the approved CP1 architecture.

The implementation includes durable project identity, SQLite Project Ledger, typed relationships, append-only audit/outbox records, optional Space capability state, content-addressed artifacts, immutable checkpoints, durable jobs, integrity diagnostics, verified export/import, the deterministic Research workflow, and a read-only Git-backed Development workflow with RepositoryBaselines, CommitObservations, file diffs, Requirements, and ChangeSets.

Research-only and Development-only work can now be paused, resumed, reported, exported, and restored independently. CP4 observes Git without modifying it, preserves branch/rewrite/rename history, and records unknown or external rationale honestly. Structural code intelligence, provider-neutral semantic AI, capture acquisition, semantic Context Pack selection, and the Continuum MCP Server remain deferred to their owning checkpoints. Gemini is a first-class future adapter rather than a domain dependency; Codex, Claude Code, Gemini CLI, and other compatible clients will use the separate CP11 MCP boundary.

## Verify

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The library is located at `crates/continuum-core`. Validation evidence lives in `docs/cp2/`, `docs/cp3/`, and `docs/cp4/`.

The provider-neutral amendment is recorded in [ADR-006](docs/adr/ADR-006-PROVIDER-NEUTRAL-AI-AND-MCP-BOUNDARIES.md). Its detailed contracts are [AI Architecture](docs/ai/AI-ARCHITECTURE.md), [MCP Continuity Interface](docs/ai/MCP-CONTINUITY-INTERFACE.md), and [Provider and MCP Delivery Plan](docs/ai/PROVIDER-AND-MCP-DELIVERY-PLAN.md).
