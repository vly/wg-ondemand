# Fedora Packaging Guide

This guide covers how to build and distribute wg-ondemand as an RPM package for Fedora and RHEL-based distributions.

## For Users: Installation via COPR (Recommended)

The easiest way to install on Fedora:

```bash
# Enable the COPR repository
sudo dnf copr enable vly/wg-ondemand

# Install wg-ondemand
sudo dnf install wg-ondemand

# Configure
sudo nano /etc/wg-ondemand/config.toml

# Enable and start
sudo systemctl enable wg-ondemand
sudo systemctl start wg-ondemand
```

To update:
```bash
sudo dnf update wg-ondemand
```

To uninstall:
```bash
sudo dnf remove wg-ondemand
```

## For Maintainers: Building RPMs

### Prerequisites

Install build dependencies:

```bash
sudo dnf install @development-tools @rpm-development-tools
sudo dnf install rust cargo clang llvm elfutils-libelf-devel kernel-devel libbpf-devel
```

### Local RPM Build

```bash
# Create RPM build structure
mkdir -p ~/rpmbuild/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

# Create source tarball
git archive --format=tar.gz --prefix=wg-ondemand-0.1.0/ HEAD > ~/rpmbuild/SOURCES/wg-ondemand-0.1.0.tar.gz

# Copy spec file
cp wg-ondemand.spec ~/rpmbuild/SPECS/

# Build RPM
cd ~/rpmbuild/SPECS
rpmbuild -ba wg-ondemand.spec

# Install locally
sudo dnf install ~/rpmbuild/RPMS/x86_64/wg-ondemand-*.rpm
```

### Testing the Package

After building:

```bash
# Install in a container for testing
podman run -it --rm fedora:latest bash
dnf install ~/rpmbuild/RPMS/x86_64/wg-ondemand-*.rpm

# Or use mock for clean builds
mock -r fedora-39-x86_64 ~/rpmbuild/SRPMS/wg-ondemand-*.src.rpm
```

## Setting Up COPR Repository

COPR (Cool Other Package Repo) is Fedora's build service for community packages.

### 1. Create COPR Account

1. Go to https://copr.fedorainfracloud.org/
2. Sign in with Fedora Account
3. Create new project: `wg-ondemand`

### 2. Configure Project Settings

- **Name**: wg-ondemand
- **Description**: Automatic WireGuard VPN activation on-demand
- **Instructions**: See installation commands above
- **Chroots**: Select Fedora 38, 39, 40, etc.
- **Build requires**: (leave default, dependencies in spec file)

### 3. Connect to GitHub (Automatic Builds)

1. In COPR project settings, go to "Integrations"
2. Select "GitHub"
3. Add webhook to your GitHub repository
4. Configure to trigger on:
   - Push to main branch
   - New tags (v*)

### 4. Manual Build via COPR CLI

Install COPR CLI:
```bash
sudo dnf install copr-cli
```

Get API token from https://copr.fedorainfracloud.org/api/ and save to `~/.config/copr`

Build from Git:
```bash
copr-cli build wg-ondemand \
  --clone-url https://github.com/vly/wg-ondemand \
  --committish v0.1.0
```

### 5. Monitor Builds

- Check build status: https://copr.fedorainfracloud.org/coprs/vly/wg-ondemand/builds/
- Build logs show any compilation errors
- Successful builds are automatically published to the repo

## Updating the Package

### For New Releases

1. Update version in `wg-ondemand.spec`:
   ```spec
   Version:        0.2.0
   ```

2. Update changelog in spec file:
   ```spec
   %changelog
   * Mon Jan 15 2024 Your Name <email> - 0.2.0-1
   - New feature: Adaptive polling
   - Bug fix: Handle tunnel reconnections
   ```

3. Commit changes and tag:
   ```bash
   git commit -am "Bump version to 0.2.0"
   git tag -a v0.2.0 -m "Release v0.2.0"
   git push origin v0.2.0
   ```

4. COPR automatically rebuilds (if webhook configured)
   Or trigger manually:
   ```bash
   copr-cli build wg-ondemand \
     --clone-url https://github.com/vly/wg-ondemand \
     --committish v0.2.0
   ```

## RPM Package Details

### Installed Files

- `/usr/bin/wg-ondemand` - Main daemon
- `/usr/bin/wg-ondemand-setup-tc` - TC qdisc setup script
- `/etc/wg-ondemand/config.toml` - Configuration (marked as config file)
- `/usr/lib/systemd/system/wg-ondemand.service` - Systemd service

### Post-Install Actions

The RPM package automatically:
- Creates systemd service
- Sets up configuration directory
- Shows installation instructions
- Does NOT auto-enable the service (user must configure first)

### Configuration Protection

The config file is marked with `%config(noreplace)`, meaning:
- User edits are preserved during upgrades
- New default config saved as `.rpmnew` if changed

## Troubleshooting

### Build Failures

**Missing dependencies:**
```bash
# Add to spec file BuildRequires section
BuildRequires: missing-package-name
```

**Rust compilation errors:**
```bash
# Ensure minimum Rust version
BuildRequires: rust >= 1.70
```

**eBPF compilation fails:**
```bash
# Check kernel headers are available
BuildRequires: kernel-devel
```

### COPR Build Issues

Check build logs in COPR web interface:
- Build tab shows real-time output
- Common issues: missing dependencies, network timeouts
- Can rebuild manually if transient failure

### Package Installation Issues

**Missing dependencies:**
```bash
# User needs to enable WireGuard repo
sudo dnf install wireguard-tools
```

**Kernel too old:**
```bash
# Check kernel version
uname -r  # Must be >= 5.8
sudo dnf update kernel  # Update if needed
```

## Alternative: Local Repository

To distribute RPMs without COPR:

```bash
# Create local repo
mkdir /var/www/html/repos/wg-ondemand
cp ~/rpmbuild/RPMS/x86_64/wg-ondemand-*.rpm /var/www/html/repos/wg-ondemand/

# Create repo metadata
createrepo /var/www/html/repos/wg-ondemand/

# Users add repo
cat > /etc/yum.repos.d/wg-ondemand.repo <<EOF
[wg-ondemand]
name=WireGuard On-Demand
baseurl=http://your-server.com/repos/wg-ondemand
enabled=1
gpgcheck=0
EOF

# Install
sudo dnf install wg-ondemand
```

## Resources

- Fedora Packaging Guidelines: https://docs.fedoraproject.org/en-US/packaging-guidelines/
- COPR Documentation: https://docs.pagure.org/copr.copr/
- RPM Packaging Guide: https://rpm-packaging-guide.github.io/
- Rust Packaging in Fedora: https://pagure.io/fedora-rust/rust2rpm
