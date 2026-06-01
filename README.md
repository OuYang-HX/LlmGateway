# LLM Gateway

高性能 LLM API 网关，Rust 实现，提供多服务商代理、负载均衡、API Key 管理、用量统计和 Web Dashboard。

## 快速开始

```bash
cargo build --release
cp config.example.toml config.toml   # 编辑配置
cargo run --release
# 打开 http://localhost:49127
```

## 核心特性

- **多服务商代理** — 加权负载均衡，支持 OpenAI / Anthropic / 自定义 API
- **双认证模式** — 静态 API Key + 动态 Token（登录获取，自动刷新）
- **统一模型层** — 对外暴露统一模型 ID，多服务商映射实现负载均衡
- **模型校验** — 未通过连接测试的模型自动拒绝
- **Mock 模式** — 开发测试不消耗真实 Token
- **用量配额** — 固定窗口 / 滑动窗口 / 步进滑动窗口
- **实时监控** — Token 速率曲线、服务商健康状态、请求日志搜索
- **Android 客户端** — 局域网/公网自动切换

## 架构

```
客户端 → LLM Gateway (Rust/axum)
           ├── REST API (管理)
           ├── /v1/* 代理 (HTTP + WebSocket)
           ├── Dashboard (单文件 HTML)
           └── SQLite (数据持久化)
                    ↓
           上游 LLM 服务商 (MiniMax / 讯飞 / OpenAI / ...)
```

## 文档索引

| 文档 | 说明 |
|------|------|
| [docs/SPEC.md](docs/SPEC.md) | **完整技术规格** — 架构、数据库、API、功能详解、前端 |
| [docs/DEV_WORKFLOW.md](docs/DEV_WORKFLOW.md) | **开发指南** — 多环境架构、部署命令、Git 工作流、开发规范 |
| [docs/LESSONS_LEARNED.md](docs/LESSONS_LEARNED.md) | **经验教训** — 踩坑记录与最佳实践 |
| [AGENTS.md](AGENTS.md) | Agent 守则 |

## 配置

详见 `config.example.toml`。环境变量覆盖：`LLM_GW_SERVER_PORT`、`LLM_GW_DATABASE_URL` 等。

## 客户端接入

```bash
# Claude Code — 注意：ANTHROPIC_BASE_URL 不要带 /v1 后缀
export ANTHROPIC_API_KEY=lgk-your-gateway-key
export ANTHROPIC_BASE_URL=http://localhost:49127
export ANTHROPIC_MODEL=claude-haiku-4-5   # 可选：覆盖默认模型

# OpenCode
# providers.gateway.url = http://localhost:49127/v1
# providers.gateway.key  = lgk-your-gateway-key
```

> **Claude Code 模型名**：必须使用**统一模型 ID**（在 Dashboard → 服务商模型 里注册的名字），不是上游原始模型名。
> 当前可用的统一模型：`claude-haiku-4-5`、`claude-opus-4-7`、`MiniMax-M2.7-highspeed`、`MiniMax-M3`、`astron-code-latest`。

## Android 客户端

支持局域网/公网双网关自动切换，APK 构建方式：

```bash
cd android && ./build_apk.sh        # debug
cd android && ./build_apk.sh release # release
```

## 测试

```bash
cargo test   # 355+ 测试用例
```