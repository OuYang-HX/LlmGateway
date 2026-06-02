# LLM Gateway — 完整规格说明书

> 本文档基于 `src/` 目录下实际代码编写，真实反映实现状态。
> 采用 **Open Spec + Superpowers** 开发流程：本文档定义系统能力边界，AI Agent 辅助实现。

---

## 1. 架构概览

```
┌─────────────────────────────────────────────────────┐
│                  LLM Gateway (axum)                  │
│                                                      │
│  ┌──────────┐  ┌──────────┐  ┌────────────────┐   │
│  │Dashboard  │  │  REST    │  │  /v1/* Proxy   │   │
│  │ (HTML/JS) │  │   API    │  │   /ws/v1       │   │
│  └──────────┘  └──────────┘  └────────────────┘   │
│                    │                    │             │
│              ┌─────┴─────┐     ┌──────┴──────┐     │
│              │    DB     │     │ LlmProxy    │     │
│              │ (SQLite)  │     │             │     │
│              └───────────┘     │ ┌──────────┐ │     │
│                               │ │AuthMgr   │ │     │
│                               │ └──────────┘ │     │
│                               │ ┌──────────┐ │     │
│                               │ │StatsColl │ │     │
│                               │ └──────────┘ │     │
│                               │ ┌──────────┐ │     │
│                               │ │MockMode  │ │     │
│                               │ └──────────┘ │     │
│                               └──────────────┘     │
└─────────────────────────────────────────────────────┘
                          │
                          ▼ upstream LLM providers
```

**技术栈**
- Backend: Rust (axum 0.7, tokio, sqlx)
- Database: SQLite（通过 sqlx 连接，迁移自动执行）
- Frontend: 单文件 HTML + Vanilla JS（无框架，Chart.js CDN）
- HTTP Client: reqwest（支持 no_proxy）
- WebSocket: axum websocket

**目录结构**
```
src/
├── api/mod.rs        # 所有 REST API Handler
├── auth/mod.rs       # API Key 验证 + 动态 Token 管理
├── auth/token_refresh.rs  # Token 刷新后台任务
├── config/mod.rs     # 配置文件解析
├── dashboard/mod.rs  # Dashboard HTML 页面
├── dashboard.html    # 前端单文件（含所有 JS/CSS）
├── db/mod.rs         # 数据库层（Schema + 所有 CRUD）
├── lib.rs            # AppState 定义
├── main.rs           # 入口、路由组装、后台任务启动
├── proxy/
│   ├── mod.rs        # LlmProxy 核心（负载均衡、转发）
│   ├── handler.rs    # 请求处理（mock、streaming、usage 提取）
│   └── ws_handler.rs # WebSocket 代理
├── stats/mod.rs      # StatsCollector（内存窗口 + 快照）
├── usage/mod.rs      # Token 用量提取（OpenAI/Anthropic/DeepSeek 格式）
└── utils/mod.rs     # 工具函数
```

---

## 2. 数据库 Schema

共 **9 张表**，迁移通过代码中的 `run_migrations()` 自动执行（`ALTER TABLE` 幂等）。

### 2.1 `api_keys`

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | TEXT PK | UUID |
| `name` | TEXT | 显示名称 |
| `api_key` | TEXT UNIQUE | 完整 API Key（`lgk-` 前缀） |
| `key_prefix` | TEXT | 前 12 位（用于显示） |
| `allowed_providers` | TEXT (JSON) | 允许的服务商 ID 数组，`null`=全部 |
| `is_active` | BOOLEAN | 是否启用 |
| `created_at` | TEXT | `datetime('now')` |
| `updated_at` | TEXT | 自动更新 |

### 2.2 `providers`

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | TEXT PK | 唯一标识，如 `openai`、`minimax` |
| `name` | TEXT | 显示名称 |
| `base_url` | TEXT | 上游 Base URL，如 `https://api.openai.com/v1` |
| `api_type` | TEXT | `openai` 或 `anthropic`，默认 `openai` |
| `auth_type` | TEXT | `api_key` 或 `dynamic_token`，默认 `api_key` |
| `api_key` | TEXT | 静态 API Key |
| `token_url` | TEXT | 动态 Token 获取 URL |
| `token_username` / `token_password` | TEXT | 动态 Token 账号密码 |
| `token_request_method` | TEXT | `POST`/`GET`，默认 `POST` |
| `token_content_type` | TEXT | `json`/`form`，默认 `json` |
| `token_body_template` | TEXT | 自定义请求体模板，含 `{{username}}`/`{{password}}` 占位符 |
| `token_extra_headers` | TEXT | JSON 格式额外请求头 |
| `token_field` | TEXT | Token 在响应中的路径，默认 `token` |
| `refresh_token_field` | TEXT | Refresh Token 字段名，默认 `refreshToken` |
| `token_header_field` | TEXT | 请求头字段名，默认 `Authorization` |
| `token_header_prefix` | TEXT | 请求头前缀，默认 `Bearer ` |
| `token_expiry_seconds` | INTEGER | Token 有效期秒数，默认 86400 |
| `current_token` / `current_refresh_token` | TEXT | 当前有效 Token |
| `token_expires_at` | TEXT | Token 过期时间（RFC3339） |
| `is_active` | BOOLEAN | 是否启用 |
| `weight` | INTEGER | 负载均衡权重，默认 1 |
| `bypass_proxy` | BOOLEAN | 是否绕过系统代理（访问内网 API） |
| `mock_mode` | BOOLEAN | **模拟模式**，开启后不请求上游，返回模拟响应 |
| `response_content_path` | TEXT | 响应内容 JSON 路径，默认 `choices.0.message.content` |
| `response_reasoning_path` | TEXT | 推理内容路径，默认 `choices.0.delta.reasoning_content` |
| `chart_color` | TEXT | Dashboard 图表颜色 |
| `subscription_start` | TEXT | 配额订阅开始时间（UTC） |
| `created_at` / `updated_at` | TEXT | 时间戳 |

### 2.3 `models`（统一模型）

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | TEXT PK | 对外 ID，全局唯一，如 `gpt-4o`、`MiniMax-M2.7-highspeed` |
| `name` | TEXT | 显示名称 |
| `description` | TEXT | 模型描述 |
| `model_type` | TEXT | `chat`/`embedding` 等，默认 `chat` |
| `is_active` | BOOLEAN | 是否启用 |
| `priority` | INTEGER | 优先级（数值越小优先级越高） |
| `config` | TEXT | 扩展配置（JSON） |
| `created_at` / `updated_at` | TEXT | 时间戳 |

### 2.4 `model_mappings`（统一模型 → 服务商模型映射）

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | |
| `model_id` | TEXT FK | 统一模型 ID |
| `provider_id` | TEXT FK | 服务商 ID |
| `provider_model_id` | TEXT | 服务商提供的实际模型 ID（如 `gpt-4-0613`） |
| `is_active` | BOOLEAN | 该映射是否启用 |
| `weight` | INTEGER | 该映射权重（影响负载均衡概率） |
| `cost_multiplier` | REAL | 成本倍率，默认 1.0 |
| `created_at` | TEXT | 时间戳 |

> **唯一约束**: `(model_id, provider_id, provider_model_id)` 唯一

### 2.5 `provider_models`（服务商模型注册）

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | |
| `provider_id` | TEXT FK | 服务商 ID |
| `model_id` | TEXT | 服务商模型 ID |
| `is_active` | BOOLEAN | 是否启用 |
| `last_test_status` | TEXT | `success`/`error`/`untested` |
| `last_test_message` | TEXT | 测试消息 |
| `last_tested_at` | TEXT | 测试时间 |
| `created_at` / `updated_at` | TEXT | 时间戳 |

> **唯一约束**: `(provider_id, model_id)` 唯一

### 2.6 `request_logs`（请求日志）

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | |
| `api_key_id` | TEXT FK | 使用哪个 API Key |
| `provider_id` | TEXT FK | 请求路由到哪个服务商 |
| `model` | TEXT | 模型 ID（可能为 null） |
| `request_path` | TEXT | 请求路径，如 `/v1/chat/completions` |
| `request_method` | TEXT | HTTP 方法 |
| `request_headers` | TEXT | 请求头（JSON） |
| `request_body` | TEXT | 请求体（原始 JSON 字符串） |
| `response_status` | INTEGER | HTTP 响应状态码 |
| `response_headers` | TEXT | 响应头（JSON） |
| `response_body` | TEXT | 响应体（原始 JSON） |
| `prompt_tokens` / `completion_tokens` / `total_tokens` | INTEGER | Token 统计 |
| `duration_ms` | INTEGER | 耗时毫秒 |
| `is_streaming` | BOOLEAN | 是否流式请求 |
| `is_throttled` | BOOLEAN | 是否被限流 |
| `error_message` | TEXT | 错误信息 |
| `created_at` | TEXT | 时间戳 |

### 2.7 `token_rate_snapshots`（Token 速率快照）

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | |
| `provider_id` | TEXT FK | 服务商 ID |
| `tokens_per_second` | REAL | 该快照周期的 Token 速率 |
| `prompt_tokens` / `completion_tokens` | INTEGER | 该周期 Token 数 |
| `request_count` | INTEGER | 该周期请求数 |
| `elapsed_seconds` | REAL | 周期时长（秒） |
| `snapshot_time` | TEXT | 快照时间 |

### 2.8 `provider_quotas`（服务商配额）

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | |
| `provider_id` | TEXT FK | 服务商 ID |
| `quota_type` | TEXT | 配额类型，如 `fixed:5h`、`sliding:7d`、`rolling:1h:15m` |
| `window_mode` | TEXT | `fixed`/`sliding`/`rolling`，默认 `fixed` |
| `window_size` | TEXT | 窗口大小，如 `5h`、`7d` |
| `window_start_override` | TEXT | 手动指定窗口开始时间 |
| `limit_count` | INTEGER | 配额上限 |
| `is_enabled` | BOOLEAN | 是否启用 |
| `rolling_step` | TEXT | 滑动步长（如 `15m`，仅 rolling 模式） |
| `rolling_step_tz` | TEXT | 滑动步长时区（如 `Asia/Shanghai`） |
| `created_at` / `updated_at` | TEXT | 时间戳 |

> **配额类型格式**：
> - 固定窗口：`fixed:5h` / `fixed:7d` / `fixed:30d`
> - 滑动窗口：`sliding:5h`（从当前时刻往前滑动）
> - 步进滑动窗口：`rolling:1h:15m` = 1小时窗口，每15分钟步进

### 2.9 `provider_quota_calibrations`（配额校准）

| 列名 | 类型 | 说明 |
|------|------|------|
| `id` | INTEGER PK | |
| `provider_id` | TEXT FK | 服务商 ID |
| `quota_type` | TEXT | 配额类型 |
| `calibration_offset` | INTEGER | 校准偏移量（外部已用量） |
| `calibrated_at` | TEXT | 校准时间 |
| `calibration_window_start` / `calibration_window_end` | TEXT | 校准时的窗口范围 |
| `note` | TEXT | 备注 |

---

## 3. REST API 完整清单

所有路径前缀 `/api/v1`，响应格式均为 JSON。

### 3.1 API Key 管理

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/api-keys` | 创建 API Key |
| `GET` | `/api-keys` | 列出所有 API Key |
| `GET` | `/api-keys/:id` | 获取单个 API Key |
| `PUT` | `/api-keys/:id` | 更新 API Key（名称、允许服务商、状态） |
| `DELETE` | `/api-keys/:id` | 删除 API Key |
| `POST` | `/api-keys/:id/regenerate` | 重新生成 API Key（保留 ID） |
| `POST` | `/api-keys/batch-update-status` | 批量启用/停用 API Key |

### 3.2 服务商管理

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/providers` | 创建服务商 |
| `GET` | `/providers` | 列出所有服务商 |
| `GET` | `/providers/:id` | 获取服务商详情（含 mock_mode 字段） |
| `PUT` | `/providers/:id` | 更新服务商（支持 mock_mode 切换） |
| `DELETE` | `/providers/:id` | 删除服务商 |
| `POST` | `/providers/batch-update-status` | 批量启用/禁用服务商 |
| `POST` | `/providers/:id/refresh-token` | 手动刷新动态 Token |
| `PUT` | `/providers/:id/chart-color` | 更新图表颜色 |

**服务商模型管理**：

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/providers/:id/models` | 列出该服务商的所有模型 |
| `POST` | `/providers/:id/models` | 添加服务商模型 |
| `DELETE` | `/providers/:id/models/:model_id` | 删除服务商模型 |
| `POST` | `/providers/:id/models/:model_id/test` | 测试单个模型连通性 |
| `POST` | `/providers/:id/models/test-all` | 测试所有模型 |

### 3.3 统一模型管理

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/models` | 创建统一模型 |
| `GET` | `/models` | 列出所有统一模型 |
| `GET` | `/models/:id` | 获取模型详情（含 mappings 嵌套） |
| `PUT` | `/models/:id` | 更新模型 |
| `DELETE` | `/models/:id` | 删除模型（级联删除 mappings） |

**模型映射管理**：

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/models/:id/mappings` | 列出该模型的所有映射 |
| `POST` | `/models/:id/mappings` | 添加映射（model_id + provider_id + provider_model_id） |
| `PUT` | `/models/:id/mappings/:provider_id` | 更新映射（is_active、weight、cost_multiplier） |
| `DELETE` | `/models/:id/mappings/:provider_id` | 删除映射 |

### 3.4 配额管理

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST` | `/quotas/:provider_id` | 设置/更新配额（支持 fixed/sliding/rolling） |
| `GET` | `/quotas/:provider_id` | 获取配额使用情况 |
| `DELETE` | `/quotas/:provider_id/:quota_type` | 删除配额 |
| `POST` | `/quotas/:provider_id/calibration` | 校准配额（设置外部已用量偏移） |
| `GET` | `/quotas/:provider_id/calibration` | 获取校准记录 |
| `GET` | `/quotas/usage` | 获取所有配额使用情况 |

### 3.5 统计与日志

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/stats` | 全局统计（总请求、总 Token、活跃数等） |
| `GET` | `/stats/bucketed` | 分桶统计（支持 1h/4h/1d/7d/30d 粒度） |
| `GET` | `/stats/by-api-key` | 按 API Key 分组统计（支持时间过滤） |
| `GET` | `/stats/usage-trend` | 用量趋势（支持 days 参数） |
| `GET` | `/logs` | 请求日志列表（支持分页、搜索、过滤） |
| `GET` | `/logs/:id` | 日志详情（包含 request_body 和 response_body） |
| `DELETE` | `/logs/:id` | 删除单条日志 |
| `POST` | `/logs/batch-delete` | 批量删除日志（按过滤条件） |
| `POST` | `/logs/delete-all` | 清空所有日志 |

### 3.6 Dashboard API

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` | `/dashboard/summary` | 概览摘要（API Key 数、请求数、Token 数等） |
| `GET` | `/dashboard/health` | 服务商健康状态（24h 请求数、错误率、限流次数、平均延迟） |
| `GET` | `/dashboard/top-provider` | 请求量最高的服务商 |
| `GET` | `/dashboard/token-rate` | Token 速率数据（最近 1 小时快照） |

### 3.7 LLM 代理端点

| 方法 | 路径 | 说明 |
|------|------|------|
| `POST/GET` | `/v1/*path` | 代理到上游（/v1/chat/completions、/v1/models 等） |
| `GET` | `/ws/v1` | WebSocket 代理 |

---

## 4. 核心功能详解

### 4.1 认证流程

**静态 API Key 认证**（默认）：
1. 请求头 `Authorization: Bearer <api_key>` 传入
2. `AuthManager` 验证 API Key 存在且 `is_active = true`
3. 检查 `allowed_providers`：若非空，仅允许访问指定服务商

**动态 Token 认证**：
1. 网关启动时从 `token_url` 获取 Token（POST 账号密码）
2. Token 缓存在 `providers.current_token`，过期前 5 分钟自动刷新
3. 支持 Refresh Token 续期
4. 支持自定义请求体模板（`{{username}}`/`{{password}}` 占位）
5. 支持额外请求头（如 `X-App-Id`）

**Token 刷新后台任务**：每 60 秒检查一次所有动态 Token 提供商，主动刷新即将过期的 Token。

### 4.2 负载均衡

**选择算法**：加权轮询（Weighted Round-Robin）

```
1. 收集所有 is_active=true 的服务商
2. 计算总权重 = Σ(provider.weight)
3. 随机数 [0, total_weight)，按权重累积选择服务商
4. 若指定 allowed_providers，仅从允许列表中选择
5. 若选中的服务商模型列表为空或不活跃，失败
```

**Provider Selection 时机**：
- 用户请求中指定统一模型 ID
- 从 `model_mappings` 中获取该模型所有可用映射
- 按权重在映射间选择（权重来自 mapping.weight）

### 4.3 Mock 模拟模式

当 `provider.mock_mode = true` 时，请求**完全跳过上游调用**：

```
用户请求 → LlmProxy.select_provider() → 检测 mock_mode
  → handle_mock_request()
    → generate_mock_content(target_len)  # 从预设句子库随机拼接
    → 记录 request_log（prompt_tokens 估算 + completion_tokens 估算）
    → record_usage() 到 StatsCollector
    → 返回模拟 JSON 响应
```

**Mock 响应格式**（非流式）：
```json
{
  "id": "chatcmpl-uuid",
  "object": "chat.completion",
  "created": 1234567890,
  "model": "<model_id>",
  "choices": [{"index": 0, "message": {"role": "assistant", "content": "<模拟内容>"}, "finish_reason": "stop"}],
  "usage": {"prompt_tokens": N, "completion_tokens": M, "total_tokens": N+M}
}
```

**Mock 响应格式**（流式）：分批发送 SSE chunks，每批 ~8 字符，间隔 15ms，模拟打字效果。

**用途**：开发/测试阶段，不消耗真实 LLM Token。

### 4.4 用量提取

支持从响应中提取 Token 用量：

**OpenAI 格式**（`response.usage` 字段）：
```json
{"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30}
```

**Anthropic 格式**（从 `content_block` 计算）：
```json
{"type": "message", "usage": {"input_tokens": 10, "output_tokens": 20}}
```

**DeepSeek/o1 格式**：从响应体内容估算字符数 ÷ 2.5。

**流式 SSE**：从最终的 `data: [DONE]` 前一条包含 `usage` 的 chunk 中提取。

**Fallback**：若响应无 usage 字段，从内容估算：`字符数 ÷ 2.5`

### 4.5 配额管理

**固定窗口（fixed）**：配额在窗口起点初始化，窗口内累减，过期重置。

```
窗口: [period_start, period_start + window_size)
配额: limit_count - (已用量 + calibration_offset)
```

**滑动窗口（sliding）**：配额从当前时刻往前取 window_size 持续滑动。

**步进滑动窗口（rolling）**：
- 窗口大小为 `window_size`（如 1 小时）
- 步长为 `rolling_step`（如 15 分钟）
- 步进对齐到 `rolling_step_tz` 时区的整点（如上海时区 00:00、00:15、00:30...）
- 支持配额校准（手动设置外部已用量偏移）

**配额检查时机**：在 `select_provider` 之前，若配额耗尽则拒绝请求。

### 4.6 请求日志

**记录内容**：
- 完整请求体和响应体（JSON 字符串，存 TEXT 列）
- Token 统计（从响应提取或估算）
- 耗时（从请求到收到完整响应）
- 是否流式、是否限流、错误信息

**查询参数**（`GET /api/v1/logs`）：
- `api_key_id`、`provider_id`、`model`（精确过滤）
- `search`（搜索 request_body 和 response_body）
- `start_time`、`end_time`（时间范围）
- `status_filter`（响应状态码过滤）
- `page`、`page_size`（分页）

### 4.7 流式（SSE）处理

**非流式路径**：`forward_and_collect()` → 转发请求 → 等待完整响应 → 提取 usage → 返回 JSON

**流式路径**：`forward_streaming()` → 转发请求 → 流式转发 SSE chunks → 实时提取 usage → 最终 chunk 记录日志

**SSE 错误检测**：解析 SSE chunk 中的 JSON，若包含 `error` 字段，则标记为错误。

**Thinking Tag 剥离**：MiniMax 等模型的 `<think>...</think>` 标签会被剥离，不写入 response_body。

### 4.8 WebSocket 代理

- 接收第一个 Text/Binary 消息作为 LLM 请求
- 解析出 `model`、`messages`、`api_key`
- 验证 API Key
- 转发到上游 `/v1/chat/completions`
- 流式转发响应 chunks

---

## 5. 前端 Dashboard

单文件 `src/dashboard.html`，Vanilla JS，Chart.js CDN，6 个 Tab：

### 5.1 概览（tab-overview）
- 统计卡片：活跃 API Key 数、24h 请求数、24h Token 数、24h 限流次数
- **服务商健康表**（provider-health-table）：24h 请求数、错误数、限流次数、平均延迟、最近请求时间
  - 服务商名称**可点击**，点击跳转到日志 Tab 并过滤该服务商
- **服务商状态表**（overview-provider-table）：显示所有服务商，支持批量启用/禁用
- **API Key 使用量柱状图**（stats-by-apikey）：支持 24h / 7d / 30d 时间范围，stacked prompt/completion token
- **Token 速率曲线**：输出 Token 速率 + 输入 Token 速率双曲线，支持显示/隐藏各服务商

### 5.2 服务商管理（tab-providers）
- 服务商列表（ID、名称、Base URL、认证方式、权重、状态、Token 状态、Mock 状态、操作按钮）
- **Mock 列**：显示黄色 `Mock` 徽标或灰色 `-`
- 创建/编辑服务商 Modal：
  - 静态 API Key 区域（API Key、Header 字段名、前缀）
  - 动态 Token 区域（Token URL、账号密码、请求方式、字段名、过期秒数、Body 模板、额外请求头）
  - 响应路径配置（content_path、reasoning_path）
  - 订阅信息（subscription_start）
  - **模拟模式开关**（是/否）
- 模型管理子面板：添加/删除服务商模型，设置对外 ID

### 5.3 API Key 管理（tab-apikeys）
- API Key 列表（名称、Key、允许服务商、状态、创建时间）
- **全选复选框**：表头全选，支持批量操作
- **批量启用/停用按钮**：一键启用或停用选中的 API Key
- **状态快捷切换**：点击状态徽标即可切换启用/停用
- 创建/编辑 Modal：名称、允许服务商复选框、状态
- 重新生成按钮（保留 ID 和名称）
- 复制 Key 到剪贴板
- **概览页 API Key 表格**：显示启用/停用按钮，点击状态徽标可切换

### 5.4 用量配额（tab-quotas）
- 配额列表（服务商、窗口模式、窗口大小、配额上限、已用、剩余百分比）
- **配额详情 Modal**：
  - 窗口模式选择（固定/滑动/步进滑动窗口）
  - 窗口大小输入（如 `5h`、`7d`）
  - 步长输入（如 `15m`，仅 rolling 模式）
  - 时区选择（如 `Asia/Shanghai`）
  - 限流次数输入
  - 校准功能（设置外部已用量偏移）
  - 显示校准窗口期范围

### 5.5 请求日志（tab-logs）
- **过滤栏**：时间范围（时间预设 1h/24h/7d/30d + 自定义）、服务商下拉、API Key 下拉、模型过滤、搜索框
- **日志列表**（点击表头可排序：时间、状态码、输入 Token、输出 Token、耗时）
- **分页控件**：上一页/下一页、每页条数选择、总条数显示
- **CSV 导出**：导出匹配过滤条件的最多 10,000 条日志（UTF-8 BOM）
- **日志详情 Modal**：
  - 显示请求体/响应体（带复制按钮）
  - JSON 格式化展示
  - 模型名称可点击（点击过滤日志）
  - 请求 ID 复制

### 5.6 统计分析（tab-stats）
- 时间桶统计（1h / 4h / 1d / 7d / 30d 粒度切换）
- 请求数、Token 数、错误数、限流数、平均延迟的时序折线图
- 支持按服务商过滤

---

## 6. 配置

环境变量或配置文件（`config.yaml`）：

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `LLM_GW_DATABASE_URL` | `sqlite:llm_gateway.db` | 数据库连接 URL |
| `LLM_GW_SERVER_HOST` | `0.0.0.0` | 监听地址 |
| `LLM_GW_SERVER_PORT` | `8080` | 监听端口 |
| `RUST_LOG` | `info` | 日志级别 |

---

## 7. 测试覆盖

测试文件位于 `tests/` 目录，共 355+ 个测试用例：

| 测试文件 | 覆盖范围 |
|----------|----------|
| `integration_test.rs` | 数据库 CRUD、代理逻辑、配额、模型映射、Token 刷新 |
| `http_api_test.rs` | 所有 REST API 端点的 HTTP 测试 |
| `models_api_test.rs` | 统一模型和映射的 CRUD |
| `dashboard_html_test.rs` | Dashboard HTML 结构和 JS 函数存在性验证 |
| `usage_test.rs` | Token 用量提取（OpenAI/Anthropic/流式/空响应等） |
| `token_rate_stats_test.rs` | StatsCollector 快照和内存窗口 |
| `token_refresh_test.rs` | 动态 Token 刷新逻辑 |
| `sse_error_test.rs` | SSE 错误检测和 thinking tag 剥离 |
| `usage_integration_test.rs` | 端到端用量提取集成测试 |
| `model_cleanup_test.rs` | 模型和映射级联删除 |

---

## 8. 已废弃/历史字段兼容

以下旧格式在迁移中自动转换：

| 旧 quota_type | 新格式 |
|---------------|--------|
| `5h` | `sliding:5h` |
| `weekly` | `fixed:7d` |
| `monthly` | `fixed:30d` |

---

## 9. 已知限制

1. **单数据库**：当前仅支持 SQLite，不支持多实例共享同一数据库
2. **无用户/权限体系**：API Key 是顶层凭证，无细粒度权限控制
3. **配额无硬实时保证**：配额检查基于数据库查询，高并发下存在竞态条件
4. **Token 估算**：无 usage 字段时使用字符数 ÷ 2.5 估算，与实际 Token 数有偏差
5. **Mock 响应内容**：预定义句子库，内容固定，不基于 prompt 生成

---


---

## 10. 需求：服务商模型支持多个对外模型 ID

### 背景

当前 `model_mappings` 表的唯一约束为 `(model_id, provider_id)`，即同一统一模型下同一服务商只能有一条映射。
这导致同一服务商模型（如 `gpt-4o`）只能映射到一个对外 ID，无法同时暴露为多个对外 ID（如 `my-gpt4o` 和 `company-gpt4o`）。

### 数据库变更

1. **`model_mappings` 表唯一约束变更**：
   - 旧约束：`UNIQUE(model_id, provider_id)`
   - 新约束：`UNIQUE(model_id, provider_id, provider_model_id)`
   - 意义：允许同一服务商的同一统一模型下有多条映射（不同 `provider_model_id`），也允许同一服务商模型映射到多个不同的统一模型

2. **迁移 SQL**：
   ```sql
   -- 先删除旧唯一索引
   DROP INDEX IF EXISTS idx_model_mappings_unique;
   -- SQLite 不支持 ALTER CONSTRAINT，需重建表
   ```
   由于 SQLite 不支持直接修改约束，采用重建表策略。

### API 变更

1. **`DELETE /api/v1/models/:id/mappings/:provider_id`** → 改为 `DELETE /api/v1/models/:id/mappings/:provider_id/:provider_model_id`
   - 需要同时指定 `provider_model_id` 才能唯一确定一条映射

2. **`POST /api/v1/providers/:id/models`** 保持不变

### 核心逻辑变更

1. `db::remove_model_mapping()` 签名增加 `provider_model_id` 参数
2. `db::cleanup_mappings_for_provider_model()` 无需改动（已按 `provider_model_id` 匹配）
3. `db::add_model_mapping()` 无需改动（INSERT 已包含 `provider_model_id`）

### 前端变更

1. 模型管理页面的"对外ID"列改为支持显示多个对外 ID
2. "设置对外ID"按钮改为可追加多个对外 ID
3. 删除对外 ID 时需指定具体的映射

### 测试用例

1. 同一服务商模型映射到多个不同统一模型 ✅
2. 同一统一模型下同一服务商可有多条不同 `provider_model_id` 的映射 ✅
3. 删除映射需指定 `provider_model_id` ✅
4. 原有功能不受影响 ✅

*文档版本: 2026-05-24 | 代码版本对应: dev 分支 latest commit*

---

## 变更：Anthropic API 格式转发支持

### 背景

大模型服务商通常提供 OpenAI 和 Anthropic 两种 API 格式。当前 Gateway 仅支持 OpenAI 格式转发（`/v1/chat/completions`），需要增加对 Anthropic 格式（`/v1/messages`）的原生转发支持。

### 核心差异

| 维度 | OpenAI 格式 | Anthropic 格式 |
|------|------------|----------------|
| 端点 | `/v1/chat/completions` | `/v1/messages` |
| 认证 | `Authorization: Bearer <key>` | `x-api-key: <key>` + `anthropic-version: 2023-06-01` |
| 请求体 | `{"model","messages",...}` | `{"model","messages","max_tokens",...}` |
| Streaming | `data: {"choices":[{"delta":{}}]}` | `event: content_block_delta\ndata: {"delta":{"text"}}` |
| 用量 | `usage.prompt_tokens/completion_tokens` | `usage.input_tokens/output_tokens` |
| 错误 | `{"error":{"message":"..."}}` | `{"type":"error","error":{"type":"...","message":"..."}}` |

### 设计原则

**Gateway 只做透明转发 + 智能路由 + 用量提取**，不做格式转换。

**同一个服务商两种格式的对接方式：创建两个 Provider。**

理由：
1. 两种格式的 **base_url 不同**（如 AWS Bedrock 的 OpenAI/Anthropic 端点 URL 不同）
2. 两种格式的 **认证方式可能不同**（Anthropic 用 `x-api-key`，OpenAI 用 `Bearer`）
3. 两种格式的 **api_type 不同**（决定了 header 注入、用量提取、SSE 解析逻辑）
4. 两种格式的 **计费/配额可能不同**

配置示例：

```
Provider: aws-bedrock-openai
  base_url: https://bedrock-runtime.us-east-1.amazonaws.com/openai/v1
  api_type: openai
  auth_type: api_key
  token_header_field: Authorization
  token_header_prefix: Bearer 

Provider: aws-bedrock-anthropic  
  base_url: https://bedrock-runtime.us-east-1.amazonaws.com/anthropic/v1
  api_type: anthropic
  auth_type: api_key
  token_header_field: x-api-key
  token_header_prefix: (空)
```

然后通过模型映射，把同一个统一模型映射到两个 Provider：

```
统一模型: claude-3.5-sonnet
  → aws-bedrock-openai / claude-3-5-sonnet  (weight: 1)
  → aws-bedrock-anthropic / claude-3-5-sonnet-20241022  (weight: 1)
```

**智能路由**：Gateway 根据客户端请求的 API 格式自动选择匹配的 provider：
- 客户端请求 `/v1/chat/completions` → 优先选择 `api_type=openai` 的 provider
- 客户端请求 `/v1/messages` → 优先选择 `api_type=anthropic` 的 provider
- 无匹配时 fallback 到任意可用 provider

### 数据库变更

无。`providers` 表已有 `api_type` 字段（`TEXT NOT NULL DEFAULT 'openai'`），当前支持 `"openai"` / `"anthropic"` / `"custom"`。

### API 变更

1. Provider 创建/更新 API 已支持 `api_type` 字段，无需变更
2. Dashboard 前端 provider 表单的 `api_type` 下拉增加 `anthropic` 选项（当前仅有 `openai`）

### 核心逻辑变更

#### 1. 认证头适配 (`auth/mod.rs`)

```rust
// get_auth_header() 中根据 provider.api_type 返回不同认证头
match provider.api_type.as_str() {
    "anthropic" => ("x-api-key".into(), api_key.clone()),
    _ => ("Authorization".into(), format!("Bearer {}", api_key)),
}
```

Anthropic 额外需要 `anthropic-version` 头，在 proxy 转发时注入。

#### 2. 请求头注入 (`proxy/mod.rs`)

在 `forward_and_collect` / `forward_streaming` / `forward_streaming_no_log` 中，当 `api_type == "anthropic"` 时：
- 注入 `anthropic-version: 2023-06-01` 头
- 不剥离 `content-type` 头（Anthropic 需要 `application/json`）

#### 3. 用量提取适配 (`usage/mod.rs`)

新增 `extract_anthropic_usage()` 函数：
- 从 Anthropic 响应中提取 `usage.input_tokens` / `usage.output_tokens`
- 映射到 Gateway 的 `prompt_tokens` / `completion_tokens`

```rust
pub fn extract_anthropic_usage(response: &Value) -> (i64, i64, i64) {
    let usage = response.get("usage");
    match usage {
        Some(u) => {
            let input = u.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            let output = u.get("output_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
            (input, output, input + output)
        }
        None => (0, 0, 0),
    }
}
```

在 `forward_and_collect` 中根据 `api_type` 选择提取函数。

#### 4. SSE 流处理适配 (`proxy/handler.rs`)

Anthropic SSE 格式与 OpenAI 不同：
- OpenAI: `data: {"choices":[{"delta":{"content":"..."}}]}`
- Anthropic: `event: content_block_delta\ndata: {"type":"content_block_delta","delta":{"type":"text_delta","text":"..."}}`

在 `proxy_request` 的流处理分支中，当 `api_type == "anthropic"` 时：
- 解析 `event:` 行判断事件类型
- 从 `content_block_delta` 事件中提取 `delta.text` 作为内容
- 从 `message_delta` 事件中提取 `usage.output_tokens` 作为用量
- 最终 `[DONE]` 对应 Anthropic 的 `event: message_stop`

#### 5. 路径映射

当前逻辑已正确：`/v1/messages` 请求进来后，`strip_prefix("/v1")` 得到 `/messages`，拼接 `base_url` 得到 `https://api.anthropic.com/v1/messages`。

Anthropic 的 `base_url` 应配置为 `https://api.anthropic.com/v1`，这样 `/v1/messages` → strip `/v1` → `/messages` → `base_url + /messages` = `https://api.anthropic.com/v1/messages`。

### 前端变更

1. Provider 表单的 `api_type` 下拉增加 `anthropic` 选项
2. Provider 表单在 `api_type == "anthropic"` 时提示用户 `base_url` 应填写 Anthropic 的 API 地址
3. Provider 列表增加 API 类型标签显示

### 测试用例

1. `extract_anthropic_usage` — 标准/部分/空响应
2. `extract_anthropic_streaming_usage` — message_delta 事件
3. Anthropic 认证头 — `x-api-key` 而非 Bearer
4. Anthropic SSE 内容提取 — `content_block_delta` 事件
5. Anthropic 错误格式识别 — `type: "error"`
6. 集成测试：provider `api_type=anthropic` 全链路转发
7. Dashboard HTML 验证：anthropic 选项、类型标签

---

## 变更：Provider 分组（group_id）— 同一服务商多格式共享统计与配额

### 背景

同一服务商提供 OpenAI 和 Anthropic 两种 API 格式时，需要创建两个 Provider（因为 base_url、认证方式不同）。但统计和配额应该按服务商维度聚合，而不是按单个 provider 拆分。

### 设计

给 `providers` 表增加 `group_id` 字段：

- `group_id` 为 NULL 时，按自身 `id` 计算统计和配额（向后兼容）
- `group_id` 有值时，同组的 provider 共享统计和配额
- `group_id` 本身不必对应某个真实 provider id，只是一个分组标签

**示例**：

```
Provider: bedrock-openai     → group_id: "bedrock"
Provider: bedrock-anthropic  → group_id: "bedrock"

→ 统计 /v1/chat/completions 和 /v1/messages 的总用量
→ 配额在 "bedrock" 组上设一次，两个 provider 都消耗同一份
```

### 数据库变更

1. `providers` 表增加 `group_id TEXT` 列（NULLable，无默认值）
2. 新增 migration: `ALTER TABLE providers ADD COLUMN group_id TEXT`

### API 变更

1. `CreateProviderRequest` 增加 `group_id: Option<String>`
2. `UpdateProviderRequest` 增加 `group_id: Option<String>`
3. `create_provider` / `update_provider` 签名增加 `group_id: Option<&str>` 参数

### 核心逻辑变更

#### 1. request_logs — 写入时用 group_id 替代 provider_id

在 `insert_request_log` 中：
- 查 provider 的 `group_id`
- 如果有 `group_id`，写入 `group_id` 作为 `provider_id`
- 如果没有，写入原始 `provider_id`

这样 `request_logs` 天然按组聚合，统计和配额查询无需修改。

#### 2. count_provider_requests — 无需修改

因为 `request_logs.provider_id` 已经是 group_id，现有查询直接按组统计。

#### 3. provider_quotas — 查询时用 group_id

配额查询时：
- 如果 provider 有 `group_id`，查 `provider_quotas WHERE provider_id = group_id`
- 如果没有，查 `provider_quotas WHERE provider_id = provider.id`

配额设置在 group_id 上（如 `bedrock`），两个 provider 都会找到并消耗同一份配额。

### 前端变更

1. Provider 表单增加 `group_id` 输入框（可选）
2. Provider 列表显示分组标签
3. 统计和配额页面按组聚合显示

### 测试用例

1. `db_provider_group_id` — 创建/更新 provider 设置 group_id
2. `db_request_log_grouped` — 同组 provider 的请求日志写入 group_id
3. `db_quota_shared_by_group` — 同组 provider 共享配额
4. `db_stats_grouped` — 同组 provider 的统计聚合
5. 向后兼容：group_id 为空时行为不变

---

## 变更:客户端认证兼容 `x-api-key` 头(Claude Code 接入)

### 背景

Claude Code CLI 的官方 Anthropic SDK 在 `ANTHROPIC_API_KEY` 被设置时,会用 `x-api-key: <key>` 头对 API 请求做认证;同时也会注入 `anthropic-version: 2023-06-01` 头。

当前网关 `extract_api_key()` 只解析 `Authorization` 头,导致 Claude Code 发出 `x-api-key` 头时返回 401 "Missing API key"。

实测复现(修复前):

```
$ curl -i -H 'x-api-key: lgk-...' -H 'anthropic-version: 2023-06-01' \
       -H 'Content-Type: application/json' \
       -X POST http://127.0.0.1:49127/v1/messages \
       -d '{"model":"claude-haiku-4-5","messages":[...], "max_tokens":20}'

HTTP/1.1 401 Unauthorized
content-length: 15
Missing API key
```

### 设计

扩展 `proxy::handler::extract_api_key()` 的解析顺序:

1. `x-api-key` 头(Claude Code / Anthropic SDK 默认)
2. `Authorization: Bearer <key>`(OpenAI 兼容客户端、`ANTHROPIC_AUTH_TOKEN`)
3. `Authorization: <raw>`(无前缀的旧格式,向后兼容)

`x-api-key` 优先是因为:
- Claude Code 一次只会发出一种认证头(要么 `x-api-key` 要么 `Authorization`),所以两者互斥。
- 把 Anthropic 原生格式放在最前,能让"配置 `ANTHROPIC_API_KEY`"的场景零改动跑通。

数据库无变更。API 无变更。Provider 鉴权头(`token_header_field`)逻辑不变。

### 核心逻辑变更

`src/proxy/handler.rs::extract_api_key()`:

```rust
pub fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    // 1. Claude Code / Anthropic SDK 默认头
    if let Some(v) = headers.get("x-api-key").and_then(|h| h.to_str().ok()) {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }
    // 2. OpenAI 风格 / ANTHROPIC_AUTH_TOKEN
    let auth_header = headers.get("authorization")?.to_str().ok()?;
    if auth_header.starts_with("Bearer ") {
        Some(auth_header[7..].to_string())
    } else {
        Some(auth_header.to_string())
    }
}
```

### 测试用例

1. `test_extract_api_key_x_api_key` — `x-api-key: sk-test-123` 正确提取
2. `test_extract_api_key_x_api_key_preferred` — 同时有 `x-api-key` 和 `Authorization` 时优先 `x-api-key`
3. `test_extract_api_key_x_api_key_empty` — `x-api-key: `(空值)回退到 Authorization
4. 原有 4 个测试用例保持通过(向后兼容)

### 客户端接入指南

Claude Code 用户最小配置(写入 `~/.bashrc` / shell rc):

```bash
export ANTHROPIC_API_KEY=lgk-<your-key>
export ANTHROPIC_BASE_URL=http://<gateway-host>:49127/v1
export ANTHROPIC_MODEL=claude-haiku-4-5   # 可选:覆盖默认模型
```

模型 ID 必须是网关里已注册的**统一模型 ID**(如 `claude-haiku-4-5`、`MiniMax-M3`),不是上游原始模型名。

> **重要**:`ANTHROPIC_BASE_URL` **不要**带 `/v1` 后缀。Anthropic SDK 内部会拼上 `/v1/messages`,带 `/v1` 会得到 `/v1/v1/messages` 双 v1 路径,axum 路由不会匹配。
>
> 可用的统一模型:`claude-haiku-4-5`、`claude-opus-4-7`、`MiniMax-M2.7-highspeed`、`MiniMax-M3`、`astron-code-latest`(以 Dashboard → 服务商模型 页面为准)。

---

## 变更:`test_model_connection` 支持 anthropic 格式 provider

### 背景

Dashboard 的"测试连接"功能(`POST /api/v1/providers/:id/models/:model_id/test` 和 `test-all`)对所有 provider **写死**用 OpenAI 格式请求:

```rust
let target_url = format!("{}/chat/completions", base_url);
let test_body = json!({"model":..., "messages":[...], "max_tokens":1});
```

这对 `api_type=openai` 的 provider 没问题,但对 `api_type=anthropic` 的 provider(如 `MiniMax-anthropic` 配置 `base_url=https://api.minimaxi.com/anthropic/v1`)会 404,因为:

- 上游 Anthropic 端点**只接受 `/v1/messages`**,不接受 `/chat/completions`
- Anthropic 协议需要 `anthropic-version: 2023-06-01` 头,测试代码不注入

**复现**:

```
$ curl -X POST http://127.0.0.1:49127/api/v1/providers/MiniMax-anthropic/models/MiniMax-M2.7-highspeed/test
{"status":"failed","message":"HTTP 404: 404 page not found",...}
```

但**实际代理路径**(`/v1/messages` 走 `proxy_request` handler)是**正常**的——`request_logs` 显示 Claude Code 调用 MiniMax-anthropic 全部 200。问题仅在 dashboard 测试功能。

### 设计

`test_model_connection` 根据 `provider.api_type` 选协议:

| `api_type` | URL 路径 | 额外头 | Body 字段 |
|------------|----------|--------|-----------|
| `anthropic` | `/messages` | `anthropic-version: 2023-06-01` | 同 OpenAI 格式(`model`/`messages`/`max_tokens` 都是 Anthropic 协议的合法字段) |
| 其他(含 `openai`) | `/chat/completions` | 无 | OpenAI 格式 |

重构为两个步骤:
1. **构建请求**(纯函数):`build_test_request(provider, model_id) -> (url, headers, body)`
2. **执行请求**:`send_test_request(client, url, headers, body) -> Result<...>`

纯函数 `build_test_request` 易单元测试,覆盖 openai / anthropic 两种分支。

### 核心逻辑变更

`src/api/mod.rs::test_model_connection` 内部:

```rust
let (path_suffix, extra_headers) = match provider.api_type.as_str() {
    "anthropic" => ("/messages", vec![("anthropic-version", "2023-06-01")]),
    _ => ("/chat/completions", vec![]),
};
let target_url = format!("{}{}", base_url.trim_end_matches('/'), path_suffix);

let mut req_builder = client.post(&target_url)
    .header(&auth_header_name, &auth_header_value)
    .header("Content-Type", "application/json");
for (k, v) in extra_headers {
    req_builder = req_builder.header(k, v);
}
req_builder = req_builder.json(&test_body);
```

### 测试用例

`tests/proxy_util_test.rs` 增补(如果 build_test_request 被公开)或新建 `tests/api_test.rs`:

1. `test_build_test_request_anthropic` — 路径 = `base_url + /messages`,headers 含 `anthropic-version`
2. `test_build_test_request_openai` — 路径 = `base_url + /chat/completions`,无 `anthropic-version`
3. 集成测试:启动 mock upstream,跑 dashboard 测试 API,验证 anthropic 格式返回 200 / OpenAI 格式返回 200

---

## 变更:Provider 配置 `strip_thinking_tags_in_response` 剥离 `<think>` 块

### 背景

`MiniMax-M3` 走 OpenAI 协议(`/v1/chat/completions`)时,响应里 **thinking 内容用 `<think>...</think>` markdown 标签直接放在 `content` 字段**——这违反 OpenAI 2025 reasoning 字段标准(标准应放 `reasoning_content` 字段)。

实测:
```json
{"choices":[{"message":{
  "content": "<think>\nThe user wants...\n</think>\nRust is a modern...",
  "role": "assistant"
}}]}
```

**对客户端的影响**:
- pi agent 等 client 把整段 content 渲染给用户,thinking 部分也显示
- thinking 内容可能 1-3K 字符,加上终端 buffer 限制,用户感知"agent 经常截断"
- 实际上功能正常(工具仍被调用),只是显示问题

**与 Anthropic 协议路径不同**:Anthropic 协议下 MiniMax 用结构化 `type: thinking` content block,SDK 能正确解析;OpenAI 协议下用 markdown 标签混杂,SDK/客户端无法区分。

### 设计

加 provider 配置 `strip_thinking_tags_in_response: bool`(默认 `false`,保持协议透明)。

**作用域**:
- 仅对 OpenAI 协议响应生效(Anthropic 协议响应是结构化 block,SDK 能解析,不需要 strip)
- 仅在 `forward_and_collect` (non-streaming) 路径生效
- streaming 路径留 TODO(复杂,需要状态机 buffer;短期影响小因为 pi agent 走 non-streaming)

**实现**:
- 在 `proxy_request` non-streaming 路径,拿到 `proxy_response.body` 后
- 检查 `provider.strip_thinking_tags_in_response` && `provider.api_type == "openai"`
- parse JSON,找到 `choices[*].message.content`
- 用现有 `handler::strip_thinking_tags` 函数去除 `<think>...</think>` 块
- 重新序列化,替换 body
- 日志记录字段使用**原始**(未 strip)的 content(让 dashboard 仍能看到 thinking)

**为什么 opt-in**:
- 默认 `false` 保持"透明转发"原则
- 开启的副作用:dashboard 日志里 response_body 仍是 thinking 内容(因为日志用原始),但客户端收到的 content 字段已 strip
- 留 dashboard 完整审计,只对客户端展示清理

### 数据库变更

`providers` 表加列:
```sql
ALTER TABLE providers ADD COLUMN strip_thinking_tags_in_response BOOLEAN NOT NULL DEFAULT 0
```

### API 变更

- `CreateProviderRequest` 增加 `strip_thinking_tags_in_response: Option<bool>` (默认 None = false)
- `UpdateProviderRequest` 增加 `strip_thinking_tags_in_response: Option<bool>`
- `ProviderRow` 增加 `pub strip_thinking_tags_in_response: bool`
- `create_provider` / `update_provider` 签名增加 `strip_thinking_tags_in_response: bool` 参数

### 核心逻辑变更

`src/proxy/handler.rs::proxy_request` non-streaming 路径:

```rust
let mut response_body = proxy_response.body.to_vec();

// Opt-in: strip <think>...</think> tags from OpenAI responses
if provider.api_type == "openai" && provider.strip_thinking_tags_in_response {
    if let Ok(mut v) = serde_json::from_slice::<serde_json::Value>(&response_body) {
        let mut modified = false;
        if let Some(choices) = v.get_mut("choices").and_then(|c| c.as_array_mut()) {
            for choice in choices {
                if let Some(content) = choice.get_mut("message")
                    .and_then(|m| m.get_mut("content"))
                    .and_then(|c| c.as_str())
                {
                    let stripped = strip_thinking_tags(content);
                    if stripped != content {
                        *choice.get_mut("message").unwrap()
                            .get_mut("content").unwrap() = serde_json::Value::String(stripped);
                        modified = true;
                    }
                }
            }
        }
        if modified {
            if let Ok(new_bytes) = serde_json::to_vec(&v) {
                response_body = new_bytes;
            }
        }
    }
}
```

注意:日志记录**用原始** `proxy_response.body`(修改前),让 dashboard 仍显示完整 thinking 内容。

### 前端变更(可选,Dashboard 暂不实现)

- Provider 编辑表单加 checkbox "剥离响应中的 <think> 标签"(默认 unchecked)
- 解释文字:"开启后,从该 provider 转发的 OpenAI 协议响应会移除 `<think>...</think>` 块,适用于 MiniMax 等用 markdown 标签表示 thinking 的非标准实现"

### 测试用例

1. `test_strip_thinking_from_openai_response_basic` — 简单 thinking 块被移除
2. `test_strip_thinking_from_openai_response_multiple_blocks` — 多个 `<think>` 块都被移除
3. `test_strip_thinking_from_openai_response_no_thinking` — 无 thinking 时原样返回
4. `test_strip_thinking_from_openai_response_multiline_thinking` — 多行 thinking 内容
5. `test_strip_thinking_from_openai_response_only_thinking` — 纯 thinking 无正文时返回空字符串
6. `test_strip_thinking_from_openai_response_malformed_json` — 响应不是 JSON 时跳过(不报错)
7. `test_strip_thinking_from_openai_response_no_message_field` — 响应缺 message 字段时跳过

