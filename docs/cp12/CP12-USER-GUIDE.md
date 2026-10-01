# Continuum 0.12 Linux Pilot — User Guide

Updated for the 1 October 2026 Linux follow-up. Earlier CP12 pages are retained as design history; current behavior also depends on the later workspace, recording, report, and project-snapshot changes linked in [HANDOVER](../HANDOVER.md).

## Start the Application

On the current Fedora/Linux build machine, use the latest local pilot AppImage. From the repository root:

```bash
chmod +x releases/0.12.0-github-upload-responsive-2026-10-01/Continuum_0.12.0_amd64.AppImage
releases/0.12.0-github-upload-responsive-2026-10-01/Continuum_0.12.0_amd64.AppImage
```

Debian/Ubuntu users may install the `.deb` with their normal graphical package installer or distribution package command. Keep the package checksum with the installer.

## Create Your First Project

1. Choose **Create**.
2. Enter a project name and select a parent folder.
3. Choose **Research only**, **Development only**, or **Connected R&D**.
4. Select **Create and open**. Continuum creates and verifies the local ledger before showing the workspace.

**New folder name** starts empty and can be fully replaced or cleared. If left empty, submission derives a folder name from the project name. The sidebar and workspace header identify the active mode.

## Start Real Work

- In Research Space, use **New research**, enter a short title/question, optionally add context and desired outcome, then choose **Start research**. The board is the main work surface; detail forms live in the relevant feature pages.
- Paste or drop screenshots onto the board. **Live screenshots** watches for new clipboard images after you enable it; take a screenshot with the system shortcut and copy it to the clipboard. No global shortcut is registered by Continuum. **Stop live** ends that clipboard intake.
- **Recent captures** provides evidence title/description and removal. The X archives the Evidence from the board while preserving the original in history. Enable the prompt for title and description after each capture so AI can understand your intent without guessing from the image alone.
- **Float toolbar** creates a small always-on-top window; **Open workspace** restores it. Screen/system-audio/microphone options are under **Record & sources** / **Screen / audio**; availability depends on the native WebView and OS permissions.
- In Development Space, use **Connect and analyze a Git repository**. This selects a local folder containing `.git`; Continuum observes it, imports a bounded commit page, and analyzes supported code. This is not a connection to your GitHub/GitLab project-snapshot repository and does not modify the development repository.
- In Connected R&D, both work areas are available and the shared ledger can preserve the Evidence → Decision → Requirement → ChangeSet → Code → Test chain.

## Bookmark and Resume

1. Sources are saved as you work, and changes request an automatic resume checkpoint. Wait for the save status if you need confirmation that the resume summary caught up.
2. Use **Research history** to resume a session. On launch, Continuum attempts to reopen the last valid project and restore your selected research.
3. Open **Resume & memory** to inspect saved context and continue from a bookmark. Use the available Context Pack export when you need a portable handover; advanced checkpoint/context controls remain reviewable there.
4. Before sharing a pack, inspect its contents and omit unnecessary or sensitive material. An abrupt exit before the checkpoint timer may leave the summary behind the already-saved source ledger.

## Reports and Diagrams

- **Workspace** is the main activity view; **Reports** contains the editable Markdown-first report and HTML/Markdown exports. Mermaid diagrams are optional and should help explain the actual evidence, not fill a fixed template.
- Colors have meaning: green Research, blue Development, purple Decision/Requirement, cyan Continuity, amber Feedback, and red attention states.
- The interactive graph supports search, pan, zoom, branch collapse, and a text alternative.
- Mermaid flows support horizontal scrolling and 60–200% zoom. Only structured validated nodes and links are rendered.
- **Export HTML** publishes an offline reading copy; **Export Markdown** preserves the editable text. The saved report, not a previously exported file elsewhere on disk, is included in a complete project snapshot.

## Protect Your Work

- Use **Run health check** before a milestone or handoff.
- Use **Backup database** for a consistent SQLite snapshot.
- Use **Export restorable copy** for the full project, including artifacts. Do not choose a destination inside the active project.
- Use **Restore** on Home to copy and verify an export into a new folder. Project settings can also upload a verified snapshot to a separate private GitHub/GitLab repository; Home can import that snapshot into a new local folder. Git LFS is required for media. This is separate from the read-only Development Git connection.
- API keys and one-time MCP tokens are not placed in project exports.

## AI Access

**AI connections** detects Codex, Claude Code, or Gemini CLI and connects a selected installed client in one click. Continuum creates the scoped access secret and installs the packaged STDIO server configuration privately; restart the client once after connecting. The Codex option also serves ChatGPT Desktop on the same Codex host. ChatGPT web is not a local client and requires a future remote MCP/plugin path. **Other MCP client** keeps the copyable one-time settings fallback.

For automatic evidence organization, connect a supported local AI client with proposal submission enabled and choose **Analyze & connect with AI**. The assistant receives bounded project context and can propose descriptions and relationships; inspect these in proposal review before accepting. Rejecting a proposal leaves canonical knowledge unchanged. The UI offers Hermes as an optional assistant choice, but availability still depends on an installed, functioning local client and a successful real tool call; detecting a client or installing configuration alone is not proof of connection.

AI is optional. Automatic connections create a narrow, expiring local MCP grant without exposing its token. Use **Disconnect** when finished. External AI can read only granted bounded context and submit proposals; you review consequential changes before canonical acceptance.

## If Health Check Reports a Problem

Do not overwrite the project. First create a filesystem copy if possible, read the exact issue code and guidance, and restore the newest verified export into a new folder. A green health badge means the deterministic checks passed; it is not a substitute for external backup discipline.
