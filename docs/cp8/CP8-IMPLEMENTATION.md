# CP8 Implementation

> **Status:** Complete for Visual Intelligence and Human Documentation  
> **Core crate:** `continuum-core` 0.8.0  
> **Database schema:** v9  
> **Desktop UI:** `@continuum/desktop-ui` 0.8.0

## Delivered

- `src/human_document.rs`: composition, validation, privacy filtering, source snapshots, freshness, HTML/Markdown rendering, publication, and integrity checks.
- `migrations/0009_visual_intelligence.sql`: immutable Human Document/source/export records.
- `tests/cp8_visual_acceptance.rs`: eight acceptance, integration, and adversarial scenarios.
- `examples/cp8_benchmark.rs`: target-hardware release benchmark.
- `apps/continuum-desktop`: React/TypeScript report surface, lazy React Flow + ELK graph with cycle-safe branch expansion/collapse, strict Mermaid adapter, responsive design, accessibility, and Tauri v2 core bridge.
- `docs/cp8`: architecture, traceability, acceptance, validation, and benchmark evidence.

## Public Core APIs

- compose: `compose_human_document`;
- validate/render: `validate_human_document`, `render_human_document`;
- persist/query: `save_human_document`, `get_human_document`, `list_human_documents`;
- freshness: `human_document_freshness`;
- publish: `publish_human_document`.

## Desktop Commands

- `open_project`;
- `compose_human_document`;
- `save_human_document`;
- `list_human_documents`;
- `publish_human_document`.

The shell stores only the active `ContinuityStore` handle. It does not copy canonical data into React state beyond a requested renderer-neutral projection.

## Projection Strategy

The Rust core creates one Human Document. The in-app React renderer, offline HTML renderer, and Markdown compatibility renderer consume that same semantic model. `material_fingerprint` proves that all projections came from the same material structure. Output byte hashes remain format-specific.

The initial UI path contains the concise report shell and is approximately 60 KiB gzip JavaScript. Graph and Mermaid libraries load only when their blocks enter the view. Offline export uses no runtime library and has zero network dependencies.

## Deliberately Deferred

- CP9: screen/audio/video/browser capture and specialized media previews;
- CP10: final Context Pack view and token-aware retrieval presentation;
- CP11: MCP-facing machine projection endpoints;
- CP12: installer/signing/notarization, Windows certification, extended accessibility audit, very-large graph virtualization, PDF renderer, and long-running cross-platform soak tests.

These deferrals do not weaken CP8's data, privacy, export, or rendering contracts.
