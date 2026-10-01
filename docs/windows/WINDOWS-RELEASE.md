# Continuum for Windows x64 — build and release gate

This document describes the Windows port, not a claim that an installer has already been validated on Windows. Keep the existing Linux AppImage build and its PipeWire recorder unchanged. The Windows package is an NSIS `*-setup.exe` installer built with Tauri's `tauri.windows.conf.json` overlay; its bundled `continuum-mcp.exe` must be from the same Windows target.

## Build on a Windows machine

Install Microsoft C++ Build Tools, Rust with `x86_64-pc-windows-msvc`, Node.js 22, and Git for Windows. Tauri's NSIS installer will arrange the WebView2 runtime when necessary. From the repository root:

```powershell
cd apps/continuum-desktop
npm ci
npm test
cd ../..
cargo test -p continuum-core --lib
cd apps/continuum-desktop
npm run build:windows
```

The installer should appear under `apps/continuum-desktop/src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/`. Do not distribute a build merely because these commands succeed. The installer is unsigned until a trusted Windows code-signing process is configured. A manually triggered Windows workflow in `.github/workflows/windows-desktop.yml` runs these steps and uploads the unsigned installer artifact after the repository is connected to GitHub.

## Windows behavior

- Research projects, evidence originals, workspace positions/connections, bookmarks, context, messages, markdown reports and exports use the same project format as Linux. Move a complete project through Continuum's project export/import; do not copy only its SQLite file and expect media to follow.
- Windows capture uses the operating system's WebView2 share dialog and `MediaRecorder`. Linux remains on its native portal/PipeWire implementation. The capture UI must ask permission from the user for each recording. A selected source that does not supply an audio track fails rather than being labeled as a successful system-audio recording.
- Microphone needs Windows Settings → Privacy & security → Microphone access enabled. System audio requires a share choice that offers audio, with **Share audio** enabled in the chooser. Availability may vary by the selected screen/window and installed WebView2 runtime. Do not promise all three sources until the actual Windows acceptance test passes.
- The Development Git adapter uses Git for Windows. It searches `PATH` and common Git for Windows installation folders; the Git executable is an external prerequisite, not silently included in this installer.
- Codex/Gemini/Claude connections use the Windows MCP executable packaged with Continuum. CLI detection accepts both `.exe` and the `.cmd` shims commonly installed by npm. Each connection still requires a real successful MCP tool call and a user-granted scope; merely finding or installing a client does not prove integration.
- Exported HTML/Markdown reports open through the Windows shell's file association. The verified export is not run through `cmd.exe`.

## Release acceptance on a real Windows 10/11 x64 machine

Use a non-sensitive disposable project. Record Windows version, WebView2 runtime version, installer checksum, and results. Test every item on the installed `.exe`, not only a browser preview.

1. Install, launch from the Start menu, close with X, reopen, then uninstall/reinstall. Verify existing projects remain available after reinstall and no prior project is modified without opening it.
2. Create a Research-only project, a Development-only project, and a combined project. Reopen each and verify Home/recent list and capability gates.
3. Create a research question; paste, drag, import, and screenshot an image; add titles/descriptions; connect, delete, undo/redo, auto-arrange, and full-screen the workspace. Reopen and verify cards, media, positions, links, and history.
4. Capture **Screen only**, **Microphone only**, **System audio only**, **Screen + Microphone**, **Screen + System audio**, and **Screen + System audio + Microphone** separately. For every supported combination, record 5–10 seconds, stop, save, play from the Workspace, check moving video and audible selected sources, then reopen the project and replay. If the Windows chooser lacks system audio, record that limitation explicitly; never mark it as passed.
5. Deny screen and microphone permissions, cancel the chooser, switch audio devices, stop a capture early, and close the app while capture is active. Verify clear errors, stopped sensors, recoverable fragments where applicable, and no false “saved” state.
6. Attach a Git repository; inspect commits, worktree changes, code intelligence, and development status. Repeat with Git unavailable and verify the application explains the prerequisite rather than silently failing.
7. Generate/edit a Markdown-first report with a Mermaid diagram and media references; export HTML and Markdown; open each in default Windows apps; verify diagrams render, source links and images resolve, and no raw HTML is shown as plain text.
8. Connect a real installed Codex CLI client, verify one successful MCP call, run analysis, review proposals, ask Messages about saved evidence, save a bookmark, reopen and verify context. Confirm no AI-generated interpretation becomes verified knowledge without review.
9. Export a project on Linux, import it on Windows, compare project counts, source/media hashes, workspace layout, reports, messages, and bookmarks. Then export from Windows and import into a fresh Linux project. Protect original projects during this test.
10. Test paths with spaces, Indonesian text, and long names; a standard user account; offline launch after installation; missing WebView2/Git/client prerequisites; and Windows Defender/SmartScreen behavior for the unsigned installer.

Do not call the Windows port feature-equivalent until all relevant checks pass. A CI build and Linux-side unit tests cannot prove screen/audio permissions, playback, native installer behavior, or AI-client integration on Windows.
