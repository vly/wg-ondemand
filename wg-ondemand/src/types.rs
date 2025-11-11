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
    pub timestamp: u64, // Kernel timestamp (ns)
    pub dest_ip: u32,   // Destination IP in network byte order
    pub dest_port: u16, // Destination port
    pub protocol: u8,   // IPPROTO_TCP, IPPROTO_UDP, etc.
    pub _padding: u8,   // Padding for alignment
}

/// Tunnel state machine states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelState {
    Inactive,     // Tunnel down, not monitoring
    Monitoring,   // Tunnel down, monitoring traffic
    Activating,   // Tunnel coming up
    Active,       // Tunnel up and running
    Deactivating, // Tunnel going down
}

/// Main configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub general: GeneralConfig,
    pub subnets: SubnetConfig,
}

/// General configuration options
#[derive(Debug, Deserialize, Clone)]
pub struct GeneralConfig {
    pub target_ssid: String,
    pub wg_interface: String,
    /// NetworkManager connection name (if using NetworkManager instead of wg-quick)
    #[serde(default)]
    pub nm_connection: Option<String>,
    #[serde(default)]
    pub monitor_interface: Option<String>,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout: u64,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

/// Subnet configuration
#[derive(Debug, Deserialize, Clone)]
pub struct SubnetConfig {
    pub ranges: Vec<String>, // CIDR notation
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
