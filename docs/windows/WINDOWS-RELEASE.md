# Continuum 0.12.0 for Windows x64

> **2026-10-01 update:** the latest local package is Windows **fix2**, including microphone device selection/gain/test/meter. Physical microphone input is still unresolved on the reference VirtualBox guest. See [fix2 evidence and hashes](../releases/WINDOWS-0.12.0-fix2.md), the [complete build guide](../development/BUILD-AND-DEVELOP.md), [cross-platform architecture](../architecture/CROSS-PLATFORM-ARCHITECTURE.md), and [release publishing](../releases/PUBLISHING.md). The validation counts below describe earlier repair stages.

The Windows app shares the Linux 0.12.0 domain, schema, frontend, commands and MCP protocol. Platform adapters and packaging differ. Linux portal/PipeWire/GStreamer capture and AppImage packaging remain in place.

## Build

Install Node.js 22, Rust and Microsoft C++ Build Tools with the Windows SDK. The default target is x86_64-pc-windows-msvc.

```powershell
cargo test --workspace
cd apps/continuum-desktop
npm ci
npm test
node scripts/prepare-windows.mjs
cd ../..
cargo test --manifest-path apps/continuum-desktop/src-tauri/Cargo.toml
cd apps/continuum-desktop
npm run build:windows
```

The preparation script builds the matching MCP server, stages checksum-pinned official portable tools and builds the frontend. NSIS installs for the current user and provisions WebView2 when missing. Installers appear under apps/continuum-desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/.

The Windows-native GNU alternative needs the x86_64-pc-windows-gnu Rust target and compatible MinGW-w64 GCC: run npm run build:windows:gnu. This migration was built/tested with that target. Include the generated WebView2Loader.dll in portable packages; the bare executable is incomplete. NSIS includes the loader. Linux cross-builds use npm run build:windows:cross, cargo-xwin and unzip; cross-building does not validate Windows runtime behavior.

Build tools use target/.tauri instead of a global cache. The GitHub Windows workflow builds/tests MSVC. Installers remain unsigned until the owner supplies a signing identity.

## Windows adapters

- Screen capture uses WebView2's Windows screen/window chooser and the shared segmented MediaRecorder pipeline. Linux retains its native portal recorder.
- System audio uses WASAPI loopback from the default output device, converted to 48 kHz stereo PCM16. Audio-only recording needs no screen chooser. AudioWorklet renders on the audio thread with a bounded 250 ms stereo queue and 100 ms prefill. Brief IPC/UI delays rebuffer with silence and discard stale backlog rather than stopping the recording. A genuinely stalled receiver or hardware error still stops capture safely. Older WebViews retain a bounded scheduling fallback.
- Microphone uses WebView2 with a per-request Windows permission prompt. Its parent window is resolved from the initialized WebView2 controller, avoiding a startup race with the runtime's raw window handle. Only the local app origin can receive permission. Camera and remote/lookalike origins are denied. Windows microphone privacy settings still apply. The shared frontend mixes microphone, system audio and screen sources.
- Official Git for Windows 2.56.0, Git LFS 3.8.0 and GitHub CLI 2.102.0 are bundled with licenses and URL/SHA-256 provenance in tools/UPSTREAM.json. Child processes get a private tool PATH; global PATH/Git config are unchanged. Existing installations remain fallbacks.
- GitHub device login opens through the Windows shell. Its CLI configuration is isolated under %LOCALAPPDATA%/continuum/github-cli. The asynchronous repository/branch picker and explicit snapshot publish/independent restore retain LFS media. No implicit merge or force-push was added.
- Codex, Claude and Gemini use the packaged Windows MCP executable beside its tools. Client discovery accepts .exe and npm .cmd shims. Windows Codex discovery also finds valid versioned binaries under %LOCALAPPDATA%/OpenAI/Codex/bin, newest first, even when PATH lacks the desktop app. An explicitly configured CODEX_HOME is preserved. Existing copied MCP connections may require reconnection; a real model tool call remains part of live acceptance.
- Folder names reject Windows reserved devices/invalid characters. Artifact and repository reads reject reparse-point files. Safe Unicode and spaces remain supported.
- Release apps and background Git/GitHub/AI processes hide console windows. Reports/browser links use Windows shell associations.

## Feature coverage

The shared source retains Research-only, Development-only and combined projects; questions/evidence; capture history/markers; original artifacts/integrity; workspace cards, connections, layout and undo/redo; bookmarks/checkpoints; Git observations/code intelligence; reports/exports; Messages; context packs; reviewable AI proposals; MCP grants/audit; backup/import/restore; and GitHub snapshots. No reduced Windows UI/database was introduced.

Transfer complete projects through export/import, including artifacts. Copying SQLite alone omits media. Repository and installed-client paths are machine-specific; relocate/reconnect after moving between operating systems.

## Validation and remaining checks

Windows baseline validation: 149 core/MCP tests passed. The subsequent audio/layout/Codex repair passed 91 frontend tests and 16 desktop tests. The installed Codex CLI also passed an isolated MCP configuration add/get/remove roundtrip; WASAPI captured a generated tone and restarted successfully. A production AudioWorklet browser recording survived 650 ms and 900 ms UI stalls and decoded as stereo audio. Production frontend builds passed. The delivery report records packaged-server and installer results.

Automated contracts do not certify every physical device/account. On a disposable project, complete these live checks before claiming full platform equivalence:

1. Install, launch, reopen, uninstall/reinstall preserving projects; offline launch after WebView2 installation.
2. Create/reopen all project modes. Exercise import/paste/drag, workspace history, reports, bookmarks, Messages and integrity.
3. Record/save/replay/reopen screen, microphone, system audio and all combinations. Check cancellation, denial, device removal, sleep/resume and long sessions.
4. Attach/analyze/relocate a repository. Sign into GitHub, choose a real repository/branch, publish media and restore independently.
5. Connect a real AI CLI, perform a scoped MCP call, review proposals and revoke access.
6. Export Linux -> import Windows -> export Windows -> import fresh Linux. Compare artifact hashes, history, reports, layouts and bookmarks; preserve originals.
7. Check Windows 10/11 x64, standard-user permissions, Indonesian/space-containing paths, accessibility and unsigned-installer behavior.

Unchecked physical capture, real-account integration, cross-machine round-trip, signing and soak tests must remain explicitly unverified.
