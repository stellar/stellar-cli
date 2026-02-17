#!/bin/sh
set -e

# Stellar CLI installation script
# Usage: curl -fsSL https://raw.githubusercontent.com/stellar/stellar-cli/main/install.sh | sh
# Or with options: curl -fsSL https://raw.githubusercontent.com/stellar/stellar-cli/main/install.sh | sh -s -- --user

REPO="stellar/stellar-cli"
BINARY_NAME="stellar"

command_exists() {
  command -v "$1" >/dev/null 2>&1
}

detect_linux_distro() {
  if [ -r /etc/os-release ]; then
    # shellcheck disable=SC1091
    . /etc/os-release
    if [ -n "$ID" ]; then
      echo "$ID"
      return
    fi
  fi
  echo "unknown"
}

detect_linux_libc() {
  if command_exists getconf; then
    if getconf GNU_LIBC_VERSION >/dev/null 2>&1; then
      echo "glibc"
      return
    fi
  fi

  if command_exists ldd; then
    if ldd --version 2>&1 | grep -qi musl; then
      echo "musl"
      return
    fi
    if ldd --version 2>&1 | grep -qi "gnu libc"; then
      echo "glibc"
      return
    fi
  fi

  if [ -e /lib/ld-musl-x86_64.so.1 ] || [ -e /lib/ld-musl-aarch64.so.1 ]; then
    echo "musl"
    return
  fi

  echo "unknown"
}

suggest_dependency_install() {
  missing="$1"
  if [ "$OS" = "macos" ]; then
    cat <<EOF
Install missing dependencies using Homebrew:
  brew install $missing
EOF
    return
  fi

  DISTRO="$(detect_linux_distro)"
  if command_exists apt-get; then
    cat <<EOF
Install missing dependencies on Debian/Ubuntu:
  sudo apt-get update
  sudo apt-get install -y $missing
EOF
  elif command_exists dnf; then
    cat <<EOF
Install missing dependencies on Fedora/RHEL:
  sudo dnf install -y $missing
EOF
  elif command_exists yum; then
    cat <<EOF
Install missing dependencies on CentOS/RHEL:
  sudo yum install -y $missing
EOF
  elif command_exists apk; then
    cat <<EOF
Install missing dependencies on Alpine:
  sudo apk add $missing
EOF
  elif command_exists pacman; then
    cat <<EOF
Install missing dependencies on Arch:
  sudo pacman -S --needed $missing
EOF
  elif command_exists zypper; then
    cat <<EOF
Install missing dependencies on openSUSE:
  sudo zypper install -y $missing
EOF
  else
    cat <<EOF
Detected Linux distro: $DISTRO
Install these packages with your distro package manager: $missing
EOF
  fi
}

check_dependencies() {
  missing=""
  for cmd in curl grep sed tar mktemp; do
    if ! command_exists "$cmd"; then
      missing="$missing $cmd"
    fi
  done

  if [ -n "$missing" ]; then
    missing="${missing# }"
    echo "Error: Missing required command(s): $missing"
    echo ""
    suggest_dependency_install "$missing"
    echo ""
    echo "After installing dependencies, re-run this installer."
    exit 1
  fi
}

suggest_runtime_library_install() {
  missing_lib="$1"

  if [ "$OS" != "linux" ]; then
    echo "Unable to auto-suggest runtime library installation on this OS."
    return
  fi

  package=""
  if command_exists apt-get; then
    case "$missing_lib" in
      libdbus-1.so.3) package="libdbus-1-3" ;;
      libudev.so.1) package="libudev1" ;;
      *) package="" ;;
    esac
    if [ -n "$package" ]; then
      cat <<EOF
Install the missing runtime library on Debian/Ubuntu:
  sudo apt-get update
  sudo apt-get install -y $package
EOF
    else
      cat <<EOF
Install the package that provides '$missing_lib' on Debian/Ubuntu.
You can search with:
  apt-file search $missing_lib
EOF
    fi
  elif command_exists dnf; then
    case "$missing_lib" in
      libdbus-1.so.3) package="dbus-libs" ;;
      libudev.so.1) package="systemd-libs" ;;
      *) package="" ;;
    esac
    if [ -n "$package" ]; then
      cat <<EOF
Install the missing runtime library on Fedora/RHEL:
  sudo dnf install -y $package
EOF
    else
      cat <<EOF
Install the package that provides '$missing_lib' on Fedora/RHEL.
You can search with:
  dnf provides \"*/$missing_lib\"
EOF
    fi
  elif command_exists yum; then
    case "$missing_lib" in
      libdbus-1.so.3) package="dbus-libs" ;;
      libudev.so.1) package="systemd-libs" ;;
      *) package="" ;;
    esac
    if [ -n "$package" ]; then
      cat <<EOF
Install the missing runtime library on CentOS/RHEL:
  sudo yum install -y $package
EOF
    else
      cat <<EOF
Install the package that provides '$missing_lib' on CentOS/RHEL.
You can search with:
  yum provides \"*/$missing_lib\"
EOF
    fi
  elif command_exists apk; then
    case "$missing_lib" in
      libdbus-1.so.3) package="dbus-libs" ;;
      libudev.so.1) package="eudev-libs" ;;
      *) package="" ;;
    esac
    if [ -n "$package" ]; then
      cat <<EOF
Install the missing runtime library on Alpine:
  sudo apk add $package
EOF
    else
      cat <<EOF
Install the package that provides '$missing_lib' on Alpine.
EOF
    fi
  elif command_exists pacman; then
    case "$missing_lib" in
      libdbus-1.so.3) package="dbus" ;;
      libudev.so.1) package="systemd-libs" ;;
      *) package="" ;;
    esac
    if [ -n "$package" ]; then
      cat <<EOF
Install the missing runtime library on Arch:
  sudo pacman -S --needed $package
EOF
    else
      cat <<EOF
Install the package that provides '$missing_lib' on Arch.
EOF
    fi
  elif command_exists zypper; then
    case "$missing_lib" in
      libdbus-1.so.3) package="libdbus-1-3" ;;
      libudev.so.1) package="libudev1" ;;
      *) package="" ;;
    esac
    if [ -n "$package" ]; then
      cat <<EOF
Install the missing runtime library on openSUSE:
  sudo zypper install -y $package
EOF
    else
      cat <<EOF
Install the package that provides '$missing_lib' on openSUSE.
EOF
    fi
  else
    cat <<EOF
Install the package that provides '$missing_lib' using your distro package manager.
EOF
  fi
}

post_install_check() {
  installed_binary="$1"
  version_output=""
  if version_output="$("$installed_binary" --version 2>&1)"; then
    return 0
  fi

  missing_lib="$(printf '%s\n' "$version_output" | sed -n 's/.*error while loading shared libraries: \([^:]*\):.*/\1/p')"

  if [ -n "$missing_lib" ]; then
    echo ""
    echo "Warning: $BINARY_NAME was installed, but a runtime shared library is missing:"
    echo "  $missing_lib"
    echo ""
    suggest_runtime_library_install "$missing_lib"
    echo ""
    echo "After installing the runtime dependency, run:"
    echo "  $installed_binary --version"
    return 0
  fi

  echo ""
  echo "Warning: post-install check failed:"
  echo "$version_output"

  if [ "$OS" = "linux" ] && [ "${LIBC:-unknown}" = "musl" ]; then
    if [ -f "$installed_binary" ] && printf '%s\n' "$version_output" | grep -qi "not found"; then
      cat <<EOF

This usually means the downloaded binary targets glibc but this system uses musl (e.g. Alpine).
Next steps:
  1) Use a glibc-based image (Debian/Ubuntu/Fedora) for the prebuilt binary.
  2) If you must stay on Alpine, build stellar-cli from source on Alpine.
EOF
    fi
  fi

  return 0
}

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
  LIBC="$(detect_linux_libc)"
  if [ "$LIBC" = "musl" ]; then
    DISTRO="$(detect_linux_distro)"
    echo "Error: Detected Linux distro '$DISTRO' using musl libc."
    echo "The prebuilt Stellar CLI release in this script targets glibc (GNU/Linux) and is not supported on musl systems (e.g. Alpine)."
    if [ -f "${INSTALL_DIR}/${BINARY_NAME}" ]; then
      echo ""
      echo "Note: Found an existing install at ${INSTALL_DIR}/${BINARY_NAME}."
      echo "It may be from a previous run and will continue to fail on musl."
    fi
    echo ""
    echo "Recommended next steps:"
    echo "  1) Use a glibc-based image (Debian/Ubuntu/Fedora)."
    echo "     Example: docker run --platform=linux/arm64 -it --rm debian:bookworm-slim"
    echo "  2) If you must stay on Alpine, build stellar-cli from source on Alpine."
    exit 1
  fi
  TARGET="${ARCH}-unknown-linux-gnu"
elif [ "$OS" = "macos" ]; then
  TARGET="${ARCH}-apple-darwin"
fi

echo "Detected platform: $OS ($TARGET)"
check_dependencies

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

post_install_check "$INSTALL_DIR/$BINARY_NAME"

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
