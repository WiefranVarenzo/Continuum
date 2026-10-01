# CP11 Client Compatibility

> **Contract date:** 2026-09-10  
> **Meaning of PASS:** offline protocol/conformance fixture, not a permanent claim about future client releases

## Shared Profile

All clients use local STDIO, MCP initialize/initialized, tools, resources, prompts, structured tool results plus text fallback, bounded pagination, and client-side approval UI where available. Client brand never changes canonical semantics.

- Codex / ChatGPT family: PASS fixture at MCP `2025-06-18`; client name must contain `codex` or `chatgpt`.
- Claude Code family: PASS fixture at MCP `2025-03-26`; client name must contain `claude`.
- Gemini CLI family: PASS fixture at MCP `2024-11-05`; client name must contain `gemini`.
- Generic MCP host: PASS generic lifecycle fixture; any client name is accepted under a generic grant.

Fixtures cover initialization, notification behavior, catalog discovery, bounded resume/context reads, resources/templates, prompts, ping, and cancellation-notification compatibility. They do not claim UI placement, config-file syntax, installer discovery, or live-client approval behavior outside the tested protocol messages.

CP12 must record actual client version, OS, installation/config path, verified date, supported optional primitives, approval behavior, known deviation, and fallback before advertising a release client as certified.
