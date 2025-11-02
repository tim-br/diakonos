use crate::daemon::DaemonConfig;
use crate::error::{DiakonosError, Result};
use crate::ipc::{Request, Response};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

pub struct Client {
    config: DaemonConfig,
}

impl Client {
    pub fn new(config: DaemonConfig) -> Self {
        Self { config }
    }

    pub async fn send_request(&self, request: Request) -> Result<Response> {
        // Connect to daemon socket
        let stream = UnixStream::connect(&self.config.socket_path)
            .await
            .map_err(|e| {
                DiakonosError::StartError(format!(
                    "Failed to connect to daemon at {:?}: {}",
                    self.config.socket_path, e
                ))
            })?;

        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // Send request
        let request_json = serde_json::to_string(&request)
            .map_err(|e| DiakonosError::ParseError(format!("Failed to serialize request: {}", e)))?;

        writer
            .write_all(request_json.as_bytes())
            .await
            .map_err(|e| DiakonosError::StartError(format!("Failed to send request: {}", e)))?;

        writer
            .write_all(b"\n")
            .await
            .map_err(|e| DiakonosError::StartError(format!("Failed to send request: {}", e)))?;

        // Read response
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .map_err(|e| DiakonosError::StartError(format!("Failed to read response: {}", e)))?;

        let response: Response = serde_json::from_str(&line.trim())
            .map_err(|e| DiakonosError::ParseError(format!("Failed to parse response: {}", e)))?;

        Ok(response)
    }
}
