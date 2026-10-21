# Security Policy

OpenCue runs AI agents that can read web pages, email and shared files, call MCP tools and catalog apps, run commands on a
computer and keep data on your machine — and it installs and updates itself — so security reports are taken seriously
and handled privately.

## Supported versions

| Version | Supported |
|---|---|
| Latest release (`0.3.x`, currently `0.4.1`) | ✅ |
| `0.2.x` | ⚠️ critical fixes only — 0.3 updates itself, please update |
| Older | ❌ — please update first |

## Reporting a vulnerability

**Do not open a public issue, discussion or pull request for security problems.**

Report privately through GitHub: go to the repository's **Security** tab →
[**Report a vulnerability**](https://github.com/Lautaro005/opencue/security/advisories/new).

Please include:

- What is affected (desktop app, CLI, core, install scripts, updater, website) and the version / commit
- Your operating system (macOS, Linux distribution or Windows version) and how you installed OpenCue
- Which features are involved (provider, computer type, email, automations, files, MCP server or `mcp-serve`…)
- Steps to reproduce, a proof of concept, or logs (remove your API keys and personal data)
- The impact you expect (what an attacker can do)

### What to expect

- Acknowledgement within **7 days**
- An initial assessment within **14 days**
- A fix or mitigation plan for confirmed issues, coordinated with you before anything is disclosed
- Credit in the advisory and release notes, if you want it

Please give us a reasonable time to ship a fix before public disclosure (90 days by default, shorter for issues already
being exploited).

## Scope

In scope:

- **Prompt injection that bypasses approvals** — content from a web page, email, shared file, webhook payload, tool
  result or MCP / catalog app response that makes an agent run an action that should have required approval in "Ask first"
  mode (sending email, running a command or writing a file on the host or WSL, deleting memory, data-changing tools) —
  including through `opencue mcp-serve`, which must ask the user (or refuse) for those actions for CLI agents in "Ask first" mode, and agent-to-agent chats, which must not let an agent skip another agent's approvals
- **Sandbox escapes through OpenCue** — a command in the Docker or SPACE computer reaching the host or another agent's
  workspace because of how OpenCue mounts, names or talks to it; file tools escaping an agent's workspace folder
- **Browser and messenger abuse** — a web page or a WhatsApp message making an agent act without the approval the mode requires, a page reaching OpenCue's commands or the host's network, or the WhatsApp session leaking
- **Secret exposure** — API keys, email passwords, the SPACE token or an app's OAuth tokens leaking from `secrets.json` or `oauth/` (both `0600`), logs, the UI, the agent's context or network traffic; SPACE session tokens sent to a host
  other than the configured server
- **Webhooks and the desktop view** — the automation server or a Docker desktop port reachable from other machines, or
  automations firing without the correct token
- **Files** — a shared file name or `share_file` path writing or reading outside the attachment store or the agent's
  workspace; the UI opening shared files in a way that runs code without the user choosing to open them
- **Install and update** — `install.sh` / `install.ps1` fetching or running anything other than the official release,
  the updater accepting a bundle not signed with the release key, or update metadata pointing outside GitHub Releases
- **MCP handling** — a malicious MCP server escaping its process, injecting commands, or reaching tools it should not
- **VMPal machines** — an agent reaching a VMPal machine it was not given, or the TOML in Settings → Computers running a
  command other than the one written there (OpenCue only starts the `command` and `args` the user saved)
- **Local data** — other local users or processes reading or modifying the OpenCue database, memory, skills, workspaces
  or instruction files through OpenCue's fault
- **IPC / webview** — the UI calling privileged commands from untrusted content (rendered Markdown, links, email bodies),
  or the microphone being enabled without the user pressing the button
- **Supply chain** — compromised dependencies, release artifacts, update signatures or the website produced by our
  GitHub Actions workflows

Out of scope:

- Actions an agent takes in **Fully autonomous** mode, or on **This computer** after the user approved the command
- Commands that run inside the Docker or SPACE sandbox itself (that is their purpose), unless they escape it
- Behaviour of third-party services and tools themselves — model providers, the catalog apps' own servers (Google, Notion, GitHub…), ElevenLabs, email providers, SPACE,
  Docker and desktop images (e.g. linuxserver.io Webtop), MCP servers, the `codex` and `claude` CLIs — report those
  upstream
- Installers not being notarized or code-signed by Apple / Microsoft (known; update bundles are signed)
- Skills or MCP servers the user installed on purpose that are malicious by design
- Attacks that require an already-compromised user account or machine
- Missing hardening without a demonstrated impact (e.g. header suggestions, version disclosure)
- Model output quality, hallucinations or refusals

## Security model at a glance

- **Local-first**: chats, files, memory, skills, workspaces and settings stay in the OpenCue data folder. No telemetry.
- **Secrets** (API keys, email passwords, SPACE token, ElevenLabs key) are stored in `secrets.json` (mode `0600`) in the
  data folder, not in the system keychain (it asked for permission constantly). Catalog apps sign in with OAuth (PKCE) in
  the browser; the redirect goes to a loopback port the app opens only while a sign-in is pending, and the tokens are in
  `oauth/<app>.json` (`0600`). Secrets are never put in the agent's context.
- **Approvals** ("Ask first", the default): sending email, commands and file writes on the host or WSL, deleting
  memories, MCP tools and catalog app tools not marked read-only wait for the user.
- **Podman desktop**: Cua's noVNC desktop is published on `127.0.0.1` only (random port) with a fixed viewer password, so
  any program on the same computer can open it; the container drops all capabilities except SETUID, SETGID and SYS_CHROOT.
- **Built-in browser**: each agent has its own webview with its own page, cookies and storage (kept apart from the app's page,
  from your browsers and from other agents' pages); it has no access to OpenCue's commands. The browser tools are off unless you turn on the agent's
  Browser switch; they open only `http`/`https` pages (never `localhost`, private networks, `file:` or `javascript:`),
  ask before a site the agent has not used yet in "Ask first" mode, refuse to type into password fields and treat page
  text as data. "Clear browsing data" wipes the session.
- **WhatsApp** (Settings → Messenger): the app links as a companion device and keeps the session in
  `messenger/whatsapp.db`; nothing is hosted. Only direct chats are answered (groups, status and your own messages are
  ignored), optionally only from numbers you list. Each contact has an isolated conversation with the agent, the agent is
  told the sender is an outside contact, learning into memory and profile is off for those chats, and in "Ask first" mode
  any sensitive action a contact's message leads to asks you on the card in the agent's own chat.
- **Templates and imports**: the import from x.ai only fetches `https://x.ai/bot/…` pages (redirects away from x.ai are
  refused) and reads the data in them; nothing from a template is run, and imported skills are plain text files.
- **Subscription limits** (Settings → Usage) are read from Codex's local session files and from the rate-limit events
  Claude Code prints while an agent works; OpenCue never reads the CLIs' sign-in tokens.
- **Computers**: Docker and SPACE isolate commands from the host; each Docker agent sees only its own workspace at
  `/workspace`. On the host, Docker and WSL, file tools reject paths that leave the agent's workspace. SPACE output
  streams are only read from the configured host.
- **Webhooks** listen on `127.0.0.1` only and need a random 32-character token per automation. The Docker desktop view is
  published on a random `127.0.0.1` port only.
- **Files** are stored under `files/<random id>/` with names reduced to a plain file name; `share_file` only reads paths
  inside the agent's workspace.
- **CLI agents** reach OpenCue's tools through `opencue mcp-serve`, a stdio server started by the CLI itself (no network
  port), and the Claude CLI is started with `--strict-mcp-config` so it only sees OpenCue's server (plus the VMPal
  machines you chose), never your other MCP servers. Actions that need approval are asked in the app: `mcp-serve`
  writes a request into the local database and waits for your answer on the card; nothing is sent over the network.
  The CLI's own tools (Bash, Read, Edit…) are routed to the same card through `--permission-prompt-tool`, so in "Ask
  first" mode nothing runs on the host until you accept; in "Fully autonomous" mode they are allowed.
- **Agent-to-agent chats** (`send_to_agent` in 1:1 chats) run the other agent under its own settings and approvals;
  the agent that answers cannot start further conversations.
- **Updates** are downloaded from GitHub Releases and installed only if their minisign signature matches the public key
  built into the app. The install scripts download only from the official GitHub release over HTTPS.
- **VMPal**: machines are MCP servers started from the `[mcp_servers.<name>]` TOML the user saves in Settings →
  Computers. They start only when an agent that picked them replies, are never exposed to agents that did not pick
  them, and their tools ask for approval in "Ask first" mode unless they declare themselves read-only.
- **Slash commands and memory**: `/create-automation` and automations an agent creates ask first in "Ask first" mode;
  automatic memory only stores short text facts, and your *About me* text stays on this computer and is sent only to
  the model provider you chose, like the rest of the conversation.
- **Untrusted content**: web pages, emails, shared files, webhook payloads and tool results are labeled as data, and agents are
  instructed not to follow instructions found in them — that is not a guarantee, so keep agents in "Ask first" when they
  read untrusted content.
- **Voice**: the microphone is only used while the user holds a recording; audio goes to ElevenLabs only for
  transcription.
