#!/bin/sh
# envit installer (macOS, Linux). Usage: curl -fsSL https://envit.dev/install.sh | sh
# Downloads the latest release binary from GitHub and installs it.
# Set ENVIT_INSTALL_DIR to change the destination (default: ~/.local/bin).
set -eu

REPO="plannotator/envit"
INSTALL_DIR="${ENVIT_INSTALL_DIR:-$HOME/.local/bin}"

os=$(uname -s)
arch=$(uname -m)
case "$os" in
  Darwin) os_part="apple-darwin" ;;
  Linux)  os_part="unknown-linux-musl" ;;
  *) echo "error: unsupported OS: $os (Windows: see https://envit.dev/install.ps1)" >&2; exit 1 ;;
esac
case "$arch" in
  arm64|aarch64) arch_part="aarch64" ;;
  x86_64|amd64)  arch_part="x86_64" ;;
  *) echo "error: unsupported architecture: $arch" >&2; exit 1 ;;
esac

target="${arch_part}-${os_part}"
url="https://github.com/${REPO}/releases/latest/download/envit-${target}.tar.gz"

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "downloading envit (${target})..."
curl -fsSL "$url" -o "$tmp/envit.tar.gz"
curl -fsSL "$url.sha256" -o "$tmp/envit.tar.gz.sha256" || true
if [ -f "$tmp/envit.tar.gz.sha256" ] && command -v shasum >/dev/null 2>&1; then
  (cd "$tmp" && shasum -a 256 -c envit.tar.gz.sha256 >/dev/null) || {
    echo "error: checksum verification failed" >&2; exit 1;
  }
fi

if command -v gh >/dev/null 2>&1; then
  # Provenance: proves the tarball was built by plannotator/envit's release workflow.
  gh attestation verify "$tmp/envit.tar.gz" --owner plannotator >/dev/null 2>&1 \
    && echo "provenance verified (GitHub attestation)" \
    || echo "note: provenance not verified (gh attestation verify failed or unavailable)"
fi

tar -xzf "$tmp/envit.tar.gz" -C "$tmp"
mkdir -p "$INSTALL_DIR"
install -m 755 "$tmp/envit" "$INSTALL_DIR/envit"

echo "installed: $INSTALL_DIR/envit"
case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) echo "note: $INSTALL_DIR is not on your PATH" ;;
esac
"$INSTALL_DIR/envit" --version
