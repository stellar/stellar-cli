#!/bin/sh
set -e

# Stellar CLI installation script
# Usage: curl -fsSL https://raw.githubusercontent.com/stellar/stellar-cli/main/install.sh | sh
# Or with options: curl -fsSL https://raw.githubusercontent.com/stellar/stellar-cli/main/install.sh | sh -s -- --user

REPO="stellar/stellar-cli"
BINARY_NAME="stellar"

# Check if user has sudo privileges
has_sudo() {
  if command -v sudo >/dev/null 2>&1; then
    # Try sudo -n true to check if we can run sudo without password prompt
    # or if user is in sudoers
    if sudo -n true 2>/dev/null; then
      return 0
    fi
    # If sudo requires password, prompt for it once to check
    if sudo -v 2>/dev/null; then
      return 0
    fi
  fi
  return 1
}

# Parse arguments and set defaults
INSTALL_DIR=""
USER_INSTALL=false

for arg in "$@"; do
  case $arg in
    --user)
      USER_INSTALL=true
      INSTALL_DIR="${HOME}/.local/bin"
      shift
      ;;
    --dir=*)
      INSTALL_DIR="${arg#*=}"
      shift
      ;;
    --help)
      echo "Stellar CLI installer"
      echo ""
      echo "Usage: sh install.sh [options]"
      echo ""
      echo "Options:"
      echo "  --user        Install to ~/.local/bin (no sudo required)"
      echo "  --dir=PATH    Install to custom directory"
      echo "  --help        Show this help message"
      exit 0
      ;;
  esac
done

# Set default install directory if not specified
if [ -z "$INSTALL_DIR" ]; then
  if has_sudo; then
    INSTALL_DIR="/usr/local/bin"
  else
    INSTALL_DIR="${HOME}/.local/bin"
    USER_INSTALL=true
    echo "Note: No sudo privileges detected, installing to $INSTALL_DIR"
  fi
fi

# Detect OS
OS="$(uname -s)"
case "$OS" in
  Linux*)
    OS="linux"
    ;;
  Darwin*)
    OS="macos"
    ;;
  *)
    echo "Error: Unsupported operating system: $OS"
    exit 1
    ;;
esac

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
  x86_64|amd64)
    ARCH="x86_64"
    ;;
  aarch64|arm64)
    ARCH="aarch64"
    ;;
  *)
    echo "Error: Unsupported architecture: $ARCH"
    exit 1
    ;;
esac

# Construct target triple
if [ "$OS" = "linux" ]; then
  TARGET="${ARCH}-unknown-linux-gnu"
elif [ "$OS" = "macos" ]; then
  TARGET="${ARCH}-apple-darwin"
fi

echo "Detected platform: $OS ($TARGET)"

# Get latest release version
echo "Fetching latest release..."
LATEST_RELEASE=$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')

if [ -z "$LATEST_RELEASE" ]; then
  echo "Error: Could not fetch latest release"
  exit 1
fi

VERSION="${LATEST_RELEASE#v}"
echo "Latest version: $VERSION"

# Construct download URL
ARCHIVE_NAME="${REPO##*/}-${VERSION}-${TARGET}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${LATEST_RELEASE}/${ARCHIVE_NAME}"

echo "Downloading from: $DOWNLOAD_URL"

# Create temporary directory
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# Download and extract
cd "$TMP_DIR"
if ! curl -fsSL "$DOWNLOAD_URL" -o "$ARCHIVE_NAME"; then
  echo "Error: Failed to download $DOWNLOAD_URL"
  exit 1
fi

echo "Extracting archive..."
tar xzf "$ARCHIVE_NAME"

# Create install directory if it doesn't exist
if [ ! -d "$INSTALL_DIR" ]; then
  echo "Creating install directory: $INSTALL_DIR"
  mkdir -p "$INSTALL_DIR"
fi

# Install binary
echo "Installing $BINARY_NAME to $INSTALL_DIR..."
if [ "$USER_INSTALL" = true ] || [ -w "$INSTALL_DIR" ]; then
  mv "$BINARY_NAME" "$INSTALL_DIR/"
  chmod +x "$INSTALL_DIR/$BINARY_NAME"
else
  sudo mv "$BINARY_NAME" "$INSTALL_DIR/"
  sudo chmod +x "$INSTALL_DIR/$BINARY_NAME"
fi

echo ""
echo "✓ Stellar CLI installed successfully!"
echo ""
echo "Location: $INSTALL_DIR/$BINARY_NAME"
echo "Version: $VERSION"
echo ""

# Check if install directory is in PATH
case ":$PATH:" in
  *":$INSTALL_DIR:"*)
    echo "Run '$BINARY_NAME --version' to verify the installation."
    ;;
  *)
    echo "Note: $INSTALL_DIR is not in your PATH."
    echo "Add it to your PATH by adding this line to your shell profile:"
    echo ""
    echo "  export PATH=\"\$PATH:$INSTALL_DIR\""
    echo ""
    ;;
esac
