use anyhow::{Context, Result};
use clap::Parser;
use std::process::Command;

#[derive(Parser)]
enum Args {
    /// Build the eBPF program
    BuildEbpf {
        /// Build in release mode
        #[clap(long)]
        release: bool,
    },
}

fn main() -> Result<()> {
    let args = Args::parse();

    match args {
        Args::BuildEbpf { release } => build_ebpf(release),
    }
}

fn build_ebpf(release: bool) -> Result<()> {
    let mut cmd = Command::new("cargo");
    cmd.env("RUSTUP_TOOLCHAIN", "nightly");
    cmd.args([
        "build",
        "--package",
        "wg-ondemand-ebpf",
        "--target",
        "bpfel-unknown-none",
        "-Z",
        "build-std=core",
    ]);

    if release {
        cmd.arg("--release");
    }

    let status = cmd.status().context("Failed to build eBPF program")?;

    if !status.success() {
        anyhow::bail!("eBPF build failed");
    }

    println!("eBPF program built successfully");
    Ok(())
}
