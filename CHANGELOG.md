# Changelog

All notable changes to OpenCue. The website version is at <https://lautaro005.github.io/opencue/changelog/>.

## 0.3.1 — 2026-10-17

### New

- **Walkthrough**: a floating, step-by-step window on first launch — your name, a few lines about you, connect a
  provider, pick a default model, create your first agent. Replay it from Settings → General.
- **Providers and Models are separate pages.** *Providers* shows a container for every supported provider (connected,
  needs a key, or not set up) and opens a window to configure it. *Models* lists the models of the providers that are
  ready, with a search bar, a switch to show or hide each model (or all at once) and a star to set the provider's
  default model. Hidden models disappear from every model picker.
- **Pick the model when creating an agent**: provider and model (searchable) are right in the Create Agent window.
- **VMPal machines**: Settings → Computers → VMPal takes TOML in the same format as Codex MCP servers
  (`[mcp_servers.vmpal-…]` with `command` and `args`). In an agent's profile → Computer, choose machines from that
  list or paste a new table to add it on the spot. Machines start the first time the agent uses them and are never
  part of "all MCP servers".
- **Slash commands** in the chat input: type `/` to run any installed skill, plus the built-in **`/create-automation`**
  (the agent creates an automation for itself) and **`/create-skill`** (the agent writes a reusable skill). New
  `create_automation` tool (schedule or new-email triggers; asks first in "Ask first" mode).
- **Memory builds itself**: auto-learning is now on by default. After each reply the agent extracts durable facts,
  routes facts every agent should know ("global: …") to the shared memory, avoids duplicates and merges the list when
  it grows past 40 entries. The prompt also tells agents to remember proactively. Agents created before 0.3.1 are
  switched on once; you can turn it off per agent.
- **About me** (Settings → Account): a free-text description every agent reads as background. The name you set is now
  also shared with agents.

### Changed

- **Avatar picker** is a floating window with a close button that fills its width; faces are drawn on the avatar's own
  color, fixing the low contrast in dark mode. It opens from Create Agent, the agent profile and the walkthrough.
- **Chat input** now spans the same width as the header.
- **No more system widgets**: dropdowns, confirmations, tooltips and the right-click menu are drawn by the app (custom
  select with search and keyboard support; confirm window; Cut / Copy / Paste / Select all menu).
- Only the top-most window answers Escape.

### Fixed

- Pickers and confirmations opened from inside another window are no longer clipped by it.

## 0.3.0 — 2026-10-17

### New

- **macOS**: universal app (Apple silicon + Intel) and `.dmg`, built by the Release workflow.
- **One-line install**: `curl -fsSL https://lautaro005.github.io/opencue/install.sh | sh` (macOS, Linux) and
  `irm https://lautaro005.github.io/opencue/install.ps1 | iex` (Windows). Installs per user and adds an `opencue`
  command; `--uninstall` removes it and keeps your data.
- **Files in chats and groups**: attach with + → Files, drag and drop or paste (10 files, 25 MB each). Stored in
  `files/<id>/` and copied to each agent's workspace as `files/<name>`; pictures go to vision models, small text files
  inline. New `share_file` tool for agents to send files back; Files pages for agents and groups.
- **OpenCue tools for CLI agents**: Codex / Claude Code agents get OpenCue's tools over MCP via
  `opencue mcp-serve --agent <id>` (`--mcp-config` / `-c mcp_servers.opencue…`). Approval-needing actions are refused in
  Ask-first mode. Per-agent switch in profile → Model.
- **Desktop view** for Docker computers (Settings → Computers): web desktop image (default
  `lscr.io/linuxserver/webtop:ubuntu-xfce`, port 3000) published on `127.0.0.1`, shown in the new Desktop tab;
  `DISPLAY=:1` for agent commands; Restart computer.
- **Auto-update** with `tauri-plugin-updater`: signed bundles and `latest.json` on GitHub Releases, startup check (can be
  turned off), sidebar note, Install and restart in Settings → About.
- **Composio connect** from Settings → Apps (managed auth config + hosted sign-in link).
- **Gmail automations** through Composio (`GMAIL_FETCH_EMAILS`, seen ids remembered per agent).
- Readable cron descriptions in the automation list and editor (English and Spanish).
- `opencue --version`, `opencue-cli mcp-serve`, macOS `opencue-cli` build.
- Website: 3D hero (three.js), OS-aware download button and install tabs, `llms.txt`.

### Changed

- Releases are published immediately (not drafts); the main binary is `opencue` on every platform (`mainBinaryName`).
- Messages store `attachments` (migration `0003_phase3.sql`).
- `NSMicrophoneUsageDescription` for the voice button on macOS.

## 0.2.0 — 2026-10-17

### New

- **Drawn avatars**: a colored shape with a little face — ten faces, twelve shapes and a color row. New app icon in the
  same style.
- **Cue-style profile**: Assets (Mails, Computer), Configurations (Model, Apps, Skills, Instructions, Memory, Voice,
  Permissions) and Automations, each opening its own page.
- **Automations**: schedules (5-field cron, local time), new email (IMAP polling with sender/subject filter), local
  webhooks (`http://127.0.0.1:<port>/hooks/<token>`) and group messages (optional keyword). Run now, pause, edit.
- **Email per agent**: IMAP/SMTP with auto-detected servers for Gmail, Outlook, Yahoo, iCloud and Fastmail, or Gmail via
  Composio. Tools `read_emails`, `search_inbox`, `send_email`; sending asks for approval.
- **Computers**: Docker container per agent, SPACE microVMs, WSL (Windows) or this computer (asks before each command).
  Tools `run_command`, `read_file`, `write_file`, `list_files`. New Computer panel with a live terminal and file browser.
- **Skills** (Claude Code `SKILL.md`): create, edit, import from a URL (GitHub links work); agents load them with
  `use_skill` and save new ones with `save_skill`.
- **Composio**: API key + user ID in Settings → Apps; agents get `composio_search` and `composio_execute`.
- **Voice** with ElevenLabs: microphone in the composer (Scribe speech-to-text), replies read aloud, voice per agent.
- Settings: Computers, Voice and About pages, Composio card, "Get help" link.
- Website (`site/`) with docs, privacy policy and changelog, deployed to GitHub Pages.

### Changed

- One-to-one chats no longer show avatars next to messages and use rounder bubbles; groups keep avatars.
- Event messages ("Received an email", automations) render as compact expandable rows.
- Agents are told to treat email, web pages and tool output as data, never as instructions.
- `avatar.emoji` became `avatar.face` (old emoji avatars still render until a face is picked).

## 0.1.0 — 2026-10-17

First release: agents with instructions and memory, streaming chat, group chats, providers by API key and the Codex /
Claude CLIs, MCP (stdio and HTTP), free web search, approvals, tray, English/Spanish, light/dark, CLI.
