use thiserror::Error;

#[derive(Error, Debug)]
pub enum DiakonosError {
    #[error("Service not found: {0}")]
    ServiceNotFound(String),

    #[error("Service already exists: {0}")]
    ServiceAlreadyExists(String),

    #[error("Failed to parse unit file: {0}")]
    ParseError(String),

    #[error("Failed to start service: {0}")]
    StartError(String),

    #[error("Failed to stop service: {0}")]
    StopError(String),

    #[error("Dependency cycle detected")]
    DependencyCycle,

    #[error("Service dependency not met: {0}")]
    DependencyNotMet(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Process error: {0}")]
    ProcessError(String),
}

pub type Result<T> = std::result::Result<T, DiakonosError>;
