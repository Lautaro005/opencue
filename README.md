<p align="center">
  <img src="assets/logo.svg" width="88" alt="OpenCue logo">
</p>

<h1 align="center">OpenCue</h1>

<p align="center">
  Personal AI agents on your desktop — for <b>Linux and Windows</b>.<br>
  <a href="https://lautaro005.github.io/opencue/">Website</a> ·
  <a href="https://lautaro005.github.io/opencue/docs/">Docs</a> ·
  <a href="https://github.com/Lautaro005/opencue/releases/latest">Download</a> ·
  <a href="CHANGELOG.md">Changelog</a>
</p>

Inspired by [Cue](https://cue.im): every agent has its own face, instructions, memory, inbox, computer and automations,
and agents work together in group chats. Core in Rust, GUI in Tauri 2 + React. Local-first: chats, memory and settings
stay on your computer; secrets go to the system keychain. Bring your own model.

> Status: **0.2.0 (Phase 2)** — see the [Roadmap](#roadmap).

## Features

| | |
|---|---|
| **Agents** | Drawn avatars (10 faces × 12 shapes × colors), per-agent model, instructions (`INSTRUCTIONS.md`) |
| **Memory** | Per agent + shared (`MEMORY.md`, `GLOBAL.md`); `remember` / `recall` / `forget`; optional auto-learning |
| **Chat** | Streaming, Markdown, visible tool calls with approvals, stop, voice input |
| **Group chats** | `@mentions`, model-routed replies, `send_to_agent`, automatic-turn limit |
| **Automations** | Schedules (cron), new email, local webhooks, group messages — run while the app is open or in the tray |
| **Email** | Per agent: IMAP/SMTP (servers auto-detected for common providers) or Gmail via Composio; read, search, send |
| **Computers** | Docker container per agent, SPACE microVMs (Linux/remote), WSL, or this computer; live terminal + file browser |
| **Apps** | MCP servers (stdio and Streamable HTTP, `mcp.json` import) and Composio (1000+ apps) |
| **Skills** | Claude Code–compatible `SKILL.md`: write, import from URL, or let agents save their own |
| **Voice** | ElevenLabs: Scribe speech-to-text from the mic, replies read aloud, voice per agent |
| **Internet** | Free search (DuckDuckGo, Wikipedia, optional SearXNG) and page → Markdown fetch, cached locally |
| **Providers** | OpenAI, Anthropic, xAI (Grok), OpenRouter, CheapestInference, Ollama (local/cloud), LM Studio, any OpenAI-compatible API; ChatGPT/Codex and Claude via their official CLIs |
| **Safety** | "Ask first" approvals for email, host commands, deletions and data-changing tools; or fully autonomous |
| **App** | Tray icon, English / Español, light / dark / system theme, `opencue-cli` companion |

Not planned: phone numbers, payments/wallet, QR "scan & create".

### Why "backend CLI" instead of OAuth for ChatGPT / Claude

OpenAI and Anthropic don't offer third-party OAuth for consumer subscriptions, and reusing their CLI tokens in another app
is against their terms. OpenCue instead runs **your installed, logged-in** `codex` or `claude` binary as a subprocess, so
your subscription is used through the official client and OpenCue never touches the tokens. Trade-off: those agents reply
with text only — OpenCue's tools (memory, web, email, computer, MCP) need an API-key provider. Grok uses an API key.

## Install

Download the latest installer from [Releases](https://github.com/Lautaro005/opencue/releases/latest):

- **Linux**: `.AppImage`, `.deb` or `.rpm`
- **Windows**: `.msi` or `-setup.exe` (per-user, no admin)

Then open **Settings → Models**, add a provider (or start Ollama / LM Studio), and create your first agent with **+**.
The [docs](https://lautaro005.github.io/opencue/docs/) cover email, automations, computers (including
[SPACE setup](docs/space-setup.md)), Composio, skills and voice.

## Develop

Requirements: Rust (stable), Node 22+, and on Linux `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`.

```bash
cd apps/desktop
npm install
npm run tauri dev        # desktop app with hot reload
npm run dev              # UI only, in a browser, with an in-memory mock backend
```

```bash
cargo test --workspace   # core tests, incl. an end-to-end agent loop against a mock LLM
cargo run -p opencue-cli -- --help
```

macOS builds work for development but are not distributed.

### Layout

```
crates/opencue-core     Rust core: agents, providers, runtime, memory, MCP, web, skills, automations,
                        email, Composio, computers (Docker/SPACE/WSL/host), voice, storage
crates/opencue-cli      opencue-cli companion
apps/desktop            Tauri 2 shell (src-tauri) + React/Tailwind UI (src)
site/                   website (GitHub Pages): home, /docs, /privacy, /changelog
docs/                   design doc, SPACE setup guide
.github/workflows       CI (Linux/Windows/macOS), tagged releases (Linux/Windows), website deploy
```

### Data

| | Linux | Windows |
|---|---|---|
| Data folder | `~/.local/share/opencue` | `%APPDATA%\opencue\data` |

Inside: `opencue.db` (SQLite: agents, groups, messages, providers, MCP servers, automations, email accounts, settings,
web cache), `agents/<id>/` (`INSTRUCTIONS.md`, `MEMORY.md`, `workspace/`, `space.json`), `memory/GLOBAL.md` and
`skills/<name>/SKILL.md`. Set `OPENCUE_HOME` to use another folder. API keys, email passwords and tokens live in the OS
credential store; if none is available (headless Linux) they fall back to `secrets.json` with `0600` permissions.

### Releases

Push a tag `vX.Y.Z` (matching `version` in `apps/desktop/src-tauri/tauri.conf.json`): GitHub Actions builds the Linux and
Windows installers plus CLI zips and attaches them to a draft release. Changes to `site/` deploy the website.

## Roadmap

1. **Phase 1 — MVP (0.1)**: GUI, agents, streaming chat, providers, instructions, memory, groups, MCP, web.
2. **Phase 2 (0.2, this version)**: drawn avatars, automations, email, computers (Docker / SPACE / WSL / host) with
   terminal + file browser, skills, Composio, ElevenLabs voice, website.
3. **Phase 3**: file sharing in chats and groups, OpenCue tools for CLI-backed agents (via MCP), noVNC desktop view,
   auto-update, signed installers.

## Use cases

- A paralegal agent with its own inbox that emails courts, tracks replies and reminds you of hearings.
- A reminders agent that checks your boards every Monday at 9:00 and tells you what is due.
- A dev agent with a Docker computer that runs scripts and keeps notes in its workspace.
- A group chat where a planner hands tasks to a researcher and a writer.

## License

OpenCue is **free to use, modify and share, but not to sell**. It is licensed under the
[Apache License 2.0 with the Commons Clause](LICENSE): you may not sell the software, or a product or service (including
hosting or paid support) whose value derives substantially from it. Because of that condition it is source-available
rather than OSI open source.

## Security & privacy

Report vulnerabilities privately — see [SECURITY.md](SECURITY.md). What leaves your computer is described in the
[privacy policy](https://lautaro005.github.io/opencue/privacy/).
