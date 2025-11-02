use crate::error::Result;
use crate::ipc::{Request, Response};
use crate::manager::ServiceManager;
use daemonize::Daemonize;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tracing::{error, info, warn};

pub struct DaemonConfig {
    pub socket_path: PathBuf,
    pub pid_file: PathBuf,
    pub service_dir: PathBuf,
    pub log_file: PathBuf,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let daemon_dir = PathBuf::from(home).join(".diakonos");

        Self {
            socket_path: daemon_dir.join("daemon.sock"),
            pid_file: daemon_dir.join("daemon.pid"),
            service_dir: PathBuf::from("./services"),
            log_file: daemon_dir.join("daemon.log"),
        }
    }
}

pub fn start_daemon(config: DaemonConfig) -> Result<()> {
    // Create daemon directory if it doesn't exist
    if let Some(parent) = config.socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove old socket if it exists
    if config.socket_path.exists() {
        std::fs::remove_file(&config.socket_path)?;
    }

    // Daemonize the process
    let stdout = std::fs::File::create(&config.log_file)?;
    let stderr = stdout.try_clone()?;

    let daemonize = Daemonize::new()
        .pid_file(&config.pid_file)
        .working_directory(std::env::current_dir()?)
        .stdout(stdout)
        .stderr(stderr);

    match daemonize.start() {
        Ok(_) => {
            info!("Daemon started successfully");

            // IMPORTANT: Create tokio runtime AFTER daemonization
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let result = runtime.block_on(run_daemon(config));
            error!("Daemon loop exited with result: {:?}", result);
            result
        }
        Err(e) => {
            error!("Failed to daemonize: {}", e);
            Err(crate::error::DiakonosError::StartError(format!(
                "Failed to daemonize: {}",
                e
            )))
        }
    }
}

async fn run_daemon(config: DaemonConfig) -> Result<()> {
    info!("Daemon running with socket at {:?}", config.socket_path);

    // Create service manager
    let manager = Arc::new(ServiceManager::new(config.service_dir.clone()));

    // Load all services
    if let Err(e) = manager.load_all_services().await {
        warn!("Failed to load services: {}", e);
    }

    // Start supervision task
    let manager_clone = Arc::clone(&manager);
    let supervision_handle = tokio::spawn(async move {
        manager_clone.supervise().await;
        error!("Supervision loop exited!");
    });

    // Create Unix socket listener
    let listener = UnixListener::bind(&config.socket_path)
        .map_err(|e| crate::error::DiakonosError::StartError(format!("Failed to bind socket: {}", e)))?;

    info!("Listening for connections...");

    // Accept connections loop (should never exit)
    let accept_handle = tokio::spawn(async move {
        loop {
            info!("Waiting for connection...");
            match listener.accept().await {
                Ok((stream, _)) => {
                    info!("Connection accepted");
                    let manager = Arc::clone(&manager);
                    tokio::spawn(async move {
                        info!("Spawned connection handler");
                        match handle_connection(stream, manager).await {
                            Ok(_) => info!("Connection handled successfully"),
                            Err(e) => error!("Error handling connection: {}", e),
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                    break;
                }
            }
        }
        error!("Accept loop exited!");
    });

    // Wait for either task to complete (which should never happen)
    tokio::select! {
        _ = supervision_handle => {
            error!("Supervision task completed unexpectedly");
        }
        _ = accept_handle => {
            error!("Accept task completed unexpectedly");
        }
    }

    Err(crate::error::DiakonosError::StartError(
        "Daemon tasks exited unexpectedly".to_string(),
    ))
}

async fn handle_connection(
    stream: UnixStream,
    manager: Arc<ServiceManager>,
) -> std::io::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let request: Request = match serde_json::from_str(&line.trim()) {
            Ok(req) => req,
            Err(e) => {
                let response = Response::error(format!("Invalid request: {}", e));
                let response_json = serde_json::to_string(&response).unwrap();
                writer.write_all(response_json.as_bytes()).await?;
                writer.write_all(b"\n").await?;
                line.clear();
                continue;
            }
        };

        let is_shutdown = matches!(request, Request::Shutdown);
        let response = handle_request(request, &manager).await;
        let response_json = match serde_json::to_string(&response) {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to serialize response: {}", e);
                continue;
            }
        };

        if let Err(e) = writer.write_all(response_json.as_bytes()).await {
            error!("Failed to write response: {}", e);
            break;
        }
        if let Err(e) = writer.write_all(b"\n").await {
            error!("Failed to write newline: {}", e);
            break;
        }

        // If this was a shutdown request, flush and exit
        if is_shutdown {
            let _ = writer.flush().await;
            std::process::exit(0);
        }

        line.clear();
    }

    Ok(())
}

async fn handle_request(request: Request, manager: &ServiceManager) -> Response {
    info!("Handling request: {:?}", request);
    match request {
        Request::Start { service } => {
            info!("Starting service: {}", service);
            match manager.start_service(&service).await {
                Ok(_) => {
                    info!("Service '{}' started successfully", service);
                    Response::ok(format!("Service '{}' started successfully", service))
                }
                Err(e) => {
                    error!("Failed to start service '{}': {}", service, e);
                    Response::error(format!("Failed to start service '{}': {}", service, e))
                }
            }
        }

        Request::Stop { service } => match manager.stop_service(&service).await {
            Ok(_) => Response::ok(format!("Service '{}' stopped successfully", service)),
            Err(e) => Response::error(format!("Failed to stop service '{}': {}", service, e)),
        },

        Request::Restart { service } => match manager.restart_service(&service).await {
            Ok(_) => Response::ok(format!("Service '{}' restarted successfully", service)),
            Err(e) => Response::error(format!("Failed to restart service '{}': {}", service, e)),
        },

        Request::Status { service } => match manager.get_service_status(&service).await {
            Ok(state) => Response::Status { service, state },
            Err(e) => Response::error(format!("Failed to get status for '{}': {}", service, e)),
        },

        Request::List => {
            let services = manager.list_services().await;
            Response::List { services }
        }

        Request::Ping => Response::Pong,

        Request::Shutdown => {
            info!("Shutdown requested");
            Response::ok("Daemon shutting down".to_string())
        }
    }
}

pub fn is_daemon_running(config: &DaemonConfig) -> bool {
    if !config.pid_file.exists() {
        return false;
    }

    // Read PID from file
    let pid_str = match std::fs::read_to_string(&config.pid_file) {
        Ok(s) => s,
        Err(_) => return false,
    };

    let pid: i32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Check if process is running (signal 0 doesn't actually send a signal, just checks)
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    kill(Pid::from_raw(pid), None).is_ok()
}

pub fn ensure_daemon_started(config: &DaemonConfig) -> Result<()> {
    if is_daemon_running(config) {
        return Ok(());
    }

    info!("Starting daemon...");

    // Start daemon in a separate process
    let exe = std::env::current_exe()
        .map_err(|e| crate::error::DiakonosError::StartError(format!("Failed to get exe path: {}", e)))?;

    std::process::Command::new(exe)
        .arg("--daemon-start")
        .arg("--service-dir")
        .arg(&config.service_dir)
        .spawn()
        .map_err(|e| crate::error::DiakonosError::StartError(format!("Failed to start daemon: {}", e)))?;

    // Wait for daemon to start
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if config.socket_path.exists() {
            return Ok(());
        }
    }

    Err(crate::error::DiakonosError::StartError(
        "Daemon failed to start within timeout".to_string(),
    ))
}
