mod error;
mod manager;
mod service;
mod unit;

use clap::{Parser, Subcommand};
use manager::ServiceManager;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber;

#[derive(Parser)]
#[command(name = "diakonos")]
#[command(about = "A systemd-like service manager", long_about = None)]
struct Cli {
    /// Directory containing service unit files
    #[arg(short, long, default_value = "./services")]
    service_dir: PathBuf,

    #[command(subcommand)]
    command: Commands,
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
    /// Run the service manager daemon
    Daemon,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_target(false)
        .with_thread_ids(false)
        .with_level(true)
        .init();

    let cli = Cli::parse();

    // Create service directory if it doesn't exist
    if !cli.service_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&cli.service_dir) {
            error!("Failed to create service directory: {}", e);
            std::process::exit(1);
        }
    }

    let manager = ServiceManager::new(cli.service_dir.clone());

    // Load all services
    if let Err(e) = manager.load_all_services().await {
        error!("Failed to load services: {}", e);
        std::process::exit(1);
    }

    match cli.command {
        Commands::Start { service } => {
            info!("Starting service: {}", service);
            match manager.start_service(&service).await {
                Ok(_) => {
                    println!("✓ Service '{}' started successfully", service);
                }
                Err(e) => {
                    error!("Failed to start service '{}': {}", service, e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Stop { service } => {
            info!("Stopping service: {}", service);
            match manager.stop_service(&service).await {
                Ok(_) => {
                    println!("✓ Service '{}' stopped successfully", service);
                }
                Err(e) => {
                    error!("Failed to stop service '{}': {}", service, e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Restart { service } => {
            info!("Restarting service: {}", service);
            match manager.restart_service(&service).await {
                Ok(_) => {
                    println!("✓ Service '{}' restarted successfully", service);
                }
                Err(e) => {
                    error!("Failed to restart service '{}': {}", service, e);
                    std::process::exit(1);
                }
            }
        }

        Commands::Status { service } => {
            match manager.get_service_status(&service).await {
                Ok(state) => {
                    println!("Service '{}' status: {:?}", service, state);
                }
                Err(e) => {
                    error!("Failed to get status for service '{}': {}", service, e);
                    std::process::exit(1);
                }
            }
        }

        Commands::List => {
            let services = manager.list_services().await;

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

        Commands::Daemon => {
            info!("Starting diakonos daemon");
            println!("Diakonos daemon started. Supervising services...");
            println!("Press Ctrl+C to stop.");

            // Start supervision loop
            manager.supervise().await;
        }
    }
}
