# Windows 0.12.0 fix2 — unsigned pilot

Date: 2026-10-01. Application version remains `0.12.0`; `fix2` identifies the Windows package revision, not a database version or a claim of a signed stable release.

## Package

[Download Windows fix2](https://github.com/WiefranVarenzo/Continuum/releases/download/v0.12.0-windows-fix2/Continuum_0.12.0_Windows-fix2_x64-setup.exe) · [Existing Linux AppImage](https://github.com/WiefranVarenzo/Continuum/releases/download/v0.12.0-linux-pilot-2026-10-01/Continuum_0.12.0_amd64.AppImage). The Windows publication adds a separate prerelease; Linux assets are retained.

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `Continuum_0.12.0_Windows-fix2_x64-setup.exe` | 60,893,044 | `4fc2f79d7602011be0ec1208ce33c5222645a6be45b92cee5c1d6d67acfc4f34` |

Download `Continuum_0.12.0_Linux-Windows-source.zip` and its separate checksum from the Windows release, or clone/fork this repository. The source ZIP is generated with `git archive` from the published source commit; it excludes build outputs, dependency caches, and user data. It includes the later documentation/infrastructure changes, which do not alter the packaged GUI's compiled application logic. Compiler/environment differences mean rebuilding need not produce byte-identical installers.

## Changes included

- Preserves fix1's bounded system-audio buffering/recovery, responsive layout/font adjustments, and Windows Codex executable discovery.
- Adds explicit microphone device choice; a disappeared selected input is not silently substituted.
- Requests microphone conferencing filters off where supported.
- Adds microphone gain 25–400%, a microphone-only signal meter, mute/silence/suspended-context diagnostics, and a cancellable ten-second input test that saves no recording.
- Keeps microphone/system input separate until mixing; holds audio nodes for the lifetime of the recording and releases them on stop/error/cancellation.
- Uses the same project schema, domain features, and local MCP protocol as the Linux codebase.

## Evidence

Windows local build target: `x86_64-pc-windows-gnu`, Rust 1.98.1, Node 22.23.3, compatible MinGW-w64. Release build, frontend production build, and NSIS packaging succeeded. All 97 frontend tests passed; final focused microphone/mixer/capture tests passed 31 tests. Earlier Windows baseline core/MCP tests passed 149 and desktop tests passed 16; they are historical validation, not a fresh run for this documentation update.

Fresh integrated-source checks on 2026-10-02 passed: 149 core/MCP tests (one opt-in packaged-server test ignored), all 97 frontend tests, production frontend build, and 16 Windows GNU desktop tests (four opt-in device/login integration tests ignored). Eleven documentation files, 83 local links, and seven Mermaid diagrams validated. The first frontend run encountered a timeout while builds competed for VM resources; its sequential rerun passed all tests. Desktop tests passed after generating their required frontend build. Linux native recorder/audio/permission files, Linux media packaging scripts, and historical release files have no changes relative to the GitHub base commit. These local checks do not constitute a Linux build or a hosted CI result.

Actual production frontend microphone/mixer modules and system AudioWorklet passed five synthetic browser scenarios: meter without recorder, microphone-only, microphone+system, screen+both, and screen+both with microphone gain 200%. Decoding recorded WebM/Opus recovered both generated frequencies in the mixed audio; gain doubled the microphone contribution without raising system-audio gain. These were synthetic sources, not a physical-microphone test.

The installed GUI's SHA-256 matches the release executable: `8d75b5224904351d2de9e45bf4aeda38baf59efc08400eaddc88484eeb9458c8`. The reference installation was updated by backing up/replacing the GUI after sandbox installer runs did not replace the normal user's executable. Therefore this launch proves the installed build, not a clean-user installer/uninstaller acceptance cycle. A real installed Codex tool call was verified during the preceding Windows repair. User reports confirm screen and system-audio recording work on that VM.

## Open issues and limits

**Physical microphone remains unresolved on the reference Windows VirtualBox guest.** Microphone-only recording and the new Test microphone both receive no detected signal. The Windows endpoint exists, is not muted, and reports 67% input volume; host microphone forwarding and Windows' own recording test are not yet verified. A sandbox-native diagnostic was denied with `0x80070005`, so it supplied no signal samples and cannot distinguish a silent endpoint from a WebView2 issue.

Users should test the Windows input first, then inspect VirtualBox Audio Input/host microphone routing or Continuum's selected input/desktop permissions as appropriate. Do not describe this package as having fixed physical microphone input. See [installation/troubleshooting](../INSTALLATION.md).

The installer is unsigned. Signing, automatic updates, broad Windows 10/11 hardware coverage, sleep/resume/device-switch stress, full install/uninstall cycles, and Linux↔Windows project round-trip acceptance remain qualification work. macOS/ARM64 are not advertised. New hosted CI definitions are not completed CI runs.

## Release placement

Publish this installer and its checksum as a Windows pilot asset alongside the existing Linux release, or as an explicitly named Windows prerelease without deleting/replacing Linux assets. Update the README's download link/status only after confirming the asset is available. Users should download binaries from Releases and clone/fork the repository for modifications.
