#!/bin/bash
set -euo pipefail

REPO="OWNER/nissia-browser"
INSTALL_DIR="/usr/local/bin"
BINARY="nissia"
TMPDIR_INSTALL=""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}==>${NC} $1"; }
warn() { echo -e "${YELLOW}==>${NC} $1"; }
error() { echo -e "${RED}error:${NC} $1" >&2; exit 1; }

# Detect platform
detect_platform() {
    local os arch

    case "$(uname -s)" in
        Darwin) os="darwin" ;;
        Linux)  os="linux" ;;
        *)      error "Unsupported OS: $(uname -s). nissia supports macOS and Linux." ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64)  arch="x86_64" ;;
        arm64|aarch64) arch="aarch64" ;;
        *)             error "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "nissia-${os}-${arch}"
}

# Get latest release tag
get_latest_version() {
    local version

    # Method 1: GitHub redirect (no API rate limit)
    if command -v curl &>/dev/null; then
        version=$(curl -fsSLI -o /dev/null -w '%{url_effective}' "https://github.com/${REPO}/releases/latest" | sed 's|.*/||')
    fi

    # Method 2: GitHub API (fallback, subject to rate limits)
    if [ -z "$version" ] || [ "$version" = "latest" ]; then
        local api_url="https://api.github.com/repos/${REPO}/releases/latest"
        if command -v curl &>/dev/null; then
            version=$(curl -fsSL "$api_url" 2>/dev/null | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')
        elif command -v wget &>/dev/null; then
            version=$(wget -qO- "$api_url" 2>/dev/null | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')
        else
            error "Neither curl nor wget found. Please install one of them."
        fi
    fi

    if [ -z "$version" ]; then
        error "Could not determine latest version. Check https://github.com/${REPO}/releases"
    fi

    echo "$version"
}

# Download and install
install() {
    local platform version url

    platform=$(detect_platform)
    info "Detected platform: ${platform}"

    # Use provided version or fetch latest
    version="${SNACT_VERSION:-$(get_latest_version)}"
    info "Installing nissia ${version}"

    url="https://github.com/${REPO}/releases/download/${version}/${platform}.tar.gz"

    TMPDIR_INSTALL=$(mktemp -d)
    trap 'rm -rf "$TMPDIR_INSTALL"' EXIT

    info "Downloading ${url}"
    if command -v curl &>/dev/null; then
        curl -fsSL "$url" -o "${TMPDIR_INSTALL}/nissia.tar.gz"
    else
        wget -qO "${TMPDIR_INSTALL}/nissia.tar.gz" "$url"
    fi

    info "Extracting"
    tar xzf "${TMPDIR_INSTALL}/nissia.tar.gz" -C "$TMPDIR_INSTALL"

    # Install binary
    if [ -w "$INSTALL_DIR" ]; then
        mv "${TMPDIR_INSTALL}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    else
        info "Installing to ${INSTALL_DIR} (requires sudo)"
        sudo mv "${TMPDIR_INSTALL}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
    fi

    chmod +x "${INSTALL_DIR}/${BINARY}"

    info "nissia ${version} installed to ${INSTALL_DIR}/${BINARY}"
    echo ""

    # Verify and show onboarding
    if command -v nissia &>/dev/null; then
        echo "  Quick start:"
        echo ""
        echo "    nissia browser launch --background   # start Chrome"
        echo "    nissia snap https://example.com      # extract page elements"
        echo "    nissia click @e1                     # interact (auto re-snap)"
        echo "    nissia browser stop                  # stop Chrome"
        echo ""
        echo "  Project setup (optional):"
        echo ""
        echo "    cd your-project && nissia init       # create .nissia/ + AGENTS.md"
        echo ""
        echo "  Full docs: nissia --help"
    else
        warn "${INSTALL_DIR} is not in your PATH."
        echo "  Add it with: export PATH=\"${INSTALL_DIR}:\$PATH\""
    fi
}

install
