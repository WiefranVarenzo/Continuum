# CP8 — Visual Intelligence and Human Documentation Architecture

> **Status:** Implemented and validated 2026-09-08  
> **Schema:** Continuum Core v9  
> **Depends on:** CP1 presentation/privacy contracts, CP2 ledger and Artifact Store, CP3–CP5 typed records, CP6 provenance, CP7 reviewed semantic candidates

## 1. Purpose

CP8 turns verified project state into clear Human Documentation without making HTML, Markdown, SVG, or graph coordinates canonical truth. It supports Research-only, Development-only, and Connected R&D use equally.

```text
Scoped project state / compatible Checkpoint
→ deterministic source selection and privacy filtering
→ versioned renderer-neutral Human Document
→ validation, source snapshot, material fingerprint
→ in-app semantic HTML
   + lazy React Flow + ELK knowledge graph
   + lazy strict Mermaid bounded diagram
→ offline self-contained HTML and Markdown compatibility projections
→ immutable, content-addressed export Artifact
```

## 2. Authority Boundary

- Entities, relationships, source versions, checkpoints, audit events, Git observations, and Artifact hashes remain canonical.
- A Human Document is a saved immutable projection of a declared source boundary and ledger position.
- HTML, Markdown, SVG, visual layout coordinates, UI search/filter state, and render caches are derived.
- A report can be regenerated; a saved report is never silently rewritten.
- A later relevant event or changed source version marks the prior report stale instead of altering history.

## 3. Human Document Contract

Schema v1 contains:

- identity, project, report kind, audience, source Checkpoint, source ledger position, and generation time;
- aggregate privacy classification;
- omissions and limitations;
- source snapshots with kind, stable ID, version/hash, and classification;
- citations with type, title, status, origin, version, and classification;
- ordered semantic blocks;
- deterministic material fingerprint that excludes projection identity and generation timestamp.

Supported report kinds are Research Report, Development Report, Integrated R&D Report, Architecture Explanation, Timeline, and Handover.

Supported blocks are overview, status cards, heading, prose, callout, key/value facts, table, timeline, citation list, Artifact reference, bounded Mermaid specification, bounded interactive graph specification, and progressive detail group.

Every block declares source IDs and contribution metadata. Contribution is deterministic, user-authored, imported, or AI-assisted. AI-assisted blocks additionally retain candidate, attempt, and provider profile IDs.

## 4. Scope and Standalone Spaces

- Research Report selects Research entities plus optional Requirement handoff and learning feedback.
- Development Report selects repository, baseline, Requirement, ChangeSet, code intelligence, code, test, and learning feedback entities.
- Integrated, Timeline, and Handover reports may join both spaces.
- Architecture Explanation selects decisions, requirements, implementation changes, components, and validation.
- An optional compatible Checkpoint supplies an immutable bookmark scope.
- Explicit roots use a bounded four-hop relationship expansion.
- Research and Development reports remain valid and useful when the other Space is disabled or empty.

## 5. Diagram Contract

Interactive knowledge graphs receive only structured nodes and edges. The in-app renderer uses `@xyflow/react` with `org.eclipse.elk.layered` layout, pan, zoom, minimap, fit view, search/filter, focus/selection, cycle-safe branch expansion/collapse, and a textual alternative. Layout coordinates and UI expansion state are recomputed locally and are not persisted as truth.

Portable diagrams receive a structured flowchart specification. The UI maps all external IDs to local aliases and normalizes labels before generating Mermaid source. Mermaid runs locally with strict security and HTML labels disabled. Provider-authored Mermaid syntax, initialization directives, URLs, HTML, JavaScript, CSS, and event handlers are never accepted.

The self-contained offline HTML renderer does not execute JavaScript. It presents diagrams as accessible semantic node/link fallbacks so the exported report works without a server or network.

## 6. AI Boundary

CP8 does not call a model. It can consume only fresh, accepted CP7 `report_narrative` or `diagram_plan` candidates whose citations are entirely inside the report source boundary. Pending, rejected, invalid, stale, or out-of-scope candidates fail closed or are explicitly omitted. AI material never becomes canonical by appearing in a report.

## 7. Privacy and Export

- `local_project` may include all locally authorized classifications.
- `private_portable` includes public, internal, and sensitive; secret and never-send are omitted.
- `public_portable` includes only public sources.
- Filtering happens before block composition and diagram construction.
- Export classification inherits the highest included source classification.
- Offline HTML has a restrictive CSP, no forms, frames, objects, scripts, fonts, network requests, or remote assets.
- All user, repository, imported, and AI-derived text is escaped by the Rust HTML renderer.
- Published HTML and Markdown bytes enter the CP2 content-addressed Artifact Store and receive immutable export metadata.

## 8. Bounds and Failure Semantics

- source records: maximum 5,000 per request, default 1,000;
- explicit roots: maximum 50;
- accepted AI candidates: maximum 20;
- interactive graph: maximum 200 nodes and 500 edges;
- portable Mermaid: maximum 50 nodes and 100 edges;
- blocks: maximum 500;
- citations/sources: maximum 5,000;
- Human Document JSON: maximum 8 MiB.

Oversized requests fail before persistence. Bounded views declare truncation and omissions. Source drift between composition and save is a conflict. Export is generated in memory and content-addressed before a valid export row is published, so a failed render cannot create a valid-looking partial report.

## 9. Desktop Boundary

The Tauri v2 shell owns the open project and exposes a small command set: open, compose, save, list, and publish Human Documents. The React surface holds no database connection and has no arbitrary shell, provider credential, or network authority. Its CSP permits only the local app/IPC resources needed by the desktop runtime.

## 10. CP9 Handoff

CP9 may introduce captured screenshots, audio, video, browser records, and file Evidence through the existing Artifact Store. CP8 already provides an `artifact_reference` block and immutable source/export contracts. CP9 must add modality-specific preview components and accessibility alternatives without embedding untrusted media markup or weakening audience, CSP, size, and provenance rules.
