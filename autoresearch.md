# Autoresearch: LLM Gateway Service Design & Implementation

## Objective
Design and implement a high-performance LLM Gateway service in Rust that:
1. Proxies requests to multiple LLM providers with load balancing
2. Manages dynamic token authentication for internal providers
3. Provides API key management with per-key statistics
4. Exposes a web dashboard with real-time metrics, request logs, and statistics
5. Follows TDD development methodology

## Metrics
- **Primary**: features_complete (unitless, higher is better) — number of required feature test cases passing
- **Secondary**: test_count, compile_time_s

## How to Run
`./autoresearch.sh` — outputs `METRIC features_complete=N` and `METRIC test_count=N`

## Architecture Overview
- **Backend**: Rust (axum/actix-web for HTTP, tokio for async runtime)
- **Database**: SQLite (via sqlx/rusqlite) for API keys, request logs, statistics
- **Frontend**: Embedded web dashboard (SPA, served from Rust)
- **Proxy**: Streaming HTTP proxy supporting SSE/WebSocket for LLM APIs
- **Auth**: Dynamic token refresh system with configurable provider auth

## Feature Requirements (from autoresearch.md)
1. ✅ LLM request forwarding (OpenAI-compatible API)
2. ✅ Multiple transport modes (SSE streaming, non-streaming, WebSocket)
3. ✅ Dynamic token management (login with credentials, auto-refresh)
4. ✅ Statistics: rate, token consumption, request count
5. ✅ API key CRUD with per-key statistics
6. ✅ Multi-provider support with load balancing
7. ✅ API key → provider mapping (specific providers or all)
8. ✅ Web dashboard:
   a. Statistics overview
   b. Request content/result search by time range
   c. Real-time token rate curve (up to 1 hour, tokens/s)
   d. Long-term stats (5h/day/week/month: requests, tokens)
   e. Provider throttling count (5h/day/week/month)

## Files in Scope
- `src/` — All Rust source code
- `tests/` — Integration tests
- `migrations/` — Database migrations
- `web/` — Frontend dashboard assets
- `Cargo.toml` — Dependencies
- `autoresearch.sh` — Benchmark script
- `autoresearch.md` — This file

## Off Limits
- `AGENTS.md` — Project instructions
- `autoresearch.config.json` — Session config

## Constraints
- Rust backend for maximum performance
- TDD: write tests first, then implement
- No embedded modifications of third-party crates (must be extensible)
- Can use open-source frameworks but must allow upgrades
- Minimize calls to actual LLM providers during development
- All tests must pass before marking a feature complete

## What's Been Tried
## What's Been Tried
- Iteration 1: Baseline — 39 tests, core modules (db, auth, proxy, stats, api, dashboard)
- Iteration 2: Expanded to 70 tests — comprehensive coverage of all features
- Iteration 3: 96 tests — HTTP API endpoint tests, token refresh task, config template
- Iteration 4: 109 tests — token refresh tests, usage extraction, README
- Iteration 5: 124 tests — WebSocket proxy, usage extraction tests, comprehensive coverage
- Iteration 6: 136 tests — integrated usage extraction into proxy pipeline, WebSocket wired, 0 warnings
- Key wins: weighted round-robin, per-API-key stats, throttle tracking, time-bucketed stats
- Key architectural insight: split proxy into forward_and_collect (extracts usage) + forward_streaming (SSE passthrough)
- Fixed: axum 0.7 uses `:id` not `{id}` for path params; Message::Text needs `.into()`
- Architecture: SQLite + axum + reqwest, modular design with clear separation
