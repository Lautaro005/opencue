# SPACE setup (preliminary)

OpenCue's "computer" backend on Linux will talk to a [SPACE](https://github.com/perplexityai/SPACE) server — Perplexity's
open-source Firecracker microVM sandbox platform — over its HTTP API. OpenCue **connects to** a SPACE URL; it does not
install or manage SPACE itself. This integration arrives in Phase 2; this page describes what you need to prepare.

## Requirements

- Linux x86_64 with hardware virtualization and access to `/dev/kvm`
- A filesystem SPACE supports for its images (the project uses btrfs)
- Root access for networking setup (tap devices, iptables/nftables)

Run `scripts/check-space-host.sh` to check the basics on the target machine.

## Install

Follow the official documentation: <https://space.docs.perplexity.ai/>. When the server is running, note its API base URL.

## Connect from OpenCue (Phase 2)

Settings → Computers → SPACE → paste the API URL. On Windows (or Linux without KVM) point it at a remote Linux host
running SPACE.
