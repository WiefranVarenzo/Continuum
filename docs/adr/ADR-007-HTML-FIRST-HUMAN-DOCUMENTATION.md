# ADR-007 — HTML-First Human Documentation

> **Status:** Accepted
>
> **Accepted:** 2026-09-04
>
> **Owners:** CP1 architecture amendment; CP8 implementation

## Context

Continuum already separates canonical project state from generated reports and assigns styled visualization and reporting to CP8. CP3–CP5 intentionally established deterministic, cited Markdown reports before a desktop presentation layer existed. That sequence proved report content and provenance, but raw Markdown is not the intended primary reading experience for end users.

Human documentation must be concise at first glance, source-backed, comfortable to read, and capable of presenting tables, timelines, media references, and interactive diagrams. It must also remain local-first, exportable, safe when project content is untrusted, and reproducible from declared project state.

## Decision

Continuum adopts **HTML-first Human Documentation**:

1. The primary in-product Research Report, Development Documentation, Integrated R&D Report, architecture explanation, timeline, and handover experience is rendered as HTML/CSS inside the Tauri/React desktop application.
2. A renderer-neutral, versioned **Human Document Model** is composed from deterministic, project-scoped source bundles before presentation. HTML is a projection of that model and never canonical project truth.
3. The default portable human export is a local, self-contained HTML report or a verified local bundle with an entry HTML file and content-hashed assets. It must not require a Continuum server, network connection, CDN, remote font, or remote script to open.
4. Markdown remains a supported compatibility, audit, and portable-text renderer. Existing CP3–CP5 Markdown reports remain valid deterministic evidence and are not removed or reinterpreted.
5. Progressive disclosure is mandatory. The first view emphasizes current state, key findings/decisions, blockers, validation, and next actions; complete citations and details remain reachable through expandable sections or detail views.
6. Mermaid is the preferred renderer for bounded, portable flow, sequence, state, and simple relationship diagrams. React Flow with ELK remains the renderer for interactive knowledge graphs and complex diagrams that require pan, zoom, focus, filtering, and incremental expansion.
7. AI providers may propose summaries, labels, narrative blocks, or diagram plans only through versioned schemas. They may not emit executable HTML, CSS, JavaScript, Mermaid directives, URLs, or event handlers that bypass deterministic validation and rendering.
8. Machine-facing continuity remains separate. Context Packs and MCP resources expose bounded structured data, not the full HTML document, unless a user explicitly requests a report artifact.

## Human Document Model

Every saved document declares at least:

- document ID, schema version, project ID, kind, audience, and scope;
- source checkpoint or ledger position and generation time;
- deterministic and AI contribution metadata;
- freshness/staleness state;
- privacy classification and omission/limitation summary;
- ordered semantic blocks and stable entity citations;
- diagram specifications and referenced artifact IDs;
- renderer, template, and asset-manifest versions for a saved export.

Supported block families are bounded and allowlisted: summary, status cards, prose, callout, table, timeline, citation list, artifact reference, Mermaid specification, and scoped graph specification. A renderer may choose a compact presentation, but it may not discard source citations or hide uncertainty.

## Trust and Safety

- All project-controlled and imported text is escaped by default.
- Raw HTML from notes, repositories, Evidence, imports, or model output is inert text unless an explicitly trusted and sanitized format is introduced by a later ADR.
- Links and embedded resources use allowlisted schemes and project-scoped artifact resolution.
- In-product rendering and exported reports use a restrictive Content Security Policy and no ambient filesystem, shell, database, credential, provider, or network authority.
- Interactive behavior comes only from pinned Continuum renderer code operating on validated data.
- Report generation evaluates transitive privacy and export policy before including text, metadata, previews, diagrams, or referenced artifacts.

## Compatibility and Migration

This decision does not require a CP2–CP5 schema migration and does not invalidate existing report APIs:

- CP3–CP5 deterministic Markdown strings remain supported outputs and golden-test evidence.
- CP8 introduces the Human Document Model and HTML renderer behind replaceable report-renderer ports.
- Existing Markdown report composition is incrementally adapted into structured blocks; compatibility rendering is retained until an explicit deprecation decision.
- Saved HTML is a versioned GeneratedArtifact. It can become stale, while canonical entities and relationships remain authoritative.

## Alternatives Considered

### Markdown as the primary human experience

Rejected as the default presentation. It is portable and reviewable but does not provide the desired compact dashboard, accessible interaction, rich tables, or interactive graph navigation without another presentation layer.

### Arbitrary AI-generated HTML

Rejected. It is difficult to reproduce, validate, secure, theme, and keep accessible. Structured proposals plus deterministic rendering preserve flexibility without granting model output executable authority.

### One diagram renderer for every diagram

Rejected. Mermaid is effective for small portable diagrams; React Flow and ELK better serve large interactive graph exploration. Both consume validated application-owned schemas.

### Hosted report website

Rejected for the MVP. Canonical operation and report viewing must remain local-first and offline. A future sharing service would require a separate product, privacy, authentication, and deployment decision.

## Consequences

Positive:

- users receive a concise, visually coherent report rather than a wall of Markdown;
- full evidence, provenance, and uncertainty remain inspectable;
- reports work inside the desktop app and as offline exports;
- renderer choice does not change domain or storage contracts;
- AI and MCP token budgets are protected from presentation markup.

Costs:

- CP8 must implement a document schema, component library, renderer adapters, accessibility, sanitization, export packaging, and visual regression tests;
- interactive exports require pinned local assets and stricter security testing;
- Markdown and HTML renderers require parity tests for facts, citations, omissions, and freshness.

## Checkpoint Ownership

- **CP1 amendment:** lock this decision, architecture, security, performance, traceability, and acceptance contracts.
- **CP2:** preserve sufficient GeneratedArtifact metadata and safe artifact/export boundaries; no HTML renderer is required.
- **CP3–CP5:** retain deterministic report content and explicitly hand structured presentation to CP8.
- **CP6:** provide bounded provenance and graph queries suitable for visual composition.
- **CP7:** provide schema-valid optional narrative and diagram proposals.
- **CP8:** implement and validate the HTML-first Human Documentation experience.
- **CP9:** provide privacy-classified media previews and references.
- **CP10:** keep machine Context Packs separate from human HTML presentation.
- **CP11:** expose structured continuity through MCP; report artifacts are opt-in resources.
- **CP12:** certify security, accessibility, portability, performance, packaging, and recovery.

## Acceptance

The decision is satisfied when a user can open and export a source-backed HTML report, understand current state without reading a long transcript, expand full details and citations, navigate supported diagrams, work offline, and verify that the same source bundle produces semantically equivalent facts in compatibility Markdown output.
