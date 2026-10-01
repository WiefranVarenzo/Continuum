# ADR-008 — OS-Mediated Segmented Capture Behind a Replaceable Port

> **Status:** Accepted  
> **Accepted:** 2026-09-09  
> **Owners:** CP9 implementation; CP12 platform certification

## Context

Continuum must capture screen, system audio, microphone, screenshots, files, and web material without making sensor access ambient, exhausting memory, coupling the domain to one operating system, or losing partially persisted work. CP1 deferred the exact Windows capture backend while fixing the architectural direction: operating-system permission boundaries, segmented media, explicit indication, and replaceable adapters.

## Decision

1. Capture lifecycle, permissions, source identity, encoding, buffer policy, failures, segments, markers, Evidence links, and derivations are application/domain contracts in `continuum-core`; acquisition is a replaceable adapter.
2. The CP9 desktop adapter uses the installed OS WebView's `getDisplayMedia`, `getUserMedia`, and `MediaRecorder` facilities. These APIs delegate source selection and permission to the operating system/WebView and can run only from an explicit user action.
3. Capability is detected at runtime per source. A missing API, denied permission, or missing returned audio/video track is recorded as unavailable/denied and fails closed. System audio is never inferred merely because screen capture is available.
4. The recorder emits bounded five-second binary fragments into a serialized persistence queue. Each fragment is hashed and stored in the CP2 Artifact Store. A fragment is an immutable part of the capture timeline; its row does not claim that every backend emits a independently playable media file.
5. Queue size, pending fragments, and fragment size are bounded. The current WebView recorder cannot change bitrate safely in flight, so the desktop policy pauses at pressure limits instead of pretending quality was reduced. Other adapters may implement deterministic quality reduction under the same contract.
6. A screenshot is a one-shot capture session: explicit initiation, OS selection/permission, visible indicator, auditable permission observation, immutable PNG Evidence, then terminal stop.
7. Markers and selected segment ranges become Evidence by reference. The complete recording is not copied for each Evidence item.
8. OCR and transcription are off by default. If introduced, their outputs are derived Artifacts with engine/settings/source provenance and can never replace the source capture.
9. Windows 11, system-audio behavior, codecs, installers, long-duration recording, device removal, sleep/resume, and native-adapter fallback are certified in CP12. A platform/source is not advertised as supported until its conformance fixture passes.

## Consequences

The domain and database remain portable while acquisition can evolve from the WebView adapter to Windows Graphics Capture/WASAPI, PipeWire, ScreenCaptureKit, or another reviewed implementation without migrating Evidence or capture history. The tradeoff is that WebView source/codec availability varies by OS; the UI must show unavailable capability rather than silently substitute a source.

## Rejected Alternatives

- One unbounded in-memory recording: rejected because interruption loses work and memory use grows with duration.
- Bundling arbitrary recorder executables or invoking a shell: rejected for supply-chain, permission, packaging, and command-injection risk.
- Treating screenshots as unaudited file imports: rejected because a screenshot activates a screen sensor.
- Storing transcript/OCR as the original: rejected because transformations are lossy and may be wrong.

## Acceptance

CP9 accepts the application contract when permission and lifecycle transitions, bounded persistence, interruption recovery, typed Evidence, derivation provenance, migration, integrity, and UI consent boundaries pass. CP12 separately certifies concrete source/backend combinations on release operating systems.
