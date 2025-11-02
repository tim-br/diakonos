use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitFile {
    pub unit: UnitSection,
    pub service: ServiceSection,
    #[serde(skip)]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitSection {
    #[serde(rename = "Description")]
    pub description: Option<String>,

    #[serde(rename = "After")]
    pub after: Option<Vec<String>>,

    #[serde(rename = "Requires")]
    pub requires: Option<Vec<String>>,

    #[serde(rename = "Wants")]
    pub wants: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSection {
    #[serde(rename = "Type")]
    pub service_type: Option<ServiceType>,

    #[serde(rename = "ExecStart")]
    pub exec_start: String,

    #[serde(rename = "ExecStop")]
    pub exec_stop: Option<String>,

    #[serde(rename = "Restart")]
    pub restart: Option<RestartPolicy>,

    #[serde(rename = "RestartSec")]
    pub restart_sec: Option<u64>,

    #[serde(rename = "WorkingDirectory")]
    pub working_directory: Option<PathBuf>,

    #[serde(rename = "Environment")]
    pub environment: Option<Vec<String>>,

    #[serde(rename = "User")]
    pub user: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ServiceType {
    Simple,
    Forking,
    Oneshot,
}

impl Default for ServiceType {
    fn default() -> Self {
        ServiceType::Simple
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum RestartPolicy {
    Always,
    OnFailure,
    No,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        RestartPolicy::No
    }
}

impl UnitFile {
    pub fn from_file(path: &std::path::Path) -> crate::error::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let mut unit: UnitFile = toml::from_str(&content)
            .map_err(|e| crate::error::DiakonosError::ParseError(e.to_string()))?;

        unit.name = name;
        Ok(unit)
    }

    pub fn dependencies(&self) -> Vec<String> {
        let mut deps = Vec::new();

        if let Some(requires) = &self.unit.requires {
            deps.extend(requires.clone());
        }

        if let Some(wants) = &self.unit.wants {
            deps.extend(wants.clone());
        }

        deps
    }

    pub fn ordering_dependencies(&self) -> Vec<String> {
        self.unit.after.clone().unwrap_or_default()
    }
}
