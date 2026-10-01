# CP9 — Research Capture System Architecture and Contract

> **Status:** Implemented and validated 2026-09-09  
> **Schema:** Continuum Core v10  
> **Depends on:** CP1 privacy/performance/adapter boundaries, CP2 Ledger and Artifact Store, CP3 Evidence and Research Sessions, CP6 provenance, CP8 desktop/report surface

## 1. Purpose

CP9 reduces the distance between an observation and durable, traceable Evidence. Research Capture is optional: Research-only projects can use it without a repository, Development-only work remains unaffected, and Connected R&D can later carry accepted Evidence through the CP6 provenance chain.

```text
Explicit user action
→ capability snapshot and lawful-capture acknowledgement
→ OS/WebView permission per requested source
→ visible indicator + versioned capture lifecycle
→ bounded segmented persistence to Artifact Store
→ markers / selected segment ranges / external material
→ typed CP3 Evidence with stable source and provenance
```

## 2. Authority Boundary

- SQLite owns capture metadata, permission observations, lifecycle, segment coordinates, marker identity, Evidence links, derivation lineage, and audit history.
- The content-addressed Artifact Store owns original and derived bytes.
- The operating system/WebView owns sensor selection and grants.
- The acquisition adapter owns temporary MediaStreams/MediaRecorder state and has no canonical authority.
- A recording, screenshot, imported file, page excerpt, OCR result, or transcript is Evidence material—not an automatically accepted Finding or Decision.

## 3. Capture Lifecycle

```text
awaiting_permission → ready → capturing ⇄ paused → completed
          ↓             ↓         ↓          ↓
        blocked       failed   interrupted  failed
          ↓
       cancelled
```

Every transition uses optimistic `state_version`, idempotent command receipts, audit events, and a valid actor. `capturing` requires all requested sources granted, lawful-capture acknowledgement, and a non-empty indicator ID. Terminal or interruption transitions clear the indicator. Permission revocation during capture atomically interrupts the session and marks it recoverable.

Failures retain bounded error code/message and whether persisted material is recoverable. An interrupted session may still accept the final fragment emitted by the recorder before shutdown.

## 4. Source and Permission Contract

Supported source kinds are screen, system audio, and microphone. A session requests one to three unique sources and records backend ID/version/platform, a capability snapshot, and a capability ID per source. `unknown`, `granted`, `denied`, `unavailable`, and `revoked` are distinct permission observations. No source is enabled on page load, project open, report generation, or background work.

The WebView adapter asks for display and microphone streams only inside the Start Capture or Take Screenshot click handler. Missing tracks are recorded as unavailable. A screen grant does not imply a system-audio grant.

## 5. Segments, Recovery, and Backpressure

- default fragment duration: 5 seconds;
- default application buffer: 16 MiB;
- maximum fragment: 32 MiB in both the desktop policy and absolute core guard;
- maximum pending desktop fragments: 4;
- persistence is serialized and content-addressed;
- sequence and non-overlapping offsets are enforced;
- duplicate retries must match bytes and metadata;
- incomplete fragments are explicitly `recoverable`, never mislabelled complete;
- at 75% buffer use, adapters flush; at the hard queue/buffer bound they reduce quality if supported or pause.

CP9 never accumulates a complete long-duration recording in application memory. Final platform soak and process-group RSS qualification remain CP12.

## 6. Evidence Contract

- recording range → `recording_segment` Evidence linked to the existing Artifact;
- marker → `capture_marker` Evidence with timestamp and note;
- screenshot → one-shot auditable session plus original PNG `screenshot` Evidence;
- file → original bytes as `file` Evidence;
- browser/web → timestamped URI/title/excerpt as untrusted `web` Evidence;
- all paths may attach directly to a selected active CP3 Research Session;
- no path fabricates a Research Question, Finding, Decision, Requirement, or cross-Space relationship.

Marker and segment promotion creates only the Evidence reference needed for the selected observation. It never copies the complete recording.

## 7. Derivations

Transcription, OCR, thumbnails, and waveforms share one derivation contract: source Artifact, derived Artifact, kind, engine ID/version, bounded settings, creation time, and classification. Identical source/derived bytes are rejected. The source remains available and authoritative. CP9 implements the contract; OCR/transcription execution stays off by default.

## 8. Security and Privacy

- persistent in-app indicator while permission is requested, capture is active, paused with reserved tracks, or finalizing;
- immediate pause/stop controls;
- explicit lawful-capture acknowledgement;
- OS permission is never bypassed or synthesized;
- capture payload never appears in logs, events, command receipts, or failure messages;
- imported/web content is untrusted data with no command, HTML, Mermaid, shell, filesystem, network, or AI authority;
- classification is attached before Artifact persistence and inherited by derived outputs;
- no OCR/transcription or provider call occurs automatically.

## 9. Standalone and Connected Operation

CP9 requires Research Space only because outputs are Evidence. It does not require Development Space, Git, AI, graph rendering, or MCP. A selected Research Session receives the captured Evidence immediately. In Connected R&D, later accepted provenance may connect that Evidence through Finding → Decision → Requirement → ChangeSet → Code → Test without CP9 creating those links itself.

## 10. CP10 Handoff

CP10 receives stable session, segment, marker, Evidence, Artifact, classification, timestamps, and provenance IDs. Checkpoints may bookmark active, paused, blocked, failed, interrupted, or completed capture state. Context Packs should include capture metadata and selected Evidence excerpts by default, not raw full media; deeper media access remains explicit, bounded, and privacy-filtered.
