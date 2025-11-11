// State manager for tunnel lifecycle

//! State machine for tunnel lifecycle management
//!
//! This module implements the state machine that coordinates tunnel activation
//! and deactivation based on network events, traffic detection, and idle timeouts.

use crate::types::TunnelState;
use std::time::Duration;

/// Commands that trigger state transitions
#[derive(Debug, Clone, Copy)]
pub enum StateCommand {
    /// Start monitoring (connected to target SSID)
    StartMonitoring,
    /// Stop monitoring (disconnected from target SSID)
    StopMonitoring,
    /// Traffic detected to target subnet
    TrafficDetected,
    /// Tunnel successfully brought up
    TunnelUp,
    /// Tunnel brought down
    TunnelDown,
    /// Idle timeout reached (no tunnel activity)
    IdleTimeout,
    /// Tunnel already up at startup (detected during initialization)
    TunnelAlreadyUp,
}

/// Actions to take in response to state changes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StateAction {
    /// Activate the WireGuard tunnel
    ActivateTunnel,
    /// Deactivate the WireGuard tunnel
    DeactivateTunnel,
    /// Attach eBPF program
    AttachEbpf,
    /// Detach eBPF program
    DetachEbpf,
    /// No action needed
    None,
}

/// State machine manager
pub struct StateManager {
    state: TunnelState,
    idle_timeout: Duration,
}

impl StateManager {
    /// Create a new state manager
    pub fn new(idle_timeout_secs: u64) -> Self {
        Self {
            state: TunnelState::Inactive,
            idle_timeout: Duration::from_secs(idle_timeout_secs),
        }
    }

    /// Handle a state command and return the action to take
    pub fn handle_command(&mut self, cmd: StateCommand) -> StateAction {
        log::debug!("State: {:?}, Command: {:?}", self.state, cmd);

        match (self.state, cmd) {
            // Start monitoring when connected to target SSID
            (TunnelState::Inactive, StateCommand::StartMonitoring) => {
                log::info!("Starting monitoring (connected to target SSID)");
                self.state = TunnelState::Monitoring;
                StateAction::AttachEbpf
            }

            // Stop monitoring when disconnected - tear down everything
            (TunnelState::Monitoring, StateCommand::StopMonitoring) => {
                log::info!("Stopping monitoring (disconnected from target SSID)");
                self.state = TunnelState::Inactive;
                StateAction::DetachEbpf
            }

            (TunnelState::Active, StateCommand::StopMonitoring) => {
                log::info!("Disconnected from target SSID, deactivating tunnel");
                self.state = TunnelState::Deactivating;
                // First deactivate tunnel, then detach eBPF
                StateAction::DeactivateTunnel
            }

            (TunnelState::Activating, StateCommand::StopMonitoring) => {
                log::warn!("Disconnected while activating tunnel");
                self.state = TunnelState::Inactive;
                StateAction::DetachEbpf
            }

            // Traffic detected while monitoring -> activate tunnel
            (TunnelState::Monitoring, StateCommand::TrafficDetected) => {
                log::info!("Traffic detected, activating tunnel");
                self.state = TunnelState::Activating;
                StateAction::ActivateTunnel
            }

            // Tunnel already up at startup (skip activation, go straight to Active)
            (TunnelState::Monitoring, StateCommand::TunnelAlreadyUp) => {
                log::info!("Tunnel already up, transitioning to Active state");
                self.state = TunnelState::Active;
                StateAction::None // No action needed, tunnel is already up
            }

            // Tunnel successfully brought up
            (TunnelState::Activating, StateCommand::TunnelUp) => {
                log::info!("Tunnel activated successfully");
                self.state = TunnelState::Active;
                StateAction::DetachEbpf
            }

            // Tunnel brought down successfully
            (TunnelState::Deactivating, StateCommand::TunnelDown) => {
                log::info!("Tunnel deactivated, returning to monitoring");
                self.state = TunnelState::Monitoring;
                StateAction::AttachEbpf
            }

            // Idle timeout reached - deactivate tunnel
            (TunnelState::Active, StateCommand::IdleTimeout) => {
                log::info!("Idle timeout reached, deactivating tunnel");
                self.state = TunnelState::Deactivating;
                StateAction::DeactivateTunnel
            }

            // Ignore traffic events while activating, deactivating, or active
            // (eBPF traffic events only trigger tunnel activation, not idle reset)
            (TunnelState::Activating, StateCommand::TrafficDetected)
            | (TunnelState::Deactivating, StateCommand::TrafficDetected)
            | (TunnelState::Active, StateCommand::TrafficDetected) => {
                log::debug!("Traffic detected during active/transition, ignoring");
                StateAction::None
            }

            // Ignore other combinations
            _ => {
                log::debug!(
                    "No action for state {:?} with command {:?}",
                    self.state,
                    cmd
                );
                StateAction::None
            }
        }
    }

    /// Get current state
    pub fn state(&self) -> TunnelState {
        self.state
    }

    /// Get idle timeout duration (for use by main loop)
    pub fn idle_timeout(&self) -> Duration {
        self.idle_timeout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state() {
        let manager = StateManager::new(300);
        assert_eq!(manager.state(), TunnelState::Inactive);
    }

    #[test]
    fn test_start_monitoring() {
        let mut manager = StateManager::new(300);
        let action = manager.handle_command(StateCommand::StartMonitoring);
        assert_eq!(action, StateAction::AttachEbpf);
        assert_eq!(manager.state(), TunnelState::Monitoring);
    }

    #[test]
    fn test_traffic_activates_tunnel() {
        let mut manager = StateManager::new(300);
        manager.handle_command(StateCommand::StartMonitoring);

        let action = manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(action, StateAction::ActivateTunnel);
        assert_eq!(manager.state(), TunnelState::Activating);
    }

    #[test]
    fn test_tunnel_activation_flow() {
        let mut manager = StateManager::new(300);

        // Start monitoring
        manager.handle_command(StateCommand::StartMonitoring);
        assert_eq!(manager.state(), TunnelState::Monitoring);

        // Traffic detected
        manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(manager.state(), TunnelState::Activating);

        // Tunnel up - should detach eBPF
        let action = manager.handle_command(StateCommand::TunnelUp);
        assert_eq!(action, StateAction::DetachEbpf);
        assert_eq!(manager.state(), TunnelState::Active);
    }

    #[test]
    fn test_stop_monitoring_while_active() {
        let mut manager = StateManager::new(300);

        // Get to active state
        manager.handle_command(StateCommand::StartMonitoring);
        manager.handle_command(StateCommand::TrafficDetected);
        manager.handle_command(StateCommand::TunnelUp);

        // Stop monitoring should deactivate tunnel
        let action = manager.handle_command(StateCommand::StopMonitoring);
        assert_eq!(action, StateAction::DeactivateTunnel);
        assert_eq!(manager.state(), TunnelState::Deactivating);
    }

    #[test]
    fn test_multiple_start_monitoring_calls() {
        let mut manager = StateManager::new(300);

        // First call should attach eBPF
        let action1 = manager.handle_command(StateCommand::StartMonitoring);
        assert_eq!(action1, StateAction::AttachEbpf);
        assert_eq!(manager.state(), TunnelState::Monitoring);

        // Second call while already monitoring should do nothing
        let action2 = manager.handle_command(StateCommand::StartMonitoring);
        assert_eq!(action2, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Monitoring);
    }

    #[test]
    fn test_stop_monitoring_from_inactive() {
        let mut manager = StateManager::new(300);

        // Stop monitoring when not monitoring should be no-op
        let action = manager.handle_command(StateCommand::StopMonitoring);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Inactive);
    }

    #[test]
    fn test_traffic_detected_while_inactive() {
        let mut manager = StateManager::new(300);

        // Traffic detected when not monitoring should be ignored
        let action = manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Inactive);
    }

    #[test]
    fn test_traffic_detected_while_active() {
        let mut manager = StateManager::new(300);

        // Get to active state
        manager.handle_command(StateCommand::StartMonitoring);
        manager.handle_command(StateCommand::TrafficDetected);
        manager.handle_command(StateCommand::TunnelUp);

        // Traffic while active should be ignored (idle tracking in main.rs now)
        let action = manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Active);
    }

    #[test]
    fn test_traffic_detected_while_activating() {
        let mut manager = StateManager::new(300);

        // Get to activating state
        manager.handle_command(StateCommand::StartMonitoring);
        manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(manager.state(), TunnelState::Activating);

        // More traffic while activating should be ignored
        let action = manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Activating);
    }

    #[test]
    fn test_traffic_detected_while_deactivating() {
        let mut manager = StateManager::new(300);

        // Get to deactivating state by manually setting it
        manager.state = TunnelState::Deactivating;

        // Traffic while deactivating should be ignored
        let action = manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Deactivating);
    }

    #[test]
    fn test_tunnel_up_without_activation() {
        let mut manager = StateManager::new(300);
        manager.handle_command(StateCommand::StartMonitoring);

        // TunnelUp command without being in Activating state should be ignored
        let action = manager.handle_command(StateCommand::TunnelUp);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Monitoring);
    }

    #[test]
    fn test_tunnel_down_while_monitoring() {
        let mut manager = StateManager::new(300);
        manager.handle_command(StateCommand::StartMonitoring);

        // TunnelDown command while just monitoring should be ignored
        let action = manager.handle_command(StateCommand::TunnelDown);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Monitoring);
    }

    #[test]
    fn test_tunnel_down_while_inactive() {
        let mut manager = StateManager::new(300);

        // TunnelDown when inactive should be ignored
        let action = manager.handle_command(StateCommand::TunnelDown);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Inactive);
    }

    #[test]
    fn test_deactivating_to_monitoring_transition() {
        let mut manager = StateManager::new(300);

        // Get to active state
        manager.handle_command(StateCommand::StartMonitoring);
        manager.handle_command(StateCommand::TrafficDetected);
        manager.handle_command(StateCommand::TunnelUp);

        // Manually set to deactivating (in real scenario, main.rs triggers this)
        manager.state = TunnelState::Deactivating;

        // TunnelDown should return to Monitoring and re-attach eBPF
        let action = manager.handle_command(StateCommand::TunnelDown);
        assert_eq!(action, StateAction::AttachEbpf);
        assert_eq!(manager.state(), TunnelState::Monitoring);
    }

    #[test]
    fn test_stop_monitoring_while_activating() {
        let mut manager = StateManager::new(300);

        // Get to activating state
        manager.handle_command(StateCommand::StartMonitoring);
        manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(manager.state(), TunnelState::Activating);

        // Disconnect while activating should go back to inactive
        let action = manager.handle_command(StateCommand::StopMonitoring);
        assert_eq!(action, StateAction::DetachEbpf);
        assert_eq!(manager.state(), TunnelState::Inactive);
    }

    #[test]
    fn test_rapid_state_transitions() {
        let mut manager = StateManager::new(300);

        // Rapid fire commands
        manager.handle_command(StateCommand::StartMonitoring);
        assert_eq!(manager.state(), TunnelState::Monitoring);

        manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(manager.state(), TunnelState::Activating);

        // Disconnect before tunnel is up
        let action = manager.handle_command(StateCommand::StopMonitoring);
        assert_eq!(action, StateAction::DetachEbpf);
        assert_eq!(manager.state(), TunnelState::Inactive);
    }

    #[test]
    fn test_idle_timeout_getter() {
        let manager = StateManager::new(600);
        assert_eq!(manager.idle_timeout(), Duration::from_secs(600));
    }

    #[test]
    fn test_state_getter() {
        let mut manager = StateManager::new(300);
        assert_eq!(manager.state(), TunnelState::Inactive);

        manager.handle_command(StateCommand::StartMonitoring);
        assert_eq!(manager.state(), TunnelState::Monitoring);
    }

    #[test]
    fn test_idle_timeout_deactivates_tunnel() {
        let mut manager = StateManager::new(300);

        // Get to active state
        manager.handle_command(StateCommand::StartMonitoring);
        manager.handle_command(StateCommand::TrafficDetected);
        manager.handle_command(StateCommand::TunnelUp);
        assert_eq!(manager.state(), TunnelState::Active);

        // Idle timeout should trigger deactivation
        let action = manager.handle_command(StateCommand::IdleTimeout);
        assert_eq!(action, StateAction::DeactivateTunnel);
        assert_eq!(manager.state(), TunnelState::Deactivating);

        // Complete the deactivation
        let action = manager.handle_command(StateCommand::TunnelDown);
        assert_eq!(action, StateAction::AttachEbpf);
        assert_eq!(manager.state(), TunnelState::Monitoring);
    }

    #[test]
    fn test_idle_timeout_ignored_when_not_active() {
        let mut manager = StateManager::new(300);

        // IdleTimeout when inactive should be ignored
        let action = manager.handle_command(StateCommand::IdleTimeout);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Inactive);

        // IdleTimeout when monitoring should be ignored
        manager.handle_command(StateCommand::StartMonitoring);
        let action = manager.handle_command(StateCommand::IdleTimeout);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Monitoring);

        // IdleTimeout when activating should be ignored
        manager.handle_command(StateCommand::TrafficDetected);
        assert_eq!(manager.state(), TunnelState::Activating);
        let action = manager.handle_command(StateCommand::IdleTimeout);
        assert_eq!(action, StateAction::None);
        assert_eq!(manager.state(), TunnelState::Activating);
    }
}
