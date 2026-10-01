# CP10 Acceptance Checklist

> **Result:** PASS — 2026-09-10

- [x] Research-only Checkpoints/Context Packs require no Development Space or repository.
- [x] Development-only Checkpoints/Context Packs require no Research entity.
- [x] Integrated scope requires both Spaces and includes no synthetic placeholder.
- [x] Core scope remains available for cross-space project constraints.
- [x] Checkpoint captures exact immutable source versions and durable repository/capture boundaries.
- [x] Legacy CP2/CP3/CP5 Checkpoints remain readable without rewriting IDs/history.
- [x] Optional AI narrative must be accepted and fresh; AI cannot write canonical Checkpoints.
- [x] Then/Since/Now/Next and Checkpoint comparison expose material change.
- [x] Freshness detects changed/missing sources, Artifact change, repository divergence, capture advancement, and policy change.
- [x] Repeated unchanged retrieval has stable request/content fingerprints and source order.
- [x] Soft/hard tokens, bytes, items, and item-size bounds fail safely with omission metadata.
- [x] External/public audience policies and credential-pattern checks fail closed.
- [x] Artifact content is explicit, bounded, textual, available, and privacy-permitted.
- [x] Preview is local/ephemeral and items can be removed before explicit save.
- [x] State change between preview and save is rejected; retry is idempotent.
- [x] Saved pack is immutable and atomically linked to a GeneratedArtifact.
- [x] Schema v10→v11 migration preserves legacy state and final integrity is healthy.
- [x] Desktop tests, production build, Tauri strict lint, core acceptance, and target-hardware benchmark pass.

## Deferred Without Reopening CP10

- [ ] CP11 MCP client grants, sessions, resource/tool catalog, and proposal ingress.
- [ ] Optional embedding index only after a new privacy/benchmark ADR demonstrates need.
- [ ] CP12 cross-OS packaging, soak, process-group memory, and release security certification.
