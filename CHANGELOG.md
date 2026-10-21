# Changelog

All notable changes to OpenCue. The website version is at <https://lautaro005.github.io/opencue/changelog/>.

## 0.4.0 — 2026-10-20

### New

- **Built-in browser.** Computer panel → **Browser** tab: a separate webview (its own cookies and storage) that an agent
  can drive while you watch. A switch in the agent's Computer page gives it `browser_open`, `browser_snapshot`,
  `browser_click`, `browser_type` and `browser_close`; the panel opens on the Browser tab by itself when it uses them.
  A new site asks first, only public `http`/`https` pages open, passwords are never typed for you, and the pane has an
  address bar, back / forward / reload and "clear browsing data". Works for Claude Code and Codex agents too.
- **Computer panel redone**: the VM (and the browser, as tabs) on top and the terminal always below; the Files tab is
  gone (the agent's page has it).
- **Messenger → WhatsApp** (Settings, under Apps): scan a QR in WhatsApp → Linked devices and pick the agent that
  answers. It runs inside the app (no server). Each contact gets their own conversation; a message that arrives is
  answered automatically and appears in the agent's chat as a green **WhatsApp** card that unfolds into the question
  and the answer. Optional list of numbers allowed; groups and status are ignored.
- **Templates** in the sidebar's **+** menu: 31 ready-made agents and 7 teams adapted from popular Grok Bot templates
  (meta, engineering, research, inbox, growth, creative, life). The window also imports from x.ai: a marketplace bot
  (`x.ai/bot/marketplace/bots/…`) comes with its instructions, memories, skills, routines and apps; a shared link
  (`x.ai/bot/…`) only exposes a name and a description.
- **Search** (magnifier next to **+**, or ⌘K / Ctrl+K): finds a message in every chat and jumps to it.
- **Subscription limits** under the Usage table: for Codex and Claude Code, the 5-hour and weekly windows with how much
  is left (bar), and when each resets. Codex is read from its session files; Claude Code appears once an agent has
  replied.

### Changed

- Reasoning is a subtle "Reasoning ▸" fold above the answer; what an agent says between tool calls goes in it too.
- Settings → Computers: with Podman there is no image field.

## 0.3.8 — 2026-10-19

### Fixed

- **Podman desktop showed a black screen.** With Podman the VM now uses Cua's XFCE desktop image (the one OpenMausBot
  runs), started the way it needs (user namespace, capabilities, noVNC on a loopback port). The image field is hidden
  for Podman; Docker keeps its own.
- **The chosen VM is respected.** An agent set to Docker or Podman no longer opened VMPal because machines were still
  ticked there: VMPal tools are only offered when VMPal is the agent's computer, and the prompt says "the VM" is the
  local one.
- **Permission card alignment** in group chats: it lines up with the agents' replies instead of starting under the avatar.

### New

- Settings → Computers: **Stop all VMs** button, a **play** button next to the start command (Podman machine, Docker
  Desktop on macOS) that runs it and shows its output, and **desktop view on by default**.
- **Reasoning is a folded message** ("Reasoning ▸") above the answer. It comes from models that stream their thinking
  (OpenAI-compatible `reasoning`, Ollama, Claude Code) and from `<think>` tags; it is left out of the history, spoken
  replies, memory and the sidebar preview.
- The **call button is gray** until an ElevenLabs key is set; hovering says so and touching it opens Settings → Voice.
- The sidebar no longer shows "See how people use OpenCue".
- The chat's **+** menu has **Agents** and **Skills** entries that open a list to the side (hover or click) and insert
  the @mention or the `/skill`.

## 0.3.7 — 2026-10-19

### Fixed

- **Permission cards for Claude Code agents.** The CLI's own tools (Bash, Read, Edit…) used to fail with "no permission"
  in a headless run. They now ask the app (`--permission-prompt-tool`), so the yellow card with Accept / Reject shows in
  the chat and the command runs when you accept. In *Fully autonomous* mode they are allowed without asking.
- **Podman with no Docker.** The Computer page offered "Local VM (Docker)" as unavailable and would not let you pick it
  when only Podman was installed. It now reads "Local VM (Docker or Podman)", is available with either, and with
  nothing chosen yet OpenCue uses Podman when Docker is not installed.
- **Catalog sign-in errors** ("Not allowed to open url …"): web links now open through the app itself, so long OAuth
  links with paths and query strings work.
- **Agent chats from Claude Code agents** now show the "Chat with …" card (before, only API-model agents did).

### New

- **Stop button for VMs** in the Computer panel header (local VMs): stops the container without losing files or
  installed programs; the next command starts it again.
- **Live agent conversation**: when an agent talks to another in a 1:1 chat, the card has a moving **green** border and
  shows the conversation as it happens; click its title to open the full view.
- **Composio in the Catalog**, as the first card (Google Calendar left the list; it works through Composio). Gmail,
  Google Drive, GitHub and Outlook need an OAuth key of your own: if Composio is connected, touching them signs you in
  through Composio; if not, the card says to connect Composio first (or enter your own client ID). The apps connected
  through Composio show as small tags with a blue moving border under the Catalog.
- Catalog cards show their **status at the bottom**, under the description.

## 0.3.6 — 2026-10-19

### New

- **Podman as an alternative to Docker** for Local VMs: a switch in Settings → Computers → *Container engine*. The
  setup checklist follows it (install Podman; on macOS and Windows create and start the Podman machine; download the
  desktop; create the VM), short image names are spelled out for Podman (`docker.io/library/…`), and each agent can
  choose its own engine in its Computer page (default: follow the setting).
- **Catalog** (Settings → Apps → Catalog) replaces Composio: Gmail, Google Drive, Google Calendar, Notion, Trello,
  GitHub, Supabase, Vercel and Outlook, each a card with a name and a description and a **moving border: red** (not
  connected), **yellow** (connect again) or **green** (connected). Touching a card opens the browser to sign in with
  OAuth and the app comes back to the front when it is done. Tokens are kept in `oauth/<app>.json` (mode `0600`) and
  refreshed by themselves. For providers that don't register apps automatically (Google, Microsoft, GitHub) the card
  asks for a client ID you registered. In an agent's profile → Apps, a **Catalog apps** list turns each app on or off;
  CLI agents (Claude Code, Codex) get the apps through OpenCue's tools.
- **The terminal opens by itself**: when an agent runs a command on its computer (Docker/Podman VM, SPACE, WSL, this
  computer) the Computer panel opens on the terminal and shows the commands as they run (General → *Open the computer
  when an agent uses it*).
- **Right-click an agent or group** in the sidebar: Rename (in place), Pin / Unpin (pinned chats go first), Clear chat,
  Reset chat (also forgets the summary, the plan and the agent's chats with other agents) and Delete.
- **Voice calls**: the phone icon in an agent's header replaces the "⋯" menu and opens a phone-sized floating window
  (draggable). You just talk: the app listens (voice-activity detection), transcribes, sends it to the agent, reads the
  reply aloud and listens again; talking over the agent interrupts it. Mute, speaker and hang up buttons, a timer and
  live captions. While a call lasts the agent is told to answer like speech. It uses ElevenLabs (Settings → Voice) or,
  without a key, the system's speech recognition and voice when the webview offers them.

### Removed

- **Composio** (its Apps card, the `composio_*` tools, the Gmail-through-Composio email kind and its key). Its MCP server
  and key are deleted on first start.
- **The system keychain**: it asked for permission over and over. API keys, email passwords and tokens now live in a
  `0600` `secrets.json` in the data folder. **Keys saved in the keychain by earlier versions are not read any more:
  enter them again** (Settings → Providers, Voice, Computers, and each agent's email).

## 0.3.5 — 2026-10-19

Asked for as "0.3.4.1" (fixes); Cargo, Tauri and the updater only accept `major.minor.patch`, so it ships as 0.3.5.

### New

- **Agents talk to each other in 1:1 chats**, without a group. An agent can call `send_to_agent` (its prompt lists the
  other agents) and gets the other agent's reply back as the result; the exchange is saved as a conversation of its own.
  In the chat it shows as a **"Chat with …" pill** that opens that conversation (with a "Back to …" button). The agent
  that answers can't hand the work on again, so chains can't loop.
- **`@` in 1:1 chats**: type `@` to pick another agent; the agent you are talking to then asks them and answers you
  with the result.
- **Ask / permission card**: when an agent needs a decision or permission it shows a card with a **moving yellow
  border**, a title, the request and either a text field (plus optional answer buttons) or **Accept / Reject**. Agents
  get a new `ask_user` tool for it, and the existing approvals use the same card. It works for CLI agents too: in "Ask
  first" mode their approvals are now asked instead of refused. Notifications cover it as well.
- **Local VM isolation**: Settings → Computers → *Isolation*: **Shared** (one Docker computer and one desktop for every
  agent, in OpenCue's `shared` folder) or **Per agent** (as before). *Stop idle VMs after* N minutes (default 480; a
  running turn is never interrupted; stopping keeps files and installed apps; 0 never stops).
- **Docker setup checklist** (Settings → Computers → *Setup*): install Docker, start it (with the command for your
  system), download the desktop image once (with progress) and create/start the shared VM.
- **Refresh button** next to *Save* in Settings → Apps → Composio: reconnects and reloads the connected apps.

### Fixed

- **Claude Code agents failing on Composio** with "needs permission": the CLI also loaded your own MCP servers (for
  example a `brain` connector) that cannot be approved in a headless run. Agents now start the CLI with
  `--strict-mcp-config`, so only OpenCue's tools (and the VMPal machines you picked) are visible, and OpenCue's own
  Composio tools do the work.

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
