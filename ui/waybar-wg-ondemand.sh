#!/usr/bin/env bash
# Waybar module for wg-ondemand
# Outputs JSON for waybar custom module

# Get status from wg-ondemand-ctl
status_json=$(wg-ondemand-ctl status --json 2>/dev/null || echo '{"service_status":"unknown","tunnel_state":"unknown","ssid":""}')

# Parse JSON (simple grep-based parsing for bash)
status=$(echo "$status_json" | grep -o '"service_status": "[^"]*"' | cut -d'"' -f4 || echo "unknown")
tunnel_state=$(echo "$status_json" | grep -o '"tunnel_state": "[^"]*"' | cut -d'"' -f4 || echo "unknown")
ssid=$(echo "$status_json" | grep -o '"ssid": "[^"]*"' | cut -d'"' -f4 || echo "")

# Initialize variables
text=""
tooltip=""
class=""

case "$status" in
    active)
        case "$tunnel_state" in
            connected)
                text="󰖂 VPN"
                tooltip="WireGuard: Connected"
                if [[ -n "$ssid" ]]; then
                    tooltip="$tooltip - SSID: $ssid"
                fi
                class="connected"
                ;;
            monitoring)
                text="󰀂 Mon"
                tooltip="WireGuard: Monitoring"
                if [[ -n "$ssid" ]]; then
                    tooltip="$tooltip - SSID: $ssid"
                fi
                class="monitoring"
                ;;
            idle)
                text="󰀃 Idle"
                tooltip="WireGuard: Idle"
                class="idle"
                ;;
            *)
                text="󰖂 On"
                tooltip="WireGuard: Running"
                class="active"
                ;;
        esac
        ;;
    inactive)
        text="󰅛 Off"
        tooltip="WireGuard: Stopped"
        class="inactive"
        ;;
    disabled)
        text="󰅙 Dis"
        tooltip="WireGuard: Disabled"
        class="disabled"
        ;;
    *)
        text="󰀄 ?"
        tooltip="WireGuard: Unknown"
        class="unknown"
        ;;
esac

# Output JSON for waybar (ensure variables are set)
text="${text:-?}"
tooltip="${tooltip:-Unknown}"
class="${class:-unknown}"
status="${status:-unknown}"

printf '{"text":"%s","tooltip":"%s","class":"%s","alt":"%s"}\n' "$text" "$tooltip" "$class" "$status"
