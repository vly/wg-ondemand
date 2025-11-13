{
  description = "WireGuard On-Demand Activation - eBPF-based daemon for automatic VPN activation";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        # Rust toolchain - using nightly for eBPF target support
        # Note: eBPF targets (bpfel-unknown-none) are tier 3 and may not be available
        # in stable releases. We use nightly or rely on bpf-linker to handle compilation.
        rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
          extensions = [ "rust-src" "rust-analyzer" ];
          targets = [ "x86_64-unknown-linux-musl" ];
        });

        # eBPF build dependencies
        ebpfDeps = with pkgs; [
          llvmPackages_latest.llvm
          llvmPackages_latest.clang
          llvmPackages_latest.lld
          bpftools
          libbpf
          elfutils
          zlib
        ];

        # Rust build dependencies
        rustDeps = with pkgs; [
          rustToolchain
          cargo-generate
          cargo-watch
          cargo-edit
        ];

        # System dependencies
        systemDeps = with pkgs; [
          pkg-config
          openssl
          dbus
          networkmanager
          wireguard-tools
        ];

        # Static build dependencies (musl)
        muslDeps = with pkgs.pkgsStatic; [
          openssl
        ];

        # Development tools
        devTools = with pkgs; [
          # Debugging and inspection
          gdb
          strace

          # Network tools for testing
          iproute2
          iputils
          tcpdump
          wireshark-cli

          # General utilities
          jq
          ripgrep
          fd
        ];

        # Use bpf-linker from nixpkgs (version 0.9.15)
        bpf-linker = pkgs.bpf-linker;

        # Create a custom Rust platform using the nightly toolchain
        rustPlatformNightly = pkgs.makeRustPlatform {
          cargo = rustToolchain;
          rustc = rustToolchain;
        };

      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = ebpfDeps ++ rustDeps ++ systemDeps ++ muslDeps ++ devTools ++ [ bpf-linker ];

          shellHook = ''
            echo "🦀 WireGuard On-Demand Development Environment"
            echo ""
            echo "Rust toolchain: $(rustc --version)"
            echo "Cargo: $(cargo --version)"
            echo "LLVM: $(llvm-config --version)"
            echo "bpf-linker: $(bpf-linker --version 2>/dev/null || echo 'installed')"
            echo ""
            echo "📋 Available commands:"
            echo "  cargo xtask build-ebpf  - Build eBPF program"
            echo "  cargo build --release   - Build userspace daemon"
            echo "  cargo build --release --target x86_64-unknown-linux-musl - Static binary"
            echo "  sudo bpftool prog list  - List loaded eBPF programs"
            echo "  sudo tc filter show dev wlan0 egress - Show TC filters"
            echo ""
            echo "⚠️  Note: Requires Linux kernel 5.8+ with BTF support"
            echo "   Check: ls /sys/kernel/btf/vmlinux"
            echo ""

            # Set up Rust environment
            export RUST_SRC_PATH="${rustToolchain}/lib/rustlib/src/rust/library"
            export LIBCLANG_PATH="${pkgs.llvmPackages_latest.libclang.lib}/lib"

            # eBPF-specific environment
            export BPF_CLANG="${pkgs.llvmPackages_latest.clang}/bin/clang"
            export BPF_CFLAGS="-I${pkgs.libbpf}/include"

            # Musl build environment
            export PKG_CONFIG_ALLOW_CROSS=1
            export PKG_CONFIG_PATH="${pkgs.pkgsStatic.openssl.dev}/lib/pkgconfig"
          '';

          # Environment variables for build
          LIBCLANG_PATH = "${pkgs.llvmPackages_latest.libclang.lib}/lib";
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          PKG_CONFIG_ALLOW_CROSS = "1";
        };

        # Package definition (for building the project)
        # Note: Using stdenv.mkDerivation instead of buildRustPackage because
        # eBPF compilation with -Z build-std requires network access to download
        # standard library dependencies that aren't in Cargo.lock
        packages.default = pkgs.stdenv.mkDerivation {
          pname = "wg-ondemand";
          version = "0.1.0";

          src = ./.;

          nativeBuildInputs = [
            rustToolchain
            pkgs.pkg-config
            bpf-linker
          ] ++ ebpfDeps;

          buildInputs = systemDeps;

          # Set environment variables for build
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          LIBCLANG_PATH = "${pkgs.llvmPackages_latest.libclang.lib}/lib";

          buildPhase = ''
            # Set up cargo home in build directory
            export CARGO_HOME=$(mktemp -d)

            # Build eBPF program
            cargo xtask build-ebpf --release

            # Build userspace daemon
            cargo build --release --package wg-ondemand
          '';

          installPhase = ''
            mkdir -p $out/bin
            cp target/release/wg-ondemand $out/bin/
          '';

          meta = with pkgs.lib; {
            description = "eBPF-based on-demand WireGuard VPN activation daemon";
            homepage = "https://github.com/vly/wg-ondemand";
            license = licenses.mit;
            maintainers = [ ];
            platforms = platforms.linux;
          };
        };

        # NixOS module (optional, for system-wide installation)
        nixosModules.default = { config, lib, pkgs, ... }:
          with lib;
          let
            cfg = config.services.wg-ondemand;
          in {
            options.services.wg-ondemand = {
              enable = mkEnableOption "WireGuard on-demand activation daemon";

              configFile = mkOption {
                type = types.path;
                default = /etc/wg-ondemand/config.toml;
                description = "Path to configuration file";
              };

              package = mkOption {
                type = types.package;
                default = self.packages.${system}.default;
                description = "wg-ondemand package to use";
              };
            };

            config = mkIf cfg.enable {
              systemd.services.wg-ondemand = {
                description = "WireGuard On-Demand Activation Daemon";
                after = [ "network-online.target" "NetworkManager.service" ];
                wants = [ "network-online.target" ];
                wantedBy = [ "multi-user.target" ];

                serviceConfig = {
                  Type = "simple";
                  ExecStart = "${cfg.package}/bin/wg-ondemand --config ${cfg.configFile}";
                  Restart = "on-failure";
                  RestartSec = "5s";

                  # Security hardening
                  CapabilityBoundingSet = "CAP_NET_ADMIN CAP_BPF CAP_PERFMON";
                  AmbientCapabilities = "CAP_NET_ADMIN CAP_BPF CAP_PERFMON";
                  NoNewPrivileges = true;
                  PrivateTmp = true;
                  ProtectSystem = "strict";
                  ProtectHome = true;
                  ReadWritePaths = [ "/run" "/sys/fs/bpf" ];
                };
              };

              # Ensure NetworkManager is enabled
              services.networkmanager.enable = mkDefault true;

              # Ensure WireGuard is available
              networking.wireguard.enable = mkDefault true;
            };
          };
      }
    );
}
