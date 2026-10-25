<p align="center">
  <a href="https://lautaro005.github.io/opencue/"><img src="assets/banner.svg" width="100%" alt="OpenCue's agents: a drop, a star, a square, a blue bubble, a heart, a robot and a flower, each with a little face"></a>
</p>

<h1 align="center">OpenCue</h1>

<p align="center">
  <b>Your own AI agents, living on your desktop.</b><br>
  Agents with a face, memory, an inbox, a computer and files — in chats and group chats.<br>
  Free, local-first, bring your own model. For <b>macOS, Linux and Windows</b>.
</p>

<p align="center">
  <a href="https://github.com/Lautaro005/opencue/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/Lautaro005/opencue?color=2f86eb&label=release"></a>
  <a href="https://github.com/Lautaro005/opencue/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/Lautaro005/opencue/actions/workflows/ci.yml/badge.svg"></a>
  <img alt="Platforms" src="https://img.shields.io/badge/macOS%20%C2%B7%20Linux%20%C2%B7%20Windows-1d1d1f">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-Apache--2.0%20%2B%20Commons%20Clause-8b5cf6"></a>
</p>

<p align="center">
  <a href="https://lautaro005.github.io/opencue/">Website</a> ·
  <a href="https://lautaro005.github.io/opencue/docs/">Docs</a> ·
  <a href="https://github.com/Lautaro005/opencue/releases/latest">Download</a> ·
  <a href="https://lautaro005.github.io/opencue/changelog/">Changelog</a> ·
  <a href="https://lautaro005.github.io/opencue/privacy/">Privacy</a>
</p>

---

## Install

**macOS / Linux**

```bash
curl -fsSL https://lautaro005.github.io/opencue/install.sh | sh
```

**Windows** (PowerShell)

```powershell
irm https://lautaro005.github.io/opencue/install.ps1 | iex
```

Then start it with **`opencue`**. Installs for your user, no admin rights, and updates itself with signed releases.
Prefer an installer? Grab the `.dmg`, `.AppImage`, `.deb`, `.rpm`, `.exe` or `.msi` from
[Releases](https://github.com/Lautaro005/opencue/releases/latest).

First run: a short walkthrough asks your name, connects a provider (or start Ollama / LM Studio), picks a model and
creates your first agent. Replay it any time from **Settings → General**.

> [!NOTE]
> The apps aren't notarized or code-signed by Apple or Microsoft yet. A downloaded `.dmg` needs **System Settings →
> Privacy & Security → Open Anyway** the first time (the terminal install doesn't); Windows SmartScreen may ask you to
> confirm.

## What agents can do

<table>
  <tr>
    <td width="50%" valign="top"><b>🧠 Remember</b><br>Instructions and memory in plain Markdown (<code>INSTRUCTIONS.md</code>, <code>MEMORY.md</code>), a shared memory for all agents that builds itself as you talk, and an <i>About me</i> they all read.</td>
    <td width="50%" valign="top"><b>👥 Work together</b><br>Group chats with <code>@mentions</code>, model-routed replies, hand-offs with <code>send_to_agent</code> and a turn limit.</td>
  </tr>
  <tr>
    <td valign="top"><b>📎 Share files</b><br>Drop files into any chat. Agents see pictures, read documents from their workspace and send reports back with <code>share_file</code>.</td>
    <td valign="top"><b>⏰ Wake up on their own</b><br>Automations on a schedule, new email (IMAP or Gmail), local webhooks or group messages — described in plain words.</td>
  </tr>
  <tr>
    <td valign="top"><b>✉️ Own an inbox</b><br>IMAP/SMTP with auto-detected servers. Read, search and send — sending asks first.</td>
    <td valign="top"><b>🖥️ Use a computer</b><br>A Docker container (with an optional desktop you can watch), a SPACE microVM, WSL, a <b>VMPal</b> virtual machine, or your machine with approval.</td>
  </tr>
  <tr>
    <td valign="top"><b>🧩 Use your apps</b><br>MCP servers (stdio and HTTP, <code>mcp.json</code> import) and a catalog of apps (Composio, Gmail, Drive, Notion, Trello, GitHub, Supabase, Vercel, Outlook) you sign in to with OAuth.</td>
    <td valign="top"><b>✨ Learn skills</b><br>Claude Code–compatible <code>SKILL.md</code>: write them, import them from a URL, or let agents save their own. Run any skill with <code>/</code>, or ask for <code>/create-skill</code> and <code>/create-automation</code>.</td>
  </tr>
  <tr>
    <td valign="top"><b>🎙️ Talk &amp; chat</b><br>ElevenLabs voice and calls, and WhatsApp: link a number with a QR and an agent answers its chats.</td>
    <td valign="top"><b>🌐 Browse</b><br>Free web search (DuckDuckGo, Wikipedia, optional SearXNG), page → Markdown, and a built-in browser pane agents can click and type in while you watch.</td>
  </tr>
  <tr>
    <td valign="top"><b>🧱 Make things (Artifacts)</b><br>Agents hand you web pages, diagrams, code, and real <code>.xlsx</code>, <code>.docx</code>, <code>.pptx</code> and <code>.pdf</code> files in a side panel, with versions. Each one is kept in the agent's Files.</td>
    <td valign="top"><b>📊 Start from a dashboard</b><br>A home screen with what is running, plans, automations and usage. Press <code>Esc</code> in any chat to return, and run <code>/dashboard</code> to have an agent redesign it in HTML.</td>
  </tr>
</table>

**Bring your own model:** OpenAI, Anthropic, xAI (Grok), Google Gemini, Meta Muse, Mistral, Groq, OpenRouter, CheaperInference,
Ollama (local and cloud), LM Studio, any OpenAI-compatible API — or the **Codex**, **Claude Code**, **Grok**, **Muse Code**,
**Mistral Vibe** and **Antigravity** CLIs you're already signed in to.

**Safe by default:** in *Ask first* mode, sending email, commands on your machine, deletions and data-changing tools wait
for your OK. No account, no telemetry; secrets stay in a private file on your computer.

Not planned: phone numbers, payments/wallet, QR "scan & create".

### ChatGPT and Claude subscriptions, the allowed way

OpenAI and Anthropic don't offer third-party sign-in for consumer subscriptions, and reusing their tokens elsewhere breaks
their terms. OpenCue runs **your installed, logged-in** `codex`, `claude`, `grok`, `muse`, `vibe` or `agy` binary instead, so your subscription stays in
the official client and OpenCue never sees the tokens. Those agents still get OpenCue's tools — memory, web, skills,
computer, files, email, catalog apps — through a local MCP server (`opencue mcp-serve --agent <id>`).

## Develop

Requirements: Rust (stable), Node 22+; on Linux `libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev`; on
macOS the Xcode command-line tools.

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

Release builds sign update bundles, so a local `npm run tauri build` needs `TAURI_SIGNING_PRIVATE_KEY`, or add
`-c '{"bundle":{"createUpdaterArtifacts":false}}'`.

<details>
<summary><b>Repository layout</b></summary>

```
crates/opencue-core     Rust core: agents, providers, runtime, memory, files, MCP client and server, web, skills,
                        automations, email, catalog apps, computers (Docker/Podman/SPACE/WSL/host), voice, storage
crates/opencue-cli      opencue-cli companion
apps/desktop            Tauri 2 shell (src-tauri) + React/Tailwind UI (src)
site/                   website (GitHub Pages): home, /docs, /privacy, /changelog, install scripts, llms.txt
docs/                   design doc, SPACE setup guide
.github/workflows       CI (Linux/Windows/macOS), tagged releases (macOS/Linux/Windows), website deploy
```

</details>

<details>
<summary><b>Where data lives</b></summary>

| macOS | Linux | Windows |
|---|---|---|
| `~/Library/Application Support/opencue` | `~/.local/share/opencue` | `%APPDATA%\opencue\data` |

Inside: `opencue.db` (SQLite: agents, groups, messages, providers, MCP servers, automations, email accounts, settings,
web cache), `agents/<id>/` (`INSTRUCTIONS.md`, `MEMORY.md`, `workspace/`, `space.json`), `files/` (shared files),
`memory/GLOBAL.md` and `skills/<name>/SKILL.md`. Set `OPENCUE_HOME` to use another folder. API keys, email passwords and
tokens live in `secrets.json` (mode `0600`) in the data folder; catalog app tokens are in `oauth/<app>.json` (also `0600`).
The system keychain is not used.

</details>

<details>
<summary><b>Releasing</b></summary>

Bump `version` in `Cargo.toml`, `apps/desktop/package.json` and `apps/desktop/src-tauri/tauri.conf.json`, then push a
tag `vX.Y.Z`. GitHub Actions builds the macOS (universal), Linux and Windows installers, signs the update bundles with
the `TAURI_SIGNING_PRIVATE_KEY` secret, publishes `latest.json` for the in-app updater and attaches the CLI zips.
Changes to `site/` deploy the website.

</details>

## Roadmap

- [x] **0.1 — MVP**: GUI, agents, streaming chat, providers, instructions, memory, groups, MCP, web.
- [x] **0.2**: drawn avatars, automations, email, computers with terminal and file browser, skills, Composio, voice,
  website.
- [x] **0.3**: files in chats and groups, OpenCue tools for CLI agents (MCP), Docker desktop view, auto-update, macOS
  builds, one-line install.
- [x] **0.3.1**: first-run walkthrough, separate Providers and Models pages (show/hide models, search), model picker
  when creating agents, VMPal machines, `/` commands (`/create-automation`, `/create-skill`, skills), self-building
  memory, *About me*, floating avatar picker and app-drawn dropdowns, menus and dialogs.
- [x] **0.3.2**: base system prompt, agents that write their own instructions and soul (`SOUL.md`) and greet you by
  asking about their job, Grok CLI, system notifications, tray menu, VMPal as a computer option and fixes for the Claude
  and Codex CLIs, resizable panels.
- [x] **0.3.3**: per-agent working folder, searchable Composio app list (and the right key header), links as pills, real
  model lists for the Claude/Codex/Grok CLIs, and a sturdier agent loop (stable prompt prefix, long results to files,
  compaction, plan recitation, `research` helpers).
- [x] **0.3.4**: Composio consumer keys through Composio Connect, local VMs with a live desktop, memory/CPU limits and a
  folder that keeps their state, Perplexity SPACE with its GitHub link, and provider logos on the website.
- [x] **0.3.5**: agents that talk to each other in 1:1 chats (`send_to_agent`, `@` and a "Chat with …" pill), the
  yellow ask/permission card (`ask_user`), shared or per-agent Docker VMs with an idle stop and a setup checklist, a
  Composio refresh button and a fix for Claude Code agents hitting permission errors on Composio.
- [x] **0.3.6**: Podman as an alternative to Docker, a Catalog of OAuth apps instead of Composio (red/yellow/green cards),
  a terminal that opens by itself, a right-click menu on agents and groups (rename, pin, clear, reset, delete), voice
  calls in a phone-sized window, and secrets in a private file instead of the keychain.
- [x] **0.3.7**: permission cards for Claude Code agents, a stop button for VMs, Podman without Docker, Composio in the
  Catalog (with the apps it connects), a live green card for agent-to-agent chats and logos from your assets on the site.
- [x] **0.3.8**: a working Podman desktop (Cua's XFCE image), stop-all and play buttons in Settings → Computers, desktop
  view on by default, the chosen VM respected over VMPal, folded reasoning, a gray call button without an ElevenLabs
  key and an Agents / Skills menu under the chat's +.
- [x] **0.4.0**: a built-in browser agents can drive (Browser tab next to the VM, terminal below), WhatsApp through a QR with
  green cards in the chat, templates for agents and teams with an x.ai import, a message search (⌘K) and the 5-hour and
  weekly limits of your Codex and Claude Code subscriptions.
- [x] **0.4.1**: Google Gemini, Meta Muse (API and Muse Code CLI), complete templates (soul, skills, memories), agent chat
  cards in each agent's color, one browser page per agent and a reliable test notification.
- [x] **0.4.2**: Artifacts (HTML, SVG, Markdown, Mermaid, code, and real Excel, Word, PowerPoint and PDF files, with
  versions, kept in each agent's Files), a dashboard home screen with `Esc` to return and `/dashboard` to redesign it,
  copy / edit / regenerate on messages, Mistral API, Groq, Mistral Vibe and Antigravity providers with their logos, and
  browser tabs.
- [ ] Still open: notarized macOS and code-signed Windows installers (need an Apple Developer ID and a Windows certificate).

## Use cases

- A paralegal agent with its own inbox that emails courts, tracks replies and reminds you of hearings.
- A reminders agent that checks your boards every Monday at 9:00 and tells you what is due.
- A dev agent with a Docker computer and desktop that runs scripts, opens apps and sends you the results as files.
- A group chat where a planner hands tasks to a researcher and a writer.

## License

OpenCue is **free to use, modify and share, but not to sell**. It is licensed under the
[Apache License 2.0 with the Commons Clause](LICENSE): you may not sell the software, or a product or service (including
hosting or paid support) whose value derives substantially from it. Because of that condition it is source-available
rather than OSI open source.

## Security & privacy

Report vulnerabilities privately — see [SECURITY.md](SECURITY.md). What leaves your computer is described in the
[privacy policy](https://lautaro005.github.io/opencue/privacy/).

<p align="center"><sub>Inspired by <a href="https://cue.im">Cue</a>. Made with Rust, Tauri and a lot of googly eyes.</sub></p>
