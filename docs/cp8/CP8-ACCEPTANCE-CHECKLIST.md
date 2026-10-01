# CP8 Acceptance Checklist

> **Result:** PASS — 2026-09-08

- [x] Research-only, Development-only, and Connected R&D reports work independently.
- [x] Six planned Human Documentation kinds use one renderer-neutral schema.
- [x] Saved reports bind project, checkpoint/ledger position, source versions, classification, and material fingerprint.
- [x] Report blocks and citations retain source IDs and origin/status/version detail.
- [x] Source selection, graph expansion, records, blocks, citations, AI candidates, and JSON size are bounded.
- [x] Interactive graph uses React Flow and local ELK layered layout.
- [x] Graph supports pan, zoom, fit, minimap, search/filter, focus/selection, cycle-safe branch expand/collapse, and text alternative.
- [x] Portable Mermaid source is generated only from normalized structured nodes and edges.
- [x] Mermaid uses strict security and disables HTML labels.
- [x] Offline HTML executes no JavaScript and needs no server, CDN, font, script, or network.
- [x] Stored HTML, URL, and script-shaped source content is rendered inert.
- [x] Public and private-portable audience filters remove forbidden source records before composition.
- [x] Export Artifact classification inherits the highest included source class.
- [x] HTML and Markdown share one material model and fingerprint while retaining independent byte hashes.
- [x] Saved reports are immutable audit records; later changes mark them stale.
- [x] Source drift between compose and save is rejected.
- [x] Pending/stale/out-of-scope AI cannot enter a report.
- [x] Accepted fresh AI blocks retain candidate, attempt, provider, and direct-source authorship.
- [x] Tauri exposes only the bounded project/report command surface required by CP8.
- [x] In-app UI has semantic landmarks, headings, focus, responsive layout, reduced motion, and accessible tables/diagram alternatives.
- [x] Heavy graph and Mermaid code is lazy-loaded after the first useful overview.
- [x] Database migration is additive; reopen and integrity checks pass.
- [x] Rust formatter, strict linter, all core tests, UI tests, UI production build, Tauri check/lint, npm audit, and release benchmark pass.

## CP9 Entry Gate

- [x] Artifact references are first-class report blocks.
- [x] HTML/media rendering has an explicit trust and CSP boundary.
- [x] Report/project source scope can attach future capture artifacts without changing canonical authority.
- [x] Oversized media remains outside Human Document JSON and belongs in the content-addressed Artifact Store.
- [x] CP9 can add safe preview components without changing core report identity or export provenance.
