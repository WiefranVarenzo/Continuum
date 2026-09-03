# ADR-006 — Provider-Neutral AI and MCP Boundaries

> **Status:** Accepted
>
> **Date:** 2026-09-03
>
> **Amends:** ADR-004 provider binding, D-020, D-023, and D-027
> **Compatibility:** No CP2 or CP3 data migration; CP1 remains PASS through this controlled amendment.

## Context

The original architecture selected Gemini behind an application-owned AI Gateway while deferring the exact model to CP7. Continuum must now support multiple model providers and allow compatible external AI clients—such as Codex, Claude Code, and Gemini CLI—to consume Continuum context through MCP.

These are different integration directions. An outbound model API lets Continuum request semantic assistance. An inbound MCP server lets an external AI client use Continuum's permissioned context and proposal operations. Combining them would couple provider behavior to canonical project access and weaken privacy, testing, and replacement boundaries.

## Decision

### 1. Outbound semantic integration

CP7 becomes **Provider-Neutral Semantic Intelligence**. All model calls pass through one application-owned **AI Provider Gateway**. The gateway accepts versioned semantic-task contracts and delegates only the provider-specific wire exchange to adapters.

Initial adapter families are:

- native Gemini;
- native OpenAI;
- native Anthropic when selected for delivery;
- OpenAI-compatible profiles for services such as OpenRouter, DeepSeek, and BytePlus ModelArk.

Compatibility is capability-based, not name-based. Every provider/model profile declares supported modalities, structured-output subset, tool/function calling, streaming, token accounting, context/output limits, regional and retention metadata, retry semantics, and known deviations. A shared wire format does not imply equivalent behavior.

Provider routing is explicit, deterministic from recorded policy, and visible to the user. Sensitive requests cannot silently fail over to another provider. Every attempt records provider, endpoint profile, exact model identifier, task/prompt/schema versions, source IDs and hashes, privacy decision, timing, usage when available, validation result, and review state. Credentials remain in the OS credential store.

### 2. Inbound external-AI integration

CP11 implements the **Continuum MCP Server** as the primary AI Continuity Interface. Local stdio is the MVP transport. Streamable HTTP is optional after authentication, origin, network-binding, revocation, and deployment controls pass their CP11 threat-model gate.

The MCP server exposes versioned resources, prompts, and tools through application services. It never exposes raw SQLite, unrestricted artifacts, arbitrary filesystem paths, shell execution, provider credentials, or direct domain-table mutation. Reads are project- and capability-scoped. Consequential writes create reviewable proposals and pass the same deterministic validation, privacy, audit, and human-approval flow as the desktop UI.

Client-specific configuration belongs to adapters and documentation, not the domain. Codex, Claude Code, Gemini CLI, and future MCP-compatible clients consume the same Continuum contract with negotiated protocol capabilities.

### 3. Boundary rule

The AI Provider Gateway and Continuum MCP Server are separate ports:

```text
Continuum application → AI Provider Gateway → model-provider APIs
External AI client → Continuum MCP Server → Continuum application services
```

Neither boundary may call around the other to bypass privacy, authorization, budgets, provenance, schema validation, or canonical-write policy. Continuum may gain an outbound MCP client in a future ADR, but that is not required for CP7 or CP11 and is not implied by this decision.

## Consequences

- Gemini remains a supported first-class provider, not an architectural dependency.
- CP1 documents and diagrams use provider-neutral terms and distinguish inbound MCP from outbound model APIs.
- CP2 and CP3 remain valid because their canonical contracts contain no provider-specific dependency.
- CP4–CP6 must preserve stable source IDs and deterministic observations needed by any later model or MCP client.
- CP7 owns provider registry, capability negotiation, routing, adapters, normalization, conformance tests, privacy integration, and candidate provenance.
- CP10 emits provider-neutral Context Packs and token estimates with declared tokenizer uncertainty.
- CP11 owns MCP transport, scopes, tool/resource/prompt catalog, client conformance, audit, and revocation.
- CP12 tests provider degradation, provider switching, compatibility drift, MCP authorization, and cross-client behavior.

## Delivery Constraint

The architecture supports many providers without requiring every provider in the MVP. CP7 must prove the common contract with at least two independently configured provider paths, including Gemini and one non-Gemini path, plus capability/conformance tests. Additional adapters are added only when they pass the same privacy, schema, provenance, failure, and resource-budget gates.

## Supersession

ADR-004 remains authoritative for deterministic truth and human-reviewed semantic assistance. Its Gemini-specific wording is superseded only with respect to provider selection. D-020 remains stdio-first but is expanded by the MCP contract above. D-023 and D-027 are amended by the provider-selection and MCP-delivery constraints recorded here while their exact deployment choices remain deferred to their owning checkpoints.
