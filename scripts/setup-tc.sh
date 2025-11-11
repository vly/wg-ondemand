#!/bin/bash
# Setup Traffic Control (TC) qdisc for eBPF attachment
# This script reads the wg-ondemand config and ensures the monitored
# interface has a clsact qdisc for eBPF program attachment

set -e

CONFIG_FILE="${1:-/etc/wg-ondemand/config.toml}"

if [ ! -f "$CONFIG_FILE" ]; then
    echo "Config file not found: $CONFIG_FILE"
    exit 1
fi

# Extract monitor_interface from config (if specified)
INTERFACE=$(grep -E '^\s*monitor_interface\s*=' "$CONFIG_FILE" | sed -E 's/.*=\s*"([^"]+)".*/\1/' || true)

# If monitor_interface is not specified, we'll need to auto-detect it
# For now, if it's not in config, the daemon will auto-detect it
if [ -z "$INTERFACE" ]; then
    echo "monitor_interface not specified in config, daemon will auto-detect"
    exit 0
fi

echo "Setting up TC qdisc for interface: $INTERFACE"

# Check if interface exists
if ! ip link show "$INTERFACE" &>/dev/null; then
    echo "Warning: Interface $INTERFACE does not exist yet, skipping TC setup"
    exit 0
fi

# Check current qdisc
CURRENT_QDISC=$(tc qdisc show dev "$INTERFACE" | head -n1 | awk '{print $2}')

if [ "$CURRENT_QDISC" = "clsact" ]; then
    echo "Interface $INTERFACE already has clsact qdisc"
    exit 0
fi

# Add clsact qdisc (idempotent - will fail silently if already exists)
echo "Adding clsact qdisc to $INTERFACE..."
tc qdisc add dev "$INTERFACE" clsact 2>/dev/null || true

echo "TC setup complete for $INTERFACE"
