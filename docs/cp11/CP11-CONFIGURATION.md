# CP11 Local MCP Configuration

## 1. Build

From the Continuum repository:

```text
cargo build --release -p continuum-mcp
```

The executable is `target/release/continuum-mcp`.

## 2. Create Access

Open the project in the Continuum desktop app, use **AI connections**, choose Codex/ChatGPT Desktop, Claude Code, or Gemini CLI, keep proposal permission off unless needed, and press **Connect**. When the client executable is detected, Continuum creates the grant and installs the MCP entry automatically; the user never handles the token. Restart the client once so it reloads its MCP configuration.

Choose **Other MCP client** only for a client without an automatic adapter. That explicit fallback reveals a one-time token and copyable settings; the token cannot be recovered. Revoke and replace a grant if the token is lost or its authority must change.

## 3. Runtime Inputs

Configure the MCP host to launch the fixed executable with:

```text
command: /absolute/path/to/continuum-mcp
args: ["--project", "/absolute/path/to/continuum-project"]
env:
  CONTINUUM_MCP_GRANT_TOKEN: <one-time token>
```

Alternatively, set `CONTINUUM_PROJECT_ROOT` and omit `--project`. Never put the token in `args`, source control, prompts, screenshots, support messages, or shared logs. Restrict any client config containing the token to the current operating-system user.

Client configuration shapes and locations evolve. Automatic adapters call the installed client's official MCP configuration command without a shell. Manual clients must use the then-current official client documentation and map its STDIO command/args/env fields to the values above. CP12 records exact certified examples for release client versions.

## 4. Safe Operation

- Use one grant per project and client installation.
- Prefer read-only grants; enable proposals only for a concrete workflow.
- Keep artifact bytes at zero unless a separately reviewed task needs content.
- Revoke access immediately after suspected disclosure or when the client is retired.
- Inspect the recent safe audit and proposal queue in Continuum.
- If initialization fails, verify the project path, client-family grant, expiry, and token; create a replacement instead of editing the database.

## 5. Disabled Remote Transport

Do not wrap the STDIO binary in a network bridge. Streamable HTTP is not enabled in v0.11 and requires a future threat-reviewed decision.
