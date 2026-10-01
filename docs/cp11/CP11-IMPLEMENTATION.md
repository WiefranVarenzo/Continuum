# CP11 Implementation

> **Version:** 0.11.0  
> **Schema:** 12  
> **Date:** 2026-09-10

## Delivered

- `continuum-core/src/mcp.rs`: grant, session, authorization, rate, audit, proposal, privacy projection, and integrity services.
- migration `0012_ai_continuity_interface.sql`: project-scoped grants/sessions/rate buckets/audit/proposals plus immutable-authority and immutable-payload triggers.
- `continuum-mcp`: first-party STDIO JSON-RPC server, lifecycle, resources, templates, prompts, tools, structured results/errors, response budgets, and executable entry point.
- client transcript fixtures for Codex, Claude Code, and Gemini CLI across the three negotiated protocol versions.
- desktop `AiContinuityPanel`: least-privilege grant creation, one-click local installation for detected Codex/ChatGPT Desktop, Claude Code, and Gemini CLI clients, explicit manual fallback with one-time token reveal, revocation/disconnection, proposal inspection/review, and recent sanitized audit.
- schema v11→v12 migration, tamper diagnostics, acceptance tests, and benchmark.

## Storage Rules

Raw bearer tokens are never stored. Grant authority and proposal payloads are immutable. Revocation/status review are the only permitted updates. Sessions and audit reference the owning project. Proposal idempotency is unique per grant, so one client cannot observe or collide with another client's retry key.

Grant/session/audit/proposal records do not masquerade as Research or Development progress in CP10 Then/Since/Now/Next. A proposal submission creates no core ledger event. Human review creates an audit/ledger decision but still does not synthesize a Research/Development entity.

## Runtime Rules

The executable requires a project path through `--project` or `CONTINUUM_PROJECT_ROOT`, and a token through `CONTINUUM_MCP_GRANT_TOKEN`. Tokens are intentionally rejected as command-line arguments. Server startup does not enable a network listener.

Automatic desktop setup passes the token only as the MCP client's configured environment value and never as a `continuum-mcp` argument. If client installation fails, the newly created grant is revoked before the UI reports the error. Automatic disconnection revokes authority first, then removes the client entry; a cleanup failure can therefore leave only a harmless stale entry, not valid access.

CP11 operations are synchronous and bounded. Cancellation notifications are accepted without response, pending cancellation state is capped, and a queued request cancelled before execution is not run. Tool/resource time budgets fail closed after bounded application execution; terminating STDIO terminates the process. CP12 owns process-group cancellation/soak certification for longer future operations.

## Explicit Non-Deliverables

- no raw SQL/database, arbitrary files, shell, Git mutation, URL fetch, provider secret, or ambient repository tool;
- no automatic model invocation or replacement of the CP7 provider gateway;
- no direct AI canonical write;
- no Streamable HTTP listener;
- no claim of live release certification for every future client build or operating system.
