# Continuum Privacy and Security Architecture

> **Status:** Approved baseline; implemented and regression-validated through CP12 Linux pilot
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

### Stored report and diagram injection

CP8 filters the source set by export audience before composition, escapes all stored text in the Rust offline renderer, permits only versioned semantic block variants, and accepts diagrams only as bounded node/edge data. The in-app Mermaid adapter aliases IDs, normalizes labels, enables strict security, and disables HTML labels. Offline HTML executes no JavaScript and declares no remote assets; its restrictive CSP denies network, objects, frames, forms, fonts, media, and scripts. The Tauri webview receives only bounded report commands and no shell/provider credential authority.

CP4 additionally invokes Git through direct argument vectors without a shell, closes stdin, removes external-diff environment overrides, disables optional locks/filesystem monitors for observation, rejects repository-local executable clean/smudge/process filters and diff/text-conversion drivers, bounds stdout/stderr, enforces a deadline, rejects malformed/non-UTF-8 repository coordinates, and never executes repository content. Commit messages and paths remain untrusted display data.

CP5 uses pinned embedded parsers and never launches repository-selected binaries, plugins, build scripts, package managers, language servers, or tests. Immutable blobs are read in a bounded, framed Git batch and verified against expected OID/type/size. Source bodies are not stored in structural observations or logs; dotenv values are omitted, credential-shaped dependency URIs are redacted, malformed/oversized/binary/dirty inputs degrade explicitly, and all output/counts are validated before atomic commit.

### Malicious path/archive/symlink

CP5.1 stages project creation/export before atomic publication, rejects symlinks while walking project payload trees, and uses no-follow file opens for artifact sources and untracked repository files on Unix. Exact-baseline Git symlink blobs are hashed/file-level observations and are never parsed or executed. Cross-platform reparse-point and hostile concurrent-filesystem certification remains part of CP12.

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

### Over-broad or stale Context Pack

CP10 requires a declared task, scope, audience, destination label, freshness policy, and token/byte/item limits. Exact source versions and fingerprints are captured. Denied explicit roots fail closed; other denied or credential-shaped sources become visible omissions. Artifact payload is opt-in and bounded. A `current` request rejects stale Checkpoints, and any ledger/source change between preview and save requires a rebuild. External transmission is not performed by the preview/save path.

### Stored HTML, URL, and diagram injection

Controls: renderer-neutral allowlisted Human Document blocks; output escaping by default; sanitized rich-text subset only; allowlisted URL schemes; no raw project/model HTML, script, style, event handler, iframe, form, executable URL, or remote dependency; restrictive Content Security Policy; pinned local renderer assets; schema-controlled Mermaid and graph labels; offline and stored-XSS adversarial tests.

### Database or migration loss

Controls: short transactions, WAL, backups, integrity checks, migration backup/rollback, idempotency, and crash-injection tests.

## 5. Logging and Diagnostics

Logs contain correlation IDs, event/job type, timings, versions, sizes, and bounded error codes—not raw secrets, complete prompts, repository content, or capture payloads. Diagnostic export runs a second scrubber, lists included categories, and requires user confirmation.

Human report generation applies project scope and transitive classification before composition. Privacy exclusions cover prose, tables, diagram labels/topology where sensitive, thumbnails, metadata, and artifact references. Export records declared omissions and never substitutes an unrestricted local path or remote URL for excluded content.

## 6. External AI Providers and MCP

Provider configuration documents verified capabilities, deviations, retention/training posture, region, quota, rate limits, and usage/cost behavior at CP7 setup. Secret/never-send is blocked regardless of provider promises. Each provider attempt records exact destination/model and policy decision without storing credentials or prohibited source content.

The CP11 Continuum MCP Server is an inbound boundary separate from the outbound AI Provider Gateway. MCP is local STDIO; grants are project/client/scope/tool/resource/classification/budget bound, expiring, immutable, digest-authenticated, rechecked on every request, and immediately revocable. Write-like tools only create immutable proposals subject to human review and later normal domain materialization. Streamable HTTP is represented but fails closed until a new ADR and authentication, Origin, localhost/default binding, revocation, session, rate-limit, DNS-rebinding, CSRF, and deployment gates pass. No raw SQLite, arbitrary filesystem, shell, credential, unrestricted URL, repository mutation, or direct canonical-write interface is exposed.

## 7. Capture Consent

CP9 capture uses a replaceable application port and an OS/WebView-mediated desktop adapter. Every sensor requires an explicit user click, per-source permission observation, lawful-capture acknowledgement, and a persistent indicator through request, active, paused, and finalizing states. Missing APIs/tracks, denial, and revocation fail closed. Media is segmented into bounded content-addressed Artifacts; payload bytes are excluded from logs and audit events. OCR/transcription is off by default and may only create source-preserving derived Artifacts. Windows 11 and other release-platform/device/codec combinations remain subject to CP12 live certification and a native fallback adapter if required by evidence.

## 8. Security Acceptance

CP2 passes transaction, artifact, import-path, credential-location, and diagnostic foundations. CP4 passes canonical-path, direct Git invocation, bounded-output/deadline, immutable-observation, rewrite, and repository-drift foundations. CP5 passes embedded-analyzer, parser fallback, Git batch framing, resource-limit, secret-safe configuration, atomicity, integrity-corruption, and cross-project/cross-Repository suites. CP7 passes semantic prompt-injection and provider-routing/failover/conformance foundations. CP9 passes capture-consent, sensor-activation, bounded-persistence, interruption-recovery, and capture-integrity suites. CP10 passes audience classification, credential-pattern, explicit-root denial, stale-state, bounded-output, immutable-source, atomic-save, and migration/integrity suites. CP11 passes token/grant/session, cross-project, revocation, client mismatch, rate/size, secret/classification, proposal isolation/tamper, malformed protocol, and sanitized-audit suites. CP12 adds recursive export/import path rejection, release diagnostics, verified backup/export/restore/reopen, restrictive native path mediation, dependency audit, credential-pattern scan, package checksums, and the complete 140-test regression. Windows/live-client/physical-capture/signing/soak matrices remain explicit GA certification gates.
