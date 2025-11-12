# Release Process

This document describes how to create and publish releases for wg-ondemand.

## Automated Releases via Git Tags

The easiest way to create a release is by pushing a version tag:

```bash
# Create and push a new version tag
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

This will automatically:
1. Build the eBPF program and daemon
2. Create a release archive with all necessary files
3. Generate checksums
4. Create a GitHub Release with the artifacts
5. Publish release notes

## Manual Releases via GitHub Actions

You can also trigger a release manually from the GitHub Actions UI:

1. Go to the **Actions** tab in your repository
2. Select the **Build Release** workflow
3. Click **Run workflow**
4. Enter the version tag (e.g., `v0.1.0`)
5. Click **Run workflow**

This creates the release artifacts but does **not** create a GitHub Release (only tag-based releases do that).

## Release Archive Contents

Each release archive (`wg-ondemand-vX.Y.Z.tar.gz`) contains:

```
wg-ondemand-vX.Y.Z/
├── bin/
│   ├── wg-ondemand              # Main daemon binary
│   └── wg-ondemand-setup-tc     # TC qdisc setup script
├── config/
│   └── wg-ondemand.toml.example # Example configuration
├── systemd/
│   └── wg-ondemand.service      # Systemd service file
├── docs/
│   └── README.md                # Full documentation
├── install.sh                   # Installation script
├── uninstall.sh                 # Uninstallation script
├── INSTALL.md                   # Installation instructions
└── SHA256SUMS                   # Checksums for all files
```

## Version Numbering

Follow [Semantic Versioning](https://semver.org/):

- **MAJOR** version (X.0.0): Incompatible API/config changes
- **MINOR** version (0.X.0): New features, backward compatible
- **PATCH** version (0.0.X): Bug fixes, backward compatible

Examples:
- `v0.1.0` - Initial release
- `v0.1.1` - Bug fix
- `v0.2.0` - New feature (on-demand activation groups)
- `v1.0.0` - Stable release, config format finalized

## Pre-releases

For beta or release candidate versions, use tags like:
- `v0.1.0-beta.1`
- `v0.1.0-rc.1`

The GitHub Action will automatically mark these as pre-releases.

## Testing Before Release

Before tagging a release, ensure:

1. **All tests pass:**
   ```bash
   cargo test --package wg-ondemand --lib
   ```

2. **Builds successfully:**
   ```bash
   cargo xtask build-ebpf --release
   cargo build --release --package wg-ondemand
   ```

3. **Linting passes:**
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   cargo fmt -- --check
   ```

4. **Manual testing:**
   - Test on-demand activation
   - Test idle timeout
   - Test SSID monitoring
   - Test startup with existing tunnel

## Release Checklist

- [ ] Update version in `Cargo.toml` files
- [ ] Update `CHANGELOG.md` with release notes
- [ ] Run full test suite
- [ ] Test installation on clean system
- [ ] Update README if needed
- [ ] Commit all changes
- [ ] Create and push version tag
- [ ] Verify GitHub Release was created
- [ ] Test download and installation from release

## Updating Release Notes

The GitHub Action automatically generates release notes from the template in `.github/workflows/release.yml`. To customize:

1. Edit the `body:` section in `.github/workflows/release.yml`
2. Or edit the release notes directly on GitHub after creation

## Troubleshooting

### Build fails in GitHub Actions

Check the Actions log for errors. Common issues:
- Missing dependencies (add to `apt-get install` step)
- Rust toolchain issues (update `actions-rs/toolchain` version)
- BPF compilation errors (ensure `bpfel-unknown-none` target is added)

### Release not created automatically

Ensure:
- Tag format is `vX.Y.Z` (starts with `v`)
- Tag was pushed to `origin` (not just created locally)
- GitHub Actions has write permissions (Settings → Actions → General → Workflow permissions)

### Checksums don't match

This shouldn't happen if the build is reproducible. If it does:
- Re-download the archive
- Verify your download wasn't corrupted
- Check GitHub Actions log for build warnings
