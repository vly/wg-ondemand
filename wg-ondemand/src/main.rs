// WireGuard On-Demand Activation Daemon

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::mpsc;
use tokio::time::interval;
use wg_ondemand::{
    config::{self, load_config},
    ebpf_loader::EbpfManager,
    route_manager::RouteManager,
    ssid_monitor::{NetworkEvent, SsidMonitor},
    state::{StateAction, StateCommand, StateManager},
    state_file,
    types::{TrafficEvent, TunnelState},
    wg_controller::{self, WgController},
};

// Configuration constants for main event loop
/// Size of the channel buffer for network events (SSID changes)
const NETWORK_EVENT_CHANNEL_SIZE: usize = 32;

/// Size of the channel buffer for state commands
const STATE_COMMAND_CHANNEL_SIZE: usize = 32;

/// Interval for checking tunnel idle timeout (seconds)
/// Should be frequent enough to detect idle timeouts accurately
const IDLE_CHECK_INTERVAL_SECS: u64 = 60;

/// Interval for polling eBPF ringbuffer events (milliseconds)
/// 1 second balances responsiveness with battery efficiency
/// This reduces CPU wakeups from 864K/day to 86K/day
const EBPF_POLL_INTERVAL_MILLIS: u64 = 1000;

/// Maximum number of retry attempts for eBPF attachment when interface has no IP
const MAX_ATTACHMENT_RETRIES: u8 = 5;

/// Initial retry delay in seconds (exponential backoff: 1s, 2s, 4s, 8s, 16s)
const INITIAL_RETRY_DELAY_SECS: u64 = 1;

#[derive(Parser)]
#[command(name = "wg-ondemand")]
#[command(about = "On-demand WireGuard VPN activation daemon", long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "/etc/wg-ondemand/config.toml")]
    config: PathBuf,
}

/// Get the IPv4 address assigned to a network interface
/// Returns the IP as u32 in network byte order (big endian), or None if no IPv4 address assigned
fn get_interface_ip(interface: &str) -> Result<Option<u32>> {
    let interfaces = if_addrs::get_if_addrs().context("Failed to get interface addresses")?;

    for iface in interfaces {
        if iface.name == interface {
            if let if_addrs::IfAddr::V4(ipv4) = iface.addr {
                let ip_u32 = u32::from_be_bytes(ipv4.ip.octets());
                return Ok(Some(ip_u32));
            }
        }
    }

    Ok(None)
}

/// Auto-detect the active network interface
/// Attempts to find a wireless interface, falling back to the default route interface
async fn auto_detect_interface() -> Result<String> {
    // First, try to find wireless interfaces by checking /sys/class/net/*/wireless
    if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
        for entry in entries.flatten() {
            let iface_name = entry.file_name();
            let wireless_path = format!("/sys/class/net/{}/wireless", iface_name.to_string_lossy());
            if std::path::Path::new(&wireless_path).exists() {
                log::info!(
                    "Auto-detected wireless interface: {}",
                    iface_name.to_string_lossy()
                );
                return Ok(iface_name.to_string_lossy().to_string());
            }
        }
    }

    // Fall back to finding the default route interface
    log::info!("No wireless interface found, detecting default route interface...");
    let output = tokio::process::Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .await
        .context("Failed to execute 'ip route show default'")?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // Parse output like: "default via 192.168.1.1 dev eth0 proto dhcp metric 100"
        for line in stdout.lines() {
            if let Some(dev_pos) = line.find(" dev ") {
                let after_dev = &line[dev_pos + 5..];
                if let Some(iface) = after_dev.split_whitespace().next() {
                    log::info!("Auto-detected default route interface: {}", iface);
                    return Ok(iface.to_string());
                }
            }
        }
    }

    anyhow::bail!(
        "Could not auto-detect network interface. Please specify monitor_interface in config."
    )
}

/// Spawn a background task to retry eBPF attachment with exponential backoff
/// Returns true if retry task was spawned, false if one is already running
fn spawn_attachment_retry_task(
    interface: String,
    state_tx: mpsc::Sender<StateCommand>,
    retry_in_progress: Arc<AtomicBool>,
) {
    // Check if retry is already in progress
    if retry_in_progress.swap(true, Ordering::SeqCst) {
        log::debug!("eBPF attachment retry already in progress, skipping");
        return;
    }

    log::info!(
        "Spawning eBPF attachment retry task (will retry up to {} times with exponential backoff)",
        MAX_ATTACHMENT_RETRIES
    );

    tokio::spawn(async move {
        let mut delay_secs = INITIAL_RETRY_DELAY_SECS;

        for attempt in 1..=MAX_ATTACHMENT_RETRIES {
            // Wait before retry (exponential backoff)
            log::info!(
                "eBPF attachment retry attempt {}/{} in {}s...",
                attempt,
                MAX_ATTACHMENT_RETRIES,
                delay_secs
            );
            tokio::time::sleep(Duration::from_secs(delay_secs)).await;

            // Check if interface now has an IP address
            match get_interface_ip(&interface) {
                Ok(Some(_ip)) => {
                    log::info!(
                        "Interface {} now has IP address, triggering eBPF attachment",
                        interface
                    );
                    // Send retry command to trigger attachment
                    if let Err(e) = state_tx.send(StateCommand::RetryEbpfAttachment).await {
                        log::error!("Failed to send retry command: {}", e);
                    }
                    // Success - stop retrying
                    retry_in_progress.store(false, Ordering::SeqCst);
                    return;
                }
                Ok(None) => {
                    log::debug!(
                        "Interface {} still has no IP address (attempt {}/{})",
                        interface,
                        attempt,
                        MAX_ATTACHMENT_RETRIES
                    );
                }
                Err(e) => {
                    log::warn!("Failed to check interface IP during retry: {}", e);
                }
            }

            // Exponential backoff: double the delay for next attempt
            delay_secs *= 2;
        }

        // All retries exhausted
        log::error!(
            "Failed to attach eBPF after {} attempts. Interface {} still has no IP address. \
            Consider restarting the daemon after DHCP completes.",
            MAX_ATTACHMENT_RETRIES,
            interface
        );
        retry_in_progress.store(false, Ordering::SeqCst);
    });
}

/// Perform graceful shutdown: clean up resources before exiting
#[allow(unused_mut)]
async fn graceful_shutdown(
    mut ebpf_manager: EbpfManager,
    mut wg_controller: WgController,
    tunnel_state: TunnelState,
) -> Result<()> {
    log::info!("Shutting down gracefully...");

    // Detach eBPF program if attached
    if ebpf_manager.is_attached() {
        log::info!("Detaching eBPF program...");
        if let Err(e) = ebpf_manager.detach() {
            log::error!("Failed to detach eBPF program: {}", e);
        }
    }

    // Bring down tunnel if active
    if tunnel_state == TunnelState::Active || tunnel_state == TunnelState::Activating {
        log::info!("Bringing down WireGuard tunnel...");
        if let Err(e) = wg_controller.bring_down().await {
            log::error!("Failed to bring down tunnel: {}", e);
        }
    }

    log::info!("Shutdown complete");
    Ok(())
}

fn main() -> Result<()> {
    // Build custom Tokio runtime with limited thread pool
    // 2 threads is sufficient: 1 for main loop, 1 for D-Bus monitor + process spawns
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .thread_name("wg-ondemand")
        .thread_stack_size(2 * 1024 * 1024) // 2MB stack (vs 8MB default)
        .enable_time()
        .enable_io()
        .build()?;

    runtime.block_on(async_main())
}

async fn async_main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Load configuration
    let config = load_config(&args.config)
        .with_context(|| format!("Failed to load config from {:?}", args.config))?;

    // Initialize logging
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(&config.general.log_level),
    )
    .init();

    log::info!("Starting wg-ondemand daemon");

    // Log SSID filtering configuration
    if config.general.target_ssids.0.is_empty() && config.general.exclude_ssids.is_empty() {
        log::info!("SSID filtering: monitoring ALL networks");
    } else if config.general.target_ssids.0.is_empty() {
        log::info!(
            "SSID filtering: all networks EXCEPT {:?}",
            config.general.exclude_ssids
        );
    } else if config.general.exclude_ssids.is_empty() {
        log::info!("SSID filtering: ONLY {:?}", config.general.target_ssids.0);
    } else {
        log::info!(
            "SSID filtering: {:?} EXCEPT {:?}",
            config.general.target_ssids.0,
            config.general.exclude_ssids
        );
    }

    log::info!("WireGuard interface: {}", config.general.wg_interface);
    log::info!("Idle timeout: {}s", config.general.idle_timeout);
    log::info!("Target subnets: {}", config.subnets.ranges.join(", "));

    // Initialize components
    let mut wg_controller = WgController::new(
        config.general.wg_interface.clone(),
        config.general.nm_connection.clone(),
    )
    .context("Failed to create WireGuard controller")?;
    let mut state_manager = StateManager::new(config.general.idle_timeout);

    // Determine monitor interface (auto-detect if not specified)
    let monitor_iface = match config.general.monitor_interface.clone() {
        Some(iface) => {
            // Validate configured interface name
            wg_controller::validate_interface_name(&iface)
                .context("Configured monitor interface has invalid name")?;
            iface
        }
        None => {
            log::info!("Auto-detecting network interface...");
            let detected = auto_detect_interface()
                .await
                .context("Failed to auto-detect network interface")?;
            // Validate auto-detected interface name (defense-in-depth)
            wg_controller::validate_interface_name(&detected)
                .context("Auto-detected interface has invalid name")?;
            detected
        }
    };

    log::info!("Monitoring interface: {}", monitor_iface);

    // Load eBPF program (includes interface existence validation)
    let mut ebpf_manager = EbpfManager::load(&monitor_iface, &config.subnets.ranges)
        .context("Failed to load eBPF program")?;

    // Create route manager for traffic detection
    let mut route_manager = RouteManager::new(monitor_iface.clone());

    // Create SSID monitor
    let ssid_monitor = SsidMonitor::new(
        config.general.target_ssids.0.clone(),
        config.general.exclude_ssids.clone(),
    )
    .await
    .context("Failed to create SSID monitor")?;

    // Channels for communication
    let (network_tx, mut network_rx) = mpsc::channel::<NetworkEvent>(NETWORK_EVENT_CHANNEL_SIZE);
    let (state_tx, mut state_rx) = mpsc::channel::<StateCommand>(STATE_COMMAND_CHANNEL_SIZE);

    // Track whether an eBPF attachment retry task is running
    let retry_in_progress = Arc::new(AtomicBool::new(false));

    // Check initial SSID and tunnel state before spawning monitor
    let initial_connected = ssid_monitor.is_connected_to_target().await.unwrap_or(false);
    let tunnel_already_up = wg_controller.is_up().await;

    if initial_connected {
        if tunnel_already_up {
            log::info!(
                "Already connected to monitored network and tunnel is up, transitioning to Active state"
            );
            // State sequence: Inactive -> Monitoring -> Active (tunnel already up)
            state_tx.send(StateCommand::StartMonitoring).await?;
            state_tx.send(StateCommand::TunnelAlreadyUp).await?;
        } else {
            log::info!("Already connected to monitored network, starting monitoring");
            state_tx.send(StateCommand::StartMonitoring).await?;
        }
    }

    // Spawn SSID monitor task
    // Store the handle so we can monitor it for failures
    let mut monitor_handle = tokio::spawn(async move {
        if let Err(e) = ssid_monitor.monitor(network_tx).await {
            log::error!("SSID monitor error: {}", e);
            // Return error to signal failure
            Err::<(), anyhow::Error>(e)
        } else {
            Ok(())
        }
    });

    // Idle check timer
    let mut idle_timer = interval(Duration::from_secs(IDLE_CHECK_INTERVAL_SECS));

    // eBPF event check timer
    let mut ebpf_timer = interval(Duration::from_millis(EBPF_POLL_INTERVAL_MILLIS));

    log::info!("Daemon started successfully");

    // Set up signal handlers for graceful shutdown
    let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate())
        .context("Failed to set up SIGTERM handler")?;
    let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt())
        .context("Failed to set up SIGINT handler")?;

    // Track current SSID for state file updates
    let mut current_ssid: Option<String> = None;

    // Write initial state
    let _ = state_file::write_state(state_manager.state(), None);

    // Main event loop
    loop {
        tokio::select! {
            // Shutdown signals
            _ = sigterm.recv() => {
                log::info!("Received SIGTERM");
                break;
            }
            _ = sigint.recv() => {
                log::info!("Received SIGINT");
                break;
            }

            // Monitor SSID monitor task for failures (fail-fast approach)
            monitor_result = &mut monitor_handle => {
                match monitor_result {
                    Ok(Ok(())) => {
                        log::error!("SSID monitor task exited unexpectedly");
                    }
                    Ok(Err(e)) => {
                        log::error!("SSID monitor task failed: {}", e);
                    }
                    Err(e) => {
                        log::error!("SSID monitor task panicked: {}", e);
                    }
                }
                anyhow::bail!("SSID monitor task terminated, aborting daemon for systemd restart");
            }

            // Network events (SSID changes)
            Some(event) = network_rx.recv() => {
                match event {
                    NetworkEvent::ConnectedToTarget(ssid) => {
                        log::info!("Network event: Connected to target SSID");
                        current_ssid = if ssid.is_empty() { None } else { Some(ssid) };
                        state_tx.send(StateCommand::StartMonitoring).await?;
                    }
                    NetworkEvent::Disconnected => {
                        log::info!("Network event: Disconnected from target SSID");
                        current_ssid = None;
                        // Reset retry flag so a new retry can be spawned on next connection
                        retry_in_progress.store(false, Ordering::SeqCst);
                        state_tx.send(StateCommand::StopMonitoring).await?;
                    }
                }
            }

            // State commands
            Some(cmd) = state_rx.recv() => {
                let action = state_manager.handle_command(cmd);

                match action {
                    StateAction::AttachEbpf => {
                        // Check if local IP conflicts with configured subnets
                        match get_interface_ip(&monitor_iface) {
                            Ok(Some(local_ip)) => {
                                // Check if local IP is within any configured subnet
                                match config::ip_in_subnets(local_ip, &config.subnets.ranges) {
                                    Ok(true) => {
                                        let ip_bytes = local_ip.to_be_bytes();
                                        log::warn!(
                                            "Local IP {}.{}.{}.{} conflicts with configured subnet ranges. \
                                            Skipping eBPF attachment to avoid routing loops. \
                                            This network appears to use the same IP range as your home network.",
                                            ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]
                                        );
                                        // Don't attach eBPF - would cause routing issues
                                    }
                                    Ok(false) => {
                                        // Safe to attach - local IP doesn't conflict
                                        log::info!("Action: Attaching eBPF program and adding monitoring routes");

                                        // Add monitoring routes first
                                        if let Err(e) = route_manager.add_routes(&config.subnets.ranges).await {
                                            log::error!("Failed to add monitoring routes: {}", e);
                                        }

                                        // Then attach eBPF
                                        if let Err(e) = ebpf_manager.attach() {
                                            log::error!("Failed to attach eBPF: {}", e);
                                        } else {
                                            log::info!("eBPF program attached and monitoring traffic");
                                        }
                                    }
                                    Err(e) => {
                                        log::error!("Failed to check IP subnet overlap: {}", e);
                                    }
                                }
                            }
                            Ok(None) => {
                                log::warn!(
                                    "Interface {} has no IPv4 address yet. Will retry with exponential backoff.",
                                    monitor_iface
                                );
                                // Spawn retry task to check for IP address and retry attachment
                                spawn_attachment_retry_task(
                                    monitor_iface.clone(),
                                    state_tx.clone(),
                                    retry_in_progress.clone(),
                                );
                            }
                            Err(e) => {
                                log::error!("Failed to get interface IP: {}", e);
                            }
                        }
                    }

                    StateAction::DetachEbpf => {
                        log::info!("Action: Detaching eBPF program and removing monitoring routes");

                        // Detach eBPF first
                        if let Err(e) = ebpf_manager.detach() {
                            log::error!("Failed to detach eBPF: {}", e);
                        }

                        // Then remove routes
                        if let Err(e) = route_manager.remove_routes().await {
                            log::error!("Failed to remove monitoring routes: {}", e);
                        }
                    }

                    StateAction::ActivateTunnel => {
                        log::info!("Action: Activating WireGuard tunnel");
                        match wg_controller.bring_up().await {
                            Ok(_) => {
                                // Reset activity tracking when tunnel comes up
                                wg_controller.reset_activity();
                                state_tx.send(StateCommand::TunnelUp).await?;
                            }
                            Err(e) => {
                                log::error!("Failed to bring up tunnel: {}", e);
                            }
                        }
                    }

                    StateAction::DeactivateTunnel => {
                        log::info!("Action: Deactivating WireGuard tunnel");
                        match wg_controller.bring_down().await {
                            Ok(_) => {
                                state_tx.send(StateCommand::TunnelDown).await?;
                            }
                            Err(e) => {
                                log::error!("Failed to bring down tunnel: {}", e);
                            }
                        }
                    }

                    StateAction::None => {}
                }

                // Write state file after any state transition
                let ssid_ref = current_ssid.as_deref();
                if let Err(e) = state_file::write_state(state_manager.state(), ssid_ref) {
                    log::warn!("Failed to write state file: {}", e);
                }
            }

            // eBPF events (traffic detection) - check periodically
            _ = ebpf_timer.tick() => {
                // Poll cached ring buffer (no map lookup overhead)
                if let Some(rb) = ebpf_manager.poll_events() {
                    while let Some(data) = rb.next() {
                            if data.len() == std::mem::size_of::<TrafficEvent>() {
                                // Use read_unaligned to handle potentially unaligned data from ringbuffer
                                // This prevents undefined behavior on architectures with strict alignment requirements
                                let event: TrafficEvent = unsafe {
                                    std::ptr::read_unaligned(data.as_ptr() as *const TrafficEvent)
                                };

                                let ip_bytes = event.dest_ip.to_be_bytes();
                                log::debug!(
                                    "Traffic detected: {}.{}.{}.{}:{} (proto={})",
                                    ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3],
                                    event.dest_port,
                                    event.protocol
                                );

                                // Notify state manager (apply backpressure - never silently drop events)
                                // If channel fills, state manager is broken and we should fail-fast
                                if let Err(e) = state_tx.send(StateCommand::TrafficDetected).await {
                                    log::error!("State manager channel closed: {}", e);
                                    anyhow::bail!("State manager task died unexpectedly");
                                }
                            }
                        }
                    }
                }

            // Idle timer tick - check for tunnel inactivity
            _ = idle_timer.tick() => {
                // Only check idle when tunnel is active
                if state_manager.state() == TunnelState::Active {
                    // Check for WireGuard tunnel activity
                    match wg_controller.check_activity().await {
                        Ok(has_activity) => {
                            if has_activity {
                                log::debug!("Tunnel activity detected");
                            }
                        }
                        Err(e) => {
                            log::warn!("Failed to check WireGuard activity: {}", e);
                        }
                    }

                    // Check if idle timeout reached
                    if let Some(idle_duration) = wg_controller.idle_duration() {
                        let idle_timeout = state_manager.idle_timeout();
                        if idle_duration > idle_timeout {
                            log::info!(
                                "Idle timeout reached ({:.0}s of {:.0}s)",
                                idle_duration.as_secs_f32(),
                                idle_timeout.as_secs_f32()
                            );
                            // Trigger deactivation via state manager
                            state_tx.send(StateCommand::IdleTimeout).await?;
                        }
                    }
                }
            }
        }
    }

    // Clean up state file
    state_file::cleanup();

    // Perform graceful shutdown
    graceful_shutdown(ebpf_manager, wg_controller, state_manager.state()).await?;

    Ok(())
}
