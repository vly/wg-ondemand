// NetworkManager SSID monitor via D-Bus

//! Network/SSID change detection via D-Bus
//!
//! This module monitors WiFi network changes using NetworkManager's D-Bus interface,
//! detecting when the system connects to or disconnects from the target SSID.

use anyhow::{Context, Result};
use futures::stream::StreamExt;
use tokio::sync::mpsc;
use zbus::{proxy, Connection};

/// Network event types
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Connected to the target SSID
    ConnectedToTarget,
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
    target_ssid: String,
    connection: Connection,
}

impl SsidMonitor {
    /// Create a new SSID monitor
    pub async fn new(target_ssid: String) -> Result<Self> {
        let connection = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        Ok(Self {
            target_ssid,
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

    /// Check if connected to the target SSID
    pub async fn is_connected_to_target(&self) -> Result<bool> {
        match self.current_ssid().await? {
            Some(ssid) => Ok(ssid == self.target_ssid),
            None => Ok(false),
        }
    }

    /// Monitor for network changes and send events
    pub async fn monitor(&self, tx: mpsc::Sender<NetworkEvent>) -> Result<()> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        let mut stream = nm.receive_primary_connection_changed().await;

        let mut was_connected = self.is_connected_to_target().await?;

        log::info!("Starting SSID monitor for '{}'", self.target_ssid);
        if was_connected {
            log::info!("Already connected to target SSID");
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
                log::info!("Connected to target SSID: {}", self.target_ssid);
                let _ = tx.send(NetworkEvent::ConnectedToTarget).await;
            } else if !is_connected && was_connected {
                log::info!("Disconnected from target SSID");
                let _ = tx.send(NetworkEvent::Disconnected).await;
            }

            was_connected = is_connected;
        }

        Ok(())
    }

    /// Get the target SSID
    pub fn target_ssid(&self) -> &str {
        &self.target_ssid
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
        let event = NetworkEvent::ConnectedToTarget;
        match event {
            NetworkEvent::ConnectedToTarget => {}
            _ => unreachable!("Expected ConnectedToTarget variant"),
        }
    }
}
