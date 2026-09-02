# ADR-005 — Checkpoint and Context Architecture

> **Status:** Accepted  
> **Date:** 2026-08-31

## Decision

Checkpoint is an immutable scope-aware bookmark at an exact ledger sequence. Research, Development, and Integrated R&D variants share a common envelope. Current Project State is derived from a Checkpoint plus later events. Context Packs use deterministic progressive retrieval, explicit privacy and size budgets, provenance, freshness, and omission metadata.

## Rationale

A mutable summary cannot reliably answer where work stopped or what changed afterward. Immutable source state plus progressive context makes resumption auditable and token-efficient across standalone and connected workflows.

## Consequences

CP2 implements the common Checkpoint envelope; CP10 implements full current-state and Context Pack behavior. Semantic narrative is optional and separately reviewed. Checkpoints never mean project completion.
