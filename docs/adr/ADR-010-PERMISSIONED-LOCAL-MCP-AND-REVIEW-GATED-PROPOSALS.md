# ADR-010 — Permissioned Local MCP and Review-Gated Proposals

> **Status:** Accepted and implemented in CP11 — 2026-09-10  
> **Owners:** CP11 implementation; CP12 release certification  
> **Extends:** ADR-004, ADR-006, ADR-009

## Context

Continuum must let Codex, Claude Code, Gemini CLI, and other compatible AI clients resume work without turning an external model into the owner of project truth. CP10 already produces bounded, source-cited Context Packs. CP11 needs an interoperable transport, per-project authority, revocation, audit, and a safe contribution path.

## Decision

Continuum ships a first-party MCP server over local STDIO. Each client receives an explicit, expiring, immutable grant. The bearer token is displayed once and only its SHA-256 digest is persisted. Every operation revalidates project, session, grant, scope, resource/tool permission, classification ceiling, request size, rate, and expiry.

The server exposes only versioned application-level resources, prompts, and tools. It never exposes raw SQLite, arbitrary filesystem paths, SQL, shell execution, provider credentials, repository mutation, or unrestricted network fetches.

External clients may read and, when the user enables the capability, submit immutable typed proposals. Submission never changes canonical Research, Development, graph, repository, Checkpoint, or Context Pack state. A person must accept or reject the proposal in Continuum; accepted review is still followed by a normal domain command when materialization is desired.

Streamable HTTP is represented in the contract but disabled. Enabling it requires a separate ADR and passing authentication, Origin, localhost binding, DNS rebinding, CSRF, session, deployment, and remote-threat gates.

## Consequences

- Research-only, Development-only, and local deterministic workflows still work with MCP disabled or crashed.
- Client breadth is an adapter concern; canonical domain behavior is client-neutral.
- Grant changes use revoke-and-replace, preserving an auditable authority history.
- STDIO stdout remains newline-delimited JSON-RPC only; diagnostics use stderr.
- CP12 retains live client-version certification, cross-OS packaging, process termination/soak, and release adversarial testing.

## Rejected Alternatives

- Direct database/filesystem access: bypasses invariants and privacy boundaries.
- Ambient project-wide access: cannot enforce least privilege or cross-project isolation.
- Automatic AI writes: breaks deterministic authority and provenance.
- Client-specific domain forks: creates semantic drift across AI products.
- Remote HTTP by default: adds a network attack surface without an MVP need.
