# CP11 Acceptance Checklist

> **Result:** PASS — 2026-09-10

- [x] Local STDIO emits newline-delimited JSON-RPC only on stdout.
- [x] Initialize/initialized negotiation supports the declared protocol versions.
- [x] The v1 tools/resources/templates/prompts catalog is versioned and grant-filtered.
- [x] Grants are human-created, explicit, expiring, immutable, revocable, and project-scoped.
- [x] Tokens are one-time values and only SHA-256 digests persist.
- [x] Every request rechecks grant, session, scope, resource/tool, size, rate, and Space availability.
- [x] Pending cancellation state is bounded, and a request cancelled before execution is not run.
- [x] CP10 privacy, secret scan, classification, source, freshness, and budget rules remain enforced.
- [x] Artifact-content ceilings are exact and never rounded above the grant.
- [x] Research search is scope-authorized before query execution; denied search results and graph nodes/frontiers do not disclose their identifiers.
- [x] The saved Context Pack index omits task text and filters private/local audiences by the grant ceiling.
- [x] Raw SQLite, arbitrary files, shell, unrestricted URL fetch, provider credentials, and repository mutation are absent.
- [x] Proposal submission is bounded, idempotent, source-validated, fingerprinted, immutable, and non-canonical.
- [x] Proposal status is isolated to the creating grant; review is human-only and version-checked.
- [x] Revocation closes active sessions and later calls fail closed.
- [x] Allowed, denied, completed, and rejected initialization activity has sanitized audit records.
- [x] Integrity scan detects invalid live authority and proposal payload tampering.
- [x] Cross-project, revoked, oversized, rate-limited, secret, malformed, unknown, and pre-init paths fail closed.
- [x] Codex, Claude Code, Gemini CLI, and generic protocol fixtures pass.
- [x] Desktop creates/revokes access, reveals a token once, reviews proposals, and shows safe audit.
- [x] Schema v11→v12 is forward-only and preserves existing project state.
- [x] Rust lint/regression, UI tests/build, Tauri lint, benchmark, and diff hygiene pass.

## Deferred Without Reopening CP11

- [ ] CP12 live then-current client/version certification on release operating systems.
- [ ] CP12 signed installers, process supervision, cancellation/termination soak, backup/restore, and corrupted-project drills.
- [ ] Streamable HTTP remains disabled unless a new threat-reviewed ADR and deployment gate approve it.
- [ ] Automatic proposal materialization is intentionally absent; accepted intent uses a normal user/domain command.
