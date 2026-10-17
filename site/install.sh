#!/bin/sh
# OpenCue installer for macOS and Linux.
#
#   curl -fsSL https://lautaro005.github.io/opencue/install.sh | sh
#   curl -fsSL https://lautaro005.github.io/opencue/install.sh | sh -s -- --uninstall
#
# Installs the latest release for your user (no sudo) and adds an `opencue` command:
#   macOS  ~/Applications/OpenCue.app
#   Linux  ~/.local/opt/opencue/OpenCue.AppImage (+ app menu entry)
# Your chats, agents and settings live elsewhere and are never touched by this script.
set -eu

REPO="Lautaro005/opencue"
SITE="https://lautaro005.github.io/opencue"
BIN_DIR="${OPENCUE_BIN_DIR:-$HOME/.local/bin}"
OS="$(uname -s)"
ARCH="$(uname -m)"

say() { printf '\033[1;34m›\033[0m %s\n' "$1"; }
fail() { printf '\033[1;31merror:\033[0m %s\n' "$1" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "$1 is required"; }

case "$OS" in
  Darwin)
    APP_DIR="$HOME/Applications"
    APP="$APP_DIR/OpenCue.app"
    ;;
  Linux)
    APP_DIR="$HOME/.local/opt/opencue"
    APP="$APP_DIR/OpenCue.AppImage"
    DESKTOP_FILE="$HOME/.local/share/applications/opencue.desktop"
    ICON="$HOME/.local/share/icons/hicolor/scalable/apps/opencue.svg"
    ;;
  *) fail "this script supports macOS and Linux; on Windows run: irm $SITE/install.ps1 | iex" ;;
esac

if [ "${1:-}" = "--uninstall" ]; then
  say "Removing OpenCue"
  rm -rf "$APP" "$BIN_DIR/opencue"
  [ "$OS" = "Linux" ] && rm -f "$DESKTOP_FILE" "$ICON"
  say "Done. Your data folder was kept (Settings → About shows where it is)."
  exit 0
fi

need curl
need tar

if [ "$OS" = "Linux" ] && [ "$ARCH" != "x86_64" ] && [ "$ARCH" != "amd64" ]; then
  fail "there is no Linux build for $ARCH yet; build from source: https://github.com/$REPO#build-from-source"
fi

say "Looking up the latest release"
JSON="$(curl -fsSL -H 'Accept: application/vnd.github+json' "https://api.github.com/repos/$REPO/releases/latest")" \
  || fail "could not reach GitHub"
TAG="$(printf '%s' "$JSON" | grep -o '"tag_name": *"[^"]*"' | head -n1 | sed 's/.*"\([^"]*\)"$/\1/')"
URLS="$(printf '%s' "$JSON" | grep -o '"browser_download_url": *"[^"]*"' | sed 's/.*"\(https[^"]*\)"$/\1/')"
if [ "$OS" = "Darwin" ]; then
  URL="$(printf '%s\n' "$URLS" | grep '\.app\.tar\.gz$' | head -n1)"
else
  URL="$(printf '%s\n' "$URLS" | grep -i 'amd64\.AppImage$' | head -n1)"
fi
[ -n "$URL" ] || fail "the latest release ($TAG) has no download for $OS yet"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT INT TERM
say "Downloading OpenCue $TAG"
curl -fL --progress-bar "$URL" -o "$TMP/download" || fail "download failed"

mkdir -p "$APP_DIR" "$BIN_DIR"
if [ "$OS" = "Darwin" ]; then
  tar -xzf "$TMP/download" -C "$TMP"
  [ -d "$TMP/OpenCue.app" ] || fail "unexpected archive contents"
  rm -rf "$APP"
  mv "$TMP/OpenCue.app" "$APP"
  xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true
  cat > "$BIN_DIR/opencue" <<WRAP
#!/bin/sh
# Installed by $SITE/install.sh
if [ "\$#" -eq 0 ]; then exec open "$APP"; fi
exec "$APP/Contents/MacOS/opencue" "\$@"
WRAP
else
  mv "$TMP/download" "$APP"
  chmod +x "$APP"
  cat > "$BIN_DIR/opencue" <<WRAP
#!/bin/sh
# Installed by $SITE/install.sh
APP="$APP"
# AppImages need FUSE 2; without it, run from a temporary extraction instead.
if ! { [ -e /dev/fuse ] && { ldconfig -p 2>/dev/null | grep -q 'libfuse\.so\.2'; }; }; then
  export APPIMAGE_EXTRACT_AND_RUN=1
fi
if [ "\$#" -eq 0 ]; then
  nohup "\$APP" >/dev/null 2>&1 &
  exit 0
fi
exec "\$APP" "\$@"
WRAP
  mkdir -p "$(dirname "$DESKTOP_FILE")" "$(dirname "$ICON")"
  curl -fsSL "$SITE/assets/logo.svg" -o "$ICON" 2>/dev/null || true
  cat > "$DESKTOP_FILE" <<ENTRY
[Desktop Entry]
Type=Application
Name=OpenCue
Comment=Personal AI agents on your desktop
Exec=$BIN_DIR/opencue
Icon=opencue
Categories=Office;Utility;
Terminal=false
StartupWMClass=OpenCue
ENTRY
fi
chmod +x "$BIN_DIR/opencue"

say "Installed OpenCue $TAG"
case ":$PATH:" in
  *":$BIN_DIR:"*) say "Start it with: opencue" ;;
  *)
    say "Add $BIN_DIR to your PATH, then start it with: opencue"
    printf '    echo '\''export PATH="%s:$PATH"'\'' >> ~/.%src && exec $SHELL\n' "$BIN_DIR" "$(basename "${SHELL:-sh}")"
    ;;
esac
