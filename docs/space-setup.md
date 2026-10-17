# SPACE setup

OpenCue's **SPACE** computer runs each agent's commands in a [SPACE](https://github.com/perplexityai/SPACE) sandbox —
Perplexity's open-source Firecracker microVM platform — over its HTTP API. OpenCue **connects to** a SPACE server; it does
not install or manage SPACE itself.

## Requirements

- Linux x86_64 with hardware virtualization and access to `/dev/kvm`
- A filesystem SPACE supports for its images (the project uses btrfs)
- Root access for networking setup (tap devices, iptables/nftables)

Run `scripts/check-space-host.sh` on the target machine to check the basics.

## Install

Follow the official documentation: <https://space.docs.perplexity.ai/>. When the server is running, note its API base URL
(for example `http://host:8080/v1`), the template you want agents to boot, and an API token if your deployment uses one.

## Connect from OpenCue

1. **Settings → Computers → SPACE**: paste the API URL, the template (defaults to `seed:latest`) and the token, then
   **Test connection**. The token is stored in the system keychain.
2. In an agent's profile → **Computer**, choose **SPACE microVM**.
3. Open the Computer panel (screen icon in the chat header) to watch commands and browse files.

On Windows, or Linux without KVM, point OpenCue at a remote Linux host running SPACE.

## How OpenCue uses the API

| Step | Request |
|---|---|
| Create the agent's sandbox on first use | `POST /v1/sandboxes` with `{ "template": … }`, then poll `GET /v1/operations/{id}` |
| Reuse it later | `GET /v1/sandboxes/{id}`; if suspended or paused, `POST /v1/sandboxes/{id}:wake` |
| Run a command | `POST /v1/sandboxes/{id}/commands` with `{ "command": ["sh", "-lc", …], "timeout": "120s" }` |
| Read output | `GET` the returned `attach.events_url` (SSE) with the session token until `process_exited` |

Every mutating request carries an `Idempotency-Key`. Files are read and written through commands (`head`, `base64 -d`)
because SPACE has no file endpoints. The sandbox id is remembered in `agents/<id>/space.json` in the OpenCue data folder.
OpenCue refuses to send the session token if SPACE returns an output stream on a different host than the API URL.
