# CP8 Traceability

> **Status:** Complete  
> **Validated:** 2026-09-08

## Product Requirements

- FR-VIS-001 → React Flow interactive graph rendered with deterministic local ELK layered layout.
- FR-VIS-002 → bounded graph specification, default scope limits, truncation disclosure, and progressive views.
- FR-VIS-003 → pan, zoom, fit, minimap, search/filter, focus/selection, and cycle-safe branch expand/collapse.
- FR-VIS-004 → reviewed CP7 diagram candidates must be schema-valid; deterministic adapters own normalized nodes, edges, labels, and layout.
- FR-VIS-005 → Research, Development, Integrated, Architecture, Timeline, and Handover report generation.
- FR-VIS-006 → stable source citations plus visible uncertainty, omission, classification, origin, and status metadata.
- FR-VIS-007 → immutable versioned documents/exports, material fingerprint, source snapshots, and stale-state detection.
- FR-VIS-008 → semantic website-like report is the default human surface.
- FR-VIS-009 → concise overview, status cards, tables, timeline, progressive detail, and diagram navigation.
- FR-VIS-010 → self-contained HTML with zero network dependencies.
- FR-VIS-011 → versioned renderer-neutral Human Document separates composition from formats.
- FR-VIS-012 → Markdown compatibility projection from the same material model.
- FR-VIS-013 → strict normalized Mermaid for bounded portable diagrams and React Flow + ELK for interactive knowledge/traceability graphs.
- FR-VIS-014 → stored content injection and raw Mermaid/provider presentation code fail closed.
- FR-VIS-015 → semantic landmarks, keyboard controls, visible focus, zoom/reflow, accessible tables, reduced motion, and source-backed diagram alternatives.

## Non-Functional Requirements

- NFR-UX-006–008 → concise overview, progressive disclosure, semantic landmarks, ordered headings, visible focus, keyboard-capable controls, accessible tables, diagram alternatives, and reduced motion.
- SEC-024 → restrictive in-app and offline-export CSP plus no ambient report authority.
- PERF-018 → initial report shell and overview are independent of lazy graph/diagram libraries.
- PERF-019 → release benchmark p95 compose+render 371.83 ms against 5,000 ms target on target hardware.

## Acceptance Criteria

- AC-VIS-01 → standalone Research/Development and integrated fixture test.
- AC-VIS-02 → citations, source versions, classification, contribution, AI authorship, and explicit omission/uncertainty metadata.
- AC-VIS-03 → progressive overview and bounded detail preserve citations, omissions, privacy warnings, and staleness reasons.
- AC-VIS-04 → asset manifest has zero network dependencies; offline HTML contains no script.
- AC-VIS-05 → adversarial stored markup, URL, Mermaid-label, repository-content, and AI-output fixtures remain inert or fail closed.
- AC-VIS-06 → HTML and Markdown render from the same validated model/fingerprint.
- AC-VIS-07 → bounded graph supports pan, zoom, focus, filtering, cycle-safe expansion/collapse, and a source-backed text alternative.
- AC-VIS-08 → semantic structure, keyboard operation, focus, contrast, zoom/reflow, reduced motion, accessible tables, and diagram alternatives.

## Upstream and Downstream

- CP1: implements approved HTML-first, local-first, deterministic-authority architecture.
- CP2: reuses transaction, migration, Artifact Store, content hash, audit, backup, and integrity foundations.
- CP3–CP5: consumes stable Research, Development, code, test, and report source identities without rewriting them.
- CP6: consumes bounded relationship/provenance records for graph projection.
- CP7: consumes only reviewed fresh structured presentation candidates.
- CP9: receives a safe Artifact reference block and media-extension seam.
- CP10: receives stable Human Document/checkpoint/freshness semantics.
- CP11: can expose separate machine projections without scraping HTML.
- CP12: receives explicit cross-platform, accessibility, installer, and scale qualification boundaries.
