# CP12 Acceptance Checklist

> **Result:** PASS — Linux pilot scope  
> **Qualification boundary:** cross-platform GA remains conditional as documented

- [x] Existing CP1 architecture and CP2–CP11 contracts remain unchanged.
- [x] Core, MCP, UI, and desktop versions agree at 0.12.0; schema remains v12.
- [x] User can create Research-only, Development-only, or Connected R&D projects.
- [x] User can open an existing project and restore a verified export into a new path.
- [x] User can create a Research Question from the desktop.
- [x] User can attach/synchronize/analyze a Git repository without repository mutation.
- [x] User can create Checkpoints/Context Packs, capture Evidence, inspect reports, and manage scoped AI access.
- [x] Health diagnostics, backup, full export, safe close, last-project reopen, and identity-preserving restore are available.
- [x] Export/import rejects recursive source-nested destinations.
- [x] Full regression: 140 Rust tests pass.
- [x] Desktop regression: 21 UI tests pass.
- [x] Strict Rust lint passes for workspace and Tauri crate.
- [x] Production TypeScript/Vite build passes.
- [x] npm audit reports zero known vulnerabilities.
- [x] Credential/private-key pattern scan reports no findings.
- [x] All Cargo dependencies expose license or license-file metadata.
- [x] `.deb` and AppImage packages build and have recorded SHA-256 hashes.
- [x] AppImage runtime metadata validates and the final application smoke-launch remains running.
- [x] Diagrams use semantic colors and no longer show the large white default minimap in dark mode.
- [x] Graph and Mermaid provide text alternatives, focus states, responsive behavior, and reduced-motion support.
- [x] Reference-device integrated p95 report and diagnostics targets pass on the named CP12 fixture.
- [x] Known limitations and recovery instructions are published.
- [ ] Windows 11 formal build/install/capture/signing certification — external platform gate.
- [ ] Signed distribution and update channel — distribution gate, not enabled for local pilot.
- [ ] Live provider/client/capture-device matrix — required only before claiming those exact integrations/platform paths as certified.
- [ ] Standard 50k/150k stress certification — performance claim deliberately not made by the 1k/2k CP12 fixture.
