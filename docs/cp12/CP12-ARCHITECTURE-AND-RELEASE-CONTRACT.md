# CP12 — Hardening & Release Contract

> **Status:** Implemented for Linux pilot release 0.12.0  
> **Date:** 2026-09-12  
> **Architecture:** CP1–CP11 boundaries unchanged; canonical schema remains v12

## Purpose

CP12 turns the integrated Continuum system into a recoverable, inspectable desktop pilot. It introduces no new product domain and grants no new authority to AI. Research Space, Development Space, the optional R&D Bridge, deterministic canonical state, review-gated AI, local-first privacy, and bounded MCP remain exactly as approved.

## Release Surface

The desktop release owns these user-visible boundaries:

1. **Project entry:** open an existing project, create Research-only, Development-only, or Connected R&D, or restore a verified export into a new folder.
2. **Working entry:** create a canonical Research Question; attach a Git repository, observe its baseline, ingest at most 1,000 commits per action, and run bounded deterministic code intelligence without modifying Git.
3. **Continuity:** create/compare Checkpoints, inspect Then/Since/Now/Next, preview and save bounded Context Packs.
4. **Evidence capture:** explicitly initiated file, web, screenshot, screen, and audio paths governed by CP9 permissions and indicators.
5. **AI continuity:** create/revoke scoped local MCP grants and review proposals; no direct canonical model writes.
6. **Human projection:** source-backed HTML-first report, interactive React Flow + ELK graph, structured Mermaid flow, and compatibility Markdown export.
7. **Recovery:** health diagnostics, SQLite backup, complete restorable project export, identity-preserving restore, and safe close/reopen.

## Safety Invariants

- Opening and rendering never starts a sensor or external AI request.
- Project creation requires at least one Space and creates no synthetic records for the disabled Space.
- Repository synchronization invokes read-only Git inspection; it cannot commit, checkout, reset, clean, or modify repository configuration.
- New project/export/restore destinations are one safe child path and may not be nested inside their source project/export.
- Backups use SQLite's consistent backup API; full exports preserve manifest, database, artifacts, IDs, hashes, and history.
- Integrity issues are shown as issues with recovery guidance, never hidden behind a green status.
- Mermaid receives only validated node/edge data; project text is sanitized and cannot supply directives, executable markup, or renderer code.
- Heavy graph and Mermaid engines remain lazy-loaded. The initial UI bundle is approximately 75 KiB gzip; their larger chunks load only when those blocks are rendered.

## Packaging Contract

- Version: `0.12.0` in the core, MCP server, desktop Rust crate, Tauri config, and JavaScript package.
- Linux pilot artifacts: Debian package and x86-64 AppImage.
- Canonical project data is never written into an installation directory.
- Packages are unsigned local pilot artifacts. Distribution outside the pilot requires platform signing and provenance in the release pipeline.
- Windows 11 remains the first platform named by ADR-002 for formal certification. Linux packaging on the current reference machine is pilot-qualified; it does not substitute for Windows testing.

## Failure Model

- A failed create/open/restore leaves the current project unopened or unchanged.
- A failed report, graph, capture, AI, or repository analysis operation reports an error without claiming a valid result.
- A database or artifact integrity failure changes the health badge to attention and exposes deterministic guidance.
- AppImage bundling on modern Fedora uses `NO_STRIP=true` because the upstream linuxdeploy strip binary cannot parse modern RELR sections. A local pkg-config shim supplies only the already-installed librsvg runtime location.

## Exit Decision

CP12 is **PASS for the current Linux pilot scope**: integrated code, UI, regression, recovery, target-device benchmark, AppImage/deb packaging, and smoke launch pass. Formal Windows certification, signing/notarization, live provider-account conformance, live external-client matrices, and long-duration physical-device capture remain distribution gates documented in the platform/limitations record. They do not weaken local canonical correctness or the usable offline Linux pilot.
