#!/usr/bin/env bash
# Checks whether this Linux host can run SPACE (Firecracker microVMs). Read-only: changes nothing.
set -u
ok() { printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad() { printf '  \033[31m✗\033[0m %s\n' "$1"; fail=1; }
fail=0

echo "SPACE host check"
[ "$(uname -s)" = "Linux" ] && ok "Linux" || bad "Not Linux ($(uname -s)); SPACE needs Linux"
[ "$(uname -m)" = "x86_64" ] && ok "x86_64" || bad "Architecture $(uname -m) (x86_64 recommended)"
if [ -e /dev/kvm ]; then
  [ -r /dev/kvm ] && [ -w /dev/kvm ] && ok "/dev/kvm accessible" || bad "/dev/kvm exists but is not read/writable by $(whoami) (add yourself to the kvm group)"
else
  bad "/dev/kvm missing (enable virtualization in BIOS / nested virtualization in your VM)"
fi
grep -Eqw 'vmx|svm' /proc/cpuinfo 2>/dev/null && ok "CPU virtualization extensions" || bad "No vmx/svm CPU flags"
command -v mkfs.btrfs >/dev/null 2>&1 && ok "btrfs-progs installed" || bad "btrfs-progs not installed"
command -v ip >/dev/null 2>&1 && ok "iproute2 installed" || bad "iproute2 (ip) not installed"

[ $fail -eq 0 ] && echo "Looks good. Continue with https://space.docs.perplexity.ai/" || echo "Fix the items above first."
exit $fail
