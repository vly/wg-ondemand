#!/bin/bash
# Uninstallation script for wg-ondemand daemon

set -e

# Check if running as root
if [ "$EUID" -ne 0 ]; then
    echo "Please run as root (use sudo)"
    exit 1
fi

echo "Uninstalling wg-ondemand..."

# Stop and disable service
if systemctl is-active --quiet wg-ondemand; then
    echo "Stopping service..."
    systemctl stop wg-ondemand
fi

if systemctl is-enabled --quiet wg-ondemand 2>/dev/null; then
    echo "Disabling service..."
    systemctl disable wg-ondemand
fi

# Remove systemd service
if [ -f /etc/systemd/system/wg-ondemand.service ]; then
    echo "Removing systemd service..."
    rm -f /etc/systemd/system/wg-ondemand.service
    systemctl daemon-reload
fi

# Remove binaries
if [ -f /usr/local/bin/wg-ondemand ]; then
    echo "Removing binaries..."
    rm -f /usr/local/bin/wg-ondemand
    rm -f /usr/local/bin/wg-ondemand-setup-tc
fi

# Ask about config files
read -p "Remove configuration directory /etc/wg-ondemand? (y/N) " -n 1 -r
echo
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "Removing config directory..."
    rm -rf /etc/wg-ondemand
else
    echo "Keeping config directory..."
fi

echo ""
echo "✅ Uninstallation complete!"
echo ""
