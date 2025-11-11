# WireGuard On-Demand for Linux

**Automatic VPN activation when you need it, idle when you don't.**

A lightweight daemon that automatically activates your WireGuard VPN tunnel only when accessing specific networks, saving mobile data and battery life.

## What is this?

This daemon monitors your network traffic and automatically:
- Activates your WireGuard VPN when you try to access your home/private network
- Only works when connected to a specific WiFi (e.g., your mobile hotspot)
- Automatically disconnects after 5 minutes of inactivity to save data
- Minimal footprint for laptop scenario

**Primary use case:** Accessing your home network while on the go (e.g. mobile hotspot), without keeping VPN active all the time.

Tested on x86 Fedora 43, running on Lenovo Thinkpad t14s gen 6 AMD.

## Requirements

- Linux with kernel 5.8 or newer
- NetworkManager (standard on most modern Linux distributions)
- WireGuard installed and configured
- Root/sudo access for installation

**Check if your system is compatible:**
```bash
uname -r  # Should show 5.8 or higher
```

## Quick Install

### Option 1: Fedora/RHEL via DNF (Easiest!)

**For Fedora 38+ users:**

```bash
# Enable COPR repository
sudo dnf copr enable vly/wg-ondemand

# Install
sudo dnf install wg-ondemand

# Configure
sudo nano /etc/wg-ondemand/config.toml

# Enable and start
sudo systemctl enable wg-ondemand
sudo systemctl start wg-ondemand
```

Updates are automatic via `dnf update`!

### Option 2: Download Pre-built Release

1. **Download the latest release:**
   ```bash
   # Download latest release
   wget https://github.com/vly/wg-ondemand/releases/latest/download/wg-ondemand-v0.1.0.tar.gz

   # Verify checksum (optional but recommended)
   wget https://github.com/vly/wg-ondemand/releases/latest/download/wg-ondemand-v0.1.0.tar.gz.sha256
   sha256sum -c wg-ondemand-v0.1.0.tar.gz.sha256

   # Extract
   tar xzf wg-ondemand-v0.1.0.tar.gz
   cd wg-ondemand-v0.1.0/
   ```

2. **Install (requires sudo):**
   ```bash
   sudo ./install.sh
   ```

### Option 3: Build from Source

1. **Clone and build:**
   ```bash
   git clone https://github.com/vly/wg-ondemand
   cd wg-ondemand
   ```

2. **Install (requires sudo):**
   ```bash
   sudo ./install.sh
   ```

3. **Edit configuration:**
   ```bash
   sudo nano /etc/wg-ondemand/config.toml
   ```

   Update these settings:
   - `target_ssid`: WiFi name that triggers monitoring (e.g., "MyHotspot")
   - `nm_connection`: Your WireGuard connection name (or comment out if using wg-quick)
   - `wg_interface`: WireGuard interface name (usually "wg0")
   - `ranges`: Networks that should trigger VPN activation (e.g., "192.168.1.0/24")

4. **Start the service:**
   ```bash
   sudo systemctl enable wg-ondemand
   sudo systemctl start wg-ondemand
   ```

Done! The daemon is now running.

## Configuration Example

```toml
[general]
# WiFi network where on-demand activation should work
target_ssid = "MyPhoneHotspot"

# Your WireGuard interface
wg_interface = "wg0"

# NetworkManager connection name (if using NetworkManager)
# Comment out this line if using wg-quick instead
nm_connection = "HomeVPN"

# How long to wait before disconnecting (seconds)
idle_timeout = 300

# Logging level: info, debug, warn, error
log_level = "info"

[subnets]
# Networks that trigger VPN activation
ranges = [
    "192.168.1.0/24",  # Home network
    "10.0.0.0/24",     # Home server subnet
]
```

## Usage

The daemon runs automatically in the background. You don't need to do anything!

**How it works:**
1. Connect to your target WiFi (e.g., "MyPhoneHotspot")
2. Try to access a device on your home network (e.g., `ping 192.168.1.1`)
3. VPN automatically activates within 1-2 seconds
4. Access your resources normally
5. Stop using home network resources
6. After 5 minutes of idle time, VPN automatically disconnects

**View status:**
```bash
# Check if daemon is running
sudo systemctl status wg-ondemand

# View live logs
sudo journalctl -u wg-ondemand -f

# Check recent activity
sudo journalctl -u wg-ondemand --since "1 hour ago"
```

## Managing the Service

```bash
# Start daemon
sudo systemctl start wg-ondemand

# Stop daemon
sudo systemctl stop wg-ondemand

# Restart (after changing config)
sudo systemctl restart wg-ondemand

# Enable automatic start on boot
sudo systemctl enable wg-ondemand

# Disable automatic start
sudo systemctl disable wg-ondemand
```

## Uninstall

```bash
# Stop and disable the service
sudo systemctl stop wg-ondemand
sudo systemctl disable wg-ondemand

# Remove installed files
sudo rm /usr/local/bin/wg-ondemand
sudo rm /etc/systemd/system/wg-ondemand.service
sudo rm -rf /etc/wg-ondemand

# Reload systemd
sudo systemctl daemon-reload
```

## Troubleshooting

### VPN isn't activating

1. **Check you're connected to the right WiFi:**
   ```bash
   nmcli -t -f active,ssid dev wifi | grep '^yes'
   ```
   Should show your `target_ssid`.

2. **Verify daemon is running:**
   ```bash
   sudo systemctl status wg-ondemand
   ```

3. **Check logs for errors:**
   ```bash
   sudo journalctl -u wg-ondemand -n 50
   ```

4. **Test WireGuard manually:**
   ```bash
   # If using NetworkManager:
   sudo nmcli connection up YourConnectionName

   # If using wg-quick:
   sudo wg-quick up wg0
   ```

### VPN won't disconnect

- Check idle timeout setting in `/etc/wg-ondemand/config.toml`
- Make sure you've actually stopped accessing home resources
- View logs: `sudo journalctl -u wg-ondemand -f`

### Daemon won't start after crash

If the daemon was killed unexpectedly (e.g., power loss, SIGKILL), stale eBPF programs may remain attached to the network interface. The daemon automatically cleans these up on startup, but if you need to clean up manually:

```bash
# Replace 'wlan0' with your network interface name
sudo tc filter del dev wlan0 egress

# Then restart the daemon
sudo systemctl restart wg-ondemand
```

To find your network interface name:
```bash
ip link show
# or
nmcli device status
```

### Need more help?

Enable debug logging:
```bash
sudo nano /etc/wg-ondemand/config.toml
# Change: log_level = "debug"

sudo systemctl restart wg-ondemand
sudo journalctl -u wg-ondemand -f
```

## NetworkManager vs wg-quick

This daemon supports both connection methods:

**NetworkManager** (recommended):
- Easier to set up
- Better desktop integration
- Set `nm_connection = "YourConnectionName"` in config

**wg-quick** (traditional):
- Requires `/etc/wireguard/wg0.conf` file
- Comment out `nm_connection` line in config

**Technology:** Uses eBPF for efficient packet filtering, Rust for safety, D-Bus for NetworkManager integration.

## License

MIT License - See LICENSE file

## Contributing

Contributions welcome! Please test thoroughly and include appropriate documentation.
