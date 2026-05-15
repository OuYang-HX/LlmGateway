# LLM Gateway

A high-performance LLM Gateway service built in Rust that provides:

- **Multi-provider proxying** with weighted load balancing
- **Dynamic token authentication** for internal corporate LLM services
- **API key management** with per-key provider restrictions and statistics
- **Real-time monitoring** dashboard with token rate curves
- **Request logging** with full search and filtering
- **Statistics** at multiple granularities (5h, day, week, month)

## Architecture

```
┌─────────────┐     ┌──────────────────────────────────────┐
│   Client     │────▶│          LLM Gateway (Rust)          │
│ (Claude Code,│     │                                      │
│  OpenCode,   │     │  ┌──────────┐  ┌──────────────────┐ │
│  etc.)       │     │  │ API Key  │  │  Auth Manager    │ │
│              │     │  │ Manager  │  │  (static/dynamic)│ │
└─────────────┘     │  └──────────┘  └──────────────────┘ │
                    │                                      │
                    │  ┌──────────┐  ┌──────────────────┐ │
                    │  │  Proxy   │  │ Stats Collector  │ │
                    │  │ (LB/RR)  │  │  (rate tracking) │ │
                    │  └──────────┘  └──────────────────┘ │
                    │                                      │
                    │  ┌──────────────────────────────┐   │
                    │  │       SQLite Database         │   │
                    │  │  (keys, providers, logs,      │   │
                    │  │   snapshots)                  │   │
                    │  └──────────────────────────────┘   │
                    │                                      │
                    │  ┌──────────────────────────────┐   │
                    │  │     Web Dashboard (SPA)       │   │
                    │  └──────────────────────────────┘   │
                    └──────────────────────────────────────┘
                              │           │
                    ┌─────────▼───┐  ┌────▼──────────┐
                    │ Provider A  │  │ Provider B    │
                    │ (OpenAI)    │  │ (Internal)    │
                    └─────────────┘  └───────────────┘
```

## Quick Start

```bash
# Build
cargo build --release

# Copy and edit config
cp config.example.toml config.toml

# Run
cargo run --release

# Open dashboard
open http://localhost:3000
```

## Configuration

See `config.example.toml` for full configuration options.

### Environment Variables

All config values can be overridden with `LLM_GW_` prefix:
```bash
LLM_GW_SERVER_PORT=8080
LLM_GW_DATABASE_URL=sqlite:/data/gateway.db
```

## API Endpoints

### API Key Management
- `POST /api/v1/api-keys` — Create API key
- `GET /api/v1/api-keys` — List all API keys
- `GET /api/v1/api-keys/:id` — Get API key details
- `DELETE /api/v1/api-keys/:id` — Delete API key

### Provider Management
- `POST /api/v1/providers` — Create provider
- `GET /api/v1/providers` — List all providers
- `GET /api/v1/providers/:id` — Get provider details
- `DELETE /api/v1/providers/:id` — Delete provider

### Statistics
- `GET /api/v1/stats` — Get aggregate statistics
- `GET /api/v1/stats/bucketed` — Get time-bucketed statistics

### Request Logs
- `GET /api/v1/logs` — Query request logs (with pagination and filters)

### Dashboard
- `GET /` — Web dashboard
- `GET /api/v1/dashboard/summary` — Dashboard summary data
- `GET /api/v1/dashboard/token-rate` — Real-time token rate data

### LLM Proxy
- `POST /v1/*` — Proxy LLM requests (OpenAI-compatible)
- `GET /v1/*` — Proxy LLM requests

## Usage with Agent Tools

### Claude Code
```bash
export ANTHROPIC_API_KEY=lgk-your-gateway-key
export ANTHROPIC_BASE_URL=http://localhost:3000/v1
```

### OpenCode
```yaml
providers:
  gateway:
    url: http://localhost:3000/v1
    key: lgk-your-gateway-key
```

## Development

```bash
# Run tests
cargo test

# Run with logging
RUST_LOG=llm_gateway=debug cargo run
```

## Test Coverage

- 96+ tests covering:
  - Database CRUD operations
  - API key management (create, list, get, delete, deactivate)
  - Provider management (static API key, dynamic token)
  - Request logging and querying
  - Statistics aggregation and time-bucketing
  - Proxy load balancing (round-robin, weighted)
  - Auth manager (API key, custom header, dynamic token)
  - Stats collector (usage recording, snapshots)
  - HTTP API endpoints (full request/response testing)
  - Token refresh background task
  - Token usage extraction from LLM responses
