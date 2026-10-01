# Continuum 0.12.0 — Linux Pilot Release

## Workspace revision — 2026-09-15

The current packages include the activity-based workspace: clear mode indicators, focused research onboarding, screenshot board/tray, notes, opt-in live clipboard intake, compact toolbar, research history, automatic resume checkpoint requests, Markdown Context Pack export, and MCP client setup suggestions. Folder-name editing is fixed; bounded thumbnails and the graph Fit View layout are included.

Verification for this revision: 29 UI tests, two native thumbnail tests, production UI/native builds, and browser interaction checks with simulated desktop IPC passed. The final AppImage was smoke-launched. Actual native capture devices and external clients still need hands-on qualification. Organize with AI copies a guided prompt for an external client; it does not autonomously run analysis. Hermes has not been integrated.

Current package sizes: approximately 112.4 MiB AppImage and 13.2 MiB Debian. Checksums are in `SHA256SUMS`. The previous pilot packages remain recoverable under `previous-pilot-2026-09-12/`. Earlier baseline evidence below is historical and was not all rerun for this revision.

Continuum 0.12.0 closes CP12 for local Linux pilot use. A user can create/open/restore modular projects, begin Research with a canonical question, connect and analyze a Git repository without modifying it, capture Evidence, create Checkpoints and Context Packs, connect scoped MCP clients, review AI proposals, read HTML-first reports, inspect semantic-color graphs/flows, run health diagnostics, and create verified backups/exports.

## Highlights

- Complete CP1–CP12 deterministic core and schema v12.
- Research-only, Development-only, and Connected R&D project setup.
- AppImage and Debian packages.
- Project health, backup, full export, and verified restore.
- Semantic-color React Flow + ELK and Mermaid diagrams with zoom, pan/scroll, legend, dark mode, and text alternatives.
- 140 Rust tests, 21 UI tests, strict Rust lint, npm audit with zero known vulnerabilities, credential-pattern scan with no findings, and clean diff validation.
- Integrated reference-device report/render p95 1.756 s; integrity diagnostics p95 0.240 s.

## Compatibility and Safety

- No schema migration beyond CP11; existing schema-v1 through schema-v12 upgrade/regression coverage remains active.
- External AI is optional, privacy-gated, scoped, revocable, and proposal-only.
- Streamable HTTP remains disabled.
- The Linux packages are unsigned pilot artifacts. See the platform/known-limitations record before redistribution.
