# CP12 Implementation

## Implemented

- Added release-grade project launcher with native directory selection, create/open/restore flows, last-project resume, and standalone Space selection.
- Added release center with project identity, Space state, ledger position, health diagnostics, database backup, full export, and safe close.
- Added working quick start: canonical Research Question creation and read-only Git attach/baseline/commit ingestion/code analysis.
- Hardened export/import against recursive and source-nested destinations.
- Added integrated backup/export/restore/reopen and hostile-path acceptance tests.
- Enabled Tauri bundling, application metadata, icons, and narrowly scoped dialog permission.
- Kept a restrictive CSP and dedicated Rust commands for all filesystem-affecting operations.
- Added a React StrictMode-safe one-shot last-project reopen guard.
- Reworked diagram rendering with semantic colors, legends, searchable/pannable React Flow + ELK graphs, Mermaid zoom/scroll controls, light/dark themes, textual alternatives, and reduced-motion behavior.
- Removed the large white default minimap/control treatment in dark mode.
- Pinned the official Tauri dialog packages to matching `2.7.0` versions.
- Produced versioned Linux `.deb` and `.AppImage` artifacts with SHA-256 checksums.

## No Architecture Drift

No migration was added and `CORE_SCHEMA_VERSION` remains 12. CP12 only exposes and hardens the contracts already implemented by CP2–CP11. AI remains optional and proposal-only. Streamable HTTP remains disabled.

## Source Map

- Core recovery hardening: `crates/continuum-core/src/store.rs`
- CP12 acceptance: `crates/continuum-core/tests/cp12_release_acceptance.rs`
- Integrated benchmark: `crates/continuum-core/examples/cp12_benchmark.rs`
- Native release bridge: `apps/continuum-desktop/src-tauri/src/main.rs`
- Project launcher: `apps/continuum-desktop/src/ProjectLauncher.tsx`
- Work entry: `apps/continuum-desktop/src/WorkspaceQuickStart.tsx`
- Health/recovery UI: `apps/continuum-desktop/src/ReleaseCenter.tsx`
- Diagram renderers: `apps/continuum-desktop/src/KnowledgeGraph.tsx`, `MermaidDiagram.tsx`, and `diagrams.css`
- Package config: `apps/continuum-desktop/src-tauri/tauri.conf.json`
- Fedora AppImage build shim: `packaging/pkgconfig/librsvg-2.0.pc`

## Build

From `apps/continuum-desktop`:

```bash
npm install
cargo build --release --locked --manifest-path ../../Cargo.toml -p continuum-mcp
npm test
npm run build
PKG_CONFIG_PATH="$PWD/../../packaging/pkgconfig" NO_STRIP=true npm run tauri -- build --bundles deb,appimage
```

The pkg-config override is only needed on this Fedora packaging host. Debian/Ubuntu builders with `librsvg2-dev` should use their normal system metadata.
