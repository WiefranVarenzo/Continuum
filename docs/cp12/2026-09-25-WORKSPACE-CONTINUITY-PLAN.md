# Workspace, memory, and Markdown-first revision

Status: primary implementation and Linux candidate packaging complete. A native launch and one authenticated Codex CLI probe passed on 2026-09-26; full in-app model, capture-device, and cross-platform pilot verification remains. See [28 September follow-through](2026-09-28-CAPTURE-MARKDOWN-HANDOFF.md) for the newer capture permission, complete-recording, and freeform MCP report changes. This is not a claim of full strategy completion or production certification. Existing project data and prior changes are preserved.

## Delivery contract

- Durable project-scoped board positions and editable presentation connections, with undo/redo.
- Capture originals before prompting for title and description; playable media previews.
- Explicit bookmarks and close-time save, alongside continuous persistence.
- Inspectable bounded memory and project-scoped Messages with source-grounded AI replies.
- Editable Markdown as the report source; safe HTML and Mermaid rendering from the same document.
- Optional Hermes agent integration; do not make Hermes a mandatory dependency or a second canonical store.
- Retain the distinction between board suggestions and accepted project facts.
- Test migrations, conflicts/recovery, report rendering, and realistic UI workflows.

## Verification and handoff

### Implemented

- Migration 15 adds project-local versioned workspace documents. Board, Markdown report, messages, bookmarks and AI result records are presentation/working state, not an alternate canonical knowledge store. Optimistic revision checks reject stale writes; immutable previous versions survive reopen and project export/restore.
- Research board supports persisted positions, connectors, editable labels and relationship reasons, line removal, hidden cards, group labels, auto layout, and a 40-step undo/redo history. Deleted AI pairs are suppressed on later analysis. AI notes and dashed connections remain explicitly unverified. Auto layout is a separate deterministic action so analysis does not silently replace a manual layout. Development sources can use the same board controls.
- Originals are saved before the title/description prompt. Clipboard images and dropped supported image/audio/video files use the existing bounded capture store. The floating capture window expands for the prompt. Media cards load audio/video only on an explicit open action; codec availability remains platform-dependent.
- Markdown-first report editor, draft generation, explicit adoption/review, saved revisions, source references, and Markdown/HTML exports. Legacy structured reports remain available separately. Mermaid is rendered locally, with pan/zoom/fit/fullscreen controls and readable colors. HTML escapes raw markup, sanitizes output, blocks remote images, and uses a script-hashed CSP for its bundled interaction code.
- Messages uses project/session-scoped history and cited context. Chat is read-only with respect to canonical facts. AI input includes a bounded history, eligible core context, user board connections/working notes and a bookmark; it never assumes every project source was included.
- Resume & memory is reachable from the workspace header, with explicit Save bookmark and an inspector for included/omitted context. Recent completed AI results retain their exact context pack and working notes. Closing waits for document saves and a checkpoint; active captures, pending title dialogs, save errors or active assistant requests prevent an unsafe close.
- Optional Hermes/Codex direct CLI assistant adapters are separate from the existing external MCP client integration. Installed CLI does not mean authenticated model access, and a configured MCP entry does not mean a live successful tool call. Outputs must pass JSON/schema/source-boundary checks before use.

### Verification recorded 2026-09-25

- Packaged candidate: `releases/0.12.0-workspace-preview-2026-09-25/Continuum_0.12.0_amd64.AppImage`, 120,424,952 bytes (approximately 114.85 MiB). SHA-256: `3015e113a6f12479a66b79b5b2c763f55bde30bf7fa4744e87674256a4684c87`. Existing `releases/0.12.0` binaries were not replaced. Native GUI launch of this candidate has not been certified in this iteration.

- UI: 44 tests across 14 files, including real React Flow mount regression, reversible board state, queued/debounced persistence and conflicts, Markdown sanitization, self-contained HTML and zoom interaction.
- Core: 3 workspace persistence tests and 2 release export/restore tests. Includes reconstruction of a disposable v14 fixture, v15 migration and pre-migration backup verification. No real user project was migrated for these tests.
- Native: assistant output validation test passed; invalid/out-of-context references rejected. Native compilation passed.
- Browser: synthetic fixture at `apps/continuum-desktop/qa/workspace.html` through the Vite server. Verified board rendering, Auto layout/Undo, AI draft display (mock response), editing and removing a relationship, undo restoration, Messages input/output (mock response), and real Mermaid rendering/zoom. This fixture never calls a provider or writes a user project.
- The visual pass found and fixed a recursive React Flow selection update that unit tests had previously missed. Toolbar occlusion and diagram contrast were also corrected.
- Browser automation blocked the fixture's blob-URL export navigation. No bypass was attempted. Export construction and interaction code were tested locally, but that is not equivalent to inspecting a native exported file in Chrome.

### Additional checks recorded 2026-09-26

- The candidate AppImage checksum passed again. It stayed running for a 15-second native startup smoke test with isolated temporary XDG config/data/cache, exited only when the smoke timeout stopped it, and printed no startup error. This did not open or migrate the user's projects and does not prove every native screen or capture device works.
- `codex login status` reported ChatGPT login. One minimal authenticated request using the same isolated read-only Codex CLI flags as the assistant adapter returned the requested JSON successfully. No project context was sent. The CLI reported 27,537 tokens used even for that short probe; do not assume a low per-request token cost. The in-app Tauri request path, image attachment, saved response, and report generation with real project data were not exercised by this probe.
- Hermes has locally listed credentials, but no real Hermes model request was made. Credential presence alone does not prove a working model or available balance.
- Corrected the AI connections page so it explains the direct in-app Codex/Hermes CLI path, the separate external MCP path, safe-default model behavior, and token usage. Frontend build and five Workspace UI tests passed after the wording change. A fresh candidate was packaged at `releases/0.12.0-workspace-preview-2026-09-26/Continuum_0.12.0_amd64.AppImage` (120,429,048 bytes), SHA-256 `9c468262ea125b02cb126249daedf3b503bbdb4ce2e9d961c4e66383904d18af`. The earlier candidate remains intact.
- The fresh candidate's checksum passed, and it stayed running for a second isolated 12-second native startup smoke with no printed startup error. Exit 124 was the expected test timeout; it was not an application crash. A full interactive native pilot with a disposable project is still outstanding.

### Honest limits and follow-up gates

- An authenticated Codex CLI probe succeeded, but real in-app requests and Hermes are NOT certified by the mock browser workflow or that probe. Test both with the intended account/model and project copy before relying on their output. Adapters use isolated/safe CLI defaults; custom user hooks, MCP configuration and model profile customizations are not inherited. Hermes is optional, not a replacement database or a guarantee of better reasoning.
- Current per-request image budget: up to 8 previews for Codex, 1 for the installed Hermes CLI, downscaled to at most 1600 × 1200. Audio/video playback does not imply AI transcription or video understanding. No new transcription pipeline was implemented here.
- Local agent/provider history and retention follow the selected agent/provider. Temporary image files are cleaned after requests; this does not promise deletion of provider records or Hermes session history.
- Capture permission/portal issues, actual system-audio recording and codec playback still need physical Linux testing. Windows/macOS remain separate build and validation gates.
- Report drafts can be freely edited; source-link checks are not a proof that every sentence is true. Explicit human review remains necessary. AI drafts and user board interpretations are not promoted to canonical facts by editing their presentation.
- Documents are bounded to 2 MiB each, with a visible save failure rather than silent truncation. Very large conversation/board histories need future paginated storage; long-running and large-project stress tests are not claimed.
- Auto layout groups spatially; current group labels are not full nested Figma-style group containers. Spatial editing and semantic relationships are intentionally separate.
- In-app assistant requests have a five-minute timeout and no user-facing cancel control yet. Completed backend responses are retained in recent AI results even if the originating report page has been left.
- Superseded 28 September: new reports now start empty. Legacy conversion is an optional explicit import, clearly labeled as not AI-generated. Existing reports are preserved, with unknown origins labeled honestly.
- After schema upgrade, old MCP binaries may reject the newer schema. Check setup/reconnect the external MCP client and restart it to use the newly bundled server. Preserve the pre-migration backup before attempting any downgrade.
- This iteration does not include a general-purpose HTTP-provider configuration wizard, multi-agent orchestration, or automatic cross-platform certification.
