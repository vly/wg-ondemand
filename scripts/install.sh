#!/bin/bash
# Installation script for wg-ondemand daemon

set -e

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (use sudo)"
    exit 1
fi

# Build the project
echo "Building wg-ondemand..."
cargo xtask build-ebpf
cargo build --release

# Install binaries
echo "Installing binaries to /usr/local/bin..."
install -m 755 target/release/wg-ondemand /usr/local/bin/wg-ondemand
install -m 755 scripts/setup-tc.sh /usr/local/bin/wg-ondemand-setup-tc
install -m 755 scripts/wg-ondemand-ctl /usr/local/bin/wg-ondemand-ctl

# Create shared directory for scripts
echo "Installing helper scripts..."
mkdir -p /usr/local/share/wg-ondemand
install -m 755 scripts/install.sh /usr/local/share/wg-ondemand/install.sh
install -m 755 scripts/uninstall.sh /usr/local/share/wg-ondemand/uninstall.sh

# Create config directory
echo "Creating config directory..."
mkdir -p /etc/wg-ondemand

# Install example config if it doesn't exist
if [ ! -f /etc/wg-ondemand/config.toml ]; then
    echo "Installing example config..."
    install -m 644 config/wg-ondemand.toml /etc/wg-ondemand/config.toml
    echo "⚠️  Please edit /etc/wg-ondemand/config.toml with your settings"
else
    echo "Config file already exists, skipping..."
fi

# Install systemd service
echo "Installing systemd service..."
install -m 644 systemd/wg-ondemand.service /etc/systemd/system/wg-ondemand.service

# Reload systemd
echo "Reloading systemd daemon..."
systemctl daemon-reload

echo ""
echo "✅ Installation complete!"
echo ""
echo "Next steps:"
echo "  1. Edit configuration: sudo wg-ondemand-ctl config edit"
echo "  2. Ensure WireGuard config exists: /etc/wireguard/wg0.conf"
echo "  3. Enable and start: sudo wg-ondemand-ctl enable && sudo wg-ondemand-ctl start"
echo "  4. Check status: wg-ondemand-ctl status"
echo ""
echo "Quick commands:"
echo "  wg-ondemand-ctl status       - Show service status"
echo "  wg-ondemand-ctl logs -f      - Follow logs"
echo "  sudo wg-ondemand-ctl restart - Restart after config changes"
echo ""
