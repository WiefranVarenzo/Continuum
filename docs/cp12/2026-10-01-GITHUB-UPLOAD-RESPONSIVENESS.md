# GitHub upload responsiveness follow-up

The Linux user observed a desktop “not responding” dialog while uploading a roughly 44 MiB project snapshot. This was a UI responsiveness defect, not evidence that the project data was lost. A read-only check of the private GitHub repository showed a Continuum snapshot tree, and the local project's remote-link file recorded the same Git commit as the remote branch. No remote repository content or local project data was changed during that diagnosis.

## Change

- Upload and restore are explicit asynchronous Tauri commands with Git/LFS work dispatched to the blocking worker pool.
- The lightweight `ContinuityStore` handle is cloned while holding the app-state lock; the lock is released before export, Git, LFS, or network work. This prevents a slow upload from holding the global project lock and blocking other UI commands.
- The upload form shows elapsed time and explains that larger recordings may take several minutes.
- The existing no-force-push and first-upload-conflict checks remain unchanged. The local project stays the working copy.

## Verification

- `cargo check --offline` passed.
- 19 desktop backend tests passed; the opt-in local Git LFS database/media round trip passed.
- TypeScript check and 81 UI tests passed.
- AppImage built at `releases/0.12.0-github-upload-responsive-2026-10-01/Continuum_0.12.0_amd64.AppImage` with SHA-256 `125ec2364eddcf6bdd3d8e136298e359a3d5cc2d0a8dd78a6bd3962f49007523`.
- An isolated startup remained open until a planned 12-second timeout without startup errors.

A second real hosted upload using this new build has not yet been performed. The previous user's upload was verified as complete by comparing the remote branch HEAD with the saved local remote-link HEAD, not by repeating the upload.
