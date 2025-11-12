# WireGuard On-Demand for Linux

**Automatic VPN activation when you need it, idle when you don't.**

## What is it?

Access your home network while on the go without draining your mobile data or battery.

This daemon automatically activates your WireGuard VPN only when you actually try to access your home network, then disconnects after 5 minutes of inactivity. No manual toggling, no wasted mobile data when you're just browsing.

**The problem it solves:** You're on your phone's hotspot and want to access your home server. Normally you'd need to manually connect to VPN, remember to disconnect when done, or waste data keeping it connected all the time.

**What this does:** Automatically activates VPN the moment you try to access your home network (e.g., `ssh homeserver`), then disconnects automatically when you stop using it.

**Requirements:**
- Linux kernel 5.8+ (check: `uname -r`)
- NetworkManager
- WireGuard configured

Tested on Fedora 43, Lenovo ThinkPad T14s Gen 6 AMD.

## Install

### Fedora/RHEL (Recommended)

```bash
# Enable repository
sudo dnf copr enable vly/wg-ondemand

# Install
sudo dnf install wg-ondemand
```

### Other Distributions

Download the latest release from [GitHub Releases](https://github.com/vly/wg-ondemand/releases):

```bash
# Download and extract
wget https://github.com/vly/wg-ondemand/releases/latest/download/wg-ondemand-v0.1.0.tar.gz
tar xzf wg-ondemand-v0.1.0.tar.gz
cd wg-ondemand-v0.1.0/

# Install
sudo ./install.sh
```

### Build from Source

```bash
git clone https://github.com/vly/wg-ondemand
cd wg-ondemand
sudo ./install.sh
```

## Getting Started

1. **Configure** `/etc/wg-ondemand/config.toml`:

```toml
[general]
# WiFi networks to monitor
# Option 1: Single WiFi network
target_ssid = "MyPhoneHotspot"

# Option 2: Multiple networks
# target_ssids = ["MyPhoneHotspot", "CoffeeShopWiFi"]

# Option 3: All networks except home/office
# exclude_ssids = ["HomeWiFi", "OfficeWiFi"]

# Your WireGuard interface
wg_interface = "wg0"

# NetworkManager connection name (comment out if using wg-quick)
nm_connection = "HomeVPN"

# Idle timeout in seconds
idle_timeout = 300

[subnets]
# Home networks that trigger VPN activation
ranges = [
    "192.168.1.0/24",
    "10.0.0.0/24",
]
```

2. **Start the service:**

```bash
sudo systemctl enable --now wg-ondemand
```

3. **Test it:**

```bash
# Connect to your target WiFi
# Try to access your home network
ping 192.168.1.1

# Watch it activate automatically
sudo journalctl -u wg-ondemand -f
```

That's it. The daemon runs in the background and handles everything automatically.

**View logs:**
```bash
sudo journalctl -u wg-ondemand -f
```

**Restart after config changes:**
```bash
sudo systemctl restart wg-ondemand
```

**Uninstall:**
```bash
sudo systemctl stop wg-ondemand
sudo systemctl disable wg-ondemand
sudo rm /usr/local/bin/wg-ondemand
sudo rm /etc/systemd/system/wg-ondemand.service
sudo rm -rf /etc/wg-ondemand
sudo systemctl daemon-reload
```

## Bugs and Contributing

**Found a bug?** [Open an issue](https://github.com/vly/wg-ondemand/issues)

**Want to contribute?** Pull requests welcome! Please test thoroughly and include documentation.

**Technology:** eBPF for efficient packet filtering, Rust for safety, D-Bus for NetworkManager integration.

**License:** MIT - See LICENSE file
