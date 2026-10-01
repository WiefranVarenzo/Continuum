# Capture and freeform Markdown follow-through — 28 September 2026

## Scope and status

Continuation of the interrupted 27 September changes. Existing user projects, release candidates, and unrelated work were preserved. No schema migration was added. This is a testable Linux candidate, not a claim that every research/development workflow or capture device has been certified.

## Implemented

- Screen sharing is requested directly in the initiating click, before persistence/IPC awaits. Screenshot capture follows the same rule. Web Audio is unlocked during that click when a mixer is needed.
- WebKitGTK media permission requests now have an explicit Allow/Cancel dialog for the local app origin only. Screen selection remains the desktop portal's responsibility; camera requests are not enabled. Unanswered app prompts expire. A cancelled or timed-out frontend request releases any late media stream.
- Linux system audio can use the current output monitor through the installed PulseAudio/PipeWire client `parec`, independently of portal audio support. Only an explicit, consented recording starts it. Packets are bounded to 50 ms with acknowledgement, startup/stall timeouts and cleanup on window destruction. Microphone and desktop audio are mixed to one audio track.
- Recording fragments are assembled in sequence before becoming playable workspace Evidence. Assembly rejects incomplete/inconsistent sequences and does not lower restricted-source privacy. Originals remain unchanged. A title and optional description are collected; duplicate adoption returns the existing Evidence. Current assembled playback limit: 64 MiB. Longer originals remain saved but are not automatically assembled.
- Recent recordings are accessible after reopening (latest 25 capture sessions). Partial/interrupted recordings retain their fragments; they are not falsely advertised as complete playable videos. Failed persistence does not produce a success message.
- Reports start with empty Markdown, not an automatically converted legacy template. Existing saved reports are preserved. The legacy structured report is an explicitly selected import/reference only.
- A connected MCP client can submit a `research_synthesis` proposal whose payload includes `format: markdown`, the complete `markdown`, `research_session_id`, and `source_sequence`. Reports lists matching, valid, pending/accepted drafts and provides an explicit preview/adoption step. Local Codex/Hermes generation remains a separate route. Neither route silently replaces the current report.
- Report provenance identifies manual/local-AI/MCP/legacy content; older reports with unknown origin say so. Completed local report results can be recovered as drafts. AI context omissions/image counts are shown for newly generated local drafts.
- Mermaid reports retain readable HTML rendering and zoom/pan/fit/fullscreen controls. Resize observation fixes diagrams that stayed tiny after their container became visible or changed size; deliberate manual zoom is preserved.

## Verification

- Frontend suite: 53 tests passed across 17 files before the final resize regression; the final targeted set passed 13 tests including the additional resize test. TypeScript and production frontend build passed before the final diagram patch; final packaging reruns the build.
- Core capture acceptance: all 11 tests passed, including ordered assembly, original-byte preservation, retry identity and restricted-source protection. Fixtures use disposable projects, not the user's project.
- Core MCP Markdown integration: a real permissioned proposal submission in a disposable store preserved freeform Markdown/Mermaid, left canonical state unchanged, and became a report only after explicit local adoption. No model provider was called for this test.
- Native desktop tests: 6 passed, 1 intentionally ignored real-provider organizer test. Includes local-origin permission gating and AI output reference validation.
- Fedora live audio-source probe received exactly 9,600 bytes from `@DEFAULT_MONITOR@` at 48 kHz stereo s16le. Bytes were counted only, never saved or uploaded. The `Broken pipe` diagnostic was expected when the bounded reader closed after one packet. This proves the local audio source is available, not that the complete WebView recording has been certified.
- Browser inspection used only the synthetic workspace fixture. Markdown prose, table, actual Mermaid rendering and visible zoom controls were inspected. This inspection discovered the resize defect and led to its regression fix.

## Remaining manual gates and limits

1. In the new AppImage, use a disposable Research project. Select Screen only, grant the app prompt, select a screen/window in the desktop chooser, record 10 seconds, stop, name it and add it to the workspace. Confirm playback.
2. Repeat for microphone only, system audio only, and Screen + Microphone + System audio. Confirm the sound and video are actually present, and that devices stop after Stop/Cancel. Native UI automation is unavailable in this environment; these permission/device interactions must be performed by the user.
3. Screen permissions remain controlled by the OS. A code/test pass is not proof the portal granted access. Denial, unavailable devices and timeouts now have distinct recovery guidance.
4. `parec` must be installed for the Linux system-audio fallback. It is present on the tested Fedora host; other distributions/OSes remain separate verification targets. No desktop-wide permissions or audio defaults were changed.
5. Audio/video playback does not mean AI has transcribed or watched it. The assistant receives titles/notes and eligible metadata; no automatic transcription pipeline was added. Images still use the prior bounded preview policy.
6. MCP is a client-driven route: the user asks the connected AI to submit a draft, then refreshes incoming drafts. A configured connection alone does not invoke a model. The in-app Generate button uses the selected direct CLI adapter, not that external MCP connection.
7. Actual authenticated model calls were not repeated in this iteration. The prior Codex probe succeeded but reported high token usage. Full in-app model/session and Hermes verification remain outstanding; no automatic token-saving claim is made.
8. Report source links and schema checks do not prove every assertion true. Human review is still required. All AI interpretations remain drafts until the user chooses what to adopt.

## Release

The new candidate is saved separately under `releases/0.12.0-workspace-preview-2026-09-28/`; earlier stable/Sep25/Sep26 builds were not replaced. AppImage size: 120,531,448 bytes. SHA-256: `56d9ddf2a1c3ba1e7a8c1acf58ead8e7ee68aea922744e048df43e032ab0a992`. The copied artifact passed checksum verification. Final packaging reran TypeScript and frontend production compilation successfully. The AppImage stayed alive for a 12-second startup smoke using isolated temporary XDG directories and printed no error; exit 124 is the expected timeout. No private project was opened. This is not a full interactive capture/model certification.
