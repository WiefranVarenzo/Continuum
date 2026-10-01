# CP11 Validation

> **Date:** 2026-09-10  
> **Result:** PASS

## Validation Commands

```text
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p continuum-core --test cp11_mcp_acceptance
cargo test -p continuum-mcp --test protocol
cargo run --release -p continuum-mcp --example cp11_benchmark
npm test
npm run build
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
git diff --check
```

The npm and Tauri commands run from `apps/continuum-desktop`; the others run from the repository root.

## Evidence

- Strict Rust lint: zero warnings for the workspace and desktop Tauri crate.
- Full Rust regression: 138 tests passed, zero failed.
- CP11 core acceptance: 8 scenarios covering human-only digest grants, disabled HTTP, denied-init audit, four client profiles, cross-project/scope/tool/size/revocation failure, durable rate limit, sanitized audit, proposal idempotency/isolation/review, classification/secret policy, v11→v12 migration, and tamper detection.
- MCP protocol: 11 scenarios covering lifecycle/catalog/bounded state, malformed/unknown/pre-init behavior, bounded cancellation, exact artifact ceilings, Research-scope authorization before search, denied Research/provenance identifier suppression, sanitized unknown-prompt audit, strict resource URI matching, private Context Pack index filtering, non-canonical proposal result, clean newline STDIO, and Codex/Claude Code/Gemini CLI transcript conformance.
- Desktop: 16 tests across six files, including CP11 one-time token/default-policy and explicit proposal-review interactions; production TypeScript/Vite build passed.
- Diff whitespace validation: passed.

## Performance Evidence

On the AMD Ryzen 5 5600H / 16 GB reference device, release local single-process STDIO JSON-RPC with 10 warmups and 100 measured operations produced:

- initialize: approximately 3.55 ms;
- capability-filtered tool catalog: p50 3.56 ms, p95 3.99 ms; target p95 ≤ 50 ms;
- bounded Current Project State: p50 6.82 ms, p95 7.59 ms; target p95 ≤ 500 ms;
- resident memory delta: approximately 276 KiB; fixture target ≤ 128 MiB.

The exact result is `CP11-BENCHMARK-2026-09-10.json`. It validates this fixture only; CP12 retains release-profile Standard/stress, process-group, live-client, and long-duration qualification.

## Defects Found and Closed

- Invalid initialization initially failed closed but did not persist a denial audit. CP11 now records a sanitized `session.rejected` event without the token or raw untrusted client name.
- An integrity query compared RFC3339 text to SQLite's timestamp string. It now uses SQLite `datetime(...)` normalization.
- The CP10 migration regression expected schema 11 exactly. It now verifies the CP10 migration remains present and the opened project reaches the current schema 12.
- Strict lint reported two equivalent style patterns in the MCP server; both were normalized and the gate rerun.
- Adversarial review found potential cancellation growth, artifact-limit rounding, pre-authorization Research search, denied graph-ID disclosure, permissive resource URI matching, and private Context Pack metadata exposure. Each path is now bounded or filtered and covered by protocol tests.

## Qualification Boundary

CP11 application contracts, schema, local transport, authorization, proposal safety, audit, revocation, desktop integration, offline client fixtures, and target-device overhead are complete. The fixtures prove interoperable protocol shapes, not permanent certification of future client releases. CP12 owns signed packaging, live then-current clients on supported operating systems, process supervision and termination/cancellation soak, backup/restore and corrupt-project drills, accessibility, final attack testing, and release documentation. Streamable HTTP remains disabled and cannot be enabled as an incidental CP12 change.
