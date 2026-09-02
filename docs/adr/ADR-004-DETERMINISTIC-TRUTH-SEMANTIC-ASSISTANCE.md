# ADR-004 — Deterministic Truth and Semantic Assistance

> **Status:** Accepted  
> **Date:** 2026-08-31

## Decision

Canonical identity, validation, persistence, access, privacy, Git observations, graph constraints, checkpoint positions, and budgets are deterministic. Gemini operates only through the AI Gateway on privacy-filtered bounded sources and returns schema-validated, provenance-bearing candidates requiring human review for consequential acceptance.

## Rationale

Continuum's value depends on inspectable truth and private local ownership. Semantic models are useful for interpretation but cannot safely own provenance or permissions.

## Consequences

Every AI task needs a schema, source IDs, prompt/model metadata, failure behavior, and review state. Offline core workflows remain functional. Model selection is deployment configuration validated in CP7 rather than a domain dependency.
