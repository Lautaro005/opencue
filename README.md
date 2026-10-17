# OpenCue

Personal AI agents on your desktop — for **Linux and Windows**. Inspired by [Cue](https://cue.im): every agent has its own
name, avatar, instructions, memory and tools, and agents can work together in group chats.

Core in Rust, GUI in Tauri 2 + React. Local-first: chats, memory and settings stay on your computer; API keys go to the
system keychain.

> Status: **Phase 1 (MVP)**. See [Roadmap](#roadmap) for what ships next.

## Features

| | Status |
|---|---|
| Agents: create, edit, delete, avatar (emoji / color / shape) | ✅ |
| Chat with streaming, Markdown, visible tool calls, stop | ✅ |
| Per-agent instructions (`INSTRUCTIONS.md`) | ✅ |
| Memory per agent + shared memory (`MEMORY.md`), `remember` / `recall` / `forget`, optional auto-learning | ✅ |
| Group chats: `@mentions`, model-routed replies, `send_to_agent`, auto-turn limit | ✅ |
| MCP servers: local stdio (Claude Desktop `mcp.json` import) and remote Streamable HTTP | ✅ |
| Free local internet access: DuckDuckGo + Wikipedia search, page → Markdown fetch, SQLite cache, optional SearXNG | ✅ |
| Approvals for sensitive actions (deleting memory, destructive MCP tools) or fully autonomous mode | ✅ |
| Providers: OpenAI, Anthropic, xAI (Grok), OpenRouter, CheapestInference, Ollama (local/cloud), LM Studio, any OpenAI-compatible | ✅ API key |
| ChatGPT and Claude subscriptions via the official `codex` / `claude` CLIs ("backend CLI") | ✅ text only |
| Tray icon — agents keep running with the window closed | ✅ |
| English / Español, light / dark / follow system | ✅ |
| CLI companion (`opencue-cli`) | ✅ |
| Skills (Claude Code compatible `SKILL.md`) | Phase 2 |
| Automations (cron, IMAP, webhooks, group messages) | Phase 2 |
| Email per agent (IMAP/SMTP, Gmail via Composio) | Phase 2 |
| Composio (1000+ apps) | Phase 2 |
| Computer: SPACE (Linux), Docker/WSL2, opt-in host shell; terminal + file browser view | Phase 2 |
| Voice calls with ElevenLabs (Scribe STT → agent → TTS) | Phase 2 |

Not planned: phone numbers, payments/wallet, QR "scan & create".

### Why "backend CLI" instead of OAuth for ChatGPT / Claude

OpenAI and Anthropic don't offer third-party OAuth for consumer subscriptions, and reusing their CLI tokens in another app
is against their terms. OpenCue instead runs **your installed, logged-in** `codex` or `claude` binary as a subprocess, so
your subscription is used through the official client and OpenCue never touches the tokens. Trade-off: those agents
can't use OpenCue's own tools (memory tools, web, MCP) yet. Grok uses an API key.

## Install

Download the latest installer from [Releases](../../releases):

- **Linux**: `.AppImage`, `.deb` or `.rpm`
- **Windows**: `.msi` or `-setup.exe`

Then open **Settings → Models**, add a provider (or start Ollama/LM Studio locally), and create your first agent with **+**.

## Develop

Requirements: Rust (stable), Node 22+, and on Linux `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`.

```bash
cd apps/desktop
npm install
npm run tauri dev        # desktop app with hot reload
npm run dev              # UI only, in a browser, with an in-memory mock backend
```

```bash
cargo test --workspace   # core tests (includes an end-to-end agent loop against a mock LLM)
cargo run -p opencue-cli -- --help
```

macOS builds work for development but are not distributed.

### Layout

```
crates/opencue-core     Rust core: agents, providers, runtime, memory, MCP, web, storage
crates/opencue-cli      opencue-cli companion
apps/desktop            Tauri 2 shell (src-tauri) + React/Tailwind UI (src)
docs/                   design doc, SPACE setup guide
.github/workflows       CI (Linux/Windows/macOS) and tagged releases (Linux/Windows)
```

### Data

| | Linux | Windows |
|---|---|---|
| Data folder | `~/.local/share/opencue` | `%APPDATA%\opencue\data` |

Inside: `opencue.db` (SQLite: agents, groups, messages, providers, MCP servers, settings, web cache),
`agents/<id>/INSTRUCTIONS.md`, `agents/<id>/MEMORY.md`, `memory/GLOBAL.md`, `skills/`. Set `OPENCUE_HOME` to use another
folder. API keys live in the OS credential store; if none is available (headless Linux) they fall back to
`secrets.json` with `0600` permissions.

### Releases

Push a tag `vX.Y.Z` (matching `version` in `apps/desktop/src-tauri/tauri.conf.json`): GitHub Actions builds the Linux and
Windows installers plus CLI zips and attaches them to a draft release.

## Roadmap

1. **Phase 1 — MVP (this release)**: GUI, agents, streaming chat, providers, instructions, memory, groups, MCP, web.
2. **Phase 2**: skills, automations, email, Composio, computer backends (SPACE / Docker / host) with terminal + file
   browser, ElevenLabs voice.
3. **Phase 3**: noVNC desktop view, auto-update, signed installers.

## Use cases

- A travel agent that remembers your seat preferences and researches hotels.
- A research agent that reads the web and your local files through MCP.
- A group chat where a planner hands tasks to a writer and a fact-checker.

## License

OpenCue is **free to use, modify and share, but not to sell**. It is licensed under the
[Apache License 2.0 with the Commons Clause](LICENSE): you may not sell the software, or a product or service (including
hosting or paid support) whose value derives substantially from it. Because of that condition it is source-available
rather than OSI open source.

## Security

Please report vulnerabilities privately — see [SECURITY.md](SECURITY.md).
