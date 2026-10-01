# CP8 Validation

> **Date:** 2026-09-08  
> **Result:** PASS

## Validation Commands

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --release -p continuum-core --example cp8_benchmark
npm test
npm run build
npm audit
cargo check --manifest-path apps/continuum-desktop/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/continuum-desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
```

## Results

- Rust formatting: pass;
- core strict linting: pass with zero warnings;
- core tests: 99 passed, 0 failed;
- CP8 core acceptance/adversarial tests: 8 passed;
- CP1–CP7 regression tests: 91 passed;
- UI tests: 5 passed, 0 failed, including collapse/filter/cycle graph behavior;
- production TypeScript/Vite build: pass;
- npm dependency audit: 0 vulnerabilities;
- Tauri v2 compile and strict lint: pass;
- release benchmark: pass on AMD Ryzen 5 5600H with approximately 16 GB RAM;
- visual QA: responsive report, overview, cards, interactive graph, timeline, citations, and narrow viewport rendered correctly; selecting a graph node enabled branch controls and collapsing the sample root reduced the visible graph from 3 nodes/2 links to 1 node/0 links;
- final project integrity scan: healthy.

## Performance Evidence

The 300-entity/600-relationship report fixture produced a bounded 200-node graph and a 197,232-byte self-contained HTML report. Warm deterministic composition plus HTML/Markdown rendering measured p50 359.05 ms and p95 371.83 ms. The applicable target is p95 at most 5,000 ms. Renderer-only p95 was 4.58 ms.

The production UI keeps the approximately 60 KiB gzip initial JavaScript path separate from on-demand graph and Mermaid chunks. Heavy visualization libraries do not block the report header, overview, status, tables, timeline, or citations.

## Covered Failure Classes

- incompatible Checkpoint/report kind and invalid explicit scope;
- oversized reports and graph specifications;
- stale source snapshots and later project changes;
- public/private export classification leakage;
- stored HTML/script/remote-resource injection;
- raw or malicious Mermaid identifiers and labels;
- pending AI candidate use, invalid presentation candidates, and loss of narrative/diagram AI authorship;
- partial export publication and Artifact/hash mismatch;
- malformed or dangling persisted Human Documents and exports;
- schema migration/reopen regression;
- absence of either Research or Development Space.

## Qualification Boundary

Linux core/UI/Tauri compilation and target-device visual/performance evidence are complete. CP12 still owns signed installers, Windows-first certification from ADR-002, other OS webview differences, screen-reader matrix testing, very-large graph virtualization/soak tests, and packaging hardening. CP9 owns captured-media behavior. These are downstream qualification items, not missing CP8 contracts.
