Name:           wg-ondemand
Version:        0.1.0
Release:        1%{?dist}
Summary:        Automatic WireGuard VPN activation on-demand

License:        MIT
URL:            https://github.com/vly/wg-ondemand
Source0:        %{name}-%{version}.tar.gz

BuildRequires:  rust >= 1.70
BuildRequires:  cargo
BuildRequires:  clang
BuildRequires:  llvm
BuildRequires:  elfutils-libelf-devel
BuildRequires:  kernel-devel
BuildRequires:  libbpf-devel
BuildRequires:  systemd-rpm-macros

Requires:       wireguard-tools
Requires:       NetworkManager
Requires:       systemd

# eBPF programs require specific kernel version
Requires:       kernel >= 5.8

%description
A lightweight daemon that automatically activates your WireGuard VPN tunnel
only when accessing specific networks, saving mobile data and battery life.

Features:
- Automatic VPN activation based on traffic detection
- SSID-aware (only works on specific WiFi networks)
- Automatic idle timeout and deactivation
- eBPF-based traffic monitoring for minimal overhead
- NetworkManager integration

%prep
%setup -q

%build
# Build eBPF program
cargo xtask build-ebpf --release

# Build daemon
cargo build --release --package wg-ondemand

%install
# Create directories
install -d %{buildroot}%{_bindir}
install -d %{buildroot}%{_sysconfdir}/wg-ondemand
install -d %{buildroot}%{_unitdir}

# Install binaries
install -m 755 target/release/wg-ondemand %{buildroot}%{_bindir}/wg-ondemand
install -m 755 scripts/setup-tc.sh %{buildroot}%{_bindir}/wg-ondemand-setup-tc

# Install configuration
install -m 644 config/wg-ondemand.toml %{buildroot}%{_sysconfdir}/wg-ondemand/config.toml

# Install systemd service
install -m 644 wg-ondemand.service %{buildroot}%{_unitdir}/wg-ondemand.service

%post
%systemd_post wg-ondemand.service

# Warn user to configure
cat <<'EOF'

================================================================================
  WireGuard On-Demand has been installed!

  IMPORTANT: Edit the configuration file before starting the service:
    sudo nano /etc/wg-ondemand/config.toml

  Configure these settings:
    - target_ssid: Your hotspot SSID
    - wg_interface: Your WireGuard interface name
    - nm_connection: NetworkManager connection name (if applicable)
    - ranges: Target subnets that trigger VPN activation

  Then enable and start the service:
    sudo systemctl enable wg-ondemand
    sudo systemctl start wg-ondemand

  Check status:
    sudo systemctl status wg-ondemand

  View logs:
    sudo journalctl -u wg-ondemand -f
================================================================================

EOF

%preun
%systemd_preun wg-ondemand.service

%postun
%systemd_postun_with_restart wg-ondemand.service

%files
%license LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/wg-ondemand
%{_bindir}/wg-ondemand-setup-tc
%config(noreplace) %{_sysconfdir}/wg-ondemand/config.toml
%{_unitdir}/wg-ondemand.service

%changelog
* Mon Jan 01 2024 Your Name <your.email@example.com> - 0.1.0-1
- Initial RPM release
- On-demand WireGuard VPN activation
- eBPF-based traffic monitoring
- SSID-aware activation
- Automatic idle timeout
- Performance optimizations (89% CPU wakeup reduction)
