# ADR-004 — Deterministic Truth and Semantic Assistance

> **Status:** Accepted; provider-specific wording amended by ADR-006
> **Date:** 2026-08-31
> **Amendment:** [ADR-006 — Provider-Neutral AI and MCP Boundaries](ADR-006-PROVIDER-NEUTRAL-AI-AND-MCP-BOUNDARIES.md)

## Decision

Canonical identity, validation, persistence, access, privacy, Git observations, graph constraints, checkpoint positions, and budgets are deterministic. Semantic models operate only through the AI Provider Gateway on privacy-filtered bounded sources and return schema-validated, provenance-bearing candidates requiring human review for consequential acceptance. Gemini was the initial provider selection; ADR-006 generalizes this boundary without changing its authority rules.

## Rationale

Continuum's value depends on inspectable truth and private local ownership. Semantic models are useful for interpretation but cannot safely own provenance or permissions.

## Consequences

Every AI task needs a schema, source IDs, provider/model and adapter metadata, failure behavior, and review state. Offline core workflows remain functional. Provider/model selection is deployment configuration validated in CP7 rather than a domain dependency.
