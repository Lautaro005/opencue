# Security Policy

OpenCue runs AI agents that can read web pages, email and shared files, call MCP and Composio tools, run commands on a
computer and keep data on your machine — and it installs and updates itself — so security reports are taken seriously
and handled privately.

## Supported versions

| Version | Supported |
|---|---|
| Latest release (`0.3.x`) | ✅ |
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
  result or MCP / Composio response that makes an agent run an action that should have required approval in "Ask first"
  mode (sending email, running a command or writing a file on the host or WSL, deleting memory, data-changing tools) —
  including through `opencue mcp-serve`, which must refuse those actions for CLI agents in "Ask first" mode
- **Sandbox escapes through OpenCue** — a command in the Docker or SPACE computer reaching the host or another agent's
  workspace because of how OpenCue mounts, names or talks to it; file tools escaping an agent's workspace folder
- **Secret exposure** — API keys, email passwords, the SPACE token or the Composio key leaking from the OS keychain, the
  `secrets.json` fallback, logs, the UI, the agent's context or network traffic; SPACE session tokens sent to a host
  other than the configured server
- **Webhooks and the desktop view** — the automation server or a Docker desktop port reachable from other machines, or
  automations firing without the correct token
- **Files** — a shared file name or `share_file` path writing or reading outside the attachment store or the agent's
  workspace; the UI opening shared files in a way that runs code without the user choosing to open them
- **Install and update** — `install.sh` / `install.ps1` fetching or running anything other than the official release,
  the updater accepting a bundle not signed with the release key, or update metadata pointing outside GitHub Releases
- **MCP handling** — a malicious MCP server escaping its process, injecting commands, or reaching tools it should not
- **Local data** — other local users or processes reading or modifying the OpenCue database, memory, skills, workspaces
  or instruction files through OpenCue's fault
- **IPC / webview** — the UI calling privileged commands from untrusted content (rendered Markdown, links, email bodies),
  or the microphone being enabled without the user pressing the button
- **Supply chain** — compromised dependencies, release artifacts, update signatures or the website produced by our
  GitHub Actions workflows

Out of scope:

- Actions an agent takes in **Fully autonomous** mode, or on **This computer** after the user approved the command
- Commands that run inside the Docker or SPACE sandbox itself (that is their purpose), unless they escape it
- Behaviour of third-party services and tools themselves — model providers, Composio, ElevenLabs, email providers, SPACE,
  Docker and desktop images (e.g. linuxserver.io Webtop), MCP servers, the `codex` and `claude` CLIs — report those
  upstream
- Installers not being notarized or code-signed by Apple / Microsoft (known; update bundles are signed)
- Skills or MCP servers the user installed on purpose that are malicious by design
- Attacks that require an already-compromised user account or machine
- Missing hardening without a demonstrated impact (e.g. header suggestions, version disclosure)
- Model output quality, hallucinations or refusals

## Security model at a glance

- **Local-first**: chats, files, memory, skills, workspaces and settings stay in the OpenCue data folder. No telemetry.
- **Secrets** (API keys, email passwords, SPACE token, Composio and ElevenLabs keys) are stored in the OS credential
  store; without one they fall back to a `0600` file and a warning is logged. They are never put in the agent's context.
- **Approvals** ("Ask first", the default): sending email, commands and file writes on the host or WSL, deleting
  memories, MCP tools not marked read-only and Composio actions that may change data wait for the user.
- **Computers**: Docker and SPACE isolate commands from the host; each Docker agent sees only its own workspace at
  `/workspace`. On the host, Docker and WSL, file tools reject paths that leave the agent's workspace. SPACE output
  streams are only read from the configured host.
- **Webhooks** listen on `127.0.0.1` only and need a random 32-character token per automation. The Docker desktop view is
  published on a random `127.0.0.1` port only.
- **Files** are stored under `files/<random id>/` with names reduced to a plain file name; `share_file` only reads paths
  inside the agent's workspace.
- **CLI agents** reach OpenCue's tools through `opencue mcp-serve`, a stdio server started by the CLI itself (no network
  port). Actions that need approval are refused in "Ask first" mode.
- **Updates** are downloaded from GitHub Releases and installed only if their minisign signature matches the public key
  built into the app. The install scripts download only from the official GitHub release over HTTPS.
- **Untrusted content**: web pages, emails, shared files, webhook payloads and tool results are labeled as data, and agents are
  instructed not to follow instructions found in them — that is not a guarantee, so keep agents in "Ask first" when they
  read untrusted content.
- **Voice**: the microphone is only used while the user holds a recording; audio goes to ElevenLabs only for
  transcription.
