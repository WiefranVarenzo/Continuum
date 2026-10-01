# Continuum 0.12.0 — Linux Pilot Release

## Capture and assisted research-board revision — 2026-09-22

The Linux packages now carry the GStreamer elements WebKit needs for capture, including `appsink`, PipeWire, PulseAudio, conversion, and recording codecs. Permission requests time out with actionable guidance after 25 seconds instead of remaining indefinitely in “requesting”. The packaged AppImage was smoke-launched without the former `GStreamer element appsink not found` startup failure; the operating-system screen and microphone chooser still requires an explicit user click and selection.

Research Sessions, Questions, and Evidence descriptions are editable from the board. Board fallback links use “captured in” and disappear when reviewed canonical relationships exist. “Organize with AI” now runs a connected Codex CLI noninteractively in a read-only sandbox, attaches bounded screenshot copies, and asks Continuum MCP to submit review-gated descriptions, findings, and relationships. Accepted supported proposals are materialized into canonical notes and branching Finding relationships; AI still cannot write canonical knowledge without human acceptance. Temporary screenshot copies are deleted after each organizer job.

Verification for this revision: 33 UI tests, 122 Rust tests across the CP2–CP12 suites, frontend production build, native compilation, Debian/AppImage packaging, packaged-resource inspection, and AppImage smoke launch passed. The previous package remains recoverable under `previous-capture-ai-2026-09-22/`. Current package sizes are approximately 113.8 MiB AppImage and 13.7 MiB Debian.

## One-click MCP connection revision — 2026-09-22

The AI connections page now installs detected Codex/ChatGPT Desktop, Claude Code, and Gemini CLI MCP entries directly. Continuum still uses a project-scoped, expiring, revocable secret, but creates and installs it behind the scenes instead of asking the user to copy a token. A failed installation revokes its newly created grant. Disconnect revokes authority first and then cleans up the client entry. Protocol-generic clients retain the explicit manual fallback.

Verification for this revision: 31 UI tests, two desktop native tests, the production frontend build, native release compilation, Debian packaging, and AppImage packaging passed. The previous package remains recoverable under `previous-one-click-2026-09-22/`.

Current package sizes: approximately 111.7 MiB AppImage and 13.2 MiB Debian. Checksums are in `SHA256SUMS`.

## Workspace revision — 2026-09-15

This historical revision introduced the activity-based workspace: clear mode indicators, focused research onboarding, screenshot board/tray, notes, opt-in live clipboard intake, compact toolbar, research history, automatic resume checkpoint requests, Markdown Context Pack export, and MCP client setup suggestions. Folder-name editing was fixed; bounded thumbnails and the graph Fit View layout were included.

Verification for that historical revision: 29 UI tests, two native thumbnail tests, production UI/native builds, and browser interaction checks with simulated desktop IPC passed. At that point Organize with AI only copied a guided prompt; the newer revision above replaces that behavior. Hermes has not been integrated.

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
