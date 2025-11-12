#!/bin/bash
# Quick install script for wg-ondemand
# Usage: curl -fsSL https://raw.githubusercontent.com/vly/wg-ondemand/main/scripts/quick-install.sh | sudo bash

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

error() {
    echo -e "${RED}Error:${NC} $1" >&2
    exit 1
}

info() {
    echo -e "${BLUE}→${NC} $1"
}

success() {
    echo -e "${GREEN}✓${NC} $1"
}

# Check if running as root
if [[ $EUID -ne 0 ]]; then
    error "This script must be run as root. Use: curl -fsSL ... | sudo bash"
fi

# Check for required commands
for cmd in curl tar systemctl; do
    if ! command -v $cmd >/dev/null 2>&1; then
        error "Required command not found: $cmd"
    fi
done

# Check kernel version
KERNEL_VERSION=$(uname -r | cut -d. -f1,2)
REQUIRED_VERSION=5.8
if awk "BEGIN {exit !($KERNEL_VERSION < $REQUIRED_VERSION)}"; then
    error "Linux kernel 5.8+ required. Current version: $(uname -r)"
fi

# Detect latest release version
info "Fetching latest release information..."
LATEST_RELEASE=$(curl -fsSL https://api.github.com/repos/vly/wg-ondemand/releases/latest | grep '"tag_name"' | sed -E 's/.*"([^"]+)".*/\1/')

if [[ -z "$LATEST_RELEASE" ]]; then
    error "Failed to fetch latest release information"
fi

info "Latest version: $LATEST_RELEASE"

# Create temp directory
TEMP_DIR=$(mktemp -d)
trap "rm -rf $TEMP_DIR" EXIT

cd "$TEMP_DIR"

# Download release tarball
TARBALL_URL="https://github.com/vly/wg-ondemand/releases/download/${LATEST_RELEASE}/wg-ondemand-${LATEST_RELEASE}.tar.gz"
info "Downloading $TARBALL_URL..."

if ! curl -fsSL "$TARBALL_URL" -o wg-ondemand.tar.gz; then
    error "Failed to download release tarball"
fi

# Extract tarball
info "Extracting..."
tar xzf wg-ondemand.tar.gz
cd wg-ondemand-${LATEST_RELEASE}

# Install binaries
info "Installing binaries..."
install -m 755 bin/wg-ondemand /usr/local/bin/wg-ondemand
install -m 755 bin/wg-ondemand-setup-tc /usr/local/bin/wg-ondemand-setup-tc
install -m 755 bin/wg-ondemand-ctl /usr/local/bin/wg-ondemand-ctl

# Create shared directory for scripts
mkdir -p /usr/local/share/wg-ondemand
install -m 755 install.sh /usr/local/share/wg-ondemand/install.sh
install -m 755 uninstall.sh /usr/local/share/wg-ondemand/uninstall.sh

# Create config directory
info "Setting up configuration..."
mkdir -p /etc/wg-ondemand

# Install example config if it doesn't exist
if [[ ! -f /etc/wg-ondemand/config.toml ]]; then
    install -m 644 config/wg-ondemand.toml.example /etc/wg-ondemand/config.toml
    EDIT_CONFIG=true
else
    echo -e "${YELLOW}⚠${NC}  Config file already exists, skipping..."
    EDIT_CONFIG=false
fi

# Install systemd service
info "Installing systemd service..."
install -m 644 systemd/wg-ondemand.service /etc/systemd/system/wg-ondemand.service
systemctl daemon-reload

success "Installation complete!"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo -e "${GREEN}wg-ondemand ${LATEST_RELEASE}${NC} installed successfully!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""

if [[ "$EDIT_CONFIG" == "true" ]]; then
    echo -e "${YELLOW}⚠ Configuration required!${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Edit configuration:"
    echo "     ${BLUE}wg-ondemand-ctl config edit${NC}"
    echo ""
    echo "  2. Configure these settings:"
    echo "     • target_ssid or target_ssids - WiFi networks to monitor"
    echo "     • wg_interface - Your WireGuard interface name"
    echo "     • nm_connection - NetworkManager connection (if using NM)"
    echo "     • ranges - IP subnets that trigger VPN activation"
    echo ""
    echo "  3. Enable and start the service:"
    echo "     ${BLUE}wg-ondemand-ctl enable${NC}"
    echo "     ${BLUE}wg-ondemand-ctl start${NC}"
    echo ""
else
    echo "Service has been updated. To restart with new version:"
    echo "  ${BLUE}wg-ondemand-ctl restart${NC}"
    echo ""
fi

echo "Useful commands:"
echo "  ${BLUE}wg-ondemand-ctl status${NC}     - Check service status"
echo "  ${BLUE}wg-ondemand-ctl logs -f${NC}    - Follow logs"
echo "  ${BLUE}wg-ondemand-ctl help${NC}       - Show all commands"
echo ""
