# Continuum 0.12.0 — Linux pilot handover (1 October 2026)

This is the latest locally validated Linux x86-64 AppImage available at the source handover. It includes the GitHub upload responsiveness fix described in [the implementation follow-up](../../docs/cp12/2026-10-01-GITHUB-UPLOAD-RESPONSIVENESS.md). The package is an **unsigned pilot build**, not a certified Windows/macOS release.

Download `Continuum_0.12.0_amd64.AppImage` from the GitHub Release, verify its SHA-256 against `SHA256SUMS`, then run:

```bash
chmod +x Continuum_0.12.0_amd64.AppImage
./Continuum_0.12.0_amd64.AppImage
```

The source commit contains code, tests, design history, and maintainer instructions. It intentionally does **not** include anyone's project ledger, original screenshots/recordings, AI credentials, or GitHub credentials. To move a project between machines, export/import it in Continuum or use a separate private project-snapshot repository with Git LFS.

Verification recorded for this build: backend desktop tests, local Git LFS database/media round trip, frontend tests, and isolated startup smoke. A new hosted upload using this exact build and real Windows/macOS use remain unverified. See [platform limitations](../../docs/cp12/CP12-PLATFORM-AND-KNOWN-LIMITATIONS.md) and [Windows release gate](../../docs/windows/WINDOWS-RELEASE.md).
