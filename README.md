# Diakonos

A PM2-style service manager written in Rust with a persistent daemon architecture.

## Architecture

Diakonos uses a **daemon-client architecture** similar to PM2:

- **Persistent Daemon**: Runs in the background (`~/.diakonos/`)
- **Thin CLI Client**: Sends commands to daemon via Unix sockets
- **Process Parenting**: All managed services are children of the daemon, not your terminal
- **Auto-start**: Daemon starts automatically on first command
- **Service Persistence**: Services continue running even if the daemon crashes

## Features

- Service lifecycle management (start, stop, restart)
- Dependency resolution (Requires, Wants, After)
- Process supervision with automatic restarts
- TOML-based unit files
- Service state monitoring
- Colored CLI output
- Daemon management (status, kill)

## Installation

Build from source:

```bash
cargo build --release
```

The binary will be available at `target/release/diakonos`.

## Usage

### Commands

```bash
# List all services
diakonos list

# Start a service (and its dependencies)
diakonos start <service-name>

# Stop a service
diakonos stop <service-name>

# Restart a service
diakonos restart <service-name>

# Check service status
diakonos status <service-name>

# Check daemon status
diakonos daemon-status

# Kill the daemon (stops all managed services)
diakonos kill
```

**Note**: The daemon starts automatically on the first command, so you don't need to manually start it. Just run any command and the daemon will launch in the background if it's not already running.

### Custom Service Directory

By default, diakonos looks for service files in `./services`. You can specify a different directory:

```bash
diakonos --service-dir /path/to/services list
```

## Service Unit Files

Service files use TOML format and should have a `.service` extension.

### Basic Structure

```toml
[unit]
Description = "Service description"
After = ["other-service"]        # Start after these services
Requires = ["dependency"]         # Hard dependency
Wants = ["optional-dependency"]   # Soft dependency

[service]
Type = "simple"                   # simple, forking, or oneshot
ExecStart = "command to start"
ExecStop = "command to stop"      # Optional
Restart = "always"                # always, on-failure, or no
RestartSec = 5                    # Seconds to wait before restart
WorkingDirectory = "/path/to/dir" # Optional
Environment = ["KEY=value"]       # Optional
User = "username"                 # Optional (not yet implemented)
```

### Example Service Files

#### Simple Web Server

```toml
[unit]
Description = "Example web server"

[service]
Type = "simple"
ExecStart = "python3 -m http.server 8080"
Restart = "always"
RestartSec = 5
```

#### Service with Dependencies

```toml
[unit]
Description = "Worker service"
Requires = ["database"]
After = ["database"]

[service]
Type = "simple"
ExecStart = "sleep 10"
Environment = ["LOG_LEVEL=info", "WORKER_ID=1"]
Restart = "on-failure"
RestartSec = 3
```

## Service Types

- **simple**: The process started by ExecStart is the main process
- **forking**: The process forks and the parent exits (not fully implemented)
- **oneshot**: The process is expected to exit before starting follow-up units (not fully implemented)

## Restart Policies

- **always**: Always restart the service when it exits (creates a restart loop for services that complete successfully)
- **on-failure**: Restart only if the service exits with a non-zero status (recommended for most services)
- **no**: Never restart the service (for one-time tasks)

## Dependencies

- **Requires**: Hard dependency - the listed services must start successfully
- **Wants**: Soft dependency - attempts to start but doesn't fail if unavailable
- **After**: Ordering dependency - ensures this service starts after the listed ones

## Architecture

Diakonos consists of several key components:

- **Unit Parser**: Parses TOML service definition files
- **Service Manager**: Manages service lifecycle and dependencies
- **Service Supervisor**: Monitors running processes and handles restarts
- **Dependency Resolver**: Resolves and validates service dependencies
- **CLI**: Command-line interface for interacting with services

## Limitations

This is a learning project and has several limitations compared to systemd:

- No socket activation
- Limited user/group management
- No cgroup integration
- No resource limits
- No D-Bus integration
- Basic logging (uses tracing crate)

## Development

Run in development mode:

```bash
cargo run -- list
cargo run -- start example-web
cargo run -- daemon
```

## License

MIT
