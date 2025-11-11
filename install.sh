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
install -m 644 wg-ondemand.service /etc/systemd/system/wg-ondemand.service

# Reload systemd
echo "Reloading systemd daemon..."
systemctl daemon-reload

echo ""
echo "✅ Installation complete!"
echo ""
echo "Next steps:"
echo "  1. Edit configuration: sudo nano /etc/wg-ondemand/config.toml"
echo "  2. Ensure WireGuard config exists: /etc/wireguard/wg0.conf"
echo "  3. Enable service: sudo systemctl enable wg-ondemand"
echo "  4. Start service: sudo systemctl start wg-ondemand"
echo "  5. Check status: sudo systemctl status wg-ondemand"
echo "  6. View logs: sudo journalctl -u wg-ondemand -f"
echo ""
