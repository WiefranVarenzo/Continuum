# Analyze and report repair — 24 September 2026

## Confirmed failures repaired

1. Actual Codex tool calls attach MCP `_meta`; Continuum rejected this valid envelope field. It now accepts object metadata while still rejecting unknown domain arguments.
2. The server advertised `research_synthesis` and `diagram_plan` but the database CHECK constraint did not permit them. Migration 13 extends the table transactionally and preserves old proposals and immutability.
3. Reviewed MCP presentation was not accepted by report source/provenance validation or persistence. Migration 14 registers external proposals as report sources; validation, freshness, and integrity checks now understand reviewed MCP provenance without inventing API-provider provenance.
4. Analyze could claim success with no proposals, or only partial results. It now verifies this run's idempotency-prefixed saved proposals and requires a synthesis and diagram for completion. Failure keeps saved drafts available for review.
5. The hidden report graph initialized with unreliable dimensions. Reports and source explorers now mount when visible; explicit measured node dimensions and change handling restore the minimap. Source search uses text, not rendered React objects.

## Workflow

Capture or paste evidence, give it an intent title and optional note, then select **Analyze & connect with Codex**. The analyzer includes bounded, privacy-filtered research context and up to eight supported images. It emits elapsed progress, opens human review after completion, and never accepts drafts automatically. Accepted notes replace placeholder summaries; synthesis and diagrams enter Reports after review.

The analyzer uses the current packaged MCP binary for that run while retaining the installed grant. Detection, configuration installation, and actual tool activity remain separate connection states. The local automatic runner is Codex; this does not implement an automatic Claude/Gemini runner or a ChatGPT-web connector.

## Reports

Reviewed narrative precedes inventories. In-app and standalone HTML reports use Mermaid with pan, zoom, fit, fullscreen, and vertical/horizontal layout controls. Default scale favors readable text; Fit provides the whole-map overview. Exported HTML bundles pinned Mermaid 11.17.2 and its license, uses hashed script authorization, makes no network requests, and keeps a static/text fallback. Evidence previews remain accessible separately from the conceptual diagram. Markdown exports include fenced Mermaid source.

## Evidence

- Real Codex execution against the existing research: two screenshots produced six pending proposals, including synthesis and diagram, in approximately 102 seconds after the storage fix.
- Earlier diagnostic execution produced four additional pending drafts before discovering the database constraint. They are retained, not silently deleted or approved.
- Review and report composition were tested on a disposable copy under `/tmp/continuum-review-qa`, leaving original proposals pending.
- Browser checks: standalone HTML light/dark rendering, real Mermaid SVGs, zoom and direction changes, React report rendering, nonzero minimap nodes; no browser exceptions in these checks.
- Regression coverage: UI tests; core report and MCP acceptance tests; MCP protocol metadata test; native tests; migration preservation test; composition/save/freshness of accepted external presentation.

## Boundaries

This is not proof that every desktop interaction is bug-free. Live OS screen/audio permissions and a full new development-only session were not manually exercised in this pass. Audio/video are not automatically transcribed: analysis uses their saved metadata/notes unless a transcript exists. Diagram output quality still requires human judgment and acceptance.

Schema 14 is newer than the prior release's schema 12. Opening a project creates pre-migration SQLite backups. Archived older binaries are for forensic reference, not for opening an upgraded live project.
