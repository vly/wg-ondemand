# Waybar Widget for wg-ondemand

A waybar custom module for monitoring and controlling wg-ondemand.

## Features

- Shows current VPN state with icons
- Color-coded status (connected, monitoring, idle, stopped)
- Shows connected SSID when monitoring
- Native waybar GTK menu for quick actions (start/stop/restart/logs/config)

## Installation

### Quick Install

```bash
cd ui/
./install-waybar-widget.sh
```

The install script will:
- Copy the widget script to `/usr/local/bin/`
- Copy the menu XML to `~/.config/waybar/`
- Add CSS styles to your `~/.config/waybar/style.css`

### Manual Installation

1. **Copy scripts and menu:**
   ```bash
   sudo cp ui/waybar-wg-ondemand.sh /usr/local/bin/
   sudo chmod +x /usr/local/bin/waybar-wg-ondemand.sh
   cp ui/wg-ondemand-menu.xml ~/.config/waybar/
   ```

2. **Add to waybar config** (`~/.config/waybar/config.jsonc`):

   In "modules-right", add "custom/wg-ondemand" to your desired position:
   ```jsonc
   "modules-right": [
     "custom/wg-ondemand",
     "network",
     "battery",
     "clock",
     "tray"
   ]
   ```

   Then add the widget configuration:
   ```jsonc
   "custom/wg-ondemand": {
     "exec": "/usr/local/bin/waybar-wg-ondemand.sh",
     "return-type": "json",
     "interval": 5,
     "tooltip": true,
     "menu": "on-click",
     "menu-file": "$HOME/.config/waybar/wg-ondemand-menu.xml",
     "menu-actions": {
       "start": "pkexec wg-ondemand-ctl start",
       "stop": "pkexec wg-ondemand-ctl stop",
       "restart": "pkexec wg-ondemand-ctl restart",
       "status": "alacritty -e bash -c 'wg-ondemand-ctl status; echo; echo Press Enter to close...; read'",
       "logs": "alacritty -e wg-ondemand-ctl logs -f",
       "config": "pkexec wg-ondemand-ctl config edit"
     }
   }
   ```

   Note: Replace 'alacritty' with your preferred terminal emulator (kitty, etc.)

3. **Add styles** (`~/.config/waybar/style.css`):
   ```css
   #custom-wg-ondemand {
     padding: 0 10px;
     margin: 0 5px;
     border-radius: 5px;
   }

   #custom-wg-ondemand.connected {
     color: #50fa7b;
     background: rgba(80, 250, 123, 0.1);
   }

   #custom-wg-ondemand.active {
     color: #50fa7b;
     background: rgba(80, 250, 123, 0.1);
   }

   #custom-wg-ondemand.monitoring {
     color: #8be9fd;
     background: rgba(139, 233, 253, 0.1);
   }

   #custom-wg-ondemand.idle {
     color: #f1fa8c;
     background: rgba(241, 250, 140, 0.1);
   }

   #custom-wg-ondemand.inactive {
     color: #f1fa8c;
     background: rgba(241, 250, 140, 0.1);
   }

   #custom-wg-ondemand.disabled {
     color: #6c7086;
     background: rgba(108, 112, 134, 0.1);
   }
   ```

4. **Restart waybar:**
   ```bash
   killall waybar && waybar &
   ```

## Requirements

- waybar (with GTK menu support)
- wg-ondemand-ctl (installed with wg-ondemand)
- pkexec (polkit - for root actions)
- A terminal emulator (alacritty or kitty recommended)
- Nerd Font (for icons)

## Usage

- Widget shows current state with icon and color
- Click widget to open native GTK menu with actions:
  - Start Service
  - Stop Service
  - Restart Service
  - View Status (opens in terminal)
  - View Logs (opens in terminal, follow mode)
  - Edit Config

## States

| Icon | State | Description |
|------|-------|-------------|
| 󰖂 VPN | Connected | Tunnel is active |
| 󰀂 Mon | Monitoring | Watching for traffic |
| 󰀃 Idle | Idle | Service running but idle |
| 󰅛 Off | Stopped | Service not running |
| 󰅙 Dis | Disabled | Service disabled |

## Customization

The widget uses `wg-ondemand-ctl status --json` for status data.

**Icons and colors** - Edit `waybar-wg-ondemand.sh`:
- Change icons (text variable in each case statement)

**Polling interval** - Edit waybar config:
- Change `interval` value (default: 5 seconds)

**Menu actions** - Edit waybar config `menu-actions`:
- Change terminal emulator (replace `alacritty`)
- Modify commands for each action
- Add custom actions by editing both `wg-ondemand-menu.xml` and `menu-actions`

**CSS styling** - Edit `~/.config/waybar/style.css`:
- Change colors for each state
- Modify padding, margins, border-radius

**Status detection** - Edit `wg-ondemand-ctl`:
- Change log lookback depth (default 50 lines in cmd_status function)
