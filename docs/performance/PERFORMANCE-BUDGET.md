# Continuum Performance and Resource Budget

> **Status:** Approved target and benchmark contract  
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
- Context: tiered retrieval, hard budgets, streaming artifact excerpts, no whole-project prompt.
- Capture: segmented media, bounded encoder queue, backpressure/quality adaptation.
- AI: bounded candidate sets and concurrency; cache by source/version; provider time excluded.
- MCP: local stdio reads reuse bounded application queries; p95 server overhead for Current Project State or an existing Checkpoint first page is ≤ 500 ms beyond the equivalent query, excluding external-client/model latency.

## 7. Release Gate

CP12 release requires ≥90% of normal interactive operations to meet Standard-fixture p95 targets, no out-of-memory termination in the defined stress run, correct cancellation, and documented exception/mitigation for any miss. CP2 must establish benchmark harness hooks and meet core write/open/integrity budgets on the available reference-equivalent environment.

## 8. Implemented Evidence

- CP2 core-scale evidence: `docs/cp2/CP2-BENCHMARK-2026-08-31.json` and CP2.1 evidence.
- CP3 Research evidence: `docs/cp3/CP3-BENCHMARK-2026-09-02.json`.
- CP4 incremental Git evidence: `docs/cp4/CP4-BENCHMARK-2026-09-03.json`; 100 new commits plus file diffs completed in approximately 1.11 seconds on the reference Ryzen 5 5600H/16 GB machine, below the 60-second target.
- CP5 Code Intelligence evidence: `docs/cp5/CP5-BENCHMARK-2026-09-04.json`; a 500-file cold analysis completed in approximately 631 ms and one-changed-file incremental analysis over 501 files completed in approximately 200 ms with 500 cache hits and one miss, below the 5-second changed-file target.

These checkpoint measurements validate their declared fixtures only. CP12 retains full Standard/stress, peak-resource, cancellation, background-load, and Windows certification responsibility.
