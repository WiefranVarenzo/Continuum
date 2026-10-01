# Contributing and building your own application

Fork the repository, clone your fork, and create a branch for a focused change. Follow the [Linux/Windows build guide](docs/development/BUILD-AND-DEVELOP.md) and [architecture](docs/architecture/CROSS-PLATFORM-ARCHITECTURE.md).

## Work in the appropriate layer

Keep project rules, identity, migrations, audit, provenance, and validation in `continuum-core`. Keep rendering and interaction in the React application. Put OS behavior behind the native adapters instead of duplicating the domain model. MCP clients submit bounded proposals; external AI does not bypass human review or edit canonical storage directly.

Changing the database requires a new forward migration and compatibility/restore tests. Preserve existing migrations and original artifacts. Use a disposable project copy for migration testing. Capture changes must preserve explicit consent, cancellation, sensor cleanup, source separation, and saved segments after interruption.

## Before a pull request

- Run relevant core, UI, and desktop tests, and the production frontend build.
- Describe the user-visible trigger, resulting behavior, and affected platforms.
- Separate automated/synthetic evidence from physical-device/account checks; mention any checks not performed.
- Check Markdown links and release filenames/hashes when updating documentation.
- Commit lockfiles when changing dependencies; include source needed to rebuild your changes.

Do not commit `target/`, `node_modules/`, `dist/`, downloaded tools, installers, account credentials, grant tokens, `.env` secrets, or user projects/recordings. Put release binaries and checksums in GitHub Releases. Windows portable tool downloads are checksum-pinned in `prepare-windows-tools.mjs` and preserve upstream license files.

## A personal fork

You can edit the same source and build an AppImage or a Windows setup executable for your fork. Keep branding/application identifiers coherent if distributing a distinct app, document its limitations, and respect the repository and upstream dependency license terms. A successful compilation is not physical-device certification. Use the [release checklist](docs/releases/PUBLISHING.md) when sharing packages.

For bugs include your OS/version, Continuum package filename/version, reproduction steps, expected/actual behavior, and sanitized diagnostics. For capture, say whether Windows/Linux's own microphone test works and whether the problem occurs with microphone-only or combined sources. Never include credentials or private project content without intentionally sanitizing it.
