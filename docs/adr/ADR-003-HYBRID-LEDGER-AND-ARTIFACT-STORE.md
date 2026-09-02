# ADR-003 — Hybrid Ledger and Content-Addressed Artifact Store

> **Status:** Accepted  
> **Date:** 2026-08-31

## Decision

Use SQLite canonical current state plus append-only audit events and a transactional outbox; do not use full event sourcing. Store payload bytes in a project-local SHA-256 content-addressed Artifact Store with metadata and references in SQLite.

## Rationale

Direct current-state queries simplify a local desktop product, while audit events preserve material history and enable staleness/jobs. Separating payloads avoids database bloat and allows streaming, integrity verification, deduplication, and recoverable media handling.

## Consequences

Commands must update state and events atomically. Artifact ingestion needs staging/finalization and orphan recovery. Projections are rebuildable but not every entity is reconstructed solely from events. Backup/export must coordinate database and referenced payloads.
