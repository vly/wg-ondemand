// eBPF program loader and manager

//! eBPF program management for traffic monitoring
//!
//! This module manages the lifecycle of the eBPF traffic monitoring program,
//! including loading, attaching to network interfaces, and reading events
//! from the ringbuffer.

use crate::config::parse_cidr;
use anyhow::{Context, Result};
use aya::maps::RingBuf;
use aya::{
    include_bytes_aligned,
    maps::{Array, MapData},
    programs::{tc::SchedClassifierLinkId, SchedClassifier, TcAttachType},
    Bpf,
};

/// Manages the lifecycle of the eBPF program
pub struct EbpfManager {
    ebpf: Bpf,
    interface: String,
    link_id: Option<SchedClassifierLinkId>,
    ringbuf: Option<RingBuf<MapData>>,
}

impl EbpfManager {
    /// Load eBPF program and configure subnet map
    pub fn load(interface: &str, subnets: &[String]) -> Result<Self> {
        // Load eBPF program from embedded bytes
        let mut ebpf = Bpf::load(include_bytes_aligned!(
            "../../target/bpfel-unknown-none/release/wg-ondemand-ebpf"
        ))
        .context("Failed to load eBPF program")?;

        log::info!("Loaded eBPF program successfully");

        // Configure subnet map
        let mut subnet_map: Array<_, [u32; 2]> = Array::try_from(
            ebpf.map_mut("SUBNETS")
                .context("Failed to get SUBNETS map")?,
        )?;

        // Sentinel value for empty slots (must match eBPF code)
        const EMPTY_SENTINEL: u32 = 0xFFFFFFFF;

        for (i, subnet_cidr) in subnets.iter().enumerate() {
            if i >= 16 {
                log::warn!("Maximum 16 subnets supported, ignoring extras");
                break;
            }

            let (network, mask) = parse_cidr(subnet_cidr)?;
            subnet_map.set(i as u32, [network, mask], 0)?;
            log::info!(
                "Configured subnet {}: {} (network=0x{:08x} mask=0x{:08x})",
                i,
                subnet_cidr,
                network,
                mask
            );
        }

        // Initialize remaining slots with sentinel value to mark them as empty
        // This allows 0.0.0.0/0 (match all) to be a valid subnet configuration
        for i in subnets.len()..16 {
            subnet_map.set(i as u32, [EMPTY_SENTINEL, EMPTY_SENTINEL], 0)?;
        }

        // Load the program into the kernel once (can be attached/detached multiple times)
        let program: &mut SchedClassifier = ebpf
            .program_mut("wg_ondemand_tc")
            .context("Failed to find eBPF program 'wg_ondemand_tc'")?
            .try_into()
            .context("Failed to convert to SchedClassifier")?;

        program
            .load()
            .context("Failed to load eBPF program into kernel")?;

        log::info!("Loaded eBPF program into kernel");

        Ok(Self {
            ebpf,
            interface: interface.to_string(),
            link_id: None,
            ringbuf: None,
        })
    }

    /// Attach eBPF program to TC egress hook
    pub fn attach(&mut self) -> Result<()> {
        if self.link_id.is_some() {
            log::warn!("eBPF program already attached");
            return Ok(());
        }

        // Get TC program (already loaded when Bpf object was created)
        let program: &mut SchedClassifier = self
            .ebpf
            .program_mut("wg_ondemand_tc")
            .context("Failed to find eBPF program 'wg_ondemand_tc'")?
            .try_into()
            .context("Failed to convert to SchedClassifier")?;

        // Attach to TC egress hook and store the link ID
        let link_id = match program.attach(&self.interface, TcAttachType::Egress) {
            Ok(id) => id,
            Err(e) => {
                log::error!("TC attach error: {:?}", e);
                anyhow::bail!("Failed to attach to TC egress on {}: {}", self.interface, e);
            }
        };

        self.link_id = Some(link_id);

        // Cache ring buffer reference on attach to avoid repeated map lookups
        let rb = RingBuf::try_from(
            self.ebpf
                .take_map("EVENTS")
                .context("Failed to get EVENTS ringbuf")?,
        )
        .context("Failed to convert to RingBuf")?;
        self.ringbuf = Some(rb);

        log::info!("Attached eBPF program to {} egress", self.interface);
        Ok(())
    }

    /// Detach eBPF program from TC hook
    pub fn detach(&mut self) -> Result<()> {
        if let Some(link_id) = self.link_id.take() {
            let program: &mut SchedClassifier = self
                .ebpf
                .program_mut("wg_ondemand_tc")
                .context("Failed to find program")?
                .try_into()
                .context("Failed to convert to SchedClassifier")?;

            program
                .detach(link_id)
                .context("Failed to detach eBPF program")?;

            // Clear cached ring buffer on detach
            self.ringbuf = None;

            log::info!("Detached eBPF program from {}", self.interface);
        }
        Ok(())
    }

    /// Get mutable access to cached ring buffer for reading events
    /// Returns None if eBPF program is not attached
    ///
    /// This avoids the overhead of repeated map lookups (86K/day)
    pub fn poll_events(&mut self) -> Option<&mut RingBuf<MapData>> {
        self.ringbuf.as_mut()
    }

    /// Check if eBPF program is currently attached
    pub fn is_attached(&self) -> bool {
        self.link_id.is_some()
    }
}

impl Drop for EbpfManager {
    fn drop(&mut self) {
        let _ = self.detach();
    }
}
