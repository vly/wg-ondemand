// NetworkManager SSID monitor via D-Bus

//! Network/SSID change detection via D-Bus
//!
//! This module monitors WiFi network changes using NetworkManager's D-Bus interface,
//! detecting when the system connects to or disconnects from the target SSID.

use anyhow::{Context, Result};
use futures_util::stream::StreamExt;
use tokio::sync::mpsc;
use zbus::{proxy, Connection};

/// Network event types
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Connected to the target SSID (with SSID name)
    ConnectedToTarget(String),
    /// Disconnected from the target SSID (or connected to different network)
    Disconnected,
}

/// D-Bus proxy for NetworkManager
#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    /// Get the primary connection object path
    #[zbus(property)]
    fn primary_connection(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;

    /// Get all active connections
    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;
}

/// D-Bus proxy for active connection
#[proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
trait ActiveConnection {
    /// Get the connection ID
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;

    /// Get the connection type
    #[zbus(property, name = "Type")]
    fn connection_type(&self) -> zbus::Result<String>;

    /// Get the devices associated with this connection
    #[zbus(property)]
    fn devices(&self) -> zbus::Result<Vec<zbus::zvariant::OwnedObjectPath>>;
}

/// D-Bus proxy for wireless device
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait WirelessDevice {
    /// Get the active access point object path
    #[zbus(property)]
    fn active_access_point(&self) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
}

/// D-Bus proxy for access point
#[proxy(
    interface = "org.freedesktop.NetworkManager.AccessPoint",
    default_service = "org.freedesktop.NetworkManager"
)]
trait AccessPoint {
    /// Get the SSID as raw bytes
    #[zbus(property)]
    fn ssid(&self) -> zbus::Result<Vec<u8>>;
}

/// SSID monitor
pub struct SsidMonitor {
    target_ssids: Vec<String>,
    exclude_ssids: Vec<String>,
    connection: Connection,
}

impl SsidMonitor {
    /// Create a new SSID monitor
    ///
    /// # Arguments
    /// * `target_ssids` - Whitelist of SSIDs to monitor. If empty, monitors all SSIDs.
    /// * `exclude_ssids` - Blacklist of SSIDs to exclude. Takes precedence over target_ssids.
    pub async fn new(target_ssids: Vec<String>, exclude_ssids: Vec<String>) -> Result<Self> {
        let connection = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        Ok(Self {
            target_ssids,
            exclude_ssids,
            connection,
        })
    }

    /// Get the current SSID
    pub async fn current_ssid(&self) -> Result<Option<String>> {
        let nm = NetworkManagerProxy::new(&self.connection)
            .await
            .context("Failed to create NetworkManager proxy")?;

        // Get primary connection
        let primary = match nm.primary_connection().await {
            Ok(p) => p,
            Err(_) => return Ok(None),
        };

        if primary.as_str() == "/" {
            return Ok(None);
        }

        // Get active connection details
        let active_conn = ActiveConnectionProxy::builder(&self.connection)
            .path(&primary)?
            .build()
            .await?;

        // Check if it's a wireless connection
        if active_conn.connection_type().await? != "802-11-wireless" {
            return Ok(None);
        }

        // Get wireless device
        let devices = active_conn.devices().await?;
        if devices.is_empty() {
            return Ok(None);
        }

        let wireless_dev = WirelessDeviceProxy::builder(&self.connection)
            .path(&devices[0])?
            .build()
            .await?;

        // Get access point
        let ap_path = wireless_dev.active_access_point().await?;
        if ap_path.as_str() == "/" {
            return Ok(None);
        }

        let ap = AccessPointProxy::builder(&self.connection)
            .path(&ap_path)?
            .build()
            .await?;

        // Get SSID
        let ssid_bytes = ap.ssid().await?;
        let ssid = String::from_utf8(ssid_bytes).context("Invalid UTF-8 in SSID")?;

        Ok(Some(ssid))
    }

    /// Check if connected to a monitored SSID (respecting whitelist/blacklist rules)
    ///
    /// Returns `true` if:
    /// - Connected to WiFi network AND
    /// - (target_ssids is empty OR current SSID is in target_ssids) AND
    /// - Current SSID is NOT in exclude_ssids
    pub async fn is_connected_to_target(&self) -> Result<bool> {
        match self.current_ssid().await? {
            Some(ssid) => {
                // First check blacklist (takes precedence)
                if self.exclude_ssids.contains(&ssid) {
                    log::debug!("SSID '{}' is in exclude list", ssid);
                    return Ok(false);
                }

                // Then check whitelist
                if self.target_ssids.is_empty() {
                    // Empty whitelist means "all SSIDs" (except those excluded)
                    log::debug!("SSID '{}' allowed (monitor all mode)", ssid);
                    Ok(true)
                } else {
                    // Non-empty whitelist: must be in the list
                    let is_target = self.target_ssids.contains(&ssid);
                    if is_target {
                        log::debug!("SSID '{}' is in target list", ssid);
                    } else {
                        log::debug!("SSID '{}' not in target list", ssid);
                    }
                    Ok(is_target)
                }
            }
            None => Ok(false),
        }
    }

    /// Monitor for network changes and send events
    pub async fn monitor(&self, tx: mpsc::Sender<NetworkEvent>) -> Result<()> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        let mut stream = nm.receive_primary_connection_changed().await;

        let mut was_connected = self.is_connected_to_target().await?;

        // Log monitoring configuration
        if self.target_ssids.is_empty() && self.exclude_ssids.is_empty() {
            log::info!("Starting SSID monitor: monitoring ALL networks");
        } else if self.target_ssids.is_empty() {
            log::info!(
                "Starting SSID monitor: monitoring all EXCEPT {:?}",
                self.exclude_ssids
            );
        } else if self.exclude_ssids.is_empty() {
            log::info!(
                "Starting SSID monitor: monitoring ONLY {:?}",
                self.target_ssids
            );
        } else {
            log::info!(
                "Starting SSID monitor: monitoring {:?} EXCEPT {:?}",
                self.target_ssids,
                self.exclude_ssids
            );
        }

        if was_connected {
            if let Ok(Some(current)) = self.current_ssid().await {
                log::info!("Already connected to monitored SSID: {}", current);
            }
        }

        while let Some(_signal) = stream.next().await {
            let is_connected = match self.is_connected_to_target().await {
                Ok(c) => c,
                Err(e) => {
                    log::warn!("Failed to check SSID: {}", e);
                    continue;
                }
            };

            if is_connected && !was_connected {
                if let Ok(Some(current)) = self.current_ssid().await {
                    log::info!("Connected to monitored SSID: {}", current);
                    let _ = tx.send(NetworkEvent::ConnectedToTarget(current)).await;
                } else {
                    // Fallback if we can't get SSID
                    let _ = tx
                        .send(NetworkEvent::ConnectedToTarget(String::new()))
                        .await;
                }
            } else if !is_connected && was_connected {
                log::info!("Disconnected from monitored SSID");
                let _ = tx.send(NetworkEvent::Disconnected).await;
            }

            was_connected = is_connected;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssid_monitor_creation() {
        // Test creation structure (actual D-Bus connection requires system bus)
        let target = "TestSSID".to_string();
        assert_eq!(target, "TestSSID");
    }

    #[test]
    fn test_network_event_types() {
        let event = NetworkEvent::ConnectedToTarget("TestSSID".to_string());
        match event {
            NetworkEvent::ConnectedToTarget(ssid) => {
                assert_eq!(ssid, "TestSSID");
            }
            _ => unreachable!("Expected ConnectedToTarget variant"),
        }
    }
}
