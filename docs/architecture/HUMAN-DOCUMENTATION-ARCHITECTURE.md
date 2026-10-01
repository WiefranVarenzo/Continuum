# Continuum Human Documentation Architecture

> **Status:** Approved CP1 amendment; implemented and validated in CP8 on 2026-09-08
>
> **Decision:** [ADR-007 — HTML-First Human Documentation](../adr/ADR-007-HTML-FIRST-HUMAN-DOCUMENTATION.md)

## 1. Purpose

Human Documentation turns canonical project knowledge into a concise, inspectable, website-like report without making presentation files authoritative. The experience covers Research Reports, Development Documentation, Integrated R&D Reports, architecture explanations, timelines, and handover views.

The default human surface is HTML rendered inside the desktop application. A local self-contained HTML export provides the same semantic report outside Continuum. Markdown remains a compatibility and portable-text projection.

## 2. Architectural Boundary

    SQLite Project Ledger + Artifact Store + Git observations
                             |
                             v
                  deterministic scoped queries
                             |
                             v
                     Report Source Bundle
                             |
                  optional schema-bound AI
                  narrative/diagram proposals
                             |
                             v
                 Human Document Model (versioned)
                         /              \
                        v                v
              HTML Report Renderer   Markdown Compatibility Renderer
                    /       \
                   v         v
          Desktop Report View   Offline HTML Export

Canonical entities, relationships, events, and artifacts remain the source of truth. Source bundles, layouts, render caches, HTML, Markdown, and PDF are projections or GeneratedArtifacts.

## 3. Human Document Model

The renderer-neutral model contains:

- schema version;
- document ID, project ID, kind, title, audience, and scope;
- source checkpoint ID and/or source ledger sequence;
- generation time, freshness, and staleness reasons;
- contribution metadata for deterministic and AI-authored blocks;
- privacy classification, exclusions, omissions, uncertainty, and limitations;
- ordered semantic blocks;
- stable entity citations and artifact references;
- renderer, template, and asset versions when saved.

Allowlisted block kinds:

- overview and status-card group;
- heading and bounded prose;
- callout, warning, uncertainty, and blocker;
- key/value facts;
- paginated/filterable table;
- deterministic timeline;
- citation and provenance list;
- artifact/media reference;
- Mermaid diagram specification;
- scoped React Flow graph specification;
- expandable detail group.

Unknown major schema versions fail closed. Unknown additive minor fields may be ignored when documented. All collections and text fields have explicit limits.

## 4. Information Hierarchy

Every default report uses three layers:

1. **Overview:** checkpoint, current status, concise summary, key findings/decisions, validation, blockers, and next actions.
2. **Supporting knowledge:** requirements, ChangeSets, tests, timelines, tables, and focused diagrams.
3. **Verification detail:** complete citations, provenance paths, limitations, source metadata, and referenced artifacts.

Compact presentation never means deleting evidence. Information omitted from the initial view remains discoverable, and any exclusion caused by scope, privacy, availability, or budget is declared.

## 5. Rendering Responsibilities

### Desktop HTML renderer

The React UI renders semantic components with application themes, keyboard navigation, focus management, responsive layout, and accessible labels. Large tables and graphs paginate, virtualize, cluster, or expand on demand.

### Offline HTML exporter

The export is either one self-contained HTML file or a verified directory/archive containing:

- one entry HTML file;
- local content-hashed CSS/JavaScript assets;
- an asset manifest with hashes and versions;
- optionally included privacy-approved artifacts;
- no required remote runtime dependency.

Opening the report must not grant access to the project database, arbitrary local files, credentials, provider tokens, shell, or network. Links to omitted artifacts are visibly unavailable rather than broken silently.

### Markdown compatibility renderer

Markdown preserves the same material facts, headings, citations, uncertainty, omissions, and source boundary in a portable text form. It need not reproduce interactive behavior or visual layout.

### Diagram renderers

- Mermaid handles bounded flow, sequence, state, timeline, and simple relationship diagrams.
- React Flow plus ELK handles interactive knowledge graphs and complex traceability views.
- Diagram input is application-owned structured data. User, repository, Evidence, and model text is escaped and treated as labels, never executable syntax.

## 6. AI and Deterministic Authority

Deterministic application code owns:

- source selection and scope enforcement;
- privacy and classification decisions;
- report schema and limits;
- citation/source-ID validation;
- block construction for canonical facts;
- HTML escaping, URL policy, component selection, and layout constraints;
- freshness calculation and GeneratedArtifact metadata;
- final rendering and export integrity.

AI may propose:

- concise summaries;
- narrative ordering;
- human-friendly labels;
- contradiction/uncertainty descriptions;
- diagram plans within an allowlisted schema.

AI output remains identifiable and cannot introduce arbitrary markup, scripts, styles, network locations, citations, or canonical mutations.

## 7. Security Contract

- Escape all untrusted text by default.
- Sanitize any supported rich-text subset using an allowlist.
- Reject inline event handlers, arbitrary script, executable URLs, iframes, plugins, forms, automatic navigation, and uncontrolled external resources.
- Apply restrictive Content Security Policy to desktop and exported reports.
- Resolve project artifacts through scoped IDs, never caller-supplied unrestricted paths.
- Verify export asset hashes before publication and on supported import/restore paths.
- Apply privacy policy transitively to prose, tables, diagrams, metadata, thumbnails, and artifacts.
- Display classification and omission warnings in the report.
- Treat Mermaid and graph labels as data; renderer configuration is pinned by Continuum.

## 8. Accessibility and Usability

- semantic heading order and landmarks;
- complete keyboard operation;
- visible focus and skip navigation;
- no status conveyed by color alone;
- comfortable light/dark palettes with tested contrast;
- text reflow and zoom without information loss;
- accessible table headers/captions and diagram alternatives;
- reduced-motion support;
- printable summary and detail sections;
- source and AI-authorship indicators understandable without tooltips alone.

## 9. Freshness, Reproducibility, and Saving

A saved HTML report is a GeneratedArtifact bound to a checkpoint or ledger position, Human Document schema, renderer/template version, source IDs, and content/asset hashes. Later relevant events mark it stale without modifying the saved bytes.

Reproduction means that the same declared inputs and pinned deterministic renderer produce semantically equivalent facts, citations, omissions, and diagram topology. Pixel identity across operating systems is not required.

## 10. Performance and Degradation

- Render the overview before off-screen details.
- Paginate or virtualize large tables and timelines.
- Scope graph views before layout; never render the entire project by default.
- Compute expensive layout outside the UI thread and cache it with explicit limits.
- Lazy-load privacy-approved local media previews.
- Cancel report composition/export safely without creating a valid-looking partial artifact.
- If an interactive renderer fails, preserve a textual summary, citations, and explicit limitation.

## 11. CP3–CP5 Compatibility

CP3–CP5 report implementations remain valid deterministic content foundations. They are not silently changed to emit unsafe HTML.

CP8 introduces the structured composition layer and adapts existing report sections without changing their canonical sources:

    existing deterministic report query
                  |
                  +--> compatibility Markdown
                  |
                  +--> structured Human Document blocks --> safe HTML

Parity tests compare material facts, source IDs, status, uncertainty, and omissions across renderers. No CP3–CP5 canonical entity or database migration is required solely by this presentation decision.

## 12. Verification

CP8 and CP12 must cover:

- golden Human Document fixtures for Research-only, Development-only, and Connected R&D;
- renderer contract and schema-version tests;
- citation/freshness/omission parity across HTML and Markdown;
- script, URL, markup, Mermaid-label, and stored-XSS adversarial fixtures;
- Content Security Policy and zero-required-network tests;
- keyboard, screen-reader semantics, contrast, zoom/reflow, and reduced-motion checks;
- graph/table scale, first-useful-render, memory, cancellation, and export-size benchmarks;
- deterministic export manifest/hash validation;
- browser-engine and supported-platform visual regression tests;
- graceful fallback when diagrams, media, or optional AI output are unavailable.

CP9 implements privacy-classified capture references plus an 8 MiB hash-verified local preview boundary for captured fragments. Larger or unsupported media remains an Artifact reference with explicit fallback rather than being loaded eagerly. Release-platform playback/codec coverage remains CP12.
