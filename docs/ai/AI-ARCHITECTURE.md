# Continuum Provider-Neutral AI Architecture

> **Status:** Approved CP1 architecture amendment; implementation belongs to CP7
> **Decision:** [ADR-006 — Provider-Neutral AI and MCP Boundaries](../adr/ADR-006-PROVIDER-NEUTRAL-AI-AND-MCP-BOUNDARIES.md)
> **Providers:** Pluggable adapters; Gemini remains first-class but is not a canonical dependency.

## 1. Role and Boundary

AI assists interpretation, compression, ranking, explanation, and planning. It does not own identity, permissions, storage truth, Git truth, lifecycle transitions, graph validity, checkpoint ledger positions, budgets, or acceptance decisions.

Continuum has two deliberately separate AI integration directions:

```text
Outbound: Continuum → AI Provider Gateway → Gemini/OpenAI/Anthropic/OpenAI-compatible APIs
Inbound:  Codex/Claude Code/Gemini CLI/other clients → Continuum MCP Server → application services
```

The outbound provider gateway belongs to CP7. The inbound MCP server belongs to CP11 and is specified in [MCP Continuity Interface](MCP-CONTINUITY-INTERFACE.md). MCP is not a substitute for a model-provider adapter, and a provider adapter never grants project access to external clients.

## 2. Outbound Processing Pipeline

```text
Typed semantic task
→ deterministic candidate retrieval
→ privacy classification and transitive exclusion
→ user preview/consent when required
→ provider-independent prompt/input assembly
→ explicit routing against capability and policy
→ provider adapter call
→ normalized response envelope
→ JSON/schema validation
→ grounding/source-ID validation
→ pending-review candidate
→ user accept/edit/reject
→ normal canonical command if accepted
```

Any failure before the final canonical command produces no canonical mutation.

## 3. Component Model

### Semantic Task Service

Defines stable task types, resolves bounded source candidates, and owns task-level budgets. It does not know provider wire formats.

### AI Privacy Gateway

Evaluates source classification, destination, regional/retention profile, consent, secret rules, and redaction. It is the only outbound route for project content.

### Provider Registry

Stores non-secret provider profiles, enabled status, endpoints, model aliases, capability descriptors, policy eligibility, and health state. Credentials are resolved from the OS credential store only at call time.

### Deterministic Router

Selects only from explicitly allowed provider/model profiles using recorded task requirements and project policy. The selected route is visible before transmission where consent is required. Routing never uses model-generated text.

### Provider Adapters

Translate the stable Continuum request into a provider-specific request and normalize the response. Initial families are native Gemini, native OpenAI, optional native Anthropic, and provider-specific profiles over an OpenAI-compatible adapter for OpenRouter, DeepSeek, and BytePlus ModelArk.

### Output Normalizer and Validators

Produce a stable result envelope, validate the task schema, resolve cited source IDs, record unsupported claims, and create an immutable AI candidate for review.

## 4. Task Catalog

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

## 5. Provider-Independent Task Contract

Each request declares:

- task type and task-contract version;
- project, Space, entity, checkpoint, and ledger scope;
- source entity IDs plus versions/hashes;
- requested output schema and prompt-template versions;
- required capabilities and modalities;
- privacy audience, classification ceiling, and consent reference;
- maximum input/output budget, timeout, and cache policy;
- routing policy, allowed provider profiles, and failover permission;
- correlation and idempotency IDs.

Each attempt records:

- provider profile, endpoint class, and exact model identifier;
- adapter and capability-descriptor versions;
- prompt template and output schema versions;
- source IDs, versions, hashes, and omission summary;
- privacy-policy decision and consent reference without prohibited content;
- start/end time, latency, cache state, retry number, and normalized error;
- input/output token or size usage and cost estimate when supplied or deterministically calculable;
- schema, grounding, and provenance validation results;
- candidate and human-review state.

Exact model names are deployment configuration, not domain entity types. This prevents model retirement or provider replacement from changing canonical contracts.

## 6. Capability Descriptor

Every provider/model profile declares, at minimum:

- text, image, audio, video, and document input support;
- structured-output mode and supported JSON Schema subset;
- tool/function calling and parallel-call behavior;
- streaming and cancellation behavior;
- context and output limits;
- tokenizer/token-count confidence;
- reasoning controls and unsupported parameters;
- retryable errors, rate-limit headers, and idempotency behavior;
- data region, retention/training statement reference, and last verification time;
- adapter deviations and test-fixture version.

Unknown capability is treated as unsupported. An OpenAI-compatible wire shape does not imply feature parity. Provider profiles can share transport code while retaining separate capability and privacy records.

## 7. Routing and Failover

- The project has an explicit provider allowlist; no provider is enabled merely because credentials exist.
- Task requirements are intersected with capability, privacy, budget, and availability constraints.
- User-fixed routing always wins unless invalid under policy.
- Automatic failover is opt-in per task or project and is blocked for `sensitive` content unless the user approves the exact alternate destination.
- `secret/never-send` never receives a remote route.
- A fallback attempt creates a separate attempt record and never overwrites an earlier candidate.
- Cost/latency preferences can rank eligible routes but cannot weaken privacy or required capabilities.
- Provider health affects availability, not canonical truth.

## 8. Structured Output and Grounding

- strict task-specific JSON Schema;
- provider-specific schema transformations are adapter-owned and loss is declared;
- unknown critical fields are rejected;
- stable entity IDs are required for citations;
- claim objects distinguish sourced statement, inference, uncertainty, and unknown;
- invalid IDs or unauthorized sources invalidate the affected output;
- repair retries are bounded and never weaken the canonical task schema.

All derived factual claims require direct source IDs. Missing support displays `unsupported` and blocks automatic acceptance. There is no universal numeric grounding score that converts a claim into truth. Human review plus deterministic source resolution remains authoritative.

## 9. Privacy Integration

Default project content is `internal/project-private`. `secret/never-send` is always denied. `sensitive` requires a destination-specific preview and confirmation. Internal content may use explicit project-level consent, with request preview available. Derived summaries inherit the most restrictive source classification unless a reviewed declassification record exists.

Evidence, repository content, MCP input, and provider output are untrusted data. Prompt assembly labels source boundaries and prevents source text from changing system policy or tool permissions. Providers receive no filesystem, shell, credential, or direct canonical-write capability.

Provider policy metadata is advisory until verified by the user or a maintained registry and never overrides Continuum classification. Changing destination, endpoint region, or provider invalidates destination-specific consent when policy requires.

## 10. Cache

The cache key includes task/version, prompt/schema versions, exact provider/model and adapter profile, normalized permitted source IDs and entity versions/hashes, privacy-policy version, routing parameters, and relevant generation parameters. Source, policy, prompt, schema, adapter, capability, or provider/model change marks the affected output stale.

Cache is local, bounded LRU, protected by the host storage posture, and disabled for `secret/never-send` because such content is never transmitted. Cross-provider cache reuse is prohibited unless the result is an explicitly provider-independent deterministic artifact.

## 11. Failure and Degradation

Timeout, network loss, quota, refusal, malformed response, citation failure, unsupported capability, compatibility drift, revoked credentials, and provider unavailability return normalized retryable or non-retryable states. Deterministic work stays available offline. Cached results display provider/model, generation time, and freshness. Stale output cannot silently update canonical data.

If a provider changes behavior, its profile is disabled or degraded without disabling the Research Space, Development Space, Checkpoint inspection, deterministic retrieval, or another working provider.

## 12. Embeddings

Embeddings remain deferred to CP10. MVP retrieval begins with deterministic metadata, FTS, graph, time, and repository filters. If embeddings are adopted, they are optional, derived, versioned, privacy-gated, rebuildable, and disabled for denied content. Local versus remote embedding and cross-provider comparability require a CP10 ADR after benchmarks.

## 13. CP7 Delivery Baseline

CP7 implements:

1. stable semantic-task and result envelopes;
2. provider registry and capability descriptors;
3. AI Privacy Gateway integration;
4. deterministic routing and explicit failover policy;
5. native Gemini adapter;
6. at least one non-Gemini provider path;
7. OpenAI-compatible profile support without claiming universal parity;
8. normalization, schema/grounding validation, cache, candidate review, and provenance;
9. provider conformance and compatibility-drift tests.

Additional production provider adapters are incremental and do not change domain contracts.

## 14. Evaluation

CP7 must test schema validity, citation resolution, unsupported-claim rate, contradiction precision on golden fixtures, prompt injection resistance, secret redaction, cache invalidation, deterministic routing, denied failover, provider isolation, compatibility drift, malformed streaming, quota/refusal behavior, offline operation, and human-review preservation.

The same task fixtures run against each enabled profile. Capability-specific skips must be declared, not silently treated as passes. Acceptance rate is diagnostic and is not optimized as a success target.

## 15. Provider References

Provider documentation is reverified during CP7 onboarding because features, models, regions, pricing, and retention behavior can change:

- [Gemini integration strategies](https://ai.google.dev/gemini-api/docs/partner-integration)
- [OpenAI API documentation](https://developers.openai.com/api/docs/)
- [Anthropic API documentation](https://docs.anthropic.com/en/api/overview)
- [OpenRouter API quickstart](https://openrouter.ai/docs/quickstart)
- [DeepSeek API documentation](https://api-docs.deepseek.com/)
- [BytePlus ModelArk documentation](https://docs.byteplus.com/en/docs/modelark/1099455)
