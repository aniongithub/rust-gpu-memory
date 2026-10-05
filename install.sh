#!/usr/bin/env sh
# Installs the gpu-memory CLI from the latest GitHub release.
#
# Works on macOS and Linux (x86_64 and aarch64). The repository is currently
# PRIVATE, so set GH_TOKEN (or GITHUB_TOKEN) to a token with read access. Once
# the repo is public the token is not required and the script keeps working.
#
# Usage:
#   sh install.sh                          # install the latest release
#   GPU_MEMORY_VERSION=v0.1.0 sh install.sh  # pin a specific tag
#
# The binary is installed to ~/.local/bin/gpu-memory.

set -eu

REPO="aniongithub/rust-gpu-memory"
BIN_NAME="gpu-memory"
INSTALL_DIR="${HOME}/.local/bin"
API="https://api.github.com"
TOKEN="${GH_TOKEN:-${GITHUB_TOKEN:-}}"

die() { printf 'error: %s\n' "$1" >&2; exit 1; }

# curl with an auth header when a token is set.
api() {
  if [ -n "$TOKEN" ]; then
    curl -fsSL -H "Authorization: Bearer $TOKEN" "$@"
  else
    curl -fsSL "$@"
  fi
}

# Map uname output to the release asset's target triple.
detect_platform() {
  os="$(uname -s)"
  arch="$(uname -m)"
  case "$os" in
    Darwin)
      case "$arch" in
        arm64|aarch64) echo "aarch64-apple-darwin" ;;
        x86_64|amd64)  echo "x86_64-apple-darwin" ;;
        *) die "unsupported macOS arch: $arch" ;;
      esac ;;
    Linux)
      case "$arch" in
        aarch64|arm64) echo "aarch64-unknown-linux-gnu" ;;
        x86_64|amd64)  echo "x86_64-unknown-linux-gnu" ;;
        *) die "unsupported Linux arch: $arch" ;;
      esac ;;
    *) die "unsupported OS: $os" ;;
  esac
}

# Resolve the tag to install: pinned via GPU_MEMORY_VERSION, else latest.
resolve_tag() {
  if [ -n "${GPU_MEMORY_VERSION:-}" ]; then
    printf '%s' "$GPU_MEMORY_VERSION"
    return
  fi
  api "$API/repos/$REPO/releases/latest" \
    | tr -d '[:space:]' \
    | grep -oE '"tag_name":"[^"]*"' | head -n1 | cut -d'"' -f4
}

# Find the API url of the release asset with the given name. The GitHub API
# returns pretty-printed JSON, so whitespace is stripped first (URLs and asset
# names contain no spaces) to make the field patterns match. Each asset's
# fields appear in the order url, id, node_id, name.
asset_url() {
  tag="$1"; name="$2"
  esc_name=$(printf '%s' "$name" | sed 's/\./\\./g')
  api "$API/repos/$REPO/releases/tags/$tag" \
    | tr -d '[:space:]' \
    | grep -oE "\"url\":\"https://api.github.com/[^\"]*releases/assets/[0-9]+\",\"id\":[0-9]+,\"node_id\":\"[^\"]*\",\"name\":\"${esc_name}\"" \
    | grep -oE '"url":"[^"]*"' | head -n1 | cut -d'"' -f4
}

# --- main ------------------------------------------------------------------

if [ -z "$TOKEN" ]; then
  printf 'note: no GH_TOKEN/GITHUB_TOKEN set; the repo is currently private, so a token with read access is required.\n'
fi

PLATFORM="$(detect_platform)"
TAG="$(resolve_tag)"
if [ -z "$TAG" ]; then
  die "could not resolve a release tag. If the repo is private, set GH_TOKEN (or GITHUB_TOKEN) to a token with read access."
fi
# Tags are 'vX.Y.Z'; normalize so the API endpoint gets the exact tag name.
case "$TAG" in v*) ;; *) TAG="v$TAG" ;; esac

# Assets are named by target triple, not version (see release.yml).
ASSET="gpu-memory-${PLATFORM}.tar.gz"
URL="$(asset_url "$TAG" "$ASSET")"
[ -n "$URL" ] || die "release asset '$ASSET' not found for $TAG (platform: $PLATFORM)"

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

printf 'downloading %s (%s)...\n' "$ASSET" "$TAG"
# The API asset endpoint returns the raw bytes (no S3 redirect) and works for
# both public and private repos; the direct download URL does not for private.
api -H "Accept: application/octet-stream" "$URL" -o "$TMP/$ASSET"

mkdir -p "$INSTALL_DIR"
tar xzf "$TMP/$ASSET" -C "$TMP"
[ -f "$TMP/$BIN_NAME" ] || die "expected '$BIN_NAME' inside $ASSET"
chmod +x "$TMP/$BIN_NAME"
mv -f "$TMP/$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"

# macOS: ad-hoc sign so Gatekeeper doesn't reject an unsigned binary.
if [ "$(uname -s)" = "Darwin" ]; then
  codesign --force --sign - "$INSTALL_DIR/$BIN_NAME" 2>/dev/null || true
fi

# Verify the binary runs and report the version if it supports --version.
if ! "$INSTALL_DIR/$BIN_NAME" >/dev/null 2>&1; then
  die "installed binary failed to run: $INSTALL_DIR/$BIN_NAME"
fi
VER="$("$INSTALL_DIR/$BIN_NAME" --version 2>/dev/null | head -n1 || true)"
if printf '%s' "$VER" | grep -q "$BIN_NAME"; then
  printf 'installed: %s\n' "$VER"
else
  printf 'installed %s to %s\n' "$BIN_NAME" "$INSTALL_DIR/$BIN_NAME"
fi

# PATH check.
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) printf '\nnote: %s is not on your PATH. Add this to your shell profile:\n  export PATH="%s:$PATH"\n' "$INSTALL_DIR" "$INSTALL_DIR" ;;
esac
