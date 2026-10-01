# CP11 Traceability

> **Status:** Complete  
> **Validated:** 2026-09-10

## Functional Requirements

- FR-MCP-001 → local STDIO JSON-RPC transport with clean stdout/stderr separation.
- FR-MCP-002 → negotiated initialize/initialized lifecycle and supported protocol versions.
- FR-MCP-003 → explicit project/client/scope/resource/tool/classification/budget grant.
- FR-MCP-004 → one-time bearer token, SHA-256-only persistence, expiry, revocation, session closure.
- FR-MCP-005 → authorization, rate, request size, and active-session checks on every call.
- FR-MCP-006 → frozen v1 resource, template, prompt, and nine-tool catalog.
- FR-MCP-007 → CP10 bounded/fresh/privacy-filtered Context Pack reuse; no raw state bypass.
- FR-MCP-008 → bounded/cursor list and graph/search responses with omissions.
- FR-MCP-009 → immutable idempotent typed proposals and same-grant status visibility.
- FR-MCP-010 → human-only optimistic review with no pre-review canonical mutation.
- FR-MCP-011 → structured compatible output/error envelopes and safe correlation metadata.
- FR-MCP-012 → sanitized allow/deny/success/failure audit and integrity diagnostics.
- FR-MCP-013 → desktop grant/revoke/review/audit interface.
- FR-MCP-014 → Codex, Claude Code, Gemini CLI, and generic protocol profiles.
- FR-MCP-015 → MCP failure leaves every local deterministic workflow available.

## Acceptance Evidence

- grant/token/HTTP fail-closed test → FR-MCP-003/004/015.
- client-family profile and transcript tests → FR-MCP-002/006/014.
- revocation/cross-project/scope/tool/size test → FR-MCP-003/005.
- rate/audit/secret absence test → FR-MCP-005/011/012.
- proposal idempotency/isolation/review test → FR-MCP-009/010.
- entity classification/scope/secret test → FR-MCP-007/008.
- schema migration/tamper scan tests → FR-MCP-004/009/012.
- malformed/pre-init/unknown/STDIO framing tests → FR-MCP-001/002/011/015.
- bounded cancellation test → FR-MCP-005/011/015.
- exact artifact ceiling, pre-query Research authorization, strict URI, private Context Pack index, and denied-result/node omission tests → FR-MCP-003/005/007/008/012.
- desktop interaction tests → FR-MCP-013.
- target-device benchmark → NFR performance and memory evidence.

## Upstream / Downstream

- CP1 supplies local-first, modular Spaces, deterministic authority, privacy, budgets, and adapter boundaries.
- CP2 supplies transactions, migrations, project isolation, command receipts, audit, and integrity.
- CP3–CP6 supply typed Research/Development entities and bounded provenance.
- CP7 remains the separate outbound provider gateway and candidate authority model.
- CP8 supplies desktop and Human Documentation without being scraped by MCP.
- CP9 supplies only selected/cited durable capture evidence.
- CP10 supplies immutable Checkpoints and bounded Context Packs.
- CP12 certifies release platforms, packaging, live client versions, security drills, and soak.
