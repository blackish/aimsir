# Aimsir Network Monitoring System

Aimsir is a distributed network monitoring system built in Rust that measures network performance metrics between nodes. The name "Aimsir" comes from the Irish word for "weather," as the system monitors the "climate" of your network.

## Overview

Aimsir uses a client-server architecture to collect, analyze, and visualize network metrics between distributed nodes:

- **Clients** send UDP probes to each other to measure network performance
- **Metrics** like packet loss and jitter are calculated and reported to the server
- **Server** aggregates metrics and provides APIs for configuration and data access
- **Tags** provide a hierarchical organization for grouping and analyzing metrics

## Features

- Real-time network performance monitoring
- Distributed architecture with centralized reporting
- Hierarchical tag-based organization
- REST API for configuration and data access
- gRPC-based client-server communication
- MySQL storage for configuration and metadata
- Comprehensive jitter and packet loss measurements

## Architecture

![Aimsir Architecture](docs/architecture.png)

Aimsir consists of these main components:

1. **Server**: Central component responsible for:
   - Managing connected peers/clients
   - Collecting and aggregating metrics
   - Providing HTTP API for configuration and data access
   - Exposing gRPC service for client communication

2. **Clients**: Distributed nodes that:
   - Register with the server
   - Probe other peers to measure network performance
   - Report metrics back to the server

3. **Database**: MySQL storage for:
   - Peer information
   - Tag hierarchies for grouping and categorizing peers

## Installation

### Prerequisites

- Rust 1.58 or higher
- Docker and Docker Compose (for database setup)
- MySQL 5.7 or higher

### Building from Source

```bash
# Clone the repository
git clone https://github.com/yourusername/aimsir.git
cd aimsir

# Set up the database
docker-compose up -d

# Wait for MySQL to be ready
make wait-for-mysql

# Create the database and run migrations
make create-db
make migrate

# Build the project
cargo build --release
```

## Configuration

### Server Configuration

The server accepts these command-line arguments:

```
Usage: aimsir-server [OPTIONS] --ip <ip> --webip <webip> --db <db> --interval <interval> --aggregate <aggregate>

Options:
  -p, --ip <ip>                  Node local IP (gRPC interface)
  -w, --webip <webip>            Web UI local IP (HTTP interface)
  -b, --db <db>                  Database connection string
  -i, --interval <interval>      Probe interval in seconds
  -a, --aggregate <aggregate>    Aggregation interval in seconds
  -l, --loglevel <LOGLEVEL>      Log level [possible values: error, warn, info, debug]
  -h, --help                     Print help
  -V, --version                  Print version
```

### Client Configuration

The client accepts these command-line arguments:

```
Usage: aimsir-client [OPTIONS] --id <id> --server <server>

Options:
  -i, --id <id>                  Node ID (must be registered in the server)
  -s, --server <server>          Manager server address (e.g., http://server:8443)
  -l, --loglevel <LOGLEVEL>      Log level [possible values: error, warn, info, debug]
  -h, --help                     Print help
  -V, --version                  Print version
```

## Database Schema

Aimsir uses a MySQL database with the following tables:

- **peers**: Stores information about monitoring nodes
  - `peer_id`: Unique identifier for the peer
  - `name`: Human-readable name for the peer

- **tags**: Hierarchical organization structure for peers
  - `id`: Unique identifier for the tag
  - `parent`: Parent tag ID (null for top-level tags)
  - `name`: Tag name

- **peer_tags**: Maps peers to tags
  - `peer_id`: Reference to peers.peer_id
  - `tag_id`: Reference to tags.id

## Usage

### Starting the Server

```bash
./target/release/aimsir-server \
  --ip 0.0.0.0:8443 \
  --webip 0.0.0.0:8080 \
  --db mysql://user:password@localhost:3306/aimsir \
  --interval 5 \
  --aggregate 60 \
  --loglevel info
```

### Starting a Client

```bash
./target/release/aimsir-client \
  --id client1 \
  --server http://server:8443 \
  --loglevel info
```

## API Reference

Aimsir provides a REST API for configuration and data access.

### Endpoints

- **GET /stats**: Get aggregated metrics
- **GET /stats/:id**: Get metrics for a specific tag
- **GET /peers**: List all peers
- **POST /peers**: Add a new peer
- **DELETE /peers/:peer_id**: Remove a peer
- **GET /tags**: List all tags
- **POST /tags**: Add a new tag
- **DELETE /tags/:tag_id**: Remove a tag
- **GET /peertags**: List all peer-tag associations
- **POST /peertags**: Add a new peer-tag association
- **DELETE /peertags/:peer_id/:tag_id**: Remove a peer-tag association

## Metrics

Aimsir collects the following metrics:

- **Packet Loss (PL)**: Number of packets lost during the measurement interval
- **Jitter**: Variation in packet delay
  - **Min**: Minimum observed jitter
  - **Max**: Maximum observed jitter
  - **StdDev**: Standard deviation of jitter measurements

## Development

### Project Structure

```
aimsir/
├── benches/      # Performance benchmarks
├── migrations/   # Database migrations
├── proto/        # Protocol buffer definitions
├── src/
│   ├── aimsir/   # Core library functionality
│   │   ├── backend_manager.rs  # HTTP API implementation
│   │   ├── client_manager.rs   # Client implementation
│   │   ├── constants.rs        # Shared constants
│   │   ├── mod.rs              # Module definitions
│   │   ├── peers_controller.rs # Peer measurement implementation
│   │   ├── server_manager.rs   # Server implementation
│   │   └── model/             # Data model definitions
│   ├── client.rs  # Client binary entry point
│   └── server.rs  # Server binary entry point
├── Cargo.toml    # Rust package manifest
├── Cargo.lock    # Dependency lock file
├── compose.yml   # Docker Compose configuration
└── Makefile      # Build automation
```

### Running Tests

```bash
# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Run benchmarks
cargo bench
```

### Generating Protocol Buffers

The gRPC services are defined in `proto/aimsir.proto`. If you modify this file, you'll need to regenerate the Rust code:

```bash
cargo build
```

The build script in `build.rs` will automatically compile the protocol buffers.

