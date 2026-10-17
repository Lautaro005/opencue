# Changelog

All notable changes to OpenCue. The website version is at <https://lautaro005.github.io/opencue/changelog/>.

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
