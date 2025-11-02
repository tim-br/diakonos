# Diakonos

A systemd-like service manager written in Rust.

## Features

- Service lifecycle management (start, stop, restart)
- Dependency resolution (Requires, Wants, After)
- Process supervision with automatic restarts
- TOML-based unit files
- Service state monitoring
- Colored CLI output

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

# Run in daemon mode with supervision
diakonos daemon
```

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
ExecStart = "bash -c 'while true; do echo Working...; sleep 5; done'"
Environment = ["LOG_LEVEL=info", "WORKER_ID=1"]
Restart = "always"
RestartSec = 3
```

## Service Types

- **simple**: The process started by ExecStart is the main process
- **forking**: The process forks and the parent exits (not fully implemented)
- **oneshot**: The process is expected to exit before starting follow-up units (not fully implemented)

## Restart Policies

- **always**: Always restart the service when it exits
- **on-failure**: Restart only if the service exits with a non-zero status
- **no**: Never restart the service

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
