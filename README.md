# Continuum

This workspace contains the CP2 Continuity Core implementation derived from the approved CP1 architecture.

The implementation is intentionally narrow: durable project identity, SQLite Project Ledger, generic entities and typed relationships, append-only audit/outbox records, optional Space capability state, content-addressed artifacts, immutable checkpoint envelopes, durable jobs, integrity diagnostics, and verified export/import.

Research aggregates begin in CP3. Repository intelligence, semantic AI, capture, Context Pack selection, and MCP remain deferred to their owning checkpoints.

## Verify

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The library is located at `crates/continuum-core` and integration tests map directly to the CP2 handoff and core-data acceptance criteria.

