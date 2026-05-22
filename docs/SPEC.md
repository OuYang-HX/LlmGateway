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

> **唯一约束**: `(model_id, provider_id)` 唯一

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

*文档版本: 2026-05-21 | 代码版本对应: dev 分支 latest commit*
