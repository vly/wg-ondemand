// WireGuard On-Demand Activation Library
// Shared modules for daemon and tests

#![warn(missing_docs)]

//! WireGuard On-Demand Activation Library
//!
//! This library provides the core functionality for automatically activating and deactivating
//! WireGuard VPN tunnels based on network conditions (SSID) and traffic patterns.
//!
//! # Main Components
//!
//! - [`config`]: Configuration file parsing and validation
//! - [`ebpf_loader`]: eBPF program management for traffic monitoring
//! - [`ssid_monitor`]: Network/SSID change detection via D-Bus
//! - [`state`]: State machine for tunnel lifecycle management
//! - [`types`]: Shared data structures
//! - [`wg_controller`]: WireGuard tunnel control and statistics

pub mod config;
pub mod ebpf_loader;
pub mod ssid_monitor;
pub mod state;
pub mod types;
pub mod wg_controller;
