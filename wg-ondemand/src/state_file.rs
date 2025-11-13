// State file writer for external monitoring
//!
//! Writes current daemon state to a file for consumption by external tools
//! like wg-ondemand-ctl and waybar widgets.

use crate::types::TunnelState;
use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

const STATE_FILE: &str = "/run/wg-ondemand/state";
const STATE_DIR: &str = "/run/wg-ondemand";

/// Write current state to state file
pub fn write_state(state: TunnelState, ssid: Option<&str>) -> Result<()> {
    // Create directory if it doesn't exist
    let state_dir = Path::new(STATE_DIR);
    if !state_dir.exists() {
        fs::create_dir_all(state_dir).context("Failed to create state directory")?;
    }

    // Get current timestamp
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Convert state to string
    let state_str = match state {
        TunnelState::Inactive => "inactive",
        TunnelState::Monitoring => "monitoring",
        TunnelState::Activating => "activating",
        TunnelState::Active => "connected",
        TunnelState::Deactivating => "deactivating",
    };

    // Write state file
    let content = format!(
        "STATE={}\nSSID={}\nTIMESTAMP={}\n",
        state_str,
        ssid.unwrap_or(""),
        timestamp
    );

    fs::write(STATE_FILE, content).context("Failed to write state file")?;

    Ok(())
}

/// Remove state file on shutdown
pub fn cleanup() {
    let _ = fs::remove_file(STATE_FILE);
}
