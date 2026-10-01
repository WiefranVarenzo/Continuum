# Build, run, and customize Continuum

Run repository-level commands from the cloned repository root. Frontend/Tauri commands run from `apps/continuum-desktop`. The desktop Rust crate is excluded from the root Cargo workspace: test it separately. Keep both Cargo lockfiles and `package-lock.json` committed.

## Toolchain

| Component | Requirement / purpose |
| --- | --- |
| Git | Source checkout and Development/Git workflows |
| Node.js | Node 22.12+ within Node 22; npm installs locked React/Vite/TypeScript dependencies |
| Rust | Stable with edition 2024, minimum declared MSRV 1.88; Cargo builds core, MCP, and Tauri |
| Windows native build | MSVC C++ Build Tools, Desktop development with C++, Windows SDK, WebView2 |
| Linux native build | Compiler, pkg-config, GTK3, WebKitGTK 4.1, SSL, AppIndicator, librsvg, GStreamer and PipeWire plugins |

The Windows fix2 local release was built with Rust 1.98.1, Node 22.23.3, and a compatible MinGW-w64 GNU toolchain. The CI workflow uses stable Rust and MSVC on Windows. These are different build targets; a successful GNU build does not certify the MSVC artifact.

```bash
git clone https://github.com/WiefranVarenzo/Continuum.git
cd Continuum
cd apps/continuum-desktop
npm ci
npm test
npm run build
```

`npm run dev` serves the browser UI, useful for layout work. Browser preview cannot certify native IPC, real capture, permissions, Git credentials, or MCP integration. Use `npm run tauri -- dev` for a real desktop development window; the configured Vite port is 1420. Install your OS dependencies first.

## Windows: MSVC build

Install the tools from official sources; use a fresh terminal where Rust and the C++ SDK are discoverable. Tauri's [prerequisites](https://v2.tauri.app/start/prerequisites/) explain the Windows toolchain.

```powershell
rustup target add x86_64-pc-windows-msvc
cargo test --workspace --locked
Set-Location apps/continuum-desktop
npm ci
npm test
$env:TAURI_ENV_TARGET_TRIPLE = 'x86_64-pc-windows-msvc'
node scripts/prepare-windows.mjs
Remove-Item Env:\TAURI_ENV_TARGET_TRIPLE
cargo test --locked --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc
npm run build:windows
```

The preparation script builds the matching MCP companion, verifies/downloads official portable Git/LFS/GitHub CLI archives, stages resources, and builds the frontend. `tauri.windows.conf.json` is automatically selected by Tauri on Windows and replaces the Linux preparation/resources. NSIS tools use the local `.tauri` cache. The package is created at:

```text
apps/continuum-desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/*-setup.exe
```

Close any test app using the release executable before packaging; an open Windows executable can lock the NSIS input. Distribution includes `continuum-mcp.exe`, `tools/`, and required runtime files. The NSIS configuration installs for the current user and downloads WebView2's bootstrapper when required.

## Windows: GNU alternative

Install `x86_64-pc-windows-gnu` with rustup and a compatible MinGW-w64 GCC/binutils toolchain. Ensure `gcc`, `ar`, Cargo, and Node are on the terminal PATH; mixed or obsolete GCC installations can cause linker failures.

```powershell
rustup target add x86_64-pc-windows-gnu
$env:TAURI_ENV_TARGET_TRIPLE = 'x86_64-pc-windows-gnu'
node scripts/prepare-windows.mjs
Remove-Item Env:\TAURI_ENV_TARGET_TRIPLE
cargo test --locked --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-gnu
npm run build:windows:gnu
```

Run those commands from the desktop directory after `npm ci`. Installer output substitutes `x86_64-pc-windows-gnu` in the MSVC path. Include the generated `WebView2Loader.dll` in any deliberately assembled portable distribution. Prefer NSIS over distributing a bare executable.

Linux-to-Windows cross-build is an advanced option: install `cargo-xwin` and `unzip`, then run `npm run build:windows:cross`. For pre-build native tests/preparation on Linux explicitly provide `TAURI_ENV_TARGET_TRIPLE=x86_64-pc-windows-msvc`. SDK downloading and cross-compilation do not test Windows runtime behavior.

## Linux: native dependencies

Example for Ubuntu 24.04 / compatible Debian-based build environments:

```bash
sudo apt update
sudo apt install --no-install-recommends \
  build-essential pkg-config curl wget file patchelf \
  libgtk-3-dev libwebkit2gtk-4.1-dev libssl-dev libxdo-dev \
  libayatana-appindicator3-dev librsvg2-dev libfuse2t64 \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-tools gstreamer1.0-plugins-base \
  gstreamer1.0-plugins-good gstreamer1.0-plugins-bad \
  gstreamer1.0-plugins-ugly gstreamer1.0-libav \
  gstreamer1.0-pipewire gstreamer1.0-gl pulseaudio-utils
```

Older Ubuntu/Debian releases may name FUSE's compatibility package `libfuse2`; choose the package for your distribution. A real desktop also needs PipeWire, xdg-desktop-portal, and its GNOME/KDE/other backend. A CI container does not provide a user desktop session.

On Fedora install the equivalent development packages, including `webkit2gtk4.1-devel`, `gtk3-devel`, `openssl-devel`, `libappindicator-gtk3-devel`, `librsvg2-devel`, `libxdo-devel`, `gstreamer1-devel`, `gstreamer1-plugins-base-devel`, the base/good/bad/GL/PipeWire media plugins, and PulseAudio utilities. Package names/codec repositories vary. See [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) and verify all required plugins listed in `packaging/prepare-gstreamer-plugins.sh`.

## Linux: checks and packages

```bash
cargo test --workspace --locked
cargo build --locked --release -p continuum-mcp --bin continuum-mcp
bash packaging/prepare-gstreamer-plugins.sh
cd apps/continuum-desktop
npm ci
npm test
npm run build
cargo test --locked --manifest-path src-tauri/Cargo.toml
cd ../..
chmod +x packaging/prepare-gstreamer-plugins.sh packaging/check-recording-runtime.sh
cd apps/continuum-desktop
npm run build:linux
```

The Linux before-build command builds `continuum-mcp`, stages the required scanner/media plugins, and builds the frontend. Outputs use the native host target directory:

```text
apps/continuum-desktop/src-tauri/target/release/bundle/appimage/*.AppImage
apps/continuum-desktop/src-tauri/target/release/bundle/deb/*.deb
```

AppImage is built for the build host's ABI/glibc baseline. Building on a newer distribution can limit portability to older systems. Test the actual package on supported hosts. The historical Fedora build used `NO_STRIP=true` and `PKG_CONFIG_PATH` pointing at `packaging/pkgconfig` to work around its older linuxdeploy strip tool/missing librsvg metadata. Those are host-specific fallbacks, not normal requirements on a clean Ubuntu builder. The shim does not replace the actual librsvg runtime.

For a packaged AppDir runtime probe see `packaging/check-recording-runtime.sh` and `apps/continuum-desktop/qa/recording_probe.py`. Synthetic media tests open no physical sensors and complement, rather than replace, the live device/portal matrix.

## Source map for customization

| Change | Start here |
| --- | --- |
| App navigation/onboarding | `src/App.tsx`, `src/ProjectLauncher.tsx`, `src/ReleaseCenter.tsx` |
| Board/research/report presentation | `src/Workspace.tsx`, `src/ResearchBoard.tsx`, `src/ReportStudio.tsx` and related CSS |
| Capture controls/mixing | `src/CapturePanel.tsx`, `src/captureMedia.ts`, `src/microphone.ts`, `src/systemAudioProcessor.ts` |
| Desktop command boundary | `src-tauri/src/main.rs`, `workspace.rs`, `assistant.rs` |
| Platform behavior | `src-tauri/src/platform.rs`, `native_screen.rs`, `system_audio_windows.rs`, `media_permissions_windows.rs` |
| Canonical rules, persistence, migrations | `crates/continuum-core/src/`, `crates/continuum-core/migrations/` from repository root |
| AI protocol/grants/proposals | `crates/continuum-mcp/src/`, `crates/continuum-core/src/mcp.rs` |
| Packages/resources | `src-tauri/tauri.conf.json`, `tauri.windows.conf.json`, `scripts/prepare-windows*.mjs`, repository `packaging/` |

Paths beginning with `src/` or `src-tauri/` in the table are relative to `apps/continuum-desktop`. Fork, create a branch, change the relevant layer, and run checks before rebuilding. If distributing your own independently branded application, update its product name, Tauri identifier, icons, and package metadata consistently. Preserve migration compatibility and test existing project exports with copies.

## Validation gates

Run core tests, frontend tests/build, and the separate desktop Rust tests for the target you changed. `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` check the root workspace; apply equivalent manifest-path checks to the desktop crate when changing native code. Existing historical validation counts are not fresh CI results.

Run a package smoke test with a disposable project, then validate the affected physical devices, denied/cancelled permissions, shutdown cleanup, and data restoration. Never test migrations or restore workflows against the only copy of a user's project.

See [architecture](../architecture/CROSS-PLATFORM-ARCHITECTURE.md), [contribution guide](../../CONTRIBUTING.md), and [release workflow](../releases/PUBLISHING.md).
