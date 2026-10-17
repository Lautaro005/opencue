# Security Policy

OpenCue runs AI agents that can read web pages, call MCP tools and keep data on your computer, so security reports are
taken seriously and handled privately.

## Supported versions

| Version | Supported |
|---|---|
| Latest release (`0.1.x`) | ✅ |
| Older releases | ❌ — please update first |

## Reporting a vulnerability

**Do not open a public issue, discussion or pull request for security problems.**

Report privately through GitHub: go to the repository's **Security** tab →
[**Report a vulnerability**](https://github.com/Lautaro005/opencue/security/advisories/new).

Please include:

- What is affected (desktop app, CLI, core) and the version / commit
- Your operating system (Linux distribution or Windows version)
- Steps to reproduce, a proof of concept, or logs
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

- **Prompt injection that bypasses approvals** — content from a web page, tool result or MCP server that makes an agent
  run an action that should have required the user's approval in "Ask" mode
- **Secret exposure** — API keys leaking from the OS keychain, the `secrets.json` fallback, logs, the UI or network traffic
- **MCP handling** — a malicious MCP server escaping its process, injecting commands, or reaching tools it should not
- **Local data** — other local users or processes reading or modifying the OpenCue database, memory or instruction files
  through OpenCue's fault
- **IPC / webview** — the UI being able to call privileged commands from untrusted content (e.g. rendered Markdown or links)
- **Supply chain** — compromised dependencies or release artifacts produced by our GitHub Actions workflows

Out of scope:

- Actions an agent takes in **Autonomous** mode that the user enabled on purpose
- Behaviour of third-party model providers, MCP servers or CLIs (`codex`, `claude`) themselves — report those upstream
- Attacks that require an already-compromised user account or machine
- Missing hardening without a demonstrated impact (e.g. header suggestions, version disclosure)
- Model output quality, hallucinations or refusals

## Security model at a glance

- Data is local-first: chats, memory and settings stay in the OpenCue data folder.
- API keys are stored in the OS credential store; without one, they fall back to a `0600` file and a warning is logged.
- Sensitive tool calls (deleting memories, MCP tools not marked read-only) wait for approval unless the agent is set to
  Autonomous.
- Content from the web and from tools is treated as data; agents are instructed not to follow instructions found in it,
  but that is not a guarantee — keep agents in "Ask" mode when they use untrusted tools.
