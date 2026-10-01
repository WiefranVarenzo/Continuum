# ADR-009 — Deterministic, Bounded Context Composition

> **Status:** Accepted  
> **Date:** 2026-09-09  
> **Owner:** CP10 — Checkpoint & Context Engine

## Decision

Continuum builds the authoritative candidate set and ordering for Context Packs deterministically from canonical project state. Selection uses declared scope, explicit roots/exclusions, exact Checkpoint sources, status, bounded task-term matching, relationships, and Artifact links. AI may later propose a reviewed compression or ranking over the already-authorized set, but it cannot add denied sources, alter the source boundary, or commit canonical truth.

Every pack has independent soft-token, hard-token, byte, item-count, and per-item byte limits. The default is 16,000 soft / 32,000 hard estimated tokens, 512 KiB total, 200 items, and 16 KiB per item. The estimator is identified as `utf8_bytes_div_3_ceiling_v1` with medium uncertainty. Exact provider tokenization remains a destination adapter concern.

Embeddings are not required and remain disabled. Existing deterministic metadata, event, graph, repository, and text signals are fast enough for the CP10 target hardware. A future embedding index must be optional, derived, versioned, rebuildable, independently privacy-classified, and supported by a separate ADR and benchmark.

## Why

- Resume must work offline and without a configured model.
- Repeating an unchanged request must select the same ordered material.
- Budget overflow and privacy denial must be visible, not silently ignored.
- Provider-specific tokenizers must not become canonical storage dependencies.
- The measured deterministic path has substantial performance headroom on the target Ryzen 5 5600H / 16 GB machine.

## Consequences

- Context Pack previews are renderer- and provider-neutral structured data.
- A user can remove sources and rebuild before saving or transmitting.
- A saved pack is immutable and linked to a GeneratedArtifact; ephemeral previews do not mutate the ledger.
- Any canonical state change between preview and save forces a rebuild.
- CP11 may expose the same contracts over MCP after applying client-grant authorization; it must not bypass CP10 audience/privacy checks.

## Rejected Alternatives

- **AI-only retrieval:** nondeterministic, online-dependent, and unable to guarantee authorization boundaries.
- **Unbounded “send the whole project”:** costly and unsafe, with no honest omission semantics.
- **Provider-native history as memory:** not portable across Codex, Claude, Gemini CLI, OpenAI-compatible APIs, or future clients.
- **Embeddings by default:** additional index/privacy/lifecycle complexity without measured CP10 necessity.
