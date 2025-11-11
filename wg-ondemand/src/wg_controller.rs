// WireGuard tunnel lifecycle controller

//! WireGuard tunnel control and statistics
//!
//! This module provides an interface for managing WireGuard tunnel lifecycle
//! (bringing up/down), querying tunnel statistics, and tracking activity
//! for idle timeout detection.

use anyhow::{Context, Result};
use std::time::Instant;
use tokio::process::Command;
use wireguard_control::{Backend, Device, InterfaceName};

/// Validates that a name (interface or connection) is safe to use in shell commands.
/// Only allows alphanumeric characters, hyphens, and underscores to prevent command injection.
fn validate_name(name: &str, field_name: &str) -> Result<()> {
    if name.is_empty() {
        anyhow::bail!("{} cannot be empty", field_name);
    }

    // Check for valid characters: alphanumeric, hyphen, underscore
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!(
            "{} contains invalid characters: '{}'. Only alphanumeric, hyphens, and underscores are allowed",
            field_name,
            name
        );
    }

    Ok(())
}

/// Public wrapper for validating interface names.
/// Validates that the interface name is safe for use in shell commands and system calls.
///
/// # Errors
///
/// Returns an error if the interface name is empty or contains invalid characters.
/// Only alphanumeric characters, hyphens, and underscores are allowed.
pub fn validate_interface_name(name: &str) -> Result<()> {
    validate_name(name, "Interface name")
}

/// Controller for managing WireGuard tunnel state
pub struct WgController {
    interface: String,
    nm_connection: Option<String>,
    last_rx_bytes: u64,
    last_tx_bytes: u64,
    last_activity: Option<Instant>,
}

impl WgController {
    /// Create a new WireGuard controller for the specified interface
    ///
    /// # Errors
    ///
    /// Returns an error if the interface name or NetworkManager connection name
    /// contains invalid characters. Only alphanumeric characters, hyphens, and
    /// underscores are allowed to prevent command injection.
    pub fn new(interface: String, nm_connection: Option<String>) -> Result<Self> {
        // Validate interface name
        validate_name(&interface, "Interface name")?;

        // Validate NetworkManager connection name if provided
        if let Some(ref nm_conn) = nm_connection {
            validate_name(nm_conn, "NetworkManager connection name")?;
        }

        Ok(Self {
            interface,
            nm_connection,
            last_rx_bytes: 0,
            last_tx_bytes: 0,
            last_activity: None,
        })
    }

    /// Check if the WireGuard interface is currently up
    pub async fn is_up(&self) -> bool {
        // Check if interface exists using `ip link show`
        let output = Command::new("ip")
            .args(["link", "show", &self.interface])
            .output()
            .await;

        match output {
            Ok(output) => output.status.success(),
            Err(_) => false,
        }
    }

    /// Bring up the WireGuard interface using NetworkManager or wg-quick
    pub async fn bring_up(&self) -> Result<()> {
        if let Some(nm_conn) = &self.nm_connection {
            log::info!("Bringing up NetworkManager connection: {}", nm_conn);

            let output = Command::new("nmcli")
                .args(["connection", "up", nm_conn])
                .output()
                .await
                .context("Failed to execute nmcli connection up")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("nmcli connection up failed: {}", stderr);
            }

            log::info!("NetworkManager connection {} is up", nm_conn);
        } else {
            log::info!("Bringing up WireGuard interface: {}", self.interface);

            let output = Command::new("wg-quick")
                .args(["up", &self.interface])
                .output()
                .await
                .context("Failed to execute wg-quick up")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("wg-quick up failed: {}", stderr);
            }

            log::info!("WireGuard interface {} is up", self.interface);
        }
        Ok(())
    }

    /// Bring down the WireGuard interface using NetworkManager or wg-quick
    pub async fn bring_down(&self) -> Result<()> {
        if let Some(nm_conn) = &self.nm_connection {
            log::info!("Bringing down NetworkManager connection: {}", nm_conn);

            let output = Command::new("nmcli")
                .args(["connection", "down", nm_conn])
                .output()
                .await
                .context("Failed to execute nmcli connection down")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Don't fail if connection is already down
                if !stderr.contains("not an active connection") {
                    log::warn!("nmcli connection down warning: {}", stderr);
                }
            }

            log::info!("NetworkManager connection {} is down", nm_conn);
        } else {
            log::info!("Bringing down WireGuard interface: {}", self.interface);

            let output = Command::new("wg-quick")
                .args(["down", &self.interface])
                .output()
                .await
                .context("Failed to execute wg-quick down")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Don't fail if interface is already down
                if !stderr.contains("is not a WireGuard interface") {
                    anyhow::bail!("wg-quick down failed: {}", stderr);
                }
            }

            log::info!("WireGuard interface {} is down", self.interface);
        }
        Ok(())
    }

    /// Get the interface name
    pub fn interface(&self) -> &str {
        &self.interface
    }

    /// Get the interface name to use for querying WireGuard statistics
    ///
    /// When using NetworkManager, this returns the NetworkManager connection name
    /// instead of the actual interface name, because `wg show <name>` works with
    /// NetworkManager connection names.
    fn wg_stats_interface(&self) -> &str {
        self.nm_connection.as_deref().unwrap_or(&self.interface)
    }

    /// Parse `wg show transfer` output and sum transfer stats across all peers
    /// Format: "peer_pubkey\trx_bytes\ttx_bytes" (one line per peer)
    /// Returns (total_rx_bytes, total_tx_bytes)
    #[doc(hidden)]
    pub fn parse_wg_transfer_output(output: &str) -> (u64, u64) {
        let (mut total_rx, mut total_tx) = (0u64, 0u64);

        for line in output.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 3 {
                // parts[0] is peer public key, parts[1] is rx, parts[2] is tx
                if let (Ok(rx), Ok(tx)) = (parts[1].parse::<u64>(), parts[2].parse::<u64>()) {
                    total_rx += rx;
                    total_tx += tx;
                }
            }
        }

        (total_rx, total_tx)
    }

    /// Get current transfer statistics from WireGuard using netlink API
    /// Returns (rx_bytes, tx_bytes) summed across all peers
    ///
    /// This is 100x faster than spawning the `wg` process (~20µs vs 200µs)
    async fn get_transfer_stats(&self) -> Result<(u64, u64)> {
        let iface = self.wg_stats_interface();

        // Parse interface name for wireguard-control
        let iface_name: InterfaceName = iface
            .parse()
            .with_context(|| format!("Invalid interface name: {}", iface))?;

        // Use tokio::task::spawn_blocking for sync netlink call
        let (total_rx, total_tx) = tokio::task::spawn_blocking(move || {
            let device = Device::get(&iface_name, Backend::Kernel)
                .context("Failed to get WireGuard device info")?;

            let mut total_rx = 0u64;
            let mut total_tx = 0u64;

            for peer in device.peers {
                total_rx += peer.stats.rx_bytes;
                total_tx += peer.stats.tx_bytes;
            }

            Ok::<(u64, u64), anyhow::Error>((total_rx, total_tx))
        })
        .await
        .context("Netlink task panicked")??;

        Ok((total_rx, total_tx))
    }

    /// Check for tunnel activity and update internal state
    /// Returns true if there has been activity since last check
    pub async fn check_activity(&mut self) -> Result<bool> {
        let (rx, tx) = self.get_transfer_stats().await?;

        let has_activity = rx != self.last_rx_bytes || tx != self.last_tx_bytes;

        if has_activity {
            log::debug!(
                "Tunnel activity detected: rx={} tx={} (delta: rx={} tx={})",
                rx,
                tx,
                rx.saturating_sub(self.last_rx_bytes),
                tx.saturating_sub(self.last_tx_bytes)
            );
            self.last_activity = Some(Instant::now());
            self.last_rx_bytes = rx;
            self.last_tx_bytes = tx;
        }

        Ok(has_activity)
    }

    /// Get the duration since last tunnel activity
    /// Returns None if no activity has been recorded yet
    pub fn idle_duration(&self) -> Option<std::time::Duration> {
        self.last_activity.map(|t| t.elapsed())
    }

    /// Reset activity tracking (call when tunnel is brought up)
    pub fn reset_activity(&mut self) {
        self.last_rx_bytes = 0;
        self.last_tx_bytes = 0;
        self.last_activity = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_wg_controller_creation() {
        let controller = WgController::new("wg0".to_string(), None).unwrap();
        assert_eq!(controller.interface(), "wg0");
        assert_eq!(controller.last_rx_bytes, 0);
        assert_eq!(controller.last_tx_bytes, 0);
        assert!(controller.last_activity.is_none());
    }

    #[test]
    fn test_wg_controller_with_nm_connection() {
        let controller = WgController::new("wg0".to_string(), Some("my-vpn".to_string())).unwrap();
        assert_eq!(controller.interface(), "wg0");
    }

    #[test]
    fn test_validate_name_valid() {
        assert!(validate_name("wg0", "test").is_ok());
        assert!(validate_name("wlan0", "test").is_ok());
        assert!(validate_name("my-vpn", "test").is_ok());
        assert!(validate_name("my_vpn", "test").is_ok());
        assert!(validate_name("WireGuard-VPN_123", "test").is_ok());
    }

    #[test]
    fn test_validate_name_invalid_special_chars() {
        assert!(validate_name("wg0; rm -rf /", "test").is_err());
        assert!(validate_name("wg0 && echo pwned", "test").is_err());
        assert!(validate_name("wg0|cat /etc/passwd", "test").is_err());
        assert!(validate_name("$(malicious)", "test").is_err());
        assert!(validate_name("`whoami`", "test").is_err());
        assert!(validate_name("wg0$VAR", "test").is_err());
        assert!(validate_name("wg0'test", "test").is_err());
        assert!(validate_name("wg0\"test", "test").is_err());
        assert!(validate_name("wg0\ntest", "test").is_err());
        assert!(validate_name("wg0/test", "test").is_err());
        assert!(validate_name("wg0\\test", "test").is_err());
    }

    #[test]
    fn test_validate_name_empty() {
        assert!(validate_name("", "test").is_err());
    }

    #[test]
    fn test_wg_controller_creation_invalid_interface() {
        assert!(WgController::new("wg0; rm -rf /".to_string(), None).is_err());
        assert!(WgController::new("wg0 && echo pwned".to_string(), None).is_err());
    }

    #[test]
    fn test_wg_controller_creation_invalid_nm_connection() {
        assert!(WgController::new("wg0".to_string(), Some("vpn; malicious".to_string())).is_err());
        assert!(WgController::new("wg0".to_string(), Some("$(echo pwned)".to_string())).is_err());
    }

    #[test]
    fn test_parse_wg_show_output_single_peer() {
        let output = "peer1_pubkey\t1024\t2048\n";
        let (rx, tx) = WgController::parse_wg_transfer_output(output);
        assert_eq!(rx, 1024);
        assert_eq!(tx, 2048);
    }

    #[test]
    fn test_parse_wg_show_output_multiple_peers() {
        let output = "peer1_pubkey\t1000\t2000\npeer2_pubkey\t500\t1000\n";
        let (rx, tx) = WgController::parse_wg_transfer_output(output);
        assert_eq!(rx, 1500);
        assert_eq!(tx, 3000);
    }

    #[test]
    fn test_parse_wg_show_output_empty() {
        let output = "";
        let (rx, tx) = WgController::parse_wg_transfer_output(output);
        assert_eq!(rx, 0);
        assert_eq!(tx, 0);
    }

    #[test]
    fn test_parse_wg_show_output_whitespace_only() {
        let output = "\n\n\n";
        let (rx, tx) = WgController::parse_wg_transfer_output(output);
        assert_eq!(rx, 0);
        assert_eq!(tx, 0);
    }

    #[test]
    fn test_parse_wg_show_output_malformed_invalid_numbers() {
        let output = "peer1\tinvalid\t2000\npeer2\t500\t1000\n";
        let (rx, tx) = WgController::parse_wg_transfer_output(output);
        // Should skip invalid line and only parse valid peer2
        assert_eq!(rx, 500);
        assert_eq!(tx, 1000);
    }

    #[test]
    fn test_parse_wg_show_output_malformed_missing_fields() {
        let output = "peer1\t1000\npeer2\t500\t1000\t9999\n";
        let (rx, tx) = WgController::parse_wg_transfer_output(output);
        // First line has only 2 fields (missing tx), should skip
        // Second line has extra fields, should still parse first 3
        assert_eq!(rx, 500);
        assert_eq!(tx, 1000);
    }

    #[test]
    fn test_parse_wg_show_output_large_numbers() {
        let output = "peer1\t18446744073709551615\t18446744073709551615\n"; // u64::MAX
        let (rx, tx) = WgController::parse_wg_transfer_output(output);
        assert_eq!(rx, u64::MAX);
        assert_eq!(tx, u64::MAX);
    }

    #[test]
    fn test_parse_wg_show_output_zero_values() {
        let output = "peer1\t0\t0\npeer2\t0\t0\n";
        let (rx, tx) = WgController::parse_wg_transfer_output(output);
        assert_eq!(rx, 0);
        assert_eq!(tx, 0);
    }

    #[test]
    fn test_parse_wg_show_output_mixed_valid_invalid() {
        let output = "peer1\t100\t200\ninvalid_line\npeer2\t300\t400\npeer3\tbad\t500\n";
        let (rx, tx) = WgController::parse_wg_transfer_output(output);
        // Should only parse peer1 and peer2
        assert_eq!(rx, 400);
        assert_eq!(tx, 600);
    }

    #[test]
    fn test_idle_duration_no_activity() {
        let controller = WgController::new("wg0".to_string(), None).unwrap();
        assert_eq!(controller.idle_duration(), None);
    }

    #[test]
    fn test_idle_duration_with_activity() {
        let mut controller = WgController::new("wg0".to_string(), None).unwrap();
        controller.last_activity = Some(Instant::now());

        std::thread::sleep(Duration::from_millis(100));

        let duration = controller.idle_duration().unwrap();
        assert!(duration >= Duration::from_millis(100));
        assert!(duration < Duration::from_millis(200));
    }

    #[test]
    fn test_reset_activity() {
        let mut controller = WgController::new("wg0".to_string(), None).unwrap();
        controller.last_rx_bytes = 1000;
        controller.last_tx_bytes = 2000;

        controller.reset_activity();

        assert_eq!(controller.last_rx_bytes, 0);
        assert_eq!(controller.last_tx_bytes, 0);
        assert!(controller.last_activity.is_some());

        // Verify the timestamp is recent
        let duration = controller.idle_duration().unwrap();
        assert!(duration < Duration::from_millis(100));
    }

    // Note: Actual up/down tests would require root privileges and WireGuard setup
    // These should be integration tests run in a proper environment
}
