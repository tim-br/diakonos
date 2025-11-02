use crate::error::{DiakonosError, Result};
use crate::unit::UnitFile;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use std::collections::HashMap;
use std::process::{Child, Command};
use std::sync::{Arc, Mutex};
use tokio::time::{sleep, Duration};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

pub struct Service {
    pub unit: UnitFile,
    pub state: ServiceState,
    pub pid: Option<u32>,
    process: Option<Arc<Mutex<Child>>>,
    restart_count: u32,
}

impl Service {
    pub fn new(unit: UnitFile) -> Self {
        Self {
            unit,
            state: ServiceState::Stopped,
            pid: None,
            process: None,
            restart_count: 0,
        }
    }

    pub async fn start(&mut self) -> Result<()> {
        if self.state == ServiceState::Running {
            return Ok(());
        }

        info!("Starting service: {}", self.unit.name);
        self.state = ServiceState::Starting;

        let exec_start = self.unit.service.exec_start.clone();
        let parts: Vec<&str> = exec_start.split_whitespace().collect();

        if parts.is_empty() {
            return Err(DiakonosError::StartError("Empty ExecStart".to_string()));
        }

        let mut cmd = Command::new(parts[0]);
        if parts.len() > 1 {
            cmd.args(&parts[1..]);
        }

        // Set working directory if specified
        if let Some(ref wd) = self.unit.service.working_directory {
            cmd.current_dir(wd);
        }

        // Set environment variables if specified
        if let Some(ref env_vars) = self.unit.service.environment {
            for env in env_vars {
                if let Some((key, value)) = env.split_once('=') {
                    cmd.env(key, value);
                }
            }
        }

        let child = cmd
            .spawn()
            .map_err(|e| DiakonosError::StartError(e.to_string()))?;

        self.pid = Some(child.id());
        self.process = Some(Arc::new(Mutex::new(child)));
        self.state = ServiceState::Running;

        info!(
            "Service {} started with PID {}",
            self.unit.name,
            self.pid.unwrap()
        );

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        if self.state == ServiceState::Stopped {
            return Ok(());
        }

        info!("Stopping service: {}", self.unit.name);
        self.state = ServiceState::Stopping;

        // First try custom stop command if specified
        if let Some(ref exec_stop) = self.unit.service.exec_stop {
            let parts: Vec<&str> = exec_stop.split_whitespace().collect();
            if !parts.is_empty() {
                let mut cmd = Command::new(parts[0]);
                if parts.len() > 1 {
                    cmd.args(&parts[1..]);
                }
                let _ = cmd.spawn();
                sleep(Duration::from_secs(2)).await;
            }
        }

        // Then send SIGTERM to the process
        if let Some(pid) = self.pid {
            let pid = Pid::from_raw(pid as i32);
            if let Err(e) = signal::kill(pid, Signal::SIGTERM) {
                warn!("Failed to send SIGTERM to PID {}: {}", pid, e);
            } else {
                // Wait a bit for graceful shutdown
                sleep(Duration::from_secs(3)).await;

                // If still running, send SIGKILL
                if signal::kill(pid, Signal::SIGTERM).is_ok() {
                    warn!("Process {} did not respond to SIGTERM, sending SIGKILL", pid);
                    let _ = signal::kill(pid, Signal::SIGKILL);
                }
            }
        }

        self.pid = None;
        self.process = None;
        self.state = ServiceState::Stopped;

        info!("Service {} stopped", self.unit.name);
        Ok(())
    }

    pub async fn restart(&mut self) -> Result<()> {
        info!("Restarting service: {}", self.unit.name);
        self.stop().await?;
        sleep(Duration::from_secs(1)).await;
        self.start().await?;
        Ok(())
    }

    pub async fn check_status(&mut self) -> ServiceState {
        if let Some(ref process) = self.process {
            let mut child = process.lock().unwrap();
            match child.try_wait() {
                Ok(Some(status)) => {
                    if status.success() {
                        info!("Service {} exited successfully", self.unit.name);
                        self.state = ServiceState::Stopped;
                    } else {
                        error!(
                            "Service {} failed with exit code: {:?}",
                            self.unit.name,
                            status.code()
                        );
                        self.state = ServiceState::Failed;
                    }
                    self.pid = None;
                }
                Ok(None) => {
                    // Still running
                    self.state = ServiceState::Running;
                }
                Err(e) => {
                    error!("Error checking service {} status: {}", self.unit.name, e);
                    self.state = ServiceState::Failed;
                    self.pid = None;
                }
            }
        }
        self.state
    }

    pub fn should_restart(&self) -> bool {
        use crate::unit::RestartPolicy;

        let policy = self
            .unit
            .service
            .restart
            .unwrap_or(RestartPolicy::No);

        match policy {
            RestartPolicy::Always => true,
            RestartPolicy::OnFailure => self.state == ServiceState::Failed,
            RestartPolicy::No => false,
        }
    }

    pub fn get_restart_delay(&self) -> Duration {
        Duration::from_secs(self.unit.service.restart_sec.unwrap_or(5))
    }
}
