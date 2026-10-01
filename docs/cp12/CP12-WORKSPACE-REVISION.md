# CP12 — Activity-based workspace revision

Updated: 2026-09-22. Status: workspace and one-click MCP connection revision built, packaged, and smoke-launched for local pilot use. Native interaction/device/client qualification remains limited as detailed below.

## Implemented experience

- New folder name starts empty, accepts full deletion/replacement, and uses a name-derived fallback only at submission.
- Research-only, Development-only, and Connected R&D remain explicit project capabilities. Sidebar and header identify the current mode.
- A focused research title/question form opens a session. Optional context/outcome fields use progressive disclosure and locally saved drafts.
- The main research workspace is a React Flow/ELK board with screenshot cards, source notes, pan/zoom, rearrangement, and source relationships.
- History restores the selected research. Durable sources are written immediately; changes trigger a policy checkpoint and report refresh. Abrupt exit before the checkpoint timer can leave the resume summary behind the saved source ledger; it does not roll back saved sources.
- Screenshots can be pasted/dropped, or captured from new clipboard images after the user explicitly enables Live screenshots. The OS screenshot shortcut must copy its image to the clipboard. Continuum does not register or intercept a global Shift+Super+S shortcut.
- Recent captures support notes and removal from the board. Removal archives Evidence and retains the original in history.
- Screen/system-audio/microphone controls are under Record & sources. A compact always-on-top toolbar is implemented; actual native window behavior and physical capture devices still need verification on the new package.
- HTML and Markdown human reports remain available. A dedicated Markdown Context Pack export is available in Context & memory.
- Installed Codex/ChatGPT Desktop, Claude Code, and Gemini CLI hosts are detected. The AI connections page now creates the scoped grant and installs the client entry in one click; the bearer token remains hidden. Failed installation revokes the new grant. Disconnect revokes first and then removes the saved client entry. Generic clients retain an explicit manual fallback. The packaged MCP binary is copied to an application-data path that survives AppImage unmounting.
- Organize with AI runs a connected Codex CLI job explicitly and asynchronously, in a read-only sandbox, with bounded temporary copies of up to eight screenshot artifacts. Codex uses the project-scoped Continuum MCP server to submit descriptions, Finding branches, and valid relationship proposals. Nothing enters canonical project knowledge until a human accepts it; supported accepted proposals are then materialized idempotently and the temporary image copies are removed.

## Completion checks

- `npm run build`: passed after removing an unsupported test-query option. Build warnings remain for large graph/Mermaid bundles and an ineffective dynamic import.
- `npm test`: 31 tests in 10 files passed. Includes onboarding, automatic checkpoint request, session restoration, archive action, development-only navigation, launcher input regressions, hidden-token automatic MCP setup, manual fallback, and failed-setup grant rollback.
- Desktop `cargo check --locked`: passed before the thumbnail addition. Subsequent desktop test compilation covers the updated native thumbnail code.
- `cargo test --locked --manifest-path apps/continuum-desktop/src-tauri/Cargo.toml --no-default-features workspace::tests`: two tests passed. They exercise PNG/JPEG/WebP scaling and malformed/excessive image rejection.
- Browser exercise with simulated desktop IPC: onboarding, research history, pasted screenshot assignment to the chosen session, note save, live-mode navigation lock, Markdown Context Pack export, and AI page passed without page errors. This is frontend evidence, not an end-to-end native capture/client certification.
- Thumbnail decode runs outside the UI thread, preserves originals, checks input byte/pixel limits, emits at most 640×480 PNG, and limits concurrent frontend requests to two. Simultaneous board/tray requests for one artifact share their pending result.
- Visual inspection found cropped graph controls caused by React Flow's inline full height plus the caption. A separate parent container now reserves the 46-pixel caption height; setting the React Flow style alone was ineffective because the library overrides it. The final browser check confirms Fit View stays inside the board and is clickable. At a 620×220 viewport, the compact toolbar's Open workspace button is visible and restores the workspace.

## Package evidence

Automatic approval review temporarily rejected execution because its selected model was at capacity. This infrastructure blocker recovered. The final package build completed successfully after the graph correction; it reran the production TypeScript/Vite build and optimized native compilation.

- AppImage: 117,139,960 bytes (about 111.7 MiB), SHA-256 `666a1c3707a19020f7b0afef258080cdabb487abd1af16037553668b8b02d33c`.
- Debian: 13,855,936 bytes (about 13.2 MiB), SHA-256 `c37e669c7448e1b39ea123ffe4fff9e0dfbb92bfb666efa8a3de99e371f41561`.
- Current artifacts and checksums: `releases/0.12.0/`. The immediately preceding package was preserved under `releases/0.12.0/previous-one-click-2026-09-22/`; the earlier pilot remains under `previous-pilot-2026-09-12/`.
- Both bundle trees contain `usr/lib/Continuum/continuum-mcp`. The Debian copy is byte-identical to the built server. AppImage packaging adds ELF RUNPATH `$ORIGIN`, changing the whole-file hash; the executable `.text` section still matches SHA-256 `56ed961c390faa97cae9bf08fdc97d7b1ad931a25a72ea1b1a9d5dbfbef434fe`.
- The one-click MCP release AppImage was started on the user's desktop and stayed running with no startup stderr/stdout errors during observation. This is a launch smoke check, not proof that every native command/device/client works.

## Next hands-on qualification

- Complete screen/system-audio/microphone recording on each advertised Linux desktop and portal combination. The AppImage now packages the required GStreamer elements and no longer emits the former missing-`appsink` startup error, but OS chooser selection, physical devices, audio routing, and playback still require hands-on qualification.

Exercise native project create/reopen, clipboard image intake, float/restore, report/context exports, and a real MCP client grant on the packaged application. Screen/system-audio/microphone behavior must be tested on the actual OS and devices. Current browser mocks validate the UI flow only; they do not certify those native integrations.

## Scope note

Hermes was discussed as an optional integration, but has not been added. Finishing this interrupted workspace revision does not install Hermes or change Continuum's agreed provider-neutral architecture.
