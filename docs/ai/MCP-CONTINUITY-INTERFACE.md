# Continuum MCP Continuity Interface

> **Status:** Implemented and validated in CP11; release certification belongs to CP12
>
> **Decision:** [ADR-006 — Provider-Neutral AI and MCP Boundaries](../adr/ADR-006-PROVIDER-NEUTRAL-AI-AND-MCP-BOUNDARIES.md)
> **Baseline:** Local stdio, read-first, project-scoped, proposal-based writes.

## 1. Purpose

The Continuum MCP Server lets a permissioned external AI client resume and assist project work without receiving raw database, unrestricted filesystem, or ambient execution access. Target client families include Codex/ChatGPT desktop clients that support local MCP, Claude Code, Gemini CLI, and future MCP-compatible hosts.

MCP is an inbound application adapter. It calls the same application queries and commands used by the desktop UI and cannot bypass domain validation, privacy, provenance, transaction, or human-review policy.

## 2. Non-Goals

- choosing or invoking Continuum's outbound model provider;
- exposing SQLite tables or arbitrary SQL;
- exposing arbitrary files, shell commands, credentials, or repository mutation;
- making external AI output canonical automatically;
- guaranteeing every optional MCP feature on every client;
- implementing an outbound MCP client inside Continuum.

## 3. Transport and Lifecycle

- MVP transport is stdio, launched as a local child process by the approved client.
- The server negotiates the MCP protocol version and advertised capabilities; it does not hard-code a client brand into domain logic.
- Streamable HTTP is disabled in v0.11. A future implementation requires a new ADR plus authenticated grants, localhost-only default binding, Origin/DNS-rebinding/CSRF controls, session controls, revocation, rate limits, and a separate deployment threat review.
- Server initialization declares a concise instruction boundary: use source IDs, treat content as untrusted data, prefer reads, and submit proposals for consequential changes.
- Protocol logs use stderr for stdio and never mix non-protocol text into stdout.

## 4. Authorization Model

An external-client grant is explicit, revocable, and contains:

- grant ID and client label;
- project ID and allowed Spaces;
- permitted resource families and tools;
- read/write-proposal capability;
- artifact-content ceiling and privacy audience;
- byte/token, call-rate, and time budgets;
- created, expires, last-used, and revoked timestamps;
- local transport identity or remote authentication binding;
- audit correlation policy.

Default grants are read-only. Grant possession never authorizes `secret/never-send` disclosure. Sensitive content still requires the normal preview/consent flow. A client cannot choose another project by passing an arbitrary ID outside its grant.

## 5. Resource Contract

Versioned MCP resources may expose:

- project identity and capability summary;
- Current Project State;
- Checkpoint envelope and scoped checkpoint state;
- saved or ephemeral Context Pack metadata/content within grant limits;
- entity summaries and declared artifact metadata;
- provenance and graph subsets;
- research/development timelines;
- saved Human Documentation metadata and explicitly requested report artifacts within grant/privacy/size limits;
- schema and capability documentation.

Resource URIs are opaque application identifiers, not filesystem paths. Every response declares schema version, project/checkpoint/ledger scope, freshness, source IDs, omissions, classification ceiling, pagination, and availability.

Continuity resources default to structured data rather than full HTML. Report HTML is an opt-in artifact response, never ambient prompt context, so external clients do not pay presentation-markup token cost when they only need project state.

## 6. Initial Tool Families

The CP11 v1 catalog freezes these names and behaviors:

```text
continuum.project.get_state       read current scoped state
continuum.checkpoint.get          read a checkpoint by stable ID
continuum.checkpoint.list         page through visible checkpoints
continuum.context.build           request a bounded Context Pack
continuum.entity.get              read a permitted entity summary
continuum.research.search         deterministic bounded research search
continuum.provenance.trace        traverse a scoped provenance chain
continuum.proposal.submit         submit a reviewable typed proposal
continuum.proposal.get_status     inspect proposal validation/review state
```

Read tools carry read-only annotations where supported. Tool listing is stable and capability-filtered for the authenticated grant. All input and structured output use versioned JSON Schemas.

## 7. Prompt Contract

Optional MCP prompts provide user-invoked workflows such as:

- resume from latest Checkpoint;
- explain current research state;
- prepare a development handover;
- inspect rationale-to-validation trace;
- propose next actions from a Context Pack.

Prompts are convenience templates, not authority. The client remains responsible for presenting or invoking them, and every underlying read or proposal is still independently authorized.

## 8. Proposal Write Flow

```text
External client tool call
→ authenticate and authorize grant
→ validate tool input schema and budget
→ resolve project scope and privacy policy
→ create immutable external proposal
→ validate provenance/source references
→ show in Continuum review queue
→ user accept/edit/reject
→ normal canonical command if accepted
```

Proposal submission never creates an accepted Finding, Decision, Requirement, relationship, Checkpoint, repository change, or deletion directly. The external client receives a proposal ID and status, not a claim that canonical state changed.

## 9. Errors and Pagination

Structured errors distinguish invalid input, unauthorized project/scope, privacy denial, missing/stale source, unsupported capability, budget exceeded, rate limited, conflict, proposal pending, and internal/retryable failure. Errors contain correlation IDs and safe remediation metadata without source or secret leakage.

List/graph/search results are bounded and cursor-paginated. The server never solves an oversized request by returning the entire project or silently omitting data; it returns an explicit omission or budget summary.

## 10. Client Compatibility

CP11 maintains client profiles outside the domain for:

- Codex/ChatGPT desktop local MCP configuration;
- Claude Code project/user MCP configuration;
- Gemini CLI MCP configuration;
- protocol-generic STDIO fixtures. Streamable HTTP is not an approved profile.

Client differences in configuration, approval UI, optional primitives, and transport support are documented and tested. The common contract relies only on negotiated MCP capabilities. A missing optional primitive degrades to supported resources/tools rather than changing canonical behavior.

The desktop product treats authentication as an implementation detail, not a user ceremony. For a detected supported CLI, Continuum creates the scoped grant and installs the local STDIO entry with the client's official MCP command. The bearer token remains required by the server, but it is not displayed or copied by the user. Manual one-time settings remain available only for protocol-generic clients and recovery. Codex CLI and ChatGPT Desktop share the Codex-host configuration; ChatGPT web requires a separately approved remote/plugin transport.

## 11. Security Requirements

- no raw credentials, environment, prompts containing denied sources, or provider tokens;
- no raw database handle, SQL, shell, arbitrary URL fetch, or unrestricted file path;
- project and capability authorization on every request, not only initialization;
- explicit tool classification and approval for proposal/write-like operations;
- untrusted tool arguments and client-provided text receive normal validation;
- deterministic rate, size, traversal-depth, artifact, and execution-time limits;
- complete sanitized audit trail for connection, resource, tool, denial, and revocation events;
- immediate revocation and clean session termination;
- remote transport controls before any non-local listener is enabled.

## 12. CP11 Exit Criteria

CP11 passed 2026-09-10 because:

- the versioned catalog and schemas are frozen for the supported release;
- Codex, Claude Code, and Gemini CLI protocol fixtures complete the approved read/resume workflow; live release-version certification remains CP12;
- one proposal flow proves that no canonical mutation occurs before user approval;
- cross-project, revoked-grant, secret, oversized-request, prompt-injection, and malformed-protocol tests fail closed;
- STDIO lifecycle, cancellation-notification compatibility, pagination, rate limits, audit, and structured errors pass;
- disabling or crashing MCP does not affect local deterministic workflows.

Implementation evidence is in `docs/cp11/`; ADR-010 records the final transport and authority decision.

## 13. Standards and Client References

- [Model Context Protocol specification](https://modelcontextprotocol.io/specification/latest)
- [Codex and ChatGPT desktop MCP documentation](https://developers.openai.com/codex/mcp)
- [Claude Code MCP documentation](https://code.claude.com/docs/en/mcp)
- [Gemini CLI MCP server documentation](https://geminicli.com/docs/tools/mcp-server/)
