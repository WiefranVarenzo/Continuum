# CP9 Traceability

> **Status:** Complete  
> **Validated:** 2026-09-09

## Product Requirements

- FR-CAP-001 → typed multi-source session, capability snapshot, OS/WebView screen/system-audio/microphone acquisition.
- FR-CAP-002 → one-shot screenshot session and timestamped markers.
- FR-CAP-003 → file/browser/web Evidence plus selectable active Research Session.
- FR-CAP-004 → source, permission history, backend/encoding/buffer settings, timestamps, failure code/message, recovery state, and audit events.
- FR-CAP-005 → immutable fragments, explicit recoverable partial state, restart/reopen acceptance fixture, and interruption lifecycle.
- FR-CAP-006 → 16 MiB buffer, 32 MiB desktop fragment, four-pending-fragment queue, early flush, pause/reduction policy, and core absolute guard.
- FR-CAP-007 → persistent request/capture/pause/finalization indicator tied to core indicator ID.
- FR-CAP-008 → sensor APIs only inside user click handlers, per-source OS/WebView observation, unavailable/denied fail closed.
- FR-CAP-009 → marker/range Evidence references existing capture Artifact; no complete-recording copy.
- FR-CAP-010 → source-preserving typed derivation and identical-byte rejection; OCR/transcription off by default.

## Security and Performance

- SEC-013 → capture and imported text remain inert untrusted data.
- SEC-015 → explicit initiation plus persistent indicator.
- SEC-016 → visible pause/resume/stop and source-ended interruption.
- SEC-017 → explicit lawful-capture/participant-consent acknowledgement.
- PERF-013–015 → bounded queue/fragments, pressure action, streaming Artifact Store, no unbounded media accumulation.
- NFR-REL-001/002 → transactional metadata, idempotency, optimistic state version, crash/reopen and migration fixtures.
- NFR-OBS-001 → sanitized event/failure metadata excludes capture payload.

## Acceptance Criteria

- AC-CAP-01 → screen/system-audio/microphone combinations use declared capabilities and explicit grants.
- AC-CAP-02 → screen selection never grants missing system audio; denial/unavailable are durable states.
- AC-CAP-03 → indicator is required for active capture and cleared at terminal/interruption state.
- AC-CAP-04 → pause/resume/stop/revocation/failure transitions reject stale/invalid state.
- AC-CAP-05 → unique ordered fragments persist; partial fragment survives reopen as recoverable.
- AC-CAP-06 → queue/fragment/page/JSON/time/encoding bounds fail closed.
- AC-CAP-07 → marker and segment Evidence preserve source references without media duplication.
- AC-CAP-08 → screenshot/file/web/browser create correctly typed Evidence.
- AC-CAP-09 → derivation cannot replace or erase source.
- AC-CAP-10 → v9 project migrates atomically to v10 and final integrity scan is healthy.
- AC-CAP-11 → UI build/test proves no sensor activation on render and pressure behavior is deterministic.

## Upstream and Downstream

- CP1: resolves D-024 via ADR-008 without changing local-first/modular architecture; confirms D-025 remains off by default.
- CP2: reuses migration backup, command/event ledger, Artifact Store, content deduplication, integrity, and reopen behavior.
- CP3: creates only valid typed Evidence and session associations.
- CP4–CP6: no synthetic Development or bridge records; later provenance can consume stable Evidence IDs.
- CP7: no automatic provider invocation.
- CP8: uses the desktop shell and Artifact reference contract; rich media preview remains bounded enhancement, not Evidence authority.
- CP10: receives capture status and selected Evidence for resumable Checkpoints/Context Packs.
- CP12: owns live platform/device/codec/installer/soak certification.
