# Continuum

This workspace contains the CP2 Continuity Core and CP3 Research Core implementations derived from the approved CP1 architecture.

The implementation includes durable project identity, SQLite Project Ledger, typed relationships, append-only audit/outbox records, optional Space capability state, content-addressed artifacts, immutable checkpoints, durable jobs, integrity diagnostics, verified export/import, and the deterministic ResearchSession → Question/Evidence/Experiment/Result/Finding/Decision workflow.

Research-only work can now be paused, resumed, searched, reported, exported, and restored without a repository or AI. Repository intelligence, semantic AI, capture acquisition, semantic Context Pack selection, and MCP remain deferred to their owning checkpoints.

## Verify

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The library is located at `crates/continuum-core`. CP2 and CP3 validation evidence lives in `docs/cp2/` and `docs/cp3/`.
