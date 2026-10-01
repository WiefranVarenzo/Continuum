# Install and use Continuum

## Choose a package

Open [Continuum Releases](https://github.com/WiefranVarenzo/Continuum/releases), read the release's platform notes, and download the matching package and checksum file. Linux uses AppImage or Debian packages; Windows uses a `*-setup.exe` NSIS installer. Development dependencies are only needed when editing/building source.

The latest local Windows package is `Continuum_0.12.0_Windows-fix2_x64-setup.exe`, published as a separate [Windows pilot release](https://github.com/WiefranVarenzo/Continuum/releases/tag/v0.12.0-windows-fix2). Its checksum and open microphone issue are recorded in [fix2 notes](releases/WINDOWS-0.12.0-fix2.md). The existing [Linux AppImage release](https://github.com/WiefranVarenzo/Continuum/releases/tag/v0.12.0-linux-pilot-2026-10-01) remains available separately.

## Windows x64

1. Download the complete setup executable. Verify its hash using PowerShell:

   ```powershell
   Get-FileHash -Algorithm SHA256 -LiteralPath .\Continuum_0.12.0_Windows-fix2_x64-setup.exe
   ```

2. Run setup under your normal Windows account. It installs for the current user and creates the application entry. The default installation is `%LOCALAPPDATA%\Continuum`; launch Continuum from Start.
3. Setup provisions Microsoft WebView2 using the Evergreen bootstrapper if needed. That initial installation can require internet access. Continuum's local workspace can run offline after its runtime is installed.
4. Choose **Create** or **Open**, then choose a project folder outside the installation folder. Back up existing projects before installing a newer build.

The pilot installer is unsigned; Windows may identify an unfamiliar publisher. Validate the release origin/hash and follow your organization's installation policy. The installer includes its MCP server and portable Git, Git LFS, and GitHub CLI with their upstream licenses. They are used through a private child-process PATH, not a global PATH replacement. AI clients themselves are not bundled.

For upgrades, save work, stop active recordings, close Continuum, then run the newer installer as the same user. Preserve project folders. Automatic updates, signed distribution, and a full install/uninstall matrix are not claimed for this pilot. Avoid copying only the GUI executable: it depends on packaged resources and, on GNU builds, the WebView2 loader.

## Linux x86-64

```bash
chmod +x Continuum_0.12.0_amd64.AppImage
./Continuum_0.12.0_amd64.AppImage
```

Use the actual filename from the release if it includes a platform suffix. AppImage needs a compatible Linux desktop and may require your distribution's FUSE support. If FUSE is unavailable, the AppImage runtime commonly supports extraction and execution:

```bash
./Continuum_0.12.0_amd64.AppImage --appimage-extract
./squashfs-root/AppRun
```

On a Debian-based distribution, install the matching `.deb` with `sudo apt install ./<downloaded-file>.deb`. The original qualification host was Fedora x86-64; other distributions still need launch and capture verification. GTK/WebKit/media compatibility is not guaranteed merely by using AppImage.

Screen recording depends on xdg-desktop-portal and the desktop's portal backend, PipeWire, and packaged GStreamer plugins. System audio depends on a working PulseAudio-compatible audio service and `parec`; the native Linux path also uses `parec` for combined system and microphone audio. Install `pulseaudio-utils` on Debian/Ubuntu or the matching PulseAudio utilities package on your distribution. A headless CI build cannot prove portal/device recording.

## First project

Choose **Research only**, **Development only**, or **Connected R&D**. A project contains `continuum.project.json`, `ledger.sqlite3`, and `artifacts/sha256/` plus supporting folders. Evidence, history, and media belong in that project. The application's recent-project list/client configuration are machine-specific settings.

- Research: add a session/question, attach evidence, and record findings and decisions.
- Development: attach a Git repository and inspect a committed baseline.
- Workspace: organize cards and links; save bookmarks and resume/context state.
- Reports: preview and export HTML/Markdown documentation.
- Backup/restore: use the app's complete export/import flow, including original media. Copying only SQLite loses artifacts.

Use independent export/import copies when moving between Linux and Windows. Repository paths and AI executable paths must be relocated/reconnected. Compare integrity diagnostics and artifact hashes after transfer; a complete physical cross-machine round trip remains a qualification check.

## Capture and microphone troubleshooting

Select sources and confirm capture consent. OS permission and chooser dialogs still require your action. Test microphone input before a combined recording. Windows fix2 provides an input selector, 25–400% microphone gain, a signal meter, and a cancellable ten-second test without saving a recording.

The reference VirtualBox guest still reports **No microphone signal detected**, even for microphone-only input. Windows detects its input endpoint at 67% volume and not muted, which does not prove audio is arriving. Test **Settings → System → Sound → Input → Microphone → Start test**. If Windows also receives no signal, check the host microphone and VirtualBox's separate **Audio Input** setting. If Windows receives sound while Continuum does not, report that distinction and the selected device. Check Windows desktop microphone permissions and the app's own permission prompt. Screen/system audio were reported working after fix1.

## Connect an AI

Install/sign into a supported local client separately. In **AI connections**, select the client and connect; Continuum creates a scoped, expiring project grant and installs its local STDIO MCP entry. Restart the client or start a fresh task if it was already running, then make a real Continuum tool call. An installed entry and a proven tool call are separate states. Disconnect revokes the grant. Local STDIO configuration does not automatically enable a separate ChatGPT web session.

For grants and manual configuration see [CP11 configuration](cp11/CP11-CONFIGURATION.md). Do not publish grant tokens, project exports, or account credentials in this source repository.

## References

- [Microsoft microphone testing](https://support.microsoft.com/en-us/windows/hardware/drivers/how-to-set-up-and-test-microphones-in-windows)
- [VirtualBox audio configuration](https://docs.oracle.com/en/virtualization/virtualbox/7.2/user/working-with-vms.html)
- [Tauri Windows installer / WebView2 packaging](https://v2.tauri.app/distribute/windows-installer/)
- [Tauri AppImage distribution](https://v2.tauri.app/distribute/appimage/)
