# CP12 Traceability

- PRD CP12 performance optimization/report → `cp12_benchmark.rs`, `CP12-BENCHMARK-2026-09-12.json`, Performance Budget Section 8.
- PRD threat-model closure/security testing → nested-source rejection, full CP2–CP11 adversarial regression, CSP, narrow dialog capability, npm audit, Cargo license metadata check, credential-pattern scan, Privacy and Security Section 8.
- PRD migration/backup/restore/crash/corruption → all historical migration regressions, artifact/database integrity suites, `cp12_release_acceptance.rs`, Release Center recovery actions.
- PRD packaging/onboarding/diagnostics → Project Launcher, Workspace Quick Start, Release Center, Tauri bundle config, AppImage/deb artifacts, User Guide.
- PRD accessibility → semantic forms/headings, status live regions, keyboard focus, textual diagram alternatives, reduced motion, responsive visual QA, existing CP8 stored-XSS/accessibility contract.
- PRD end-to-end/regression/upgrade → 140 Rust tests, 21 UI tests, strict lint, production build, schema-v1 through v12 migration coverage.
- AC-DATA-04/05 and AC-SEC-04 → atomic migrations, consistent backup, full export, verified identity-preserving restore/reopen.
- AC-SEC-01/02/05 → secret/grant suites and scan, safe import/export paths, cross-project MCP authorization regression.
- AC-PERF-01 → named hardware/fixture/protocol with p50/p95/max and correctness.
- AC-PERF-02 → component targets and integrated 1k/2k pilot target pass; Standard 50k/150k has an explicit non-GA exception and is not claimed.
- Section 21.1 standalone/connected flows → project mode selection, Research Question entry, Git entry, Evidence capture, scope-aware Checkpoints/Context Packs, reports, backup/export/restore.
- ADR-002 → Tauri/React/Rust local-first desktop preserved; current Linux package is pilot-qualified while formal Windows-first certification remains outstanding.
- ADR-004/006/010 → deterministic canonical authority, provider neutrality, local scoped MCP, and review-gated proposals unchanged.
