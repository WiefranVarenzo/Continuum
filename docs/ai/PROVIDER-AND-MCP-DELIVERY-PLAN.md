# Provider and MCP Delivery Plan

> **Status:** Implemented through CP11; CP12 release qualification remains
>
> **Date:** 2026-09-03
> **Authority:** PRD, ADR-004, and ADR-006.

## 1. Outcome

Continuum remains a local-first deterministic R&D continuity system. It becomes provider-neutral in its outbound semantic assistance and MCP-compatible in its inbound AI continuity interface. Provider or client breadth is delivered incrementally behind stable contracts and cannot expand canonical authority.

## 2. Checkpoint Impact

### CP1 — Architecture, Domain Model & System Contracts

- replace Gemini-as-architecture wording with provider-neutral AI Provider Gateway;
- preserve Gemini as a first-class adapter candidate;
- separate outbound provider API and inbound MCP boundaries;
- add provider capability, routing, provenance, privacy, and failure contracts;
- add the Continuum MCP Server contract and ADR-006;
- retain CP1 PASS as an amended baseline.

### CP2 — Continuity Core

- no migration or implementation reopening;
- existing identity, ledger, jobs, artifacts, privacy metadata, and application ports remain the foundation;
- future AI/MCP records arrive only through forward migrations owned by CP7/CP11.

### CP3 — Research Core

- no migration or behavior reopening;
- research remains fully usable without any provider or MCP client;
- existing rejection of `ai_proposal` canonical authority remains mandatory.

### CP4 — Development Core

- status: implemented and validated without provider or MCP coupling;
- preserve stable repository, baseline, commit, diff, and ChangeSet source coordinates;
- do not bind change explanations or external clients into Development domain logic;
- expose bounded queries that CP7 and CP11 may consume later.

### CP5 — Code Intelligence

- status: implemented and validated without provider or MCP coupling;
- parser/analyzer output is deterministic, versioned, baseline-addressed, and cache-keyed;
- unsupported/partial/dirty/unborn results are explicit so future provider prompts and MCP responses cannot overstate certainty.

### CP6 — Provenance & Knowledge Graph

- preserve origin and review state for user, deterministic, imported, AI-provider, and external-MCP proposals;
- provide bounded graph queries suitable for provider source selection and MCP traversal.

### CP7 — Provider-Neutral Semantic Intelligence (PASS 2026-09-08)

- implemented semantic task/result contracts, AI Privacy Gateway, provider registry, capability descriptors, deterministic router, and review queue;
- implemented Gemini native and an independently configured OpenAI-compatible provider path;
- supports explicitly registered OpenAI-compatible profiles without assuming feature parity;
- normalizes errors, usage, provenance, schema, grounding, cancellation, and bounded cache behavior; streaming remains a declared per-profile capability and is not assumed by the core;
- offline golden fixtures and adversarial acceptance tests pass; every real enabled profile must still pass the provider onboarding gate below before it is advertised as live-supported.

### CP8 — Visual Intelligence & Reports (PASS 2026-09-08)

- consume provider-neutral semantic candidates only;
- generated artifacts record the exact provider/model/adapter attempt or deterministic-only origin;
- introduce the versioned renderer-neutral Human Document Model;
- make safe semantic HTML the primary in-product and portable Human Documentation experience;
- retain Markdown as a material-fact/citation compatibility renderer;
- use Mermaid for bounded portable diagrams and React Flow + ELK for interactive knowledge graphs;
- package offline reports without a required CDN, remote font, remote script, or Continuum server;
- renderers, URL policy, escaping, component selection, layout constraints, privacy, citations, and freshness remain deterministic regardless of provider.

### CP9 — Research Capture System (PASS 2026-09-09)

- capture remains provider-independent;
- explicit-consent OS/WebView acquisition, bounded fragments, recovery, markers, and typed Evidence are implemented without provider calls;
- optional transcription/OCR, if activated later, uses the same provider registry, privacy, capability, and provenance policies or an explicitly local adapter.

### CP10 — Checkpoint & Context Engine (PASS 2026-09-09)

- produce provider-neutral Context Packs;
- declare token-estimate method and uncertainty because tokenizers differ;
- retain deterministic source selection and privacy filtering before any CP7 call or CP11 disclosure.
- keep Context Packs renderer-neutral and omit HTML/CSS/JavaScript presentation markup unless an explicitly requested report artifact is itself the task source.
- validated deterministic source ordering, audience/secret filtering, preview/removal, declared estimator uncertainty, and immutable saved-pack provenance without invoking a provider;
- embeddings remain off because the reference-hardware benchmark meets the target with substantial headroom.

### CP11 — AI Continuity Interface (PASS 2026-09-10)

- implemented Continuum as a first-party local STDIO MCP server;
- froze v1 resources, prompts, tools, scopes, pagination, structured errors, sanitized audit, and immutable proposal flow;
- validated protocol fixtures for Codex, Claude Code, Gemini CLI, and a generic host without client-specific domain forks;
- added expiring digest-authenticated grants, per-request authorization, revocation, rate/size/privacy budgets, desktop review, migration, integrity, and performance evidence;
- kept Streamable HTTP disabled pending a separate threat-reviewed ADR and deployment gate.

### CP12 — Hardening & Release

- test provider outage, routing/failover, credential revocation, compatibility drift, cost/budget enforcement, and privacy isolation;
- test MCP cross-project authorization, client revocation, malformed messages, oversized output, transport shutdown, and client compatibility;
- publish the supported provider/model/client matrix with verification dates and known limitations.

## 3. Delivery Waves

### Wave A — Contracts before breadth

Freeze task envelopes, result/provenance envelopes, capability descriptor, normalized errors, provider profile, MCP grant, and initial MCP schemas.

### Wave B — CP7 proof

Ship Gemini native plus one non-Gemini path and demonstrate identical canonical safety, privacy, validation, and offline degradation. Add selected OpenAI-compatible profiles only after conformance passes.

### Wave C — CP10/CP11 continuity

Expose bounded Context Packs and read-first continuity through local MCP. Prove resume, trace, and proposal workflows across supported clients.

### Wave D — Controlled expansion

Add providers or remote MCP only when demand justifies their maintenance, security, compatibility, and evaluation cost.

## 4. Provider Onboarding Gate

A provider/profile is not supported until it has:

- documented endpoint and credential handling;
- capability descriptor and known-deviation record;
- region/retention/training-policy reference and verification date;
- schema, grounding, streaming, timeout, retry, cancellation, and rate-limit tests;
- secret canary and prompt-injection tests;
- usage/cost accounting behavior;
- deterministic disable/degradation behavior;
- user-visible support status.

## 5. MCP Client Onboarding Gate

A client is not listed as supported until its selected transport, initialization, tool/resource discovery, structured result, pagination, error, approval/proposal, cancellation, and revocation workflow pass against a pinned Continuum MCP contract. Client-specific configuration remains documentation and adapter metadata, not a domain dependency.

## 6. Scope Control

Provider-neutral means replaceable and extensible, not every provider on day one. MCP-compatible means one stable Continuum server contract, not client-specific business logic. Any addition that bypasses the privacy gateway, deterministic source selection, provenance, proposal review, or project scope is rejected regardless of provider or client popularity.
