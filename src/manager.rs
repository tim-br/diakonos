use crate::error::{DiakonosError, Result};
use crate::service::{Service, ServiceState};
use crate::unit::UnitFile;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

pub struct ServiceManager {
    services: Arc<RwLock<HashMap<String, Service>>>,
    service_dir: PathBuf,
}

impl ServiceManager {
    pub fn new(service_dir: PathBuf) -> Self {
        Self {
            services: Arc::new(RwLock::new(HashMap::new())),
            service_dir,
        }
    }

    pub async fn load_service(&self, name: &str) -> Result<()> {
        let path = self.service_dir.join(format!("{}.service", name));

        if !path.exists() {
            return Err(DiakonosError::ServiceNotFound(name.to_string()));
        }

        let unit = UnitFile::from_file(&path)?;
        let service = Service::new(unit);

        let mut services = self.services.write().await;
        if services.contains_key(name) {
            return Err(DiakonosError::ServiceAlreadyExists(name.to_string()));
        }

        services.insert(name.to_string(), service);
        info!("Loaded service: {}", name);
        Ok(())
    }

    pub async fn load_all_services(&self) -> Result<()> {
        let entries = std::fs::read_dir(&self.service_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("service") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Err(e) = self.load_service(name).await {
                        warn!("Failed to load service {}: {}", name, e);
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn start_service(&self, name: &str) -> Result<()> {
        // First resolve dependencies
        let deps = self.resolve_dependencies(name).await?;

        // Start dependencies first
        for dep in deps {
            if dep != name {
                self.start_service_internal(&dep).await?;
            }
        }

        // Then start the requested service
        self.start_service_internal(name).await
    }

    async fn start_service_internal(&self, name: &str) -> Result<()> {
        let mut services = self.services.write().await;

        let service = services
            .get_mut(name)
            .ok_or_else(|| DiakonosError::ServiceNotFound(name.to_string()))?;

        if service.state == ServiceState::Running {
            return Ok(());
        }

        service.start().await
    }

    pub async fn stop_service(&self, name: &str) -> Result<()> {
        let mut services = self.services.write().await;

        let service = services
            .get_mut(name)
            .ok_or_else(|| DiakonosError::ServiceNotFound(name.to_string()))?;

        service.stop().await
    }

    pub async fn restart_service(&self, name: &str) -> Result<()> {
        let mut services = self.services.write().await;

        let service = services
            .get_mut(name)
            .ok_or_else(|| DiakonosError::ServiceNotFound(name.to_string()))?;

        service.restart().await
    }

    pub async fn get_service_status(&self, name: &str) -> Result<ServiceState> {
        let services = self.services.read().await;

        let service = services
            .get(name)
            .ok_or_else(|| DiakonosError::ServiceNotFound(name.to_string()))?;

        Ok(service.state)
    }

    pub async fn list_services(&self) -> Vec<(String, ServiceState)> {
        let services = self.services.read().await;

        services
            .iter()
            .map(|(name, service)| (name.clone(), service.state))
            .collect()
    }

    async fn resolve_dependencies(&self, name: &str) -> Result<Vec<String>> {
        let services = self.services.read().await;

        let service = services
            .get(name)
            .ok_or_else(|| DiakonosError::ServiceNotFound(name.to_string()))?;

        let mut resolved = Vec::new();
        let mut visited = HashSet::new();

        self.resolve_deps_recursive(name, &services, &mut resolved, &mut visited)?;

        Ok(resolved)
    }

    fn resolve_deps_recursive(
        &self,
        name: &str,
        services: &HashMap<String, Service>,
        resolved: &mut Vec<String>,
        visited: &mut HashSet<String>,
    ) -> Result<()> {
        if visited.contains(name) {
            return Err(DiakonosError::DependencyCycle);
        }

        visited.insert(name.to_string());

        if let Some(service) = services.get(name) {
            let deps = service.unit.dependencies();

            for dep in deps {
                // Remove .service suffix if present
                let dep_name = dep.strip_suffix(".service").unwrap_or(&dep);

                if !resolved.contains(&dep_name.to_string()) {
                    if services.contains_key(dep_name) {
                        self.resolve_deps_recursive(dep_name, services, resolved, visited)?;
                    } else {
                        return Err(DiakonosError::DependencyNotMet(dep_name.to_string()));
                    }
                }
            }
        }

        if !resolved.contains(&name.to_string()) {
            resolved.push(name.to_string());
        }

        Ok(())
    }

    pub async fn supervise(&self) {
        info!("Starting supervision loop");

        loop {
            sleep(Duration::from_secs(5)).await;

            let mut services = self.services.write().await;

            for (name, service) in services.iter_mut() {
                let old_state = service.state;
                let new_state = service.check_status().await;

                if old_state != new_state {
                    info!("Service {} changed state: {:?} -> {:?}", name, old_state, new_state);

                    // Handle restarts
                    if (new_state == ServiceState::Stopped || new_state == ServiceState::Failed)
                        && service.should_restart()
                    {
                        let delay = service.get_restart_delay();
                        info!("Service {} will restart in {:?}", name, delay);

                        let name_clone = name.clone();
                        let services_clone = Arc::clone(&self.services);

                        tokio::spawn(async move {
                            sleep(delay).await;
                            let mut services = services_clone.write().await;
                            if let Some(service) = services.get_mut(&name_clone) {
                                if let Err(e) = service.start().await {
                                    error!("Failed to restart service {}: {}", name_clone, e);
                                }
                            }
                        });
                    }
                }
            }
        }
    }
}
