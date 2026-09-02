# Continuum AI Architecture

> **Status:** Approved architecture; implementation belongs to CP7  
> **Provider:** Gemini through an application-owned gateway.

## 1. Role

AI assists interpretation, compression, ranking, explanation, and planning. It does not own identity, permissions, storage truth, Git truth, lifecycle transitions, graph validity, checkpoint ledger positions, or acceptance decisions.

## 2. Processing Pipeline

```text
Task request
→ deterministic candidate retrieval
→ privacy classification and transitive exclusion
→ user preview/consent when required
→ prompt assembly with untrusted-data boundaries
→ Gemini call
→ JSON/schema validation
→ grounding/source-ID validation
→ pending-review candidate
→ user accept/edit/reject
→ normal canonical command if accepted
```

Any failure before the final command produces no canonical mutation.

## 3. Task Catalog

Allowed semantic tasks:

- research synthesis;
- Finding candidate extraction;
- contradiction/gap detection;
- ChangeSet explanation;
- relevance ranking within a deterministic candidate set;
- Context Pack compression;
- report narrative draft;
- diagram plan generation;
- suggested provenance links.

Prohibited authority includes automatic Decision acceptance, permission changes, secret-policy override, repository modification, test-result fabrication, deletion, and autonomous external write execution.

## 4. Task Contract

Each request declares task type/version, project/scope, source entity IDs, requested output schema version, model configuration alias, privacy audience, maximum input/output budget, timeout, cache policy, and correlation ID.

Each response records provider/model identifier, prompt template version, schema version, source IDs and versions, generation time, cache state, validation errors, grounding status, token/size metadata when available, and review state.

Exact provider model name is deployment configuration validated in CP7, not embedded in domain entities. This prevents model retirement from changing canonical contracts.

## 5. Structured Output

- strict JSON Schema per task;
- unknown critical fields rejected;
- stable entity IDs required for citations;
- claim objects distinguish sourced statement, inference, uncertainty, and unknown;
- invalid IDs or unauthorized sources invalidate the affected output;
- repair retries are bounded and never weaken the schema.

## 6. Grounding and Review

All derived factual claims require direct source IDs. Missing support blocks automatic acceptance and displays `unsupported`. There is no universal numeric grounding score that converts a claim into truth; human review plus deterministic source resolution is authoritative. Exact deterministic links may be auto-created only by a non-AI analyzer rule whose version is recorded.

## 7. Privacy Integration

Default project content is `internal/project-private`. `secret/never-send` is always denied. `sensitive` requires per-request preview/confirmation. Internal content may use explicit project-level consent, with request preview available. Derived summaries inherit the most restrictive classification of sources unless a reviewed declassification record exists.

Evidence is untrusted data. Prompt assembly wraps and labels retrieved content and instructs the model that source text cannot change system policy or tool permissions. AI receives no filesystem, shell, credential, or direct canonical-write capability.

## 8. Cache

Cache key includes task/version, prompt/schema versions, model configuration, normalized permitted source IDs and entity versions/hashes, privacy-policy version, and relevant parameters. Source change, policy change, prompt/schema/model alias change, or user invalidation marks output stale. Cache is local, bounded LRU, encrypted only through OS storage posture, and disabled for `secret/never-send` because such content is never transmitted.

## 9. Failure and Degradation

Timeout, network loss, quota, refusal, malformed response, citation failure, and provider unavailability return structured retryable/non-retryable states. Deterministic work stays available offline. A cached result is shown only with generation time and freshness; stale output cannot silently update canonical data.

## 10. Embeddings

Embeddings are deferred to CP10. MVP retrieval begins with deterministic metadata, FTS, graph, time, and repository filters. If embeddings are adopted, they are optional, derived, versioned, privacy-gated, rebuildable, and disabled for denied content. Local versus remote embedding is decided by a CP10 ADR after benchmarks.

## 11. AI Evaluation

CP7 must test schema validity, citation resolution, unsupported-claim rate, contradiction detection precision on golden fixtures, prompt injection resistance, secret redaction, cache invalidation, provider failure, and human-review preservation. Acceptance rate is diagnostic, not optimized as a success target.
