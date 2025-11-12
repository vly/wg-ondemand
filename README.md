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

### Quick Install (Recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/vly/wg-ondemand/main/scripts/quick-install.sh | sudo bash
```

### Fedora/RHEL

```bash
sudo dnf copr enable vly/wg-ondemand
sudo dnf install wg-ondemand
```

### Manual Install

Download from [GitHub Releases](https://github.com/vly/wg-ondemand/releases):

```bash
wget https://github.com/vly/wg-ondemand/releases/latest/download/wg-ondemand-v0.1.0.tar.gz
tar xzf wg-ondemand-v0.1.0.tar.gz
cd wg-ondemand-v0.1.0/
sudo ./install.sh
```

### Build from Source

```bash
git clone https://github.com/vly/wg-ondemand
cd wg-ondemand
sudo ./scripts/install.sh
```

## Getting Started

After installation, configure and start the service:

```bash
# 1. Edit configuration
sudo wg-ondemand-ctl config edit

# 2. Enable and start
sudo wg-ondemand-ctl enable
sudo wg-ondemand-ctl start

# 3. Check status
wg-ondemand-ctl status
```

**Configuration example** (`/etc/wg-ondemand/config.toml`):

```toml
[general]
# Single WiFi network
target_ssid = "MyPhoneHotspot"

# Or multiple networks:
# target_ssids = ["MyPhoneHotspot", "CoffeeShopWiFi"]

# Or exclude specific networks:
# exclude_ssids = ["HomeWiFi", "OfficeWiFi"]

wg_interface = "wg0"
nm_connection = "HomeVPN"  # Or comment out if using wg-quick
idle_timeout = 300

[subnets]
ranges = ["192.168.1.0/24", "10.0.0.0/24"]
```

**Common commands:**

```bash
wg-ondemand-ctl status          # Show status
wg-ondemand-ctl logs -f         # Follow logs
sudo wg-ondemand-ctl restart    # Restart after config changes
sudo wg-ondemand-ctl uninstall  # Remove wg-ondemand
```

## Bugs and Contributing

**Found a bug?** [Open an issue](https://github.com/vly/wg-ondemand/issues)

**Want to contribute?** Pull requests welcome! Please test thoroughly and include documentation.

**Technology:** eBPF for efficient packet filtering, Rust for safety, D-Bus for NetworkManager integration.

**License:** MIT - See LICENSE file
