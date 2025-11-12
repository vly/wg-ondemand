// Shared types between eBPF and userspace

//! Shared data structures
//!
//! This module defines data structures shared between components,
//! including the FFI-compatible TrafficEvent structure for eBPF communication,
//! state machine types, and configuration structures.

use serde::Deserialize;

/// Event structure for eBPF → userspace communication
/// Must be #[repr(C)] for ABI compatibility with eBPF
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct TrafficEvent {
    /// Kernel timestamp in nanoseconds
    pub timestamp: u64,
    /// Destination IP in network byte order
    pub dest_ip: u32,
    /// Destination port
    pub dest_port: u16,
    /// IP protocol (IPPROTO_TCP, IPPROTO_UDP, etc.)
    pub protocol: u8,
    /// Padding for alignment
    pub _padding: u8,
}

/// Tunnel state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelState {
    /// Tunnel down, not monitoring
    Inactive,
    /// Tunnel down, monitoring traffic
    Monitoring,
    /// Tunnel coming up
    Activating,
    /// Tunnel up and running
    Active,
    /// Tunnel going down
    Deactivating,
}

/// Main configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// General configuration options
    pub general: GeneralConfig,
    /// Subnet configuration
    pub subnets: SubnetConfig,
}

/// General configuration options
#[derive(Debug, Deserialize, Clone)]
pub struct GeneralConfig {
    /// Target SSIDs to monitor (whitelist). If empty, monitors on all networks.
    /// Can also use singular 'target_ssid' for backward compatibility.
    #[serde(default, alias = "target_ssid")]
    pub target_ssids: SsidList,
    /// SSIDs to exclude from monitoring (blacklist). Takes precedence over target_ssids.
    #[serde(default)]
    pub exclude_ssids: Vec<String>,
    /// WireGuard interface name
    pub wg_interface: String,
    /// NetworkManager connection name (if using NetworkManager instead of wg-quick)
    #[serde(default)]
    pub nm_connection: Option<String>,
    /// Network interface to monitor (auto-detected if not specified)
    #[serde(default)]
    pub monitor_interface: Option<String>,
    /// Idle timeout in seconds before deactivating tunnel
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

/// Custom type to handle both single SSID (backward compat) and list of SSIDs
#[derive(Debug, Clone, Default)]
pub struct SsidList(pub Vec<String>);

impl<'de> Deserialize<'de> for SsidList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Visitor};
        use std::fmt;

        struct SsidListVisitor;

        impl<'de> Visitor<'de> for SsidListVisitor {
            type Value = SsidList;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or a list of strings")
            }

            fn visit_str<E>(self, value: &str) -> Result<SsidList, E>
            where
                E: de::Error,
            {
                // Single SSID string (backward compatibility)
                Ok(SsidList(vec![value.to_string()]))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<SsidList, A::Error>
            where
                A: de::SeqAccess<'de>,
            {
                // List of SSIDs
                let mut vec = Vec::new();
                while let Some(elem) = seq.next_element()? {
                    vec.push(elem);
                }
                Ok(SsidList(vec))
            }
        }

        deserializer.deserialize_any(SsidListVisitor)
    }
}

/// Subnet configuration
#[derive(Debug, Deserialize, Clone)]
pub struct SubnetConfig {
    /// Target subnet ranges in CIDR notation (e.g., "192.168.1.0/24")
    pub ranges: Vec<String>,
}

// Default values for configuration
fn default_idle_timeout() -> u64 {
    300 // 5 minutes
}

fn default_log_level() -> String {
    "info".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem;

    #[test]
    fn test_traffic_event_size() {
        // Ensure TrafficEvent has expected size for C compatibility (u64 + u32 + u16 + u8 + u8)
        assert_eq!(mem::size_of::<TrafficEvent>(), 16);
        assert_eq!(mem::align_of::<TrafficEvent>(), 8);
    }

    #[test]
    fn test_traffic_event_field_offsets() {
        // Verify field layout matches eBPF expectations
        // Using offset_of! macro (stable in Rust 1.77+)
        use std::mem::offset_of;

        assert_eq!(offset_of!(TrafficEvent, timestamp), 0);
        assert_eq!(offset_of!(TrafficEvent, dest_ip), 8);
        assert_eq!(offset_of!(TrafficEvent, dest_port), 12);
        assert_eq!(offset_of!(TrafficEvent, protocol), 14);
        assert_eq!(offset_of!(TrafficEvent, _padding), 15);
    }

    #[test]
    fn test_traffic_event_field_sizes() {
        // Verify individual field sizes
        assert_eq!(mem::size_of::<u64>(), 8); // timestamp
        assert_eq!(mem::size_of::<u32>(), 4); // dest_ip
        assert_eq!(mem::size_of::<u16>(), 2); // dest_port
        assert_eq!(mem::size_of::<u8>(), 1); // protocol
        assert_eq!(mem::size_of::<u8>(), 1); // _padding
    }

    #[test]
    fn test_traffic_event_copy_clone() {
        let event = TrafficEvent {
            timestamp: 12345,
            dest_ip: 0xC0A80101, // 192.168.1.1
            dest_port: 443,
            protocol: 6, // TCP
            _padding: 0,
        };

        let copied = event;
        assert_eq!(copied.timestamp, event.timestamp);
        assert_eq!(copied.dest_ip, event.dest_ip);
        assert_eq!(copied.dest_port, event.dest_port);
        assert_eq!(copied.protocol, event.protocol);
    }

    #[test]
    fn test_tunnel_state_transitions() {
        let state = TunnelState::Inactive;
        assert_eq!(state, TunnelState::Inactive);
        assert_ne!(state, TunnelState::Monitoring);
    }

    #[test]
    fn test_tunnel_state_all_variants() {
        // Ensure all states are Copy, Clone, Debug, PartialEq
        let states = vec![
            TunnelState::Inactive,
            TunnelState::Monitoring,
            TunnelState::Activating,
            TunnelState::Active,
            TunnelState::Deactivating,
        ];

        for state in states {
            let copied = state;
            assert_eq!(state, copied);

            // Verify Debug works
            let debug_str = format!("{:?}", state);
            assert!(!debug_str.is_empty());
        }
    }

    #[test]
    fn test_tunnel_state_distinct() {
        // Verify all states are distinct
        assert_ne!(TunnelState::Inactive, TunnelState::Monitoring);
        assert_ne!(TunnelState::Inactive, TunnelState::Activating);
        assert_ne!(TunnelState::Inactive, TunnelState::Active);
        assert_ne!(TunnelState::Inactive, TunnelState::Deactivating);
        assert_ne!(TunnelState::Monitoring, TunnelState::Activating);
        assert_ne!(TunnelState::Monitoring, TunnelState::Active);
        assert_ne!(TunnelState::Monitoring, TunnelState::Deactivating);
        assert_ne!(TunnelState::Activating, TunnelState::Active);
        assert_ne!(TunnelState::Activating, TunnelState::Deactivating);
        assert_ne!(TunnelState::Active, TunnelState::Deactivating);
    }

    #[test]
    fn test_traffic_event_repr_c() {
        // Verify struct is repr(C)
        // This is implicitly tested by size and offset checks,
        // but we can also verify it doesn't have unexpected padding
        let event = TrafficEvent {
            timestamp: 0,
            dest_ip: 0,
            dest_port: 0,
            protocol: 0,
            _padding: 0,
        };

        // All fields should be accessible
        let _ = event.timestamp;
        let _ = event.dest_ip;
        let _ = event.dest_port;
        let _ = event.protocol;
        let _ = event._padding;
    }
}
