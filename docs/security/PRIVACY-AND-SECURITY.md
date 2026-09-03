# Continuum Privacy and Security Architecture

> **Status:** Approved baseline; provider/MCP amendment accepted 2026-09-03; deterministic controls implemented through CP4
> **Posture:** Local-first, least privilege, explicit external disclosure, untrusted inputs.

## 1. Trust Boundaries

- trusted local application core;
- user-controlled project directory;
- OS credential store;
- untrusted repositories, files, web Evidence, imports, and media metadata;
- semi-trusted local adapters/processes with bounded arguments and access;
- untrusted external AI/provider boundary;
- permissioned external MCP clients.

Local OS account access is the MVP user boundary. Continuum does not claim protection from an already-compromised host.

## 2. Classification

- `public`: approved for external sharing.
- `internal/project-private`: default; external AI allowed only under explicit project consent and gateway policy.
- `sensitive`: per-request preview and confirmation required.
- `secret/never-send`: external transmission prohibited.

Derived data inherits the most restrictive source classification. Declassification is explicit, actor-attributed, and audited. Credentials are always secret/never-send and stored outside projects.

## 3. AI Privacy Gateway

Every outbound semantic request declares exact provider/model profile, destination/region class, task, audience, source IDs, policy version, routing/failover policy, and budget. The gateway resolves transitive sources, classifies, scans for secrets, excludes denied content and its unsafe derivatives, performs bounded redaction, shows a destination-specific preview when policy requires, and records sanitized audit metadata.

The gateway is the only provider adapter entry point. UI, reports, plugins, MCP, and background jobs cannot invoke any model provider directly. Credentials are resolved from the OS credential store at call time. Providers are project-allowlisted; sensitive data cannot silently fail over to another destination, and `secret/never-send` has no remote route.

## 4. Threats and Mandatory Controls

### Secret exfiltration

Controls: default-private classification, secret scanning, transitive deny, provider preview/consent, OS credential store, sanitized logs, and canary-secret tests.

### Prompt injection in Evidence/repository

Controls: retrieved material labelled as untrusted data, instruction/data separation, no model tools with ambient authority, schema validation, source-ID validation, and human review.

CP4 additionally invokes Git through direct argument vectors without a shell, closes stdin, removes external-diff environment overrides, disables optional locks/filesystem monitors for observation, rejects repository-local executable clean/smudge/process filters and diff/text-conversion drivers, bounds stdout/stderr, enforces a deadline, rejects malformed/non-UTF-8 repository coordinates, and never executes repository content. Commit messages and paths remain untrusted display data.

### Malicious path/archive/symlink

Controls: canonicalized project-relative paths, no-follow policy where required, extraction allowlist, size/count limits, staging directory, and atomic exposure after verification.

### Parser/media exploit

Controls: patched pinned dependencies, isolated adapters/processes when practical, time/memory limits, media metadata validation, least filesystem scope, and graceful fallback.

### Artifact tampering/corruption

Controls: SHA-256, size verification, atomic finalization, read verification policy, integrity scanner, backup, and quarantine.

### Unauthorized capture

Controls: explicit OS permission, user initiation, persistent visible indicator, immediate pause/stop, source metadata, and no background sensor activation.

### Unauthorized external client

Controls: local stdio by default, explicit client grant, authorization on every request, project/Space/capability scope, opaque application resource IDs, read-first APIs, proposal writes, rate/budget limits, audit, expiry, and immediate revocation. Raw database, SQL, shell, arbitrary filesystem paths, and provider credentials are never exposed.

### Provider confusion or unsafe failover

Controls: deterministic routing, per-project provider allowlist, capability fail-closed behavior, exact destination display, destination-specific consent, separate attempt records, no sensitive silent failover, and provider compatibility/conformance tests.

### MCP confused deputy or cross-project access

Controls: grant-bound project identity, per-request scope checks, no caller-selected unrestricted paths, capability-filtered tool discovery, proposal-only consequential writes, bounded responses, sanitized errors, and cross-project/revocation fixtures.

### Resource exhaustion

Controls: import/media/context limits, bounded queues, streaming, pagination, cancellation, backpressure, scoped graph rendering, and interactive priority.

### Database or migration loss

Controls: short transactions, WAL, backups, integrity checks, migration backup/rollback, idempotency, and crash-injection tests.

## 5. Logging and Diagnostics

Logs contain correlation IDs, event/job type, timings, versions, sizes, and bounded error codes—not raw secrets, complete prompts, repository content, or capture payloads. Diagnostic export runs a second scrubber, lists included categories, and requires user confirmation.

## 6. External AI Providers and MCP

Provider configuration documents verified capabilities, deviations, retention/training posture, region, quota, rate limits, and usage/cost behavior at CP7 setup. Secret/never-send is blocked regardless of provider promises. Each provider attempt records exact destination/model and policy decision without storing credentials or prohibited source content.

The CP11 Continuum MCP Server is an inbound boundary separate from the outbound AI Provider Gateway. MCP is local stdio first; reads are project-scoped and write-like tools create proposals subject to normal validation and approval. Streamable HTTP remains disabled until authentication, origin validation, localhost/default binding, revocation, session, rate-limit, and deployment controls pass. No raw SQLite, arbitrary filesystem, shell, credential, or direct canonical-write interface is exposed.

## 7. Capture Consent

Windows 11 capture uses OS-supported screen capture and WASAPI-class audio adapters in CP9, subject to final platform validation. Default container target is MP4 with recoverable segmentation where supported. OCR/transcription is deferred unless separately enabled and privacy-reviewed. User is informed of legal/participant-consent responsibility.

## 8. Security Acceptance

CP2 passes transaction, artifact, import-path, credential-location, and diagnostic foundations. CP4 passes canonical-path, direct Git invocation, bounded-output/deadline, immutable-observation, rewrite, and repository-drift foundations. Later owning checkpoints add prompt-injection, capture-consent, provider-routing/failover/conformance, analyzer, and MCP cross-project/revocation/authorization suites before their features ship.
