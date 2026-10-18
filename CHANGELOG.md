# Changelog

All notable changes to OpenCue. The website version is at <https://lautaro005.github.io/opencue/changelog/>.

## 0.3.4 — 2026-10-18

Asked for as "0.3.3.01"; Cargo, Tauri and the updater only accept `major.minor.patch`, so it ships as 0.3.4.

### New

- **Composio keys that work**: a consumer key (`ck_…`, the one Composio Connect gives for MCP clients) now connects
  through `connect.composio.dev/mcp` with the `x-consumer-api-key` header, and its tools (search, connect, run) go to
  agents with the "Composio apps" switch. A project key (`ak_…`) keeps using Composio's API. The key stays in your
  keychain (the MCP server only references it). Settings → Apps → Composio shows the connection, the number of tools
  and the apps already connected, and "Connect an app" opens the sign-in link for the app you pick.
- **Local VM (Docker) options per agent** (profile → Computer): live desktop on/off, memory (1–16 GB), CPUs and a
  **VM folder** you choose. The folder is the VM's home (desktop settings, browser profile, downloads, anything you
  install under it), so progress survives closing the app, stopping the VM or recreating it. By default it is a folder
  inside OpenCue's data. Changing these recreates the VM the next time it starts, keeping the workspace and the folder.
- **Perplexity SPACE**: named as such in Settings → Computers and in the agent's Computer page, with a GitHub link
  (github.com/perplexityai/SPACE) and its docs.
- **Provider logos on the website**: the models section shows each provider's mark instead of its name (drawn as
  simplified SVGs in the text color, so they read well in light and dark themes).

### Changed

- The Docker computer is called **Local VM (Docker)**; the desktop switch can be set per agent (it follows Settings →
  Computers until you choose).
- The Composio server for a consumer key is hidden from the Apps lists, like VMPal machines.

### Not possible

- A **built-in default Composio key** (as OpenMausBot ships): it relies on a service run by that project that hands out
  keys. OpenCue has no such service and will not bundle someone's key; each person pastes their own.

## 0.3.3 — 2026-10-18

Planned as "0.3.2.01"; Cargo, Tauri and the updater only accept `major.minor.patch`, so it ships as 0.3.3.

### New

- **Working folder** (agent profile → Working folder): shows the folder the agent works in — its own folder inside
  OpenCue's data by default — and lets you point it anywhere (native folder picker, or type a path; created if missing,
  back to the default in one click). Commands, files and shared files all use it; Docker mounts it at `/workspace`.
- **Connect an app, with a real search**: touch the field to see the apps Composio offers (most popular first, with
  logos), type to narrow them by name, pick one to start its sign-in; arrow keys and Enter work too.
- **Links are pills**: every link in a chat is a small rounded pill with the site's name (or the link's own words) and
  an arrow out; the full address shows on hover.
- **A sturdier agent loop**, from a study of Grok Bot, Manus, Muse and OpenManus:
  - *Stable prompt prefix*: what changes (today's date, the plan, the conversation summary) now goes last.
  - *Long results go to files*: a tool result over 12,000 characters is saved in the workspace
    (`.opencue/outputs/`) and the model gets the start and the path.
  - *Smart compaction*: when a conversation grows past ~90,000 characters the older part is replaced by a structured
    summary instead of being dropped; the newest messages stay word for word.
  - *Plan recitation*: new `update_plan` tool; the plan is shown back at the end of every step.
  - *Helpers with a clean context*: new `research` tool hands a reading job to a read-only helper (web, memory,
    workspace files) that returns a short report.
  - *Loop guard and timings*: the same tool call a third time in one turn is refused with a hint, and every tool call
    logs its duration.

### Changed

- Claude Code, Codex and Grok list the models they accept: Claude's aliases and ids plus anything in your Claude
  settings (`availableModels`, `ANTHROPIC_*MODEL*`), the visible models in Codex's cache, Grok 4.7 / 4.6 / 4.5 plus
  your `[model.*]` entries.
- When a CLI's sign-in has expired, the error says how to renew it (`claude` → /login, `codex login`, `grok login`).

### Fixed

- **Composio 401**: the key now goes in the header Composio expects for its type — `x-api-key` for project keys
  (`ak_…`), `x-consumer-api-key` for consumer keys (`ck_…`), `x-user-api-key` for user keys (`uak_…`) — and if one is
  refused the others are tried. A 401 now says what to check.
- **Odd-looking replies**: short answers wrapped into a narrow bubble ("4 × 4 ÷ 2" / "= 8") because of a width rule
  added in 0.3.2. Bubbles size to their text again.

## 0.3.2 — 2026-10-18

### New

- **Base system prompt for every agent**: a short built-in prompt tells each agent it is an agent inside OpenCue (it has
  tools, memory, instructions and skills, acts instead of only answering, and asks before irreversible actions). It sits
  before your per-agent instructions and is never shown or edited in the app.
- **A new agent opens the chat by asking what its job will be.** Its answer becomes its profile, written by the agent
  itself. The agent builds two documents as you talk: **instructions** (what it does) and a new **soul** (`SOUL.md`:
  who it is — personality, tone, values). New `update_instructions` and `update_soul` tools; models without tools get
  the same result from a small background pass after each reply. Both are editable in the agent's profile, and a switch
  turns the self-writing off.
- **Grok CLI** (xAI's Grok Build): a new provider that runs your installed, logged-in `grok` binary.
- **System notifications** (Settings → Notifications): a finished reply or an approval request while OpenCue is not in
  front, with switches for each kind, "only when OpenCue isn't in front", showing or hiding the message text, and a test
  button.
- **Menu-bar / tray menu**: clicking the icon opens a menu with the app name and version on top, then Open, Settings,
  Check for Updates and Quit.
- **VMPal is an option in an agent's Computer page**, above "This computer". Picking it shows the machine list (and the
  paste-a-table box).
- **Resizable panels**: drag the edge of the agent sidebar and of the agent / group settings panel. Widths are
  remembered; double-click an edge to reset it.

### Changed

- Messages (yours and the agent's) now start and end at the width of the header and the chat input.
- Claude Code and Codex agents get the VMPal machines you picked, as MCP servers.
- Claude Code, Codex and Grok list the models they can actually use: Claude's aliases and ids, the models in Codex's
  own cache, and Grok's default plus any `[model.*]` in `~/.grok/config.toml`.

### Fixed

- **CheaperInference** was spelled "CheapestInference" (and pointed at the wrong address). Existing providers are
  renamed and moved to `https://api.cheaperinference.com/v1` automatically.
- **Claude Code and Codex failed to run from the installed app**: apps opened from the Dock or Finder start with a bare
  PATH, so `claude`, `codex`, `npx` and `docker` were not found. OpenCue now reads your login shell's PATH at start-up and
  adds the usual install folders. Errors now say how to sign in.
- **Local models said they could not control a VMPal machine**: machines with similar names shared tool names, and the
  prompt did not say how to start a stopped VM or turn on agent control. Tool names are now unique and short, and the
  agent is told to call `vmpal_start` if the VM is off and to ask you to enable Settings › AI Agents in VMPal.
- Horizontal scroll in an agent's settings panel.

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
