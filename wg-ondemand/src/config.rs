// Configuration file parser

//! Configuration file parsing and validation
//!
//! This module handles loading TOML configuration files and validating
//! their contents, including CIDR subnet parsing and range checks.

use crate::types::Config;
use anyhow::{Context, Result};
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;

/// Load configuration from TOML file
pub fn load_config<P: AsRef<Path>>(path: P) -> Result<Config> {
    let contents = fs::read_to_string(path.as_ref()).context("Failed to read config file")?;

    let config: Config = toml::from_str(&contents).context("Failed to parse config file")?;

    validate_config(&config)?;
    Ok(config)
}

/// Validate configuration values
fn validate_config(config: &Config) -> Result<()> {
    // Validate SSID lists
    // Check for SSIDs that appear in both target and exclude lists
    for ssid in &config.general.target_ssids.0 {
        if config.general.exclude_ssids.contains(ssid) {
            anyhow::bail!(
                "SSID '{}' appears in both target_ssids and exclude_ssids",
                ssid
            );
        }
    }

    // Warn if both lists are empty (monitor on all networks mode)
    if config.general.target_ssids.0.is_empty() && config.general.exclude_ssids.is_empty() {
        log::warn!(
            "No SSID filtering configured (target_ssids and exclude_ssids both empty). \
            Will monitor on ALL networks. IP collision detection will prevent issues \
            when on networks with same subnet as configured ranges."
        );
    }

    // Validate WireGuard interface name
    if config.general.wg_interface.is_empty() {
        anyhow::bail!("wg_interface cannot be empty");
    }

    // Validate idle_timeout is reasonable
    if config.general.idle_timeout == 0 {
        anyhow::bail!("idle_timeout must be > 0");
    }

    // Validate subnets list is not empty
    if config.subnets.ranges.is_empty() {
        anyhow::bail!("subnets.ranges cannot be empty");
    }

    // Validate max 16 subnets (eBPF array limit)
    if config.subnets.ranges.len() > 16 {
        anyhow::bail!(
            "Maximum 16 subnets allowed, got {}",
            config.subnets.ranges.len()
        );
    }

    // Validate subnets are valid CIDR
    for subnet in &config.subnets.ranges {
        parse_cidr(subnet).with_context(|| format!("Invalid CIDR: {}", subnet))?;
    }

    Ok(())
}

/// Check if an IP address falls within any of the configured subnet ranges
///
/// # Arguments
/// * `ip` - IP address as u32 (network byte order / big endian)
/// * `subnet_cidrs` - List of CIDR strings (e.g., ["192.168.1.0/24"])
///
/// # Returns
/// `true` if the IP is within any subnet, `false` otherwise
pub fn ip_in_subnets(ip: u32, subnet_cidrs: &[String]) -> Result<bool> {
    for cidr in subnet_cidrs {
        let (network, mask) = parse_cidr(cidr)?;
        if (ip & mask) == network {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Parse CIDR notation into (network, mask) tuple
/// Returns network address and netmask in network byte order (big endian)
pub fn parse_cidr(cidr: &str) -> Result<(u32, u32)> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid CIDR format (expected X.X.X.X/N)");
    }

    let ip: Ipv4Addr = parts[0].parse().context("Invalid IP address")?;
    let prefix_len: u8 = parts[1].parse().context("Invalid prefix length")?;

    if prefix_len > 32 {
        anyhow::bail!("Prefix length must be <= 32");
    }

    // Convert IP to u32 (network byte order = big endian)
    let ip_u32 = u32::from_be_bytes(ip.octets());

    // Calculate netmask
    let mask = if prefix_len == 0 {
        0u32
    } else {
        !0u32 << (32 - prefix_len)
    };

    // Apply mask to get network address
    let network = ip_u32 & mask;

    Ok((network, mask))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SsidList;

    #[test]
    fn test_parse_cidr() {
        // Test valid CIDR
        let (network, mask) = parse_cidr("192.168.1.0/24").unwrap();
        assert_eq!(network, u32::from_be_bytes([192, 168, 1, 0]));
        assert_eq!(mask, u32::from_be_bytes([255, 255, 255, 0]));

        // Test /32
        let (network, mask) = parse_cidr("10.0.0.1/32").unwrap();
        assert_eq!(network, u32::from_be_bytes([10, 0, 0, 1]));
        assert_eq!(mask, 0xFFFFFFFF);

        // Test /16
        let (network, mask) = parse_cidr("172.16.0.0/16").unwrap();
        assert_eq!(network, u32::from_be_bytes([172, 16, 0, 0]));
        assert_eq!(mask, u32::from_be_bytes([255, 255, 0, 0]));
    }

    #[test]
    fn test_parse_cidr_invalid() {
        assert!(parse_cidr("invalid").is_err());
        assert!(parse_cidr("192.168.1.0").is_err());
        assert!(parse_cidr("192.168.1.0/").is_err());
        assert!(parse_cidr("192.168.1.0/33").is_err());
        assert!(parse_cidr("999.999.999.999/24").is_err());
    }

    #[test]
    fn test_validate_config() {
        use crate::types::{GeneralConfig, SsidList, SubnetConfig};

        // Valid config with target SSID
        let config = Config {
            general: GeneralConfig {
                target_ssids: SsidList(vec!["TestSSID".to_string()]),
                exclude_ssids: vec![],
                wg_interface: "wg0".to_string(),
                nm_connection: None,
                monitor_interface: None,
                idle_timeout: 300,
                log_level: "info".to_string(),
            },
            subnets: SubnetConfig {
                ranges: vec!["192.168.1.0/24".to_string()],
            },
        };
        assert!(validate_config(&config).is_ok());

        // Valid config with empty SSID lists (monitor all networks)
        let mut all_networks_config = config.clone();
        all_networks_config.general.target_ssids = SsidList(vec![]);
        all_networks_config.general.exclude_ssids = vec![];
        assert!(validate_config(&all_networks_config).is_ok());

        // Invalid: SSID in both lists
        let mut bad_config = config.clone();
        bad_config.general.target_ssids = SsidList(vec!["TestSSID".to_string()]);
        bad_config.general.exclude_ssids = vec!["TestSSID".to_string()];
        assert!(validate_config(&bad_config).is_err());

        // Zero timeout
        let mut bad_config = config.clone();
        bad_config.general.idle_timeout = 0;
        assert!(validate_config(&bad_config).is_err());

        // Invalid CIDR
        let mut bad_config = config.clone();
        bad_config.subnets.ranges = vec!["invalid".to_string()];
        assert!(validate_config(&bad_config).is_err());
    }

    #[test]
    fn test_ip_in_subnets() {
        let subnets = vec!["192.168.1.0/24".to_string(), "10.0.0.0/8".to_string()];

        // Test IP in first subnet
        let ip = u32::from_be_bytes([192, 168, 1, 50]);
        assert!(ip_in_subnets(ip, &subnets).unwrap());

        // Test IP in second subnet
        let ip = u32::from_be_bytes([10, 20, 30, 40]);
        assert!(ip_in_subnets(ip, &subnets).unwrap());

        // Test IP not in any subnet
        let ip = u32::from_be_bytes([172, 16, 0, 1]);
        assert!(!ip_in_subnets(ip, &subnets).unwrap());

        // Test edge case: network address itself
        let ip = u32::from_be_bytes([192, 168, 1, 0]);
        assert!(ip_in_subnets(ip, &subnets).unwrap());

        // Test edge case: broadcast address
        let ip = u32::from_be_bytes([192, 168, 1, 255]);
        assert!(ip_in_subnets(ip, &subnets).unwrap());
    }

    #[test]
    fn test_parse_cidr_edge_cases() {
        // Test /0 (all addresses)
        let (network, mask) = parse_cidr("0.0.0.0/0").unwrap();
        assert_eq!(network, 0);
        assert_eq!(mask, 0);

        // Test /31 (point-to-point link)
        assert!(parse_cidr("10.0.0.0/31").is_ok());

        // Test boundary values
        let (network, mask) = parse_cidr("255.255.255.255/32").unwrap();
        assert_eq!(network, 0xFFFFFFFF);
        assert_eq!(mask, 0xFFFFFFFF);
    }

    #[test]
    fn test_validate_config_empty_subnets() {
        use crate::types::{GeneralConfig, SubnetConfig};

        let config = Config {
            general: GeneralConfig {
                target_ssids: SsidList(vec!["TestSSID".to_string()]),
                exclude_ssids: vec![],
                wg_interface: "wg0".to_string(),
                nm_connection: None,
                monitor_interface: None,
                idle_timeout: 300,
                log_level: "info".to_string(),
            },
            subnets: SubnetConfig { ranges: vec![] },
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_too_many_subnets() {
        use crate::types::{GeneralConfig, SubnetConfig};

        let config = Config {
            general: GeneralConfig {
                target_ssids: SsidList(vec!["TestSSID".to_string()]),
                exclude_ssids: vec![],
                wg_interface: "wg0".to_string(),
                nm_connection: None,
                monitor_interface: None,
                idle_timeout: 300,
                log_level: "info".to_string(),
            },
            subnets: SubnetConfig {
                ranges: (0..17).map(|i| format!("10.{}.0.0/24", i)).collect(),
            },
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_config_max_subnets() {
        use crate::types::{GeneralConfig, SubnetConfig};

        // Exactly 16 subnets should be allowed
        let config = Config {
            general: GeneralConfig {
                target_ssids: SsidList(vec!["TestSSID".to_string()]),
                exclude_ssids: vec![],
                wg_interface: "wg0".to_string(),
                nm_connection: None,
                monitor_interface: None,
                idle_timeout: 300,
                log_level: "info".to_string(),
            },
            subnets: SubnetConfig {
                ranges: (0..16).map(|i| format!("10.{}.0.0/24", i)).collect(),
            },
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_overlapping_subnets() {
        use crate::types::{GeneralConfig, SubnetConfig};

        // Overlapping subnets should be allowed (eBPF will handle)
        let config = Config {
            general: GeneralConfig {
                target_ssids: SsidList(vec!["TestSSID".to_string()]),
                exclude_ssids: vec![],
                wg_interface: "wg0".to_string(),
                nm_connection: None,
                monitor_interface: None,
                idle_timeout: 300,
                log_level: "info".to_string(),
            },
            subnets: SubnetConfig {
                ranges: vec![
                    "192.168.0.0/16".to_string(), // Broader
                    "192.168.1.0/24".to_string(), // More specific
                ],
            },
        };

        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_idle_timeout_bounds() {
        use crate::types::{GeneralConfig, SubnetConfig};

        let base_config = Config {
            general: GeneralConfig {
                target_ssids: SsidList(vec!["TestSSID".to_string()]),
                exclude_ssids: vec![],
                wg_interface: "wg0".to_string(),
                nm_connection: None,
                monitor_interface: None,
                idle_timeout: 300,
                log_level: "info".to_string(),
            },
            subnets: SubnetConfig {
                ranges: vec!["192.168.1.0/24".to_string()],
            },
        };

        // Very small timeout should work
        let mut config = base_config.clone();
        config.general.idle_timeout = 1;
        assert!(validate_config(&config).is_ok());

        // Very large timeout should work
        let mut config = base_config.clone();
        config.general.idle_timeout = 86400; // 24 hours
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_config_empty_interface_name() {
        use crate::types::{GeneralConfig, SubnetConfig};

        let config = Config {
            general: GeneralConfig {
                target_ssids: SsidList(vec!["TestSSID".to_string()]),
                exclude_ssids: vec![],
                wg_interface: "".to_string(),
                nm_connection: None,
                monitor_interface: None,
                idle_timeout: 300,
                log_level: "info".to_string(),
            },
            subnets: SubnetConfig {
                ranges: vec!["192.168.1.0/24".to_string()],
            },
        };

        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_parse_cidr_network_bits_cleared() {
        // Ensure host bits are cleared in network address
        let (network, _) = parse_cidr("192.168.1.100/24").unwrap();
        // Should be 192.168.1.0, not 192.168.1.100
        assert_eq!(network, u32::from_be_bytes([192, 168, 1, 0]));

        let (network, _) = parse_cidr("10.0.0.255/8").unwrap();
        // Should be 10.0.0.0, not 10.0.0.255
        assert_eq!(network, u32::from_be_bytes([10, 0, 0, 0]));
    }
}
