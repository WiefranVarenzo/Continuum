# CP7 — Provider-Neutral Semantic Intelligence Architecture and Contract

> **Status:** Implemented and validated 2026-09-08  
> **Schema:** Continuum Core v8  
> **Depends on:** CP1 AI/privacy contracts, CP2 ledger and policy boundary, CP3–CP5 stable source identities, CP6 provenance graph

## 1. Purpose

CP7 adds optional semantic assistance without making any model a source of truth. Continuum remains fully usable offline. Models may interpret, compress, rank, explain, or propose; they cannot write canonical Research, Development, relationship, checkpoint, permission, or Git state.

```text
Typed semantic task
→ deterministic source/provenance closure
→ version/hash snapshot and conservative budget check
→ privacy classification + secret scan + destination consent
→ deterministic provider routing
→ Gemini or OpenAI-compatible adapter envelope
→ host-owned credential resolution and transport
→ normalized response
→ strict schema + grounding + presentation-safety validation
→ separate pending AI candidate
→ human accept/edit/reject
→ normal typed domain command later, if explicitly requested
```

No failure before the final, separate typed domain command mutates canonical project knowledge.

## 2. Authority Boundaries

- SQLite entities, relationships, checkpoints, audit sequence, Git observations, and validation remain deterministic truth.
- `ai_candidates` is not part of the canonical `entities` table.
- An accepted candidate is still only reviewed material. Acceptance does not itself create a Finding, Decision, Requirement, ChangeSet, relationship, or report.
- `ActorKind::AiProposal` remains forbidden from canonical command execution.
- The CP7 outbound provider gateway and the future CP11 inbound MCP server remain separate ports.

## 3. Stable Semantic Task Contract

Every task records:

- one of nine stable task types;
- contract, prompt-template, and output-schema versions;
- explicit entity/artifact sources plus deterministic transitive provenance closure;
- source versions/hashes, classifications, and one aggregate fingerprint;
- task capability requirements;
- preferred and allowed profiles plus failover permission;
- privacy audience and optional consent reference;
- conservative input, output, timeout, and cache budgets.

Task types are research synthesis, Finding candidates, contradiction detection, ChangeSet explanation, relevance ranking, Context Pack compression, report narrative, diagram plan, and suggested links. CP7 provides their safe shared execution contract; task-specific UI and report composition are owned by CP8, while final Context Pack retrieval is owned by CP10.

## 4. Source Resolution and Freshness

Explicit sources are limited to 200. The bounded provenance closure is limited to 500 entity/artifact records. For an entity target, CP7 walks active inbound provenance and declared direct sources, then includes attached Artifact metadata. Artifact payload bytes are not implicitly uploaded.

The task snapshot records entity version or artifact SHA-256, normalized content hash, classification, and whether the source was explicit. The aggregate fingerprint changes if a source version, classification, attachment, hash, or transitive provenance source changes. A changed task becomes `stale` before transmission. A reviewed candidate exposes a separate freshness check and stale candidates cannot be accepted.

## 5. Privacy Gateway

| Highest source class | Remote behavior |
|---|---|
| `public` | May route to an explicitly allowlisted profile. |
| `internal` | Project policy must enable remote use and a user-created, destination-specific project consent must exist. |
| `sensitive` | Requires a user-created consent bound to the exact task ID, destination profile, and source fingerprint. |
| `secret` / `never_send` | No route exists under any consent. |

Entity classification defaults to `internal`. Existing Artifact classes map `confidential` to `sensitive`. Derived entities inherit the highest class found in their bounded provenance closure. Common credential/private-key markers fail closed before transport. Consent can expire or be revoked immediately. Audit records contain IDs, fingerprints, policy decisions, sizes, timing, and sanitized errors—not prompts, response bodies, authorization headers, or credentials.

Credentials are represented only by `credential_ref`, an OS credential-store lookup name. Credential-shaped references and credential-bearing/query-string endpoints are rejected. `SemanticTransport` is the host boundary that resolves the lookup at call time and performs HTTPS; the core never receives or persists authorization headers.

## 6. Provider Registry and Routing

A profile stores its exact endpoint, model ID, adapter kind/version, enabled state, priority, capabilities, known deviations, and last-verified regional/retention/training reference. A profile is not routable until it is both enabled and explicitly included in the project policy allowlist.

Routing intersects:

1. project allowlist;
2. task allowlist and preferred profile;
3. enabled state;
4. declared task/text/structured-output capability and limits;
5. task/project budgets;
6. privacy and consent eligibility.

Unknown capability is unsupported. Ordering is deterministic: valid preferred profile, then priority, then stable profile ID. Automatic failover requires permission in both task and project policy. The project budget must cover the worst-case declared input plus output allowance across every planned failover attempt before any transport call occurs. Sensitive consent is exact-destination, so another provider cannot silently become eligible.

## 7. Adapter Contract

CP7 implements two independent envelopes:

- native Gemini `generateContent` request/response normalization;
- OpenAI-compatible Chat Completions request/response normalization.

OpenRouter, DeepSeek, BytePlus ModelArk, and future compatible services use distinct profiles and conformance evidence; wire similarity never grants undeclared capability parity. Adapter request fixtures assert schema placement. Response fixtures assert exact-model, finish state, usage, refusal/error, malformed output, and normalization behavior. Streaming is not advertised by the CP7 baseline profiles and cannot be inferred.

The transport trait makes network behavior testable and keeps OS credential resolution outside the ledger. CP7 conformance is proven with deterministic offline provider fixtures; live profile onboarding additionally requires user credentials and revalidation of the provider's current policy/capabilities.

## 8. Output Validation and Grounding

Output schemas use an allowlisted, bounded JSON Schema subset: object, array, string, number, integer, boolean, null, required/properties, `additionalProperties=false`, enum, and bounded size/length/range keywords. Unsupported schema keywords fail task creation.

Normalized output must:

- be valid JSON and no larger than 2 MiB;
- match the task schema;
- cite only IDs in the permitted source closure;
- attach non-empty `source_ids` to factual `claim` or `statement` objects;
- keep inference/unknown distinct from sourced fact;
- contain no executable HTML/URL/event-handler or Mermaid-init payload for report/diagram tasks.

Invalid output creates no candidate and no canonical mutation. It remains an immutable failed attempt with only sanitized metadata.

## 9. Candidate Review

Valid output creates a `pending` candidate with attempt, provider profile, source fingerprint, validation result, and generation time. Only a user actor can accept, edit-and-accept, or reject it using optimistic version checking. Human edits are revalidated. Stale output cannot be accepted. Rejected or edited candidates invalidate their model-output cache entry.

CP8 may consume pending/accepted report and diagram proposals through allowlisted renderer-neutral schemas. It must never render provider HTML, JavaScript, Mermaid source, URLs, or renderer configuration directly.

## 10. Cache and Failure

The local cache key covers task/contract, prompt/schema, source fingerprint, exact profile/provider/model/adapter/capabilities, policy version, requirements, route, and generation budgets. A cache hit creates a new task-specific attempt and candidate so provenance is never shared ambiguously. Expiry, policy version, source state, profile, prompt, or schema changes miss the cache. Cache metadata is LRU-bounded to 10,000 entries per project and expired entries are removed during cache writes; immutable attempts and reviewed candidates remain governed by audit retention rather than cache eviction.

Timeouts, refusals, quota responses, malformed JSON, compatibility drift, validation failure, and transport loss create no canonical mutation. Retry/failover is bounded to five attempts. Terminal attempts are immutable. Deterministic Continuum operations remain available when every provider is absent.

## 11. Persisted Schema v8

- `ai_provider_profiles`
- `ai_entity_classification`
- `ai_project_policy`
- `ai_consents`
- `ai_semantic_tasks`
- `ai_attempts`
- `ai_candidates`
- `ai_cache_entries`

Migration 0008 is additive. Existing CP1–CP6 entity and relationship rows are not rewritten.

## 12. CP8 Handoff

CP8 receives safe, structured `report_narrative` and `diagram_plan` candidates plus stable provenance. CP8 still owns the Human Document Model, HTML renderer, Mermaid graph specification, React Flow + ELK views, accessibility, export security, layout/resource tests, and human-facing freshness display. CP7 does not generate or execute raw presentation code.
