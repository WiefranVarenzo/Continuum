# Continuum 0.12.0 — Linux Pilot Release

## MCP proof, meaningful research maps, and readable reports — 2026-09-22

Continuum now distinguishes three separate AI-connection facts in the interface: whether an AI client is detected, whether this project's private MCP entry is installed, and whether a real MCP tool call has reached Continuum. A permission record alone is no longer presented as proof of a working connection. The current Second Brain audit exposed one stale Codex grant whose project MCP entry was missing and whose `last_used_at` remained empty; the user must disconnect that stale entry once and reconnect it before running the assisted organizer. Automatic setup continues to keep the scoped secret private.

The research workspace now presents screenshot interpretation as the primary text, keeps the capture timestamp as secondary metadata, and uses typed relationship labels such as “defines question”, “captures evidence”, and “supports finding”. “Analyze & connect with Codex” attaches bounded screenshot copies, asks Codex to submit review-gated descriptions and branching Findings through the named Continuum MCP server, and requires human acceptance before canonical knowledge changes.

The report experience has been rewritten around human-readable sections and navigation. The main map focuses on the project-level path from Question → Evidence → Finding → Decision → Requirement/ChangeSet/Checkpoint rather than flooding the page with low-level code symbols. Evidence thumbnails remain small provenance cues; descriptions and semantic relationships are primary. React Flow controls and the minimap now remain visible in the dark interface. Exported HTML includes a table of contents, readable research details, compact source references, and a standalone evidence-to-decision map.

The capture page now reports whether the packaged GStreamer scanner, PipeWire socket, and desktop portal session are available. The AppImage contains the scanner plus appsink, PipeWire, and PulseAudio plugins. A 20-second packaged smoke launch from the Second Brain project produced no GStreamer or PipeWire startup warning; a real screen/audio chooser still needs the user's explicit operating-system selection.

Verification for this revision: 33/33 UI tests, 141/141 Rust tests, TypeScript checks, frontend production build, native compilation, Debian/AppImage packaging, packaged-resource inspection, and packaged smoke launch passed. The immediately preceding packages remain recoverable under `previous-report-mcp-2026-09-22/`.

## Knowledge graph, real report export, and capture-runtime revision — 2026-09-22

The report graph now projects real Research Session membership as explicit, truth-preserving relationships such as “defines question” and “captures evidence”; it no longer presents related records as four disconnected boxes merely because a reviewed semantic relationship has not yet been created. Local screenshot Evidence carries a bounded, verified thumbnail into the interactive graph and the offline HTML relationship map. Node type, state, source identity, relationship labels, and a textual alternative remain available so the visualization is still auditable rather than decorative.

HTML and Markdown publication now creates a normal named file under the active project's `exports/reports/` directory in addition to the immutable content-addressed artifact. HTML exports open in the system browser as rendered pages; the app only opens canonicalized `.html` or `.md` files inside that project's export directory.

The AppImage now bundles and configures `gst-plugin-scanner` as well as its capture plugins. Package inspection confirmed that the scanner is executable inside the AppImage, and a 20-second packaged smoke launch from the Second Brain project completed with no GStreamer scanner or PipeWire warning. The operating-system chooser still requires the user's explicit selection for a real screen/microphone capture.

Verification for this revision: 33 UI tests, 141 Rust tests across the core, CP2–CP12, and MCP protocol suites, frontend production build, native release build, focused screenshot-graph regression, Debian/AppImage packaging, packaged-resource inspection, and packaged smoke launch passed. The immediately preceding packages remain recoverable under `previous-graph-export-2026-09-22/`. Current package sizes are approximately 113.8 MiB AppImage and 13.7 MiB Debian.

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
