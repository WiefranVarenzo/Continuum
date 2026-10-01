# Project repository snapshots (Linux)

Continuum's live project remains a local folder. The Development screen's Git-folder connection is a **read-only development source** on this computer, not a backup destination. Project settings has a separate optional GitHub/GitLab repository feature for portable project snapshots.

## Upload from this computer

1. Create an **empty private** repository on github.com or gitlab.com, and install Git LFS (`sudo dnf install git-lfs` on Fedora).
2. For GitHub on this Linux build, open Project settings and choose **Login with GitHub**. The official GitHub CLI (`gh`) starts browser authorization; no Continuum GitHub App or personal access token entry is required. After authorization, choose a writable private repository and branch. Alternatively, enter a repository URL and use your existing Git HTTPS/SSH credentials. GitLab still uses existing Git credentials; browser login is GitHub-only in this build.
3. Review evidence for sensitive material, confirm that the destination is private, then choose **Save local version & upload**.
4. Continuum flushes local workspace/report edits and saves a resume point. It exports the verified project into a disposable Git clone, commits a complete snapshot, and pushes without force.
5. The live project is not pulled, rebased, or overwritten. Future uploads use the same linked repository and branch. If its branch has changed elsewhere, upload stops and leaves both sides unchanged.

The snapshot contains the project manifest, consistent SQLite ledger, and artifact directory. That covers workspace positions and relationships, evidence, recordings, saved report Markdown, messages, and persisted context/memory. Git LFS stores the ledger and artifacts as media objects; old versions may continue consuming storage quota. Files generated separately outside the project export (for example, an external HTML file) are not automatically included, but the report's saved Markdown is included.

AI client credentials, machine-specific application settings, and Git authentication are deliberately **not** portable project data. On a second computer, reconnect the AI client and configure Git access there. This preserves the research memory without copying secrets between devices.

The privacy checkbox is a user confirmation, **not** an automated check of GitHub/GitLab visibility for manually entered URLs. The GitHub picker lists only writable private repositories. Do not upload secrets or sensitive recordings without reviewing them. Continuum never asks for a personal access token in the URL. GitHub browser login uses the official `gh` CLI's OAuth credential, stored outside the project in the current user's private Continuum configuration directory. The CLI may request broader repository access than the one project you select, and it may save its credential in a local file if an OS keyring is unavailable. Neither the credential nor machine-specific settings are part of the project snapshot. Hosting limits and charges depend on the user's GitHub/GitLab plan.

## Restore on another computer

On Home, choose **Import from GitHub / GitLab**. For GitHub, sign in through the picker and choose the repository and branch; a manual URL remains available for GitHub/GitLab. Select a **new** local destination folder. Continuum clones the repository into temporary storage, downloads and checks out the Git LFS objects, validates the project export, then imports it into the new folder. It will not overwrite an existing folder or silently merge with an open project. After restore, the imported project keeps the repository link for later uploads.

Git and Git LFS must be installed on each computer. Repository access is required for private repos. A local bare-repository integration test verifies that the SQLite database and video artifact are restored as their original bytes, not Git LFS pointer files. A real hosted GitHub/GitLab end-to-end test still requires a private test repository and authentication.

This feature is a versioned snapshot transfer, not continuous synchronization or multi-user conflict merging. Keep the local project as the working copy. If another device uploads first, inspect its remote changes before deciding whether to restore it into a separate folder; Continuum never discards local updates to make a push succeed.

## Verification on the Fedora build host

- Frontend: 77 UI tests passed and the production frontend compiled.
- The complete Rust workspace test suite passed after updating an old v11 migration fixture to remove the current workspace-document tables before replaying migrations. No production migration logic was changed by that fixture update.
- Backend: URL/branch/conflict tests passed. The local bare-repository Git LFS round trip passed after restoring the original SQLite database and video bytes. The test found and fixed a missing `git lfs install --local` step in a fresh import clone.
- Linux AppImage built and copied to `releases/0.12.0-project-sync-2026-09-30/Continuum_0.12.0_amd64.AppImage` without replacing the earlier release. SHA-256: `80b0a0e188a249939a6c09bdb12c133f46dc6b47ddd79492c7262d019df79f5a`.
- An isolated 12-second startup smoke remained open until the planned timeout (exit 124), with no startup error output. This checks packaging/startup, not a live hosted upload.
- Git LFS 3.7.1 was installed into this user's `~/.local/bin` from a Fedora RPM whose signatures and digests verified. Git and Git LFS are still prerequisites on any other computer.

A real private GitHub/GitLab repository has not been supplied, so authentication, hosted LFS quota, and round-trip behavior against those services remain unverified. Do not interpret the local bare-repository test as hosted-service certification.

## Browser-login implementation check (2026-09-30)

- Fedora's official `gh` package is installed on the build host. The application uses `gh auth login --web` and a Continuum-scoped `GH_CONFIG_DIR`; it does not create its own account system or require the user to register a GitHub App.
- GitHub's private writable repositories and branches are discovered after login. The chosen branch is used for transfer; the first upload still requires an empty destination branch.
- Git credentials are passed to Git and Git LFS through a per-process credential helper. No global Git configuration is modified. The local project remains authoritative until the user explicitly imports into a new folder.
- TypeScript check passed; 79 UI tests passed. Rust desktop backend: 19 tests passed, 2 ignored. The local bare-Git/Git-LFS round trip passed in a separate opt-in run, including exact database and media bytes.
- A separate AppImage was built at `releases/0.12.0-github-web-login-2026-09-30/Continuum_0.12.0_amd64.AppImage` (SHA-256 `a68d22fd421edb591881cace5e4c354e259f9f8b29752481621ab1ac563d71e0`). An isolated startup remained open until its planned 12-second timeout without startup errors. The prior `0.12.0-project-sync-2026-09-30` AppImage was not replaced.
- Hosted login and upload/import still need a user-authorized private test repository. Browser authorization was not performed on the user's behalf.

## Default-browser login follow-up

The first browser-login build showed a one-time code but did not reliably open the default browser from the AppImage. Continuum now waits for the code, then opens GitHub's device-authorization page through the Linux desktop's default browser with AppImage library overrides removed. The login screen also offers **Open GitHub authorization page** as a fallback. The user still enters the one-time code on GitHub; this is an inherent step of the GitHub CLI device flow, not a Continuum account registration. After approval, Continuum polls for connected status and refreshes the private repository picker. The app never copies the code into project data or displays a token.

The follow-up AppImage is `releases/0.12.0-github-browser-auto-2026-09-30/Continuum_0.12.0_amd64.AppImage`, SHA-256 `8b229f38f9ee806110db6907ffa45f747acb41bf46cb0ce8ba2a9832e3f3de3e`. TypeScript check, 79 UI tests, and 19 backend tests passed; an isolated 12-second startup reached the planned timeout without startup errors. The existing desktop shortcut points to the build output now containing this version. Whether the browser automatically appears and the user authorizes GitHub still requires an interactive test on this machine; this was not inferred from the startup smoke.

## Import feedback follow-up

After browser login was confirmed working, the Home import button could appear to do nothing: it was disabled until all fields were supplied without telling the user which field was missing, while backend errors appeared only above the Home content. The new form allows a click, explains missing repository URL, local parent folder, or folder name beside the button, suggests a local folder name from a selected repository, and shows download progress and errors in place. A repository branch without a Continuum snapshot now receives a specific explanation before downloading LFS media. Upload and import commands are dispatched off the Tauri UI thread so a lengthy Git/LFS operation does not freeze the window.

The new AppImage is `releases/0.12.0-github-import-feedback-2026-09-30/Continuum_0.12.0_amd64.AppImage`, SHA-256 `589c1314347a2cace76e4d9fe49518080055a4e709946f08e0d8bbfb675ab92b`. TypeScript check, 81 UI tests, 19 backend tests, and the opt-in local Git LFS database/media round trip passed. An isolated 12-second startup had no startup errors. A real hosted import still requires a user-selected repository and branch containing a prior Continuum snapshot; no remote user project was modified during these tests.
