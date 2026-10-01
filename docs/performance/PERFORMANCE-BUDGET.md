# Continuum Performance and Resource Budget

> **Status:** Approved target and benchmark contract; component evidence through CP12 Linux pilot
> **Reference device:** AMD Ryzen 5 5600H, 16 GB RAM, SSD, no discrete GPU requirement.

## 1. Principles

Interactive work outranks indexing, capture processing, and AI preparation. Work is incremental, streamed, paginated, bounded, cancellable, and cached only with explicit limits. A useful partial result is preferred to UI blocking.

## 2. Fixtures

- Small: 5k entities, 10k relationships, 10k repository files.
- Standard: 50k entities, 150k relationships, 50k files, 10k commits, 5 GB referenced artifacts.
- Stress: 250k entities, 1m relationships, 150k files, 50k commits, 50 GB artifacts; graceful degradation required, interactive SLO not guaranteed.

Golden repository corpus combines TypeScript/JavaScript and Rust with npm/Cargo manifests, Vitest/Jest-style tests, and Rust tests. Synthetic deterministic generators version entity/relationship/media metadata fixtures.

## 3. Interactive SLOs on Standard Fixture

- warm usable overview p95 ≤ 3 s;
- cold open excluding migration/rebuild p95 ≤ 8 s;
- entity write p95 ≤ 150 ms;
- first search page p95 ≤ 500 ms;
- one-hop graph traversal p95 ≤ 300 ms;
- 500-node/1,500-edge scoped graph first useful render p95 ≤ 2 s;
- cached scoped HTML report first useful overview p95 ≤ 2 s;
- deterministic normal-scope Human Document composition and local HTML export p95 ≤ 5 s, excluding AI/network latency;
- existing Checkpoint overview p95 ≤ 1 s;
- deterministic Context Pack selection p95 ≤ 2 s;
- changed-file analysis first useful result ≤ 5 s;
- 100 new commits incremental ingestion ≤ 60 s.

Provider/network latency is reported separately from local processing.

## 4. Resource Budgets

- idle application target ≤ 500 MB RSS;
- typical interaction target ≤ 1.5 GB RSS;
- heavy indexing/capture soft ceiling ≤ 4 GB process-group RSS;
- UI/main process remains responsive under background load;
- default background CPU concurrency ≤ 50% logical cores, configurable and adaptive;
- write queue and media/context buffers are bounded;
- derived cache default maximum is the lesser of 5 GB or 10% free disk, user-configurable;
- within that shared budget, the implemented CP5 analyzer cache has a stricter 256 MiB LRU fail-safe with persisted byte/access metadata; later configuration may lower it, while raising it requires new evidence;
- SQLite connections and graph nodes are bounded/paginated.

At soft limits, Continuum throttles workers, evicts rebuildable cache, reduces preview quality, pauses nonessential analysis, and informs the user. It does not kill canonical transactions or silently drop capture data.

## 5. Benchmark Protocol

- record OS/build, CPU, RAM, storage, power mode, fixture version, database/analyzer versions;
- measure cold and warm separately;
- five warmups where applicable, then at least 30 samples for interactive p95;
- report p50, p95, max, peak RSS, CPU time, disk reads/writes, and result correctness;
- disable network measurements from local SLO aggregation;
- run while a bounded background job is active to test responsiveness;
- verify cancellation and post-cancellation integrity;
- store results as versioned CP12 evidence.

## 6. Component Strategies

- SQLite: WAL, prepared statements, short writes, indexed pagination, query plans in benchmark regressions.
- Artifact Store: streaming hash/copy, content deduplication, no whole-media buffering.
- Git/code: ingest new commits and changed files only; cache by content hash and analyzer version.
- Graph: start from task scope, cap visible nodes, expand on demand, compute ELK layout off UI thread.
- Human Documentation: render overview first, paginate or virtualize tables/timelines, lazy-load local media, scope diagrams before layout, cache only rebuildable outputs with explicit limits, and never publish a valid-looking partial export.
- Context: tiered retrieval, hard budgets, streaming artifact excerpts, no whole-project prompt.
- Capture: segmented media, bounded encoder queue, backpressure/quality adaptation.
- AI: bounded candidate sets and concurrency; cache by source/version; provider time excluded.
- MCP: local stdio reads reuse bounded application queries; p95 server overhead for Current Project State or an existing Checkpoint first page is ≤ 500 ms beyond the equivalent query, excluding external-client/model latency.

## 7. Release Gate

CP12 release requires ≥90% of normal interactive operations to meet Standard-fixture p95 targets, no out-of-memory termination in the defined stress run, correct cancellation, offline HTML export and safe fallback under renderer failure, and documented exception/mitigation for any miss. CP2 must establish benchmark harness hooks and meet core write/open/integrity budgets on the available reference-equivalent environment.

## 8. Implemented Evidence

- CP2 core-scale evidence: `docs/cp2/CP2-BENCHMARK-2026-08-31.json` and CP2.1 evidence.
- CP3 Research evidence: `docs/cp3/CP3-BENCHMARK-2026-09-02.json`.
- CP4 incremental Git evidence: `docs/cp4/CP4-BENCHMARK-2026-09-03.json`; 100 new commits plus file diffs completed in approximately 1.11 seconds on the reference Ryzen 5 5600H/16 GB machine, below the 60-second target.
- CP5 Code Intelligence evidence: `docs/cp5/CP5-BENCHMARK-2026-09-04.json`; a 500-file cold analysis completed in approximately 631 ms and one-changed-file incremental analysis over 501 files completed in approximately 200 ms with 500 cache hits and one miss, below the 5-second changed-file target.
- CP7 Semantic Intelligence evidence: `docs/cp7/CP7-BENCHMARK-2026-09-08.json`; 30 task-specific cache-hit executions had p50 approximately 18.43 ms and p95 approximately 19.77 ms, with zero canonical mutations. This measures local gateway/validation/cache overhead only; live provider latency remains separately reported.
- CP8 Visual Intelligence evidence: `docs/cp8/CP8-BENCHMARK-2026-09-08.json`; a 300-entity/600-relationship Human Document with a bounded 200-node graph composed and rendered to HTML plus Markdown at p50 approximately 359.05 ms and p95 approximately 371.83 ms on the reference device, below the 5-second target. Renderer-only p95 was approximately 4.58 ms and the offline dependency count was zero.
- CP9 Research Capture evidence: `docs/cp9/CP9-BENCHMARK-2026-09-09.json`; 30 unique 512 KiB capture fragments persisted at p50 approximately 10.30 ms and p95 approximately 11.00 ms on the reference device, below the local 500 ms gate. The 15 MiB fixture completed with marker-to-Evidence traceability, a healthy integrity scan, and an approximately 1,080 KiB measured high-water delta; CP12 retains long-duration/process-group/platform qualification.
- CP10 Checkpoint & Context evidence: `docs/cp10/CP10-BENCHMARK-2026-09-09.json`; over 750 canonical records and 50 release-mode samples, Current Project State completed at p50 approximately 12.21 ms / p95 14.12 ms against the 1-second target, while deterministic Context Pack selection completed at p50 approximately 73.89 ms / p95 78.62 ms against the 2-second target. Fingerprints remained stable, budgets/omissions were enforced, final integrity was healthy, and measured high-water delta was approximately 1,932 KiB; CP12 retains Standard/stress/process-group qualification.
- CP11 MCP interface evidence: `docs/cp11/CP11-BENCHMARK-2026-09-10.json`; over 100 measured release-profile local STDIO calls after warmup, capability-filtered tool catalog completed at p50 approximately 3.56 ms / p95 3.99 ms against 50 ms, and bounded current-state responses at p50 approximately 6.82 ms / p95 7.59 ms against 500 ms. Initialization was approximately 3.55 ms and measured resident-memory delta approximately 276 KiB. CP12 retains Standard/stress, live-client, process-group, and soak qualification.
- CP12 integrated Linux-pilot evidence: `docs/cp12/CP12-BENCHMARK-2026-09-12.json`; over 1,000 entities, 2,000 relationships, both Spaces, and 30 release-mode samples, Human Document composition plus offline HTML/Markdown rendering completed at p50 approximately 1.714 s / p95 1.755 s against 5 s, and full integrity diagnostics completed at p50 231 ms / p95 240 ms. Backup completed in approximately 254 ms, full export in 484 ms, and verified identity-preserving restore in 235 ms. This is an integrated pilot fixture, not the Standard 50k/150k claim.

These checkpoint measurements validate their declared fixtures only. The Linux pilot passes its named integrated fixture. Full Standard/stress, peak process-group, background-load/soak, and Windows certification remain explicit GA qualification gates.
