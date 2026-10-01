# CP12 Validation

> **2026-09-22 graph/export/runtime revision:** The complete Rust workspace now passes 141 tests and the UI passes 33 tests. A focused regression proves Research Session → Question/Evidence graph relationships, bounded screenshot thumbnails, and offline HTML relationship rendering. HTML/Markdown publication creates normal files under the project's `exports/reports/` directory. The rebuilt AppImage contains an executable `gst-plugin-scanner`; a packaged smoke launch from the Second Brain project produced no scanner or PipeWire warning. A real OS screen/microphone chooser interaction remains a manual device-session check because it cannot be granted noninteractively.

> **2026-09-15 workspace revision:** The activity-based workspace passes the UI build, 29 UI tests, two focused native preview tests, and browser interaction checks with simulated desktop IPC. Both Linux packages were rebuilt and the final AppImage was launched without startup output/errors during observation. Detailed native capture/device/client verification remains outstanding. See [workspace revision status](CP12-WORKSPACE-REVISION.md). The historical evidence below describes the earlier baseline; its complete regression/performance suite was not repeated for this UI revision.

> **Date:** 2026-09-12  
> **Result:** PASS for Linux pilot release 0.12.0

## Automated Evidence

- Rust workspace: 140 tests passed, zero failed.
- UI: 21 tests in 9 files passed, zero failed.
- Strict lint: workspace and desktop Tauri crate passed with warnings denied.
- Production UI and Tauri release builds passed.
- npm dependency audit: zero known vulnerabilities.
- Credential/private-key pattern scan: no findings.
- Cargo license metadata: no dependency lacked both license and license-file metadata.
- Patch whitespace validation: passed.

## Recovery and Security Evidence

CP12 tests verify a configured Connected R&D project can create a consistent database backup, export the complete project, restore it with the same project identity and Space capabilities, pass integrity verification, close, and reopen. Adversarial paths nested within their own source project/export are rejected before copy. Earlier migration, tamper, artifact corruption, prompt-injection, unsafe URL/HTML, cross-project authorization, secret classification, capture permission, rate/size, and proposal tests remain in the full regression.

The desktop uses native directory selection only to choose a path. Dedicated Rust commands canonicalize/validate paths and perform every create/backup/export/restore operation. CSP remains restrictive and no shell, raw SQL, unrestricted filesystem, remote script, or direct model write is exposed.

## Performance Evidence

Release benchmark on AMD Ryzen 5 5600H / 16 GB, Linux x86-64, 1,000 entities, 2,000 accepted relationships, Research + Development enabled, 3 warmups and 30 samples:

- integrated Human Document compose + offline HTML/Markdown render: p50 1.714 s, p95 1.755 s, max 1.758 s (target ≤ 5 s);
- full integrity diagnostics: p50 231 ms, p95 240 ms, max 241 ms (CP12 fixture target ≤ 5 s);
- consistent SQLite backup: 254 ms;
- complete verified export: 484 ms;
- identity-preserving verified restore: 235 ms;
- measured process high-water increase across sampled operations: 116 KiB.

This proves the named integrated pilot fixture only. It is not presented as Standard 50k/150k certification.

## Package Evidence

- `Continuum_0.12.0_amd64.deb`: approximately 11 MiB; valid ar archive with Debian binary/control/data members.
- `Continuum_0.12.0_amd64.AppImage`: approximately 110 MiB; valid x86-64 Type 2 AppImage runtime; smoke-launched successfully on the reference host.
- Durable release copies and checksums are stored in `releases/0.12.0`; the build-output copies remain under the Tauri target directory.

## Visual QA

The report was rendered in a narrow dark-mode viewport. React Flow nodes showed distinct Research/Decision/Development colors; the former white minimap/control block was replaced with themed chrome; Mermaid rendered the Connected R&D loop with semantic fills, legend, horizontal pan, zoom/reset controls, and no console warnings/errors. Narrow layout remained readable with deliberate horizontal diagram scrolling rather than compressed illegible text.

## Decision

All software-controlled Linux pilot gates are closed. Formal Windows-first certification, signed public distribution, live provider/client versions, physical capture-device combinations, and Standard-scale stress are explicit subsequent qualification gates. Continuum 0.12.0 is therefore usable now as an unsigned local Linux pilot, while no unsupported cross-platform or scale claim is made.
