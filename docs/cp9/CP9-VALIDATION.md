# CP9 Validation

> **Date:** 2026-09-09  
> **Result:** PASS

## Validation Commands

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run --release -p continuum-core --example cp9_benchmark
npm test
npm run build
cargo check --manifest-path apps/continuum-desktop/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/continuum-desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
```

## Checkpoint Evidence

- CP9 core acceptance: 9 passed, including v9→v10 migration, crash/reopen recovery, and incomplete Evidence publication diagnostics.
- Full Rust regression: 108 passed, 0 failed; strict workspace lint completed with zero warnings.
- Desktop UI tests: 10 passed, including no sensor activation on render, matching-track permission observation, and bounded pressure behavior.
- Production TypeScript/Vite build: pass.
- Tauri Rust compile and strict lint: pass.
- Local visual QA: pass; the capture permission gate, persistent status, disabled-state explanation, controls, and responsive review surface were inspected without activating a sensor. A low-contrast gate state found during inspection was corrected before this validation.
- Release benchmark on AMD Ryzen 5 5600H/approximately 16 GB: segment persistence p50 10.30 ms, p95 11.00 ms, max 11.18 ms against the CP9 local 500 ms p95 gate.
- Benchmark fixture: 30 unique 512 KiB fragments (15 MiB total), marker-to-Evidence linkage, terminal completion, and healthy integrity scan.
- Measured process high-water delta during the benchmark: approximately 1,080 KiB. This is checkpoint evidence, not the CP12 process-group/long-duration claim.

## Covered Failure Classes

- disabled Research Space, missing/duplicate/unsupported source, invalid encoding/buffer/time/page bounds;
- missing consent or indicator, denied/unavailable/revoked permission, stale state version, illegal lifecycle transition;
- out-of-order/overlapping/oversized fragment and idempotency-key content mismatch;
- interrupted partial fragment, restart/reopen recovery, and backend/persistence failure state;
- full-recording duplication during Evidence promotion;
- derived Artifact replacing or matching source;
- external Evidence fingerprint mismatch and dangling/mismatched capture references;
- sensor acquisition during render/background work;
- migration rollback/upgrade and final database/Artifact integrity.

## Qualification Boundary

Core, migration, desktop integration, consent boundary, and bounded persistence are CP9-complete. Automated tests do not grant real OS sensor permission. CP12 must certify each advertised OS/source/codec combination with human-approved live capture, playback/recovery, device removal, sleep/resume, disk pressure, long-duration soak, packaging, and release security evidence. Unsupported runtime capabilities remain visibly unavailable and do not weaken deterministic Research workflows.
