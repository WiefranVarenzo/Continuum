# CP7 Implementation

> **Status:** Complete for the CP7 provider-neutral semantic core  
> **Crate:** `continuum-core` 0.7.0  
> **Schema:** v8

## Delivered

- `src/semantic.rs`: task/provider/privacy/routing/adapter/validation/cache/review services.
- `migrations/0008_semantic_intelligence.sql`: forward-only CP7 storage and guards.
- `tests/cp7_semantic_acceptance.rs`: eight acceptance and adversarial scenarios.
- `examples/cp7_benchmark.rs`: release-mode local gateway/cache benchmark.
- `docs/cp7/`: architecture, traceability, acceptance, validation, and benchmark evidence.

## Public API Groups

- Provider configuration: `register_provider_profile`, `get_provider_profile`, `list_provider_profiles`, `set_provider_profile_enabled`.
- Project policy: `ai_project_policy`, `set_ai_project_policy`.
- Classification and consent: `set_entity_ai_classification`, `record_ai_consent`, `revoke_ai_consent`, `set_semantic_task_consent`.
- Tasks: `create_semantic_task`, `get_semantic_task`, `list_semantic_tasks`, `preview_semantic_task`, `execute_semantic_task`, `cancel_semantic_task`.
- Candidates: `get_semantic_candidate`, `list_semantic_candidates`, `semantic_candidate_freshness`, `review_semantic_candidate`.
- Host integration: `SemanticTransport`, `ProviderRequest`, `ProviderResponse`, `ProviderTransportError`.

## Execution Characteristics

- No network work occurs inside a SQLite transaction.
- Every transmitted task is frozen to source versions/hashes and an inspected destination.
- Input size uses a conservative UTF-8 byte upper-bound instead of assuming one provider tokenizer.
- Provider usage is recorded when returned; network latency remains separate from local SLOs.
- A cache hit still receives a unique attempt and candidate.
- Cache metadata removes expired rows on write and enforces a 10,000-entry per-project LRU ceiling.
- Successful candidate creation and terminal task state are atomic.
- Concurrent completion is rejected rather than producing two candidates.
- Failed attempts and terminal states retain sanitized audit metadata.
- Generic and typed canonical APIs continue to reject AI actors/origins.

## Deliberately Deferred

- CP8: HTML Human Document Model and interactive diagram/report rendering.
- CP9: media capture and modality-specific preprocessing.
- CP10: final Context Pack selection, tokenizer-aware estimates, optional embeddings.
- CP11: inbound MCP server for Codex, Claude Code, Gemini CLI, and other clients.
- CP12: live-provider matrix, OS-specific credential backend certification, load/cancellation/streaming certification, and long-running retention/eviction tests.

The CP7 adapter contract is complete without storing credentials or binding the domain to an HTTP library. Desktop/host code supplies a `SemanticTransport` that resolves the recorded credential reference from the OS credential store and sends the already validated request.
