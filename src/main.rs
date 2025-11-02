mod client;
mod daemon;
mod error;
mod ipc;
mod manager;
mod service;
mod unit;

use clap::{Parser, Subcommand};
use client::Client;
use daemon::{DaemonConfig, ensure_daemon_started, is_daemon_running, start_daemon};
use ipc::{Request, Response};
use std::path::PathBuf;
use tracing::error;

#[derive(Parser)]
#[command(name = "diakonos")]
#[command(about = "A PM2-like service manager", long_about = None)]
struct Cli {
    /// Directory containing service unit files
    #[arg(short, long, default_value = "./services")]
    service_dir: PathBuf,

    /// Start in daemon mode (internal use only)
    #[arg(long, hide = true)]
    daemon_start: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start a service
    Start {
        /// Name of the service to start
        service: String,
    },
    /// Stop a service
    Stop {
        /// Name of the service to stop
        service: String,
    },
    /// Restart a service
    Restart {
        /// Name of the service to restart
        service: String,
    },
    /// Show status of a service
    Status {
        /// Name of the service to check
        service: String,
    },
    /// List all services
    List,
    /// Show daemon status
    DaemonStatus,
    /// Kill the daemon (stops all services)
    Kill,
}

fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();

    let mut config = DaemonConfig::default();
    config.service_dir = cli.service_dir.clone();

    // Create service directory if it doesn't exist
    if !config.service_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&config.service_dir) {
            eprintln!("Failed to create service directory: {}", e);
            std::process::exit(1);
        }
    }

    // Handle daemon start (internal) - use sync code path
    if cli.daemon_start {
        if let Err(e) = start_daemon(config) {
            error!("Failed to start daemon: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // Run client code with tokio runtime
    run_client(cli, config);
}

#[tokio::main]
async fn run_client(cli: Cli, config: DaemonConfig) {

    // Handle commands
    let command = cli.command.unwrap_or(Commands::List);

    match command {
        Commands::DaemonStatus => {
            if is_daemon_running(&config) {
                println!("✓ Daemon is running");
                println!("  Socket: {:?}", config.socket_path);
                println!("  PID file: {:?}", config.pid_file);
            } else {
                println!("✗ Daemon is not running");
            }
            return;
        }

        Commands::Kill => {
            if !is_daemon_running(&config) {
                println!("Daemon is not running");
                return;
            }

            println!("Killing daemon...");
            let client = Client::new(config);

            match client.send_request(Request::Shutdown).await {
                Ok(_) => println!("✓ Daemon killed"),
                Err(e) => {
                    eprintln!("Failed to kill daemon: {}", e);
                    std::process::exit(1);
                }
            }
            return;
        }

        _ => {}
    }

    // Ensure daemon is running
    if let Err(e) = ensure_daemon_started(&config) {
        eprintln!("Failed to start daemon: {}", e);
        std::process::exit(1);
    }

    // Create client and send request
    let client = Client::new(config);

    let request = match command {
        Commands::Start { service } => Request::Start { service },
        Commands::Stop { service } => Request::Stop { service },
        Commands::Restart { service } => Request::Restart { service },
        Commands::Status { service } => Request::Status { service },
        Commands::List => Request::List,
        _ => unreachable!(),
    };

    match client.send_request(request).await {
        Ok(response) => handle_response(response),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}

fn handle_response(response: Response) {
    match response {
        Response::Ok { message } => {
            println!("✓ {}", message);
        }
        Response::Error { message } => {
            eprintln!("✗ Error: {}", message);
            std::process::exit(1);
        }
        Response::Status { service, state } => {
            println!("Service '{}' status: {:?}", service, state);
        }
        Response::List { services } => {
            if services.is_empty() {
                println!("No services loaded");
            } else {
                println!("\nLoaded services:");
                println!("{:<30} {:<15}", "SERVICE", "STATE");
                println!("{}", "-".repeat(45));

                for (name, state) in services {
                    let state_str = format!("{:?}", state);
                    let colored_state = match state {
                        service::ServiceState::Running => format!("\x1b[32m{}\x1b[0m", state_str),
                        service::ServiceState::Failed => format!("\x1b[31m{}\x1b[0m", state_str),
                        service::ServiceState::Stopped => format!("\x1b[90m{}\x1b[0m", state_str),
                        _ => state_str,
                    };
                    println!("{:<30} {:<15}", name, colored_state);
                }
            }
        }
        Response::Pong => {
            println!("Daemon is alive");
        }
    }
}
