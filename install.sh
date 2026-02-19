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

run_as_root() {
  if [ "$(id -u)" = "0" ]; then
    "$@"
    return
  fi
  if command_exists sudo; then
    sudo "$@"
    return
  fi
  echo "Warning: '$*' requires root privileges, but neither root access nor sudo is available." >&2
  return 1
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
  missing_cmds="$1"
  # Map missing command names to their actual package names
  pkgs=""
  for cmd in $missing_cmds; do
    case "$cmd" in
      sed)    [ "$OS" = "macos" ] && pkgs="$pkgs gnu-sed" || pkgs="$pkgs sed" ;;
      tar)    [ "$OS" = "macos" ] && pkgs="$pkgs gnu-tar" || pkgs="$pkgs tar" ;;
      mktemp) pkgs="$pkgs coreutils" ;;
      *)      pkgs="$pkgs $cmd" ;;
    esac
  done
  pkgs="${pkgs# }"

  if [ "$OS" = "macos" ]; then
    cat <<EOF
Install missing dependencies using Homebrew:
  brew install $pkgs
EOF
    return
  fi

  if command_exists apt-get; then
    cat <<EOF
Install missing dependencies on Debian/Ubuntu:
  ${SUDO}apt-get update && ${SUDO}apt-get install -y $pkgs
EOF
  elif command_exists dnf; then
    cat <<EOF
Install missing dependencies on Fedora/RHEL:
  ${SUDO}dnf install -y $pkgs
EOF
  elif command_exists yum; then
    cat <<EOF
Install missing dependencies on CentOS/RHEL:
  ${SUDO}yum install -y $pkgs
EOF
  elif command_exists pacman; then
    cat <<EOF
Install missing dependencies on Arch:
  ${SUDO}pacman -S --needed $pkgs
EOF
  elif command_exists zypper; then
    cat <<EOF
Install missing dependencies on openSUSE:
  ${SUDO}zypper install -y $pkgs
EOF
  else
    cat <<EOF
Detected Linux distro: $(detect_linux_distro)
Install packages providing these commands with your distro package manager: $missing_cmds
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
  ${SUDO}apt-get update
  ${SUDO}apt-get install -y $package
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
  ${SUDO}dnf install -y $package
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
  ${SUDO}yum install -y $package
EOF
    else
      cat <<EOF
Install the package that provides '$missing_lib' on CentOS/RHEL.
You can search with:
  yum provides \"*/$missing_lib\"
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
  ${SUDO}pacman -S --needed $package
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
  ${SUDO}zypper install -y $package
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

has_dev_dependency() {
  dep="$1"
  case "$dep" in
    dbus)
      if command_exists pkg-config && pkg-config --exists dbus-1 2>/dev/null; then
        return 0
      fi
      if [ -f /usr/include/dbus-1.0/dbus/dbus.h ]; then
        return 0
      fi
      ;;
    udev)
      if command_exists pkg-config && pkg-config --exists libudev 2>/dev/null; then
        return 0
      fi
      if [ -f /usr/include/libudev.h ]; then
        return 0
      fi
      ;;
  esac
  return 1
}

attempt_runtime_deps_setup() {
  if [ "$OS" != "linux" ]; then
    return
  fi

  pkg_dbus_dev=""
  pkg_udev_dev=""
  manager=""
  missing_pkgs=""

  if command_exists apt-get; then
    manager="apt-get"
    pkg_dbus_dev="libdbus-1-dev"
    pkg_udev_dev="libudev-dev"
  elif command_exists dnf; then
    manager="dnf"
    pkg_dbus_dev="dbus-devel"
    pkg_udev_dev="systemd-devel"
  elif command_exists yum; then
    manager="yum"
    pkg_dbus_dev="dbus-devel"
    pkg_udev_dev="systemd-devel"
  elif command_exists pacman; then
    manager="pacman"
    pkg_dbus_dev="dbus"
    pkg_udev_dev="systemd"
  elif command_exists zypper; then
    manager="zypper"
    pkg_dbus_dev="dbus-1-devel"
    pkg_udev_dev="systemd-devel"
  else
    echo "Warning: Could not detect a supported package manager to install development dependencies."
    return
  fi

  if [ -n "$pkg_dbus_dev" ] && ! has_dev_dependency dbus; then
    missing_pkgs="$missing_pkgs $pkg_dbus_dev"
  fi
  if [ -n "$pkg_udev_dev" ] && ! has_dev_dependency udev; then
    missing_pkgs="$missing_pkgs $pkg_udev_dev"
  fi

  missing_pkgs="${missing_pkgs# }"
  if [ -z "$missing_pkgs" ]; then
    return
  fi

  echo ""
  echo "Attempting development dependency setup (--install-deps): $missing_pkgs"

  case "$manager" in
    apt-get)
      if ! run_as_root apt-get update; then
        echo "Warning: apt-get update failed."
      fi
      # shellcheck disable=SC2086
      if ! run_as_root apt-get install -y $missing_pkgs; then
        echo "Warning: Failed to install development dependencies via apt-get."
      fi
      ;;
    dnf)
      # shellcheck disable=SC2086
      if ! run_as_root dnf install -y $missing_pkgs; then
        echo "Warning: Failed to install development dependencies via dnf."
      fi
      ;;
    yum)
      # shellcheck disable=SC2086
      if ! run_as_root yum install -y $missing_pkgs; then
        echo "Warning: Failed to install development dependencies via yum."
      fi
      ;;
    pacman)
      # shellcheck disable=SC2086
      if ! run_as_root pacman -S --needed --noconfirm $missing_pkgs; then
        echo "Warning: Failed to install development dependencies via pacman."
      fi
      ;;
    zypper)
      # shellcheck disable=SC2086
      if ! run_as_root zypper install -y $missing_pkgs; then
        echo "Warning: Failed to install development dependencies via zypper."
      fi
      ;;
  esac
}

suggest_rust_install() {
  if [ "$OS" = "macos" ]; then
    cat <<EOF
Suggested Rust setup on macOS:
  brew install rustup-init
  rustup-init -y
  . "\$HOME/.cargo/env"
EOF
    return
  fi

  if command_exists apt-get; then
    cat <<EOF
Suggested Rust setup on Debian/Ubuntu:
  ${SUDO}apt-get update
  ${SUDO}apt-get install -y curl build-essential pkg-config libssl-dev
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "\$HOME/.cargo/env"
EOF
  elif command_exists dnf; then
    cat <<EOF
Suggested Rust setup on Fedora/RHEL:
  ${SUDO}dnf install -y curl gcc make pkgconfig openssl-devel
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "\$HOME/.cargo/env"
EOF
  elif command_exists yum; then
    cat <<EOF
Suggested Rust setup on CentOS/RHEL:
  ${SUDO}yum install -y curl gcc make pkgconfig openssl-devel
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "\$HOME/.cargo/env"
EOF
  elif command_exists pacman; then
    cat <<EOF
Suggested Rust setup on Arch:
  ${SUDO}pacman -S --needed rustup
  rustup default stable
EOF
  elif command_exists zypper; then
    cat <<EOF
Suggested Rust setup on openSUSE:
  ${SUDO}zypper install -y curl gcc make pkg-config libopenssl-devel
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  . "\$HOME/.cargo/env"
EOF
  else
    cat <<EOF
Rust is required for smart contract development.
Install rustup from https://rustup.rs, then run:
  rustup default stable
EOF
  fi
}

attempt_rust_setup() {
  echo ""
  echo "Attempting Rust setup (--install-deps)..."

  if [ "$OS" = "macos" ]; then
    if ! command_exists rustup && ! command_exists rustup-init; then
      if command_exists brew; then
        if ! brew install rustup-init; then
          echo "Warning: Failed to install rustup-init with Homebrew."
        fi
      else
        echo "Warning: Homebrew not found. Install Homebrew or rustup manually."
      fi
    fi
    if ! command_exists rustup && command_exists rustup-init; then
      if ! rustup-init -y; then
        echo "Warning: rustup-init failed."
      fi
    fi
  else
    if ! command_exists rustup; then
      if command_exists apt-get; then
        if ! run_as_root apt-get update; then
          echo "Warning: apt-get update failed."
        fi
        if ! run_as_root apt-get install -y curl build-essential pkg-config libssl-dev; then
          echo "Warning: Failed to install Rust prerequisites via apt-get."
        fi
      elif command_exists dnf; then
        if ! run_as_root dnf install -y curl gcc make pkgconfig openssl-devel; then
          echo "Warning: Failed to install Rust prerequisites via dnf."
        fi
      elif command_exists yum; then
        if ! run_as_root yum install -y curl gcc make pkgconfig openssl-devel; then
          echo "Warning: Failed to install Rust prerequisites via yum."
        fi
      elif command_exists pacman; then
        if ! run_as_root pacman -S --needed --noconfirm rustup; then
          echo "Warning: Failed to install rustup via pacman."
        fi
      elif command_exists zypper; then
        if ! run_as_root zypper install -y curl gcc make pkg-config libopenssl-devel; then
          echo "Warning: Failed to install Rust prerequisites via zypper."
        fi
      fi
    fi

    if ! command_exists rustup; then
      if command_exists curl; then
        if ! curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; then
          echo "Warning: rustup installer failed."
        fi
      else
        echo "Warning: curl is required to run rustup installer."
      fi
    fi
  fi

  if [ -f "$HOME/.cargo/env" ]; then
    # shellcheck disable=SC1090,SC1091
    . "$HOME/.cargo/env" || true
  fi

  if command_exists rustup; then
    if ! rustup default stable; then
      echo "Warning: Failed to set default Rust toolchain to stable."
    fi
    if ! rustup target add wasm32v1-none; then
      echo "Warning: Failed to install wasm32v1-none target."
    fi
  fi
}

print_contract_tooling_summary() {
  if [ "$INSTALL_DEPS" = true ]; then
    if ! command_exists rustup || ! command_exists cargo || ! command_exists rustc; then
      attempt_rust_setup
    elif command_exists rustup; then
      if ! rustup target list --installed 2>/dev/null | grep -q '^wasm32v1-none$'; then
        attempt_rust_setup
      fi
    fi
  fi

  echo ""
  echo "Smart contract Rust environment summary:"

  has_rustup="false"
  has_cargo="false"
  has_rustc="false"
  has_wasm_target="false"

  if command_exists rustup; then
    has_rustup="true"
    echo "  [OK] rustup"
  else
    echo "  [MISSING] rustup"
  fi

  if command_exists cargo; then
    has_cargo="true"
    echo "  [OK] cargo"
  else
    echo "  [MISSING] cargo"
  fi

  if command_exists rustc; then
    has_rustc="true"
    echo "  [OK] rustc"
  else
    echo "  [MISSING] rustc"
  fi

  if [ "$has_rustup" = "true" ]; then
    if rustup target list --installed 2>/dev/null | grep -q '^wasm32v1-none$'; then
      has_wasm_target="true"
      echo "  [OK] wasm32v1-none target"
    else
      echo "  [MISSING] wasm32v1-none target"
    fi
  else
    echo "  [MISSING] wasm32v1-none target (requires rustup)"
  fi

  if [ "$has_rustup" = "false" ] || [ "$has_cargo" = "false" ] || [ "$has_rustc" = "false" ]; then
    echo ""
    if [ "$INSTALL_DEPS" != true ]; then
      suggest_rust_install
      echo ""
    fi
    echo "Then run:"
    echo "  rustup default stable"
    echo "  rustup target add wasm32v1-none"
    return
  fi

  if [ "$has_wasm_target" = "false" ]; then
    echo ""
    echo "Install missing target with:"
    echo "  rustup target add wasm32v1-none"
    return
  fi

  echo "  Ready for smart contract builds."
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
    printf '  %s\n' "$missing_lib"
    echo ""
    suggest_runtime_library_install "$missing_lib"
    echo ""
    echo "After installing the runtime dependency, run:"
    printf '  %s --version\n' "$installed_binary"
    return 1
  fi

  echo ""
  echo "Warning: post-install check failed:"
  printf '%s\n' "$version_output"

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

  return 1
}

# Check if user has sudo privileges
has_sudo() {
  if command -v sudo >/dev/null 2>&1; then
    # Non-interactive check only; avoid prompting during auto-detection.
    if sudo -n true 2>/dev/null; then
      return 0
    fi
  fi
  return 1
}

# Parse arguments and set defaults
INSTALL_DIR=""
USER_INSTALL=false
INSTALL_DEPS=false

for arg in "$@"; do
  case $arg in
    --user)
      USER_INSTALL=true
      INSTALL_DIR="${HOME}/.local/bin"
      ;;
    --dir=*)
      INSTALL_DIR="${arg#*=}"
      ;;
    --install-deps)
      INSTALL_DEPS=true
      ;;
    --help)
      echo "Stellar CLI installer"
      echo ""
      echo "Usage: sh install.sh [options]"
      echo ""
      echo "Options:"
      echo "  --user        Install to ~/.local/bin (no sudo required)"
      echo "  --dir=PATH    Install to custom directory"
      echo "  --install-deps  Attempt to install missing libdbus/libudev dev dependencies and Rust toolchain"
      echo "  --help        Show this help message"
      exit 0
      ;;
    *)
      echo "Error: Unknown option: $arg"
      echo "Run with --help to see available options."
      exit 1
      ;;
  esac
done

# Determine sudo prefix for suggested commands (empty if already root or sudo unavailable)
if [ "$(id -u)" = "0" ] || ! command_exists sudo; then
  SUDO=""
else
  SUDO="sudo "
fi

# Set default install directory if not specified
if [ -z "$INSTALL_DIR" ]; then
  if [ "$(id -u)" = "0" ] || has_sudo; then
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

# Validate basic installer dependencies before other checks use them
check_dependencies

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
    echo "Error: Detected Linux distro '$(detect_linux_distro)' using musl libc."
    echo "This installer downloads a prebuilt Stellar CLI release that targets glibc (GNU/Linux) and is not supported on musl systems (e.g. Alpine)."
    if [ -f "${INSTALL_DIR}/${BINARY_NAME}" ]; then
      echo ""
      echo "Note: Found an existing '$BINARY_NAME' binary at ${INSTALL_DIR}/${BINARY_NAME}."
      echo "This binary is built for glibc and will not run correctly on musl-based systems."
    fi
    echo ""
    echo "Recommended next steps:"
    echo "  1) Use a glibc-based image (Debian/Ubuntu/Fedora)."
    echo "     Example: docker run -it --rm debian:bookworm-slim"
    echo "  2) If you must stay on Alpine, build stellar-cli from source on Alpine."
    exit 1
  fi
  TARGET="${ARCH}-unknown-linux-gnu"
elif [ "$OS" = "macos" ]; then
  TARGET="${ARCH}-apple-darwin"
fi

echo "Detected platform: $OS ($TARGET)"

# Create temporary directory (used for both release response and binary extraction)
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT INT TERM HUP

# Get latest release version
echo "Fetching latest release..."
RELEASE_RESPONSE_FILE="$TMP_DIR/release.json"
if ! RELEASE_HTTP_STATUS="$(curl -sSL -o "$RELEASE_RESPONSE_FILE" -w "%{http_code}" "https://api.github.com/repos/${REPO}/releases/latest")"; then
  echo "Error: Could not reach GitHub API to fetch latest release"
  exit 1
fi

if [ "$RELEASE_HTTP_STATUS" != "200" ]; then
  if [ "$RELEASE_HTTP_STATUS" = "403" ] && grep -qi "rate limit" "$RELEASE_RESPONSE_FILE"; then
    echo "Error: GitHub API rate limit exceeded while fetching latest release."
    echo "Try again later."
  else
    echo "Error: Could not fetch latest release (GitHub API HTTP $RELEASE_HTTP_STATUS)"
  fi
  exit 1
fi

LATEST_RELEASE="$(grep '"tag_name":' "$RELEASE_RESPONSE_FILE" | sed -E 's/.*"([^"]+)".*/\1/')"

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

if [ "$INSTALL_DEPS" = true ]; then
  attempt_runtime_deps_setup
fi

POST_INSTALL_OK=true
if ! post_install_check "$INSTALL_DIR/$BINARY_NAME"; then
  POST_INSTALL_OK=false
fi

echo ""
if [ "$POST_INSTALL_OK" = true ]; then
  echo "[OK] Stellar CLI installed successfully!"
else
  echo "Warning: Stellar CLI binary was installed, but failed post-install verification."
fi
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

print_contract_tooling_summary

if [ "$POST_INSTALL_OK" != true ]; then
  exit 1
fi
