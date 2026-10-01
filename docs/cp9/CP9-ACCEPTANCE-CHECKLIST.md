# CP9 Acceptance Checklist

> **Result:** PASS — 2026-09-09

- [x] Research-only operation requires no repository, Development Space, AI provider, or MCP client.
- [x] Screen, system audio, and microphone are independently declared and permission-observed.
- [x] No sensor API runs on render, project open, report generation, or background work.
- [x] Lawful-capture acknowledgement and persistent indicator are mandatory.
- [x] Pause, resume, stop, interruption, failure, cancellation, denial, unavailable, and revocation have explicit state semantics.
- [x] Fragment sequence, offsets, bytes, MIME, source set, idempotency, and bounds are validated.
- [x] Partial fragment and interrupted session remain recoverable after reopen.
- [x] Screenshot permission is audited through a one-shot capture session.
- [x] File/web/browser/screenshot material creates typed CP3 Evidence.
- [x] Active Research Session association is selectable and validated.
- [x] Markers and segment ranges become Evidence without copying the full recording.
- [x] OCR/transcription derivations preserve originals and are off by default.
- [x] Capture payload is absent from audit/failure metadata.
- [x] Schema v9→v10 migration and integrity diagnostics pass.
- [x] Core acceptance, UI tests, production build, Tauri compile/lint, and target-hardware benchmark pass.
- [x] CP10 handoff defines how active/interrupted capture and selected Evidence enter bookmarks/context.

## CP12 Qualification Items (not reopened CP9 contracts)

- [ ] Windows 11 live source/codec conformance and signed installer.
- [ ] Other supported OS/device matrices, hot-unplug, sleep/resume, and permission-revocation smoke tests.
- [ ] Long-duration capture soak, disk-full behavior, process-group RSS, and recovery-media playback matrix.
- [ ] Native fallback adapter only if the release WebView cannot satisfy a declared platform/source combination.
