#![no_std]
#![no_main]

use aya_ebpf::{
    bindings::TC_ACT_OK,
    macros::{classifier, map},
    maps::{Array, RingBuf},
    programs::TcContext,
};
use aya_log_ebpf::info;
use network_types::{
    eth::{EthHdr, EtherType},
    ip::{IpProto, Ipv4Hdr},
    tcp::TcpHdr,
    udp::UdpHdr,
};

/// Ringbuf for sending events to userspace
/// 16KB = 1024 events, provides 10x safety margin for realistic traffic bursts
/// At 1s polling interval and 100 packets/sec peak: 1.6KB needed, 16KB provides buffer
#[map]
static EVENTS: RingBuf = RingBuf::with_byte_size(16 * 1024, 0);

/// Array to store subnet configurations (network, mask pairs)
/// Max 16 subnets, each entry is [network_u32, mask_u32]
#[map]
static SUBNETS: Array<[u32; 2]> = Array::with_max_entries(16, 0);

/// Event structure matching userspace definition
#[repr(C)]
struct TrafficEvent {
    timestamp: u64,
    dest_ip: u32,
    dest_port: u16,
    protocol: u8,
    _padding: u8,
}

#[classifier]
pub fn wg_ondemand_tc(ctx: TcContext) -> i32 {
    match try_wg_ondemand_tc(ctx) {
        Ok(ret) => ret,
        Err(_) => TC_ACT_OK,
    }
}

fn try_wg_ondemand_tc(ctx: TcContext) -> Result<i32, ()> {
    // Parse Ethernet header
    let ethhdr: EthHdr = ctx.load(0).map_err(|_| ())?;

    // Only process IPv4
    match ethhdr.ether_type {
        EtherType::Ipv4 => {}
        _ => return Ok(TC_ACT_OK),
    }

    // Parse IPv4 header
    let ipv4hdr: Ipv4Hdr = ctx.load(EthHdr::LEN).map_err(|_| ())?;
    let dest_ip = u32::from_be(ipv4hdr.dst_addr);

    // Check if destination matches any configured subnet
    if !is_target_subnet(dest_ip) {
        return Ok(TC_ACT_OK);
    }

    // Get destination port based on protocol
    let dest_port = match ipv4hdr.proto {
        IpProto::Tcp => {
            let tcphdr: TcpHdr = ctx.load(EthHdr::LEN + Ipv4Hdr::LEN).map_err(|_| ())?;
            u16::from_be(tcphdr.dest)
        }
        IpProto::Udp => {
            let udphdr: UdpHdr = ctx.load(EthHdr::LEN + Ipv4Hdr::LEN).map_err(|_| ())?;
            u16::from_be(udphdr.dest)
        }
        _ => 0,
    };

    // Log traffic detection (visible with aya-log)
    info!(
        &ctx,
        "Traffic detected to {:i}:{} proto={}", dest_ip, dest_port, ipv4hdr.proto as u8
    );

    // Emit event to userspace
    if let Some(mut entry) = EVENTS.reserve::<TrafficEvent>(0) {
        let event = TrafficEvent {
            timestamp: unsafe { aya_ebpf::helpers::bpf_ktime_get_ns() },
            dest_ip,
            dest_port,
            protocol: ipv4hdr.proto as u8,
            _padding: 0,
        };

        unsafe {
            core::ptr::write_unaligned(entry.as_mut_ptr() as *mut TrafficEvent, event);
        }
        entry.submit(0);
    }

    Ok(TC_ACT_OK)
}

/// Check if the given IP matches any configured subnet
fn is_target_subnet(ip: u32) -> bool {
    // Sentinel value for empty slots: 0xFFFFFFFF/0xFFFFFFFF
    // This allows 0.0.0.0/0 (match all) to be a valid configuration
    const EMPTY_SENTINEL: u32 = 0xFFFFFFFF;

    // Iterate through configured subnets
    for i in 0..16 {
        if let Some(subnet) = SUBNETS.get(i) {
            let network = subnet[0];
            let mask = subnet[1];

            // Check if this slot is empty (sentinel value)
            if network == EMPTY_SENTINEL && mask == EMPTY_SENTINEL {
                continue;
            }

            // Check if IP matches this subnet
            if (ip & mask) == network {
                return true;
            }
        }
    }
    false
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe { core::hint::unreachable_unchecked() }
}
