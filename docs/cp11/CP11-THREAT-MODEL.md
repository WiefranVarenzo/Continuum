# CP11 Threat Model

> **Scope:** local MCP interface v0.11  
> **Result:** implementation threats closed; release-environment qualification remains CP12

## Assets and Trust Boundaries

Protected assets are canonical R&D records, provenance, captured artifacts, repository metadata, private Context Packs, credentials, grant tokens, and project isolation. Untrusted inputs include client identity/capabilities, JSON-RPC messages, tool arguments, proposal text/payload, source content returned to a model, and client process behavior.

Trust boundaries are user→desktop grant creation, MCP host→STDIO process, server→Continuity application API, application API→SQLite/artifact store, and proposal→human review→normal domain command.

## Threats and Controls

- Token theft/replay: one-time reveal, digest-only persistence, expiry, immutable authority, per-client/project grant, immediate revocation.
- Cross-project access: project-bound store and grant; no project selector in tools; source/entity checks are project-scoped.
- Confused client: family-to-client-name validation plus generic profile only when explicitly selected.
- Initialization-only authorization: every call rechecks token, session, grant, capability, scope, size, rate, and expiry.
- Prompt injection in project content: server instruction declares content untrusted; deterministic schemas/queries; no shell/network/file authority; source IDs retained.
- Secret exfiltration: CP10 audience/classification policy, explicit ceiling, credential-pattern scan, artifact content off by default, exact non-rounded artifact limit, filtered Context Pack index, and bounded responses.
- Direct AI write: only immutable proposal records; human review required; normal domain command remains the materialization boundary.
- Proposal swapping/tampering: grant-scoped idempotency, source validation, fingerprint, immutable trigger, optimistic review version, integrity scan.
- Denial of service: pre-init frame limit, grant request/response/context/item/artifact/rate/time budgets, bounded query depth/pages, and a bounded pending-cancellation set.
- Audit leakage: no raw token, raw client name, stack trace, or denied content; safe-detail secret scan and correlation IDs.
- Protocol/log injection: UTF-8 newline JSON-RPC only on stdout; diagnostics on stderr; malformed/unknown paths produce structured errors.
- Network attack: no listener. Streamable HTTP grant creation fails closed.
- MCP crash/absence: adapter is isolated; local deterministic Continuum workflows do not depend on the MCP process.

## Residual Risks Owned by CP12

- tokens may be persisted insecurely by a third-party client's configuration store;
- a compromised authorized client can read everything its grant permits until revocation;
- OS process/environment inspection and malware are outside application isolation;
- future client/protocol drift may alter optional capability behavior;
- process-group termination, long-running cancellation, crash/soak, packaging permissions, and cross-OS behavior need release testing;
- any network transport creates new DNS rebinding, Origin, CSRF, authentication, exposure, and deployment risks and needs a new ADR.

The residual risks are visible and bounded; none grants canonical authority or bypasses project isolation by design.
