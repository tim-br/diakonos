use crate::service::ServiceState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Start { service: String },
    Stop { service: String },
    Restart { service: String },
    Status { service: String },
    List,
    Ping,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Ok { message: String },
    Error { message: String },
    Status { service: String, state: ServiceState },
    List { services: Vec<(String, ServiceState)> },
    Pong,
}

impl Response {
    pub fn ok(message: impl Into<String>) -> Self {
        Response::Ok {
            message: message.into(),
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Response::Error {
            message: message.into(),
        }
    }
}
