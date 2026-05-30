use std::time::Duration;

use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    config::Config,
    protocol::{CapabilityInventory, RunnerEvent, RunnerState, RunnerStatus},
};

#[derive(Debug)]
pub struct Daemon {
    config: Config,
    runner_id: Uuid,
}

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("daemon received shutdown signal")]
    Shutdown,
}

impl Daemon {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            runner_id: Uuid::now_v7(),
        }
    }

    pub fn runner_id(&self) -> Uuid {
        self.runner_id
    }

    pub fn capability_inventory(&self) -> CapabilityInventory {
        CapabilityInventory {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            runtimes: self.config.capabilities.runtimes.clone(),
            tools: self.config.capabilities.tools.clone(),
            mcp_host_modes: self.config.capabilities.mcp_host_modes.clone(),
            local_llm_endpoints: self.config.capabilities.local_llm_endpoints.clone(),
            network_zone: self.config.capabilities.network_zone.clone(),
            labels: self.config.capabilities.labels.clone(),
            max_concurrency: self.config.max_concurrency,
        }
    }

    pub fn status(&self, state: RunnerState) -> RunnerStatus {
        RunnerStatus {
            state,
            active_jobs: 0,
            max_concurrency: self.config.max_concurrency,
            warnings: self.startup_warnings(),
        }
    }

    pub async fn run_until_shutdown(self) -> Result<(), DaemonError> {
        info!(
            runner_id = %self.runner_id,
            runner_name = %self.config.runner_name,
            endpoint = %self.config.control_plane.endpoint,
            "starting taskotter remote daemon scaffold"
        );
        info!(
            capability_inventory = ?self.capability_inventory(),
            "runner capability inventory"
        );

        let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = heartbeat.tick() => {
                    let event = RunnerEvent::Status(self.status(RunnerState::Idle));
                    info!(?event, "runner heartbeat");
                }
                _ = tokio::signal::ctrl_c() => {
                    warn!("shutdown signal received");
                    return Err(DaemonError::Shutdown);
                }
            }
        }
    }

    fn startup_warnings(&self) -> Vec<String> {
        if self.config.registration_token_is_available() {
            Vec::new()
        } else {
            vec![format!(
                "{} is not set; registration cannot complete",
                self.config.control_plane.registration_token_env
            )]
        }
    }
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::config::{CapabilityConfig, ControlPlaneConfig, Transport};

    use super::*;

    #[test]
    fn maps_config_to_capability_inventory() {
        let daemon = Daemon::new(Config {
            runner_name: "test-runner".to_string(),
            working_group_id: uuid!("00000000-0000-0000-0000-000000000000"),
            max_concurrency: 2,
            control_plane: ControlPlaneConfig {
                endpoint: "https://api.example.test".to_string(),
                registration_token_env: "TASKOTTER_TEST_TOKEN".to_string(),
                transport: Transport::GrpcStream,
            },
            capabilities: CapabilityConfig {
                runtimes: vec!["shell".to_string()],
                tools: vec!["git".to_string()],
                mcp_host_modes: vec!["stdio".to_string()],
                local_llm_endpoints: Vec::new(),
                network_zone: "test".to_string(),
                labels: vec!["ci".to_string()],
            },
        });

        let inventory = daemon.capability_inventory();
        assert_eq!(inventory.max_concurrency, 2);
        assert_eq!(inventory.runtimes, ["shell"]);
    }
}
