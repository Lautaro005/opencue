# SPACE setup

OpenCue's **SPACE** computer runs each agent's commands in a [SPACE](https://github.com/perplexityai/SPACE) sandbox —
Perplexity's open-source Firecracker microVM platform — over its HTTP API.

## Automatic install (0.5.0, Linux x86_64 with KVM)

**Settings → Computers → Perplexity SPACE → Install SPACE** does everything below for you: it runs the host checks, clones
`perplexityai/SPACE` into `<data folder>/space/SPACE`, runs `install-prereqs.sh`, `install-fc-vendor.sh` and `up.sh`
(SPACE's own scripts, as its installation guide describes), waits for the API on `127.0.0.1:8080`, signs in as `root@local`
with a random password that OpenCue keeps in `secrets.json`, and fills in the API URL, the `seed:latest` template and the
token (renewed automatically). **Start** repeats the bring-up after a reboot and **Stop** runs SPACE's `teardown.sh`.
It needs Docker, `/dev/kvm`, `/dev/net/tun`, an apt-based system, root or passwordless `sudo` and 30 GiB free. On macOS
and Windows use a SPACE server on another Linux computer, as described next.

## Connecting to a SPACE you run yourself

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
   **Test connection**. The token is stored on this computer (`secrets.json`, mode `0600`).
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
