//! Dynamic route management for traffic monitoring
//!
//! Manages temporary routes that direct monitored subnets through the WiFi gateway,
//! allowing eBPF egress hooks to detect traffic even when the VPN is down.

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::net::Ipv4Addr;
use tokio::process::Command;

/// Manages temporary routes for traffic monitoring
pub struct RouteManager {
    interface: String,
    gateway: Option<Ipv4Addr>,
    active_routes: HashSet<String>,
}

impl RouteManager {
    /// Create a new route manager for the given interface
    pub fn new(interface: String) -> Self {
        Self {
            interface,
            gateway: None,
            active_routes: HashSet::new(),
        }
    }

    /// Detect gateway IP by parsing `ip route show dev <interface>`
    async fn detect_gateway(&self) -> Result<Ipv4Addr> {
        let output = Command::new("ip")
            .args(["route", "show", "dev", &self.interface])
            .output()
            .await
            .context("Failed to get routes")?;

        anyhow::ensure!(output.status.success(), "ip route command failed");

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find_map(|line| {
                line.find(" via ")
                    .and_then(|pos| line[pos + 5..].split_whitespace().next())
                    .and_then(|s| s.parse::<Ipv4Addr>().ok())
            })
            .with_context(|| format!("No gateway found for {}", self.interface))
    }

    /// Add monitoring routes for configured subnets
    pub async fn add_routes(&mut self, subnets: &[String]) -> Result<()> {
        if self.gateway.is_none() {
            self.gateway = Some(self.detect_gateway().await?);
        }
        let gateway = self.gateway.unwrap();

        for subnet in subnets {
            if self.active_routes.contains(subnet) {
                continue;
            }

            let success = Command::new("ip")
                .args([
                    "route",
                    "add",
                    subnet,
                    "via",
                    &gateway.to_string(),
                    "dev",
                    &self.interface,
                ])
                .status()
                .await?
                .success();

            if success || self.route_exists(subnet, &gateway).await? {
                log::info!(
                    "Route active: {} via {} dev {}",
                    subnet,
                    gateway,
                    self.interface
                );
                self.active_routes.insert(subnet.clone());
            }
        }

        Ok(())
    }

    /// Remove all managed routes
    pub async fn remove_routes(&mut self) -> Result<()> {
        for subnet in self.active_routes.drain() {
            let _ = Command::new("ip")
                .args(["route", "del", &subnet])
                .status()
                .await;
            log::info!("Removed route: {}", subnet);
        }
        Ok(())
    }

    async fn route_exists(&self, subnet: &str, gateway: &Ipv4Addr) -> Result<bool> {
        let output = Command::new("ip")
            .args(["route", "show", subnet])
            .output()
            .await?;

        Ok(output.status.success() && {
            let out = String::from_utf8_lossy(&output.stdout);
            out.contains(&gateway.to_string()) && out.contains(&self.interface)
        })
    }

    /// Clear cached gateway (useful when interface state changes)
    pub fn clear_gateway_cache(&mut self) {
        self.gateway = None;
    }

    /// Check if any routes are currently active
    pub fn has_active_routes(&self) -> bool {
        !self.active_routes.is_empty()
    }
}

impl Drop for RouteManager {
    fn drop(&mut self) {
        if !self.has_active_routes() {
            return;
        }

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            for subnet in self.active_routes.drain() {
                let _ = handle.block_on(async {
                    Command::new("ip")
                        .args(["route", "del", &subnet])
                        .status()
                        .await
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let rm = RouteManager::new("wlan0".to_string());
        assert_eq!(rm.interface, "wlan0");
        assert!(rm.gateway.is_none());
        assert!(!rm.has_active_routes());
    }

    #[test]
    fn test_active_routes() {
        let mut rm = RouteManager::new("wlan0".to_string());
        rm.active_routes.insert("192.168.1.0/24".to_string());
        assert!(rm.has_active_routes());
    }

    #[test]
    fn test_clear_gateway() {
        let mut rm = RouteManager::new("wlan0".to_string());
        rm.gateway = Some(Ipv4Addr::new(192, 168, 1, 1));
        rm.clear_gateway_cache();
        assert!(rm.gateway.is_none());
    }
}
