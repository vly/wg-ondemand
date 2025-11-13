#!/usr/bin/env bash
# Install waybar widget for wg-ondemand

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WAYBAR_CONFIG_DIR="$HOME/.config/waybar"
WAYBAR_STYLE="$WAYBAR_CONFIG_DIR/style.css"

# Check if running as root
if [[ $EUID -eq 0 ]]; then
    echo "Error: Do not run this script as root. It installs to your user's waybar config."
    exit 1
fi

# Create waybar config directory if it doesn't exist
mkdir -p "$WAYBAR_CONFIG_DIR"

# Copy widget script to system bin
echo "Installing waybar widget script..."
sudo cp "$SCRIPT_DIR/waybar-wg-ondemand.sh" /usr/local/bin/
sudo chmod +x /usr/local/bin/waybar-wg-ondemand.sh

echo "✓ Widget script installed to /usr/local/bin/"

# Copy menu XML to waybar config directory
echo "Installing menu XML..."
cp "$SCRIPT_DIR/wg-ondemand-menu.xml" "$WAYBAR_CONFIG_DIR/"

echo "✓ Menu XML installed to $WAYBAR_CONFIG_DIR/"

# Check and add CSS if not present
if [[ -f "$WAYBAR_STYLE" ]]; then
    if grep -q "#custom-wg-ondemand" "$WAYBAR_STYLE"; then
        echo "✓ CSS already present in $WAYBAR_STYLE"
    else
        echo "Adding CSS to $WAYBAR_STYLE..."
        cat >> "$WAYBAR_STYLE" << 'EOF'

/* wg-ondemand widget styles */
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
EOF
        echo "✓ CSS added to $WAYBAR_STYLE"
    fi
else
    echo "Warning: Waybar style.css not found at $WAYBAR_STYLE"
    echo "Creating basic style.css with wg-ondemand styles..."
    cat > "$WAYBAR_STYLE" << 'EOF'
/* wg-ondemand widget styles */
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
EOF
    echo "✓ CSS created at $WAYBAR_STYLE"
fi

cat << 'EOF'

Installation complete!

Next steps:
1. Add the widget to your waybar config (~/.config/waybar/config.jsonc):

   In "modules-right", add "custom/wg-ondemand" to your desired position, then add:

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

   Note: Replace 'alacritty' with your preferred terminal emulator (kitty, etc.)

2. Restart waybar:
   killall waybar && waybar &

EOF
