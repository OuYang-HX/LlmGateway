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

## Android Client

项目包含一个 Android 客户端，支持局域网/公网自动切换。

### 功能
- 🏠🌐 **双网关配置**：可同时配置局域网和公网地址
- 🔄 **自动切换**：局域网可达时优先使用局域网，不可达时自动切换公网
- 📊 **完整 Dashboard**：WebView 加载 Web 管理界面
- 📋 **下拉刷新**：支持下拉刷新页面
- 🔍 **连接状态**：顶部显示当前连接类型（局域网/公网）
- ⚙️ **设置入口**：菜单可随时修改网关地址

### 构建 APK

```bash
# 前置条件：Android SDK + JDK 21
# 首次需要安装 Android SDK（已自动配置）

cd android

# 构建 debug APK
./build_apk.sh

# 构建 release APK
./build_apk.sh release

# APK 输出路径
# Debug: android/app/build/outputs/apk/debug/app-debug.apk
# Release: android/app/build/outputs/apk/release/app-release-unsigned.apk
```

### 安装到手机

```bash
# 通过 adb 安装
adb install android/app/build/outputs/apk/debug/app-debug.apk

# 或直接将 APK 文件传到手机安装
```

### 使用流程
1. 首次打开 → 欢迎页 → 点击「配置网关地址」
2. 输入局域网地址（如 `http://192.168.50.188:49127`）
3. 输入公网地址（如 `http://47.117.247.155:49127`）
4. 点击「测试连接」验证地址可用
5. 保存配置 → 自动进入 Dashboard
6. 之后打开 App 自动连接，局域网优先

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
