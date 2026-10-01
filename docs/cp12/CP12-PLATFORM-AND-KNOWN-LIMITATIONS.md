# CP12 Platform Support and Known Limitations

> **Windows pilot update — 2026-10-01:** Windows x64 now has a built/launched GNU NSIS package, platform adapters, packaged MCP/tools, and local Windows validation. This is not full Windows certification; physical microphone input on the reference VirtualBox guest remains unresolved. See the [current platform matrix](../architecture/CROSS-PLATFORM-ARCHITECTURE.md) and [Windows fix2 status](../releases/WINDOWS-0.12.0-fix2.md). The historical Linux qualification and remaining certification gates below are retained as evidence.

## Qualified Now

- Host: AMD Ryzen 5 5600H, 16 GB RAM, x86-64 Linux/Fedora 44.
- Formats built: x86-64 AppImage and amd64 Debian package.
- Verified: production compile, smoke launch, local project/recovery contracts, offline reports, semantic diagram palette, full automated regression, and integrated benchmark.

## Distribution Gates Still Required

- **Windows 11:** build, install/uninstall, WebView2, file picker, credential storage, capture source/audio behavior, sleep/resume, update, signing, and full regression on the ADR-002 first-certified platform.
- **Other Linux distributions:** clean-machine install/launch and dependency/capture verification on the explicitly supported matrix.
- **macOS:** not yet a declared supported MVP platform.
- **Signing/update:** Linux artifacts are unsigned pilot builds; no automatic update channel is enabled.
- **Live external services:** each enabled provider/model account still requires current privacy, retention, region, capability, cost, failure, and structured-output conformance. No provider is enabled by default.
- **Live MCP clients:** protocol fixtures pass for Codex, Claude Code, and Gemini CLI, but each then-current released client version must be rechecked before it is advertised as certified.
- **Capture:** sensor activation remains explicit. Actual system-audio availability, codecs, portal implementation, and long-duration stability depend on OS/session/device and require a physical-device matrix. Browser preview never proves native capture support.
- **Scale:** CP12 integrated measurements use 1,000 entities and 2,000 relationships. Standard-profile 50,000/150,000 and long-duration capture stress remain qualification work; bounded/paginated behavior is implemented, but Standard performance is not claimed from the smaller fixture.

## Non-Blocking Technical Notes

- In the 2026-09-15 workspace revision, React Flow/ELK are imported by the main workspace; Mermaid remains a separate report chunk. Graph (~504 KiB gzip) and Mermaid (~820 KiB gzip) chunks remain large, and the initial app chunk is ~84 KiB gzip excluding imported graph dependencies. First diagram load may be slower on cold disk; do not reuse the earlier lazy-graph/75 KiB claim for this revision.
- Current AppImage is about 112.4 MiB because it carries GTK/WebKit/media libraries for portability. The `.deb` is about 13.2 MiB and uses system dependencies.
- AppImage creation on this Fedora host needs `NO_STRIP=true` and the repository's pkg-config shim because upstream linuxdeploy ships an older strip tool and the `librsvg2-devel` metadata package is absent. Runtime librsvg is already installed.
- The desktop quick-start exposes the primary entry actions. Advanced lifecycle transitions and batch operations implemented in the core remain available through later dedicated UI expansion or permissioned interfaces; the release never fabricates a simplified canonical transition.

## Release Classification

The codebase is complete through CP12 and the current Linux artifact is suitable for a local pilot. It is not a signed, cross-platform general-availability release. These boundaries are explicit release evidence, not hidden defects.
