# CP9 Implementation

> **Status:** Complete  
> **Build:** `continuum-core` and desktop `0.9.0`; schema v10

## Delivered

- Forward-only `0010_research_capture.sql` migration with sessions, source/permission history, segments, markers, Evidence links, external captures, derivations, indexes, constraints, and immutable-record triggers.
- Typed Rust capture model, validation, state machine, idempotency, optimistic concurrency, pagination, integrity diagnostics, and replaceable `CaptureBackend` port.
- Content-addressed segmented persistence with SHA-256/size verification and no complete-recording memory buffer.
- Recovery for partial output and permission-revocation interruption.
- CP3 Evidence promotion for segment ranges and markers without whole-recording duplication.
- File, screenshot, browser, and web Evidence ingestion with active Research Session attachment.
- Source-preserving OCR/transcription/thumbnail/waveform derivation contract; execution disabled by default.
- Tauri command facade with no raw SQLite, shell, credential, or arbitrary filesystem command.
- React capture panel with runtime capability checks, explicit consent, persistent indicator, pause/resume/stop, automatic pressure pause, bookmark, screenshot, file, web/browser source, and active Research Session selection.
- Runtime format negotiation and per-source OS/WebView permission observations.

## Important Failure Semantics

- denied/unavailable permission leaves a blocked, auditable session;
- revocation during capture interrupts atomically and clears the indicator;
- recorder/backend/persistence failure creates a typed failed state with a bounded message;
- unexpected source end produces an interrupted recoverable state;
- a fragment above the policy bound is rejected before Artifact persistence;
- reused idempotency keys with changed bytes/metadata are conflicts;
- denied or failed sensor acquisition creates no screenshot Evidence; if a process ends after Evidence persistence but before its capture-link row, integrity diagnostics identify the incomplete publication and an idempotent retry completes it;
- terminal sessions cannot silently restart or mutate permissions.

## Platform Boundary

The shipped adapter is capability-detected and OS/WebView-mediated. CP9 validates domain, persistence, UI, consent, failure, and migration behavior on Linux. CP12 owns the release matrix for Windows 11 and other platforms, live screen/system-audio/microphone devices, codec availability, long-duration recording, sleep/resume, hot-unplug, signed packaging, and any required native fallback adapter.
