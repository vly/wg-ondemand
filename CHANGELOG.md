# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- GitHub Actions workflow for automated releases
- Pre-built release archives with all dependencies
- TC qdisc setup script integrated into systemd service
- Startup detection of existing WireGuard tunnels
- Proper idle timeout monitoring for pre-existing tunnels

### Changed
- eBPF polling interval increased from 100ms to 1000ms (90% CPU wakeup reduction)
- Tokio worker threads limited to 2 (from N cores)
- eBPF ring buffer reduced from 256KB to 16KB
- Replaced process spawning with netlink API for stats (100x performance improvement)
- Cached ring buffer reference to eliminate repeated map lookups

### Fixed
- Daemon now properly detects and manages existing tunnels at startup
- Idle timeout now works correctly for tunnels that were already up
- TC qdisc issues on network interfaces with noqueue

### Performance
- CPU wakeups reduced from ~88,500/day to <10,000/day (89% reduction)
- Memory usage reduced by 50-120MB
- Binary size reduced by 400-600KB
- Eliminated 1,440 process spawns per day
- Eliminated 86,000 map lookups per day

## [0.1.0] - YYYY-MM-DD

### Added
- Initial release
- On-demand WireGuard VPN activation based on traffic detection
- eBPF-based traffic monitoring for target subnets
- SSID-aware activation (only works on specific WiFi networks)
- Automatic idle timeout and tunnel deactivation
- NetworkManager and wg-quick support
- Systemd service integration
- Configurable via TOML file
- Graceful shutdown handling

### Features
- Auto-detects network interface if not specified
- Monitors up to 16 target subnets
- Supports TCP, UDP, and ICMP traffic detection
- 5-minute configurable idle timeout
- Debug logging support
- Minimal resource usage (~27MB RAM, <1% CPU)

[Unreleased]: https://github.com/vly/wg-ondemand/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/vly/wg-ondemand/releases/tag/v0.1.0
