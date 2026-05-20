# LlmGateway 需求设计与技术实现方案

> 本文档记录 LlmGateway 项目的功能需求、技术架构和实现细节。
> 每次开发新需求时，先在此文档中梳理需求设计与技术方案，再进行实现。

---

## 一、项目概述

LlmGateway 是一个 LLM API 网关服务，提供统一的 API 代理入口，支持多服务商负载均衡、API Key 管理、用量统计和请求日志等功能。

### 技术栈
- **语言**: Rust
- **Web 框架**: Axum
- **数据库**: SQLite (sqlx)
- **HTTP 客户端**: reqwest
- **序列化**: serde / serde_json
- **异步运行时**: Tokio
- **前端**: 内嵌 HTML (单文件 dashboard.html)
- **图表**: Chart.js

### 服务端口
- 本地: `http://127.0.0.1:49127`
- 局域网: `http://192.168.50.188:49127`
- 外网: `http://47.117.247.155:49127` (frp 转发)

---

## 二、数据库设计

### 2.1 api_keys 表
| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | API Key ID |
| name | TEXT NOT NULL | 名称 |
| api_key | TEXT NOT NULL UNIQUE | 完整 key (仅创建时返回) |
| key_prefix | TEXT NOT NULL | 前缀 (如 lgk-xxxx) |
| allowed_providers | TEXT | 允许的服务商 JSON 数组，null 表示全部 |
| is_active | BOOLEAN DEFAULT 1 | 是否启用 |
| created_at | TEXT | 创建时间 |
| updated_at | TEXT | 更新时间 |

### 2.2 providers 表
| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 服务商 ID |
| name | TEXT NOT NULL | 名称 |
| base_url | TEXT NOT NULL | API 基础 URL |
| api_type | TEXT DEFAULT 'openai' | API 类型: openai/anthropic/custom |
| auth_type | TEXT DEFAULT 'api_key' | 认证类型: api_key/dynamic_token |
| api_key | TEXT | 静态 API Key |
| token_url | TEXT | 动态 Token 获取 URL |
| token_username | TEXT | Token 用户名 |
| token_password | TEXT | Token 密码 |
| token_request_method | TEXT DEFAULT 'POST' | Token 请求方法 |
| token_content_type | TEXT DEFAULT 'json' | Token 请求内容类型 |
| token_username_field | TEXT DEFAULT 'username' | 用户名字段名 |
| token_password_field | TEXT DEFAULT 'password' | 密码字段名 |
| token_body_template | TEXT | 自定义请求体模板 |
| token_extra_headers | TEXT | 额外请求头 JSON |
| token_cookies | TEXT | Cookie 存储 |
| token_field | TEXT DEFAULT 'token' | 响应中 token 字段路径 |
| refresh_token_field | TEXT DEFAULT 'refreshToken' | 响应中 refresh token 字段路径 |
| token_header_field | TEXT DEFAULT 'Authorization' | 请求头字段名 |
| token_header_prefix | TEXT DEFAULT 'Bearer ' | 请求头前缀 |
| token_expiry_seconds | INTEGER DEFAULT 86400 | Token 有效期(秒) |
| current_token | TEXT | 当前 token |
| current_refresh_token | TEXT | 当前 refresh token |
| token_expires_at | TEXT | Token 过期时间 |
| is_active | BOOLEAN DEFAULT 1 | 是否启用 |
| weight | INTEGER DEFAULT 1 | 负载均衡权重 |
| bypass_proxy | BOOLEAN DEFAULT 0 | 是否绕过代理 |
| response_content_path | TEXT | 响应内容提取路径 |
| response_reasoning_path | TEXT | 推理内容提取路径 |
| created_at | TEXT | 创建时间 |
| updated_at | TEXT | 更新时间 |

### 2.3 provider_models 表
| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK AUTO | 自增 ID |
| provider_id | TEXT NOT NULL | 所属服务商 |
| model_id | TEXT NOT NULL | 服务商模型 ID |
| is_active | BOOLEAN DEFAULT 1 | 是否启用 |
| last_test_status | TEXT | 最近测试状态 |
| last_test_message | TEXT | 最近测试消息 |
| last_tested_at | TEXT | 最近测试时间 |
| created_at | TEXT | 创建时间 |
| updated_at | TEXT | 更新时间 |

### 2.4 models 表 (统一模型层)
| 字段 | 类型 | 说明 |
|------|------|------|
| id | TEXT PK | 对外模型 ID (全局唯一) |
| name | TEXT NOT NULL | 显示名称 |
| description | TEXT | 描述 |
| model_type | TEXT DEFAULT 'chat' | 模型类型 |
| is_active | BOOLEAN DEFAULT 1 | 是否启用 |
| priority | INTEGER DEFAULT 0 | 优先级 |
| config | TEXT | 配置 JSON |
| created_at | TEXT | 创建时间 |
| updated_at | TEXT | 更新时间 |

### 2.5 model_mappings 表
| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK AUTO | 自增 ID |
| model_id | TEXT NOT NULL | 对外模型 ID (关联 models.id) |
| provider_id | TEXT NOT NULL | 服务商 ID |
| provider_model_id | TEXT NOT NULL | 服务商侧模型 ID |
| is_active | BOOLEAN DEFAULT 1 | 是否启用 |
| weight | INTEGER DEFAULT 1 | 负载均衡权重 |
| cost_multiplier | REAL DEFAULT 1.0 | 成本系数 |
| created_at | TEXT | 创建时间 |

### 2.6 request_logs 表
| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK AUTO | 自增 ID |
| api_key_id | TEXT NOT NULL | 使用的 API Key |
| provider_id | TEXT NOT NULL | 使用的服务商 |
| model | TEXT | 请求模型 |
| request_path | TEXT NOT NULL | 请求路径 |
| request_method | TEXT DEFAULT 'POST' | 请求方法 |
| request_headers | TEXT | 请求头 JSON |
| request_body | TEXT | 请求体 JSON |
| response_status | INTEGER | 响应状态码 |
| response_headers | TEXT | 响应头 JSON |
| response_body | TEXT | 响应体 JSON (截断) |
| prompt_tokens | INTEGER DEFAULT 0 | 输入 token 数 |
| completion_tokens | INTEGER DEFAULT 0 | 输出 token 数 |
| total_tokens | INTEGER DEFAULT 0 | 总 token 数 |
| duration_ms | INTEGER | 耗时(毫秒) |
| is_streaming | BOOLEAN DEFAULT 0 | 是否流式 |
| is_throttled | BOOLEAN DEFAULT 0 | 是否被限流 |
| error_message | TEXT | 错误信息 |
| created_at | TEXT | 创建时间 |

### 2.7 token_rate_snapshots 表
| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK AUTO | 自增 ID |
| provider_id | TEXT | 服务商 ID |
| tokens_per_second | REAL NOT NULL | 每秒 token 数 |
| prompt_tokens | INTEGER DEFAULT 0 | 输入 token |
| completion_tokens | INTEGER DEFAULT 0 | 输出 token |
| request_count | INTEGER DEFAULT 0 | 请求数 |
| elapsed_seconds | REAL DEFAULT 10 | 经过秒数 |
| snapshot_time | TEXT | 快照时间 |

### 2.8 provider_quotas 表
| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK AUTO | 自增 ID |
| provider_id | TEXT NOT NULL | 服务商 ID |
| quota_type | TEXT NOT NULL | 配额类型 |
| limit_count | INTEGER NOT NULL | 限制数量 |
| is_enabled | BOOLEAN DEFAULT 1 | 是否启用 |
| created_at | TEXT | 创建时间 |

### 2.9 provider_quota_calibrations 表
| 字段 | 类型 | 说明 |
|------|------|------|
| id | INTEGER PK AUTO | 自增 ID |
| provider_id | TEXT NOT NULL | 服务商 ID |
| quota_type | TEXT NOT NULL | 配额类型 |
| calibration_offset | INTEGER DEFAULT 0 | 校准偏移量 |
| calibrated_at | TEXT | 校准时间 |

---

## 三、API 端点设计

### 3.1 Dashboard 页面
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/` | Dashboard 主页 |
| GET | `/dashboard` | Dashboard 主页 (别名) |

### 3.2 API Key 管理
| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/api-keys` | 创建 API Key |
| GET | `/api/v1/api-keys` | 列出所有 API Key |
| GET | `/api/v1/api-keys/:id` | 获取单个 API Key |
| DELETE | `/api/v1/api-keys/:id` | 删除 API Key |
| POST | `/api/v1/api-keys/:id` | 重新生成 API Key |
| PUT | `/api/v1/api-keys/:id` | 更新 API Key (名称/状态/允许服务商) |

### 3.3 服务商管理
| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/providers` | 创建服务商 |
| GET | `/api/v1/providers` | 列出所有服务商 |
| GET | `/api/v1/providers/:id` | 获取单个服务商 |
| PUT | `/api/v1/providers/:id` | 更新服务商 |
| DELETE | `/api/v1/providers/:id` | 删除服务商 |
| POST | `/api/v1/providers/batch-update-status` | 批量启用/停用服务商 |
| POST | `/api/v1/providers/:id/refresh-token` | 手动刷新动态 Token |

### 3.4 服务商模型管理
| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/providers/:id/models` | 添加服务商模型 |
| GET | `/api/v1/providers/:id/models` | 列出服务商模型 |
| DELETE | `/api/v1/providers/:id/models/:model_id` | 删除服务商模型 |
| POST | `/api/v1/providers/:id/models/:model_id/test` | 测试单个模型连接 |
| POST | `/api/v1/providers/:id/models/test-all` | 测试所有模型连接 |

### 3.5 统一模型层
| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/models` | 创建统一模型 |
| GET | `/api/v1/models` | 列出统一模型 (仅含有效映射的) |
| GET | `/api/v1/models/:id` | 获取单个统一模型 |
| PUT | `/api/v1/models/:id` | 更新统一模型 |
| DELETE | `/api/v1/models/:id` | 删除统一模型 |
| POST | `/api/v1/models/:id/mappings` | 添加模型映射 |
| GET | `/api/v1/models/:id/mappings` | 列出模型映射 |
| PUT | `/api/v1/models/:id/mappings` | 更新模型映射 |
| DELETE | `/api/v1/models/:id/mappings/:provider_id` | 删除模型映射 |

### 3.6 配额管理
| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/api/v1/quotas/:provider_id` | 设置服务商配额 |
| GET | `/api/v1/quotas/:provider_id` | 获取服务商配额 |
| DELETE | `/api/v1/quotas/:provider_id/:quota_type` | 删除配额 |
| GET | `/api/v1/quotas/usage` | 获取所有配额使用情况 |
| POST | `/api/v1/quotas/:provider_id/calibration` | 设置配额校准 |
| GET | `/api/v1/quotas/:provider_id/calibration` | 获取配额校准 |

### 3.7 统计与日志
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/stats` | 获取统计数据 |
| GET | `/api/v1/stats/bucketed` | 获取时间分桶统计 |
| GET | `/api/v1/stats/by-api-key` | 按 API Key 统计 |
| GET | `/api/v1/stats/usage-trend` | 用量趋势 |
| GET | `/api/v1/logs` | 请求日志列表 (分页/搜索/过滤) |
| GET | `/api/v1/logs/:id` | 日志详情 |
| DELETE | `/api/v1/logs/:id` | 删除单条日志 |
| POST | `/api/v1/logs/batch-delete` | 批量删除日志 |
| POST | `/api/v1/logs/delete-all` | 删除所有日志 |

### 3.8 Dashboard API
| 方法 | 路径 | 说明 |
|------|------|------|
| GET | `/api/v1/dashboard/summary` | 概览摘要 |
| GET | `/api/v1/dashboard/health` | 服务商健康状态 |
| GET | `/api/v1/dashboard/token-rate` | 实时 Token 速率 |
| GET | `/api/v1/dashboard/top-provider` | Top 服务商 |

### 3.9 代理转发
| 方法 | 路径 | 说明 |
|------|------|------|
| POST | `/v1/*path` | LLM API 代理 (HTTP) |
| GET | `/v1/*path` | LLM API 代理 (HTTP GET) |
| GET | `/ws/v1` | WebSocket 代理 (流式) |

---

## 四、核心功能实现

### 4.1 认证机制 (auth/mod.rs)

**两种认证方式**：
1. **API Key 认证** (`auth_type: "api_key"`)
   - 静态 API Key，配置后直接使用
   - 请求头格式: `Authorization: Bearer <api_key>`

2. **动态 Token 认证** (`auth_type: "dynamic_token"`)
   - 通过 token_url 获取临时 token
   - 支持 POST/GET 请求方式
   - 支持 JSON/Form 请求体
   - 支持自定义请求头和 Cookie
   - Token 自动刷新 (token_refresh.rs)
   - 支持从响应中通过 JSON Path 提取 token

**API Key 校验流程**：
1. 从请求头 `Authorization: Bearer lgk-xxx` 提取 key
2. SHA256 哈希后与数据库比对
3. 检查 key 是否启用
4. 检查 key 是否允许访问目标服务商

### 4.2 代理转发 (proxy/)

**请求处理流程**：
1. 提取请求中的 API Key 并校验
2. 从请求体提取模型名称
3. 查找模型映射 → 确定服务商和服务商侧模型 ID
4. 按权重选择服务商 (加权随机)
5. 构建认证头 (API Key 或动态 Token)
6. 转发请求到服务商
7. 记录请求日志和用量统计
8. 返回响应 (支持流式 SSE)

**WebSocket 代理** (ws_handler.rs)：
- 客户端通过 WebSocket 连接 `/ws/v1`
- 网关将请求转为 HTTP 发送给服务商
- 服务商 SSE 响应逐行转发回 WebSocket

**负载均衡**：
- 按服务商权重加权随机选择
- 跳过未启用的服务商
- 支持 API Key 限制允许的服务商列表

### 4.3 统一模型层

**设计思路**：
- 服务商模型 (provider_models) 是服务商侧的模型 ID
- 统一模型 (models) 是对外暴露的模型 ID
- 模型映射 (model_mappings) 连接两者，支持多对多
- 同一对外 ID 可映射到多个服务商，实现负载均衡

**模型校验**：
- 服务商模型必须通过连接测试才可使用
- 测试失败自动标记为不可用
- 代理请求时拒绝未测试/失败的模型

### 4.4 用量统计 (usage/mod.rs)

**Token 提取**：
- 支持 OpenAI 格式: `usage.prompt_tokens / completion_tokens / total_tokens`
- 支持 Anthropic 格式
- 支持流式响应: 从 SSE data 行中提取最终 chunk 的 usage
- 支持从响应体提取模型名称

### 4.5 统计收集 (stats/mod.rs)

**StatsCollector**：
- 内存中累积用量数据
- 定期生成 token_rate_snapshots 快照
- 记录 prompt/completion tokens 分离统计
- 使用实际 elapsed_seconds 计算速率 (非硬编码)

### 4.6 Token 自动刷新 (auth/token_refresh.rs)

**TokenRefreshTask**：
- 后台定时检查所有 dynamic_token 服务商
- 检查 token 是否即将过期
- 自动调用 token_url 刷新 token
- 跳过 api_key 类型的服务商

### 4.7 Dashboard (dashboard/ + dashboard.html)

**单文件 HTML 前端**，包含：
- 概览页: 摘要统计、用量趋势图、服务商健康状态
- 服务商管理: CRUD、批量启用/停用、模型管理、连接测试
- API Key 管理: CRUD、重新生成、编辑
- 请求日志: 分页、搜索、过滤、CSV 导出、批量删除
- Token 速率图表: 实时 Input/Output 曲线，图例点击切换

---

## 五、测试用例覆盖

### 5.1 集成测试 (integration_test.rs, 113 tests)
- 数据库 CRUD: api_keys, providers, request_logs
- 认证: API Key 校验、动态 Token
- 代理: 服务商选择、权重、跳过不可用
- 统计: 时间分桶、按 Key 分组、按服务商分组
- Dashboard: 摘要、健康状态
- JSON Path 提取: 嵌套字段、缺失字段
- 模型映射: 多映射、CRUD

### 5.2 HTTP API 测试 (http_api_test.rs, 70 tests)
- 完整 HTTP 请求/响应测试
- Dashboard 页面 HTML 验证
- API Key/Provider 完整生命周期
- 统计/日志/配额端点
- 统一模型列表端点

### 5.3 模型 API 测试 (models_api_test.rs, 10 tests)
- 统一模型 CRUD
- 模型映射 CRUD
- API Key 更新 (名称/状态/允许服务商)
- 列表过滤 (排除不可用/无映射的)

### 5.4 用量测试 (usage_test.rs, 15 tests)
- OpenAI 格式用量提取
- Anthropic 格式用量提取
- 流式用量提取
- 零 token / 无 usage 场景
- 限流检测

### 5.5 用量集成测试 (usage_integration_test.rs, 13 tests)
- MiniMax/Astron 响应用量提取
- 代理日志用量记录
- Dashboard 摘要与实际用量

### 5.6 Token 速率统计测试 (token_rate_stats_test.rs, 15 tests)
- 累积/重置/快照机制
- elapsed_seconds 非硬编码
- prompt/completion 分离统计
- 速率计算准确性

### 5.7 Token 刷新测试 (token_refresh_test.rs, 5 tests)
- 过期检测
- 跳过 api_key 服务商
- 无过期时间处理

### 5.8 Dashboard HTML 测试 (dashboard_html_test.rs, 20 tests)
- Chart.js CDN 不重复加载
- Token 速率图: 默认隐藏 Input、图例点击、elapsed_seconds
- 本地时区显示
- API Key 用量图表和详情表
- 日志: 错误行高亮、分页、CSV 导出、模型过滤
- 批量服务商操作

---

## 六、源码模块结构

```
src/
├── main.rs              # 入口: 路由注册、服务启动、配置加载
├── lib.rs               # 模块声明、AppState 定义
├── api/mod.rs           # API 处理函数 (40+ handlers)
├── auth/
│   ├── mod.rs           # 认证管理: API Key 校验、动态 Token 获取
│   └── token_refresh.rs # 后台 Token 自动刷新任务
├── config/mod.rs        # 配置结构体、请求/响应类型定义
├── dashboard/mod.rs     # Dashboard HTML 页面服务
├── db/mod.rs            # 数据库操作 (SQLite, 50+ 方法)
├── proxy/
│   ├── mod.rs           # 代理核心: 服务商选择、请求转发、日志记录
│   ├── handler.rs       # HTTP 代理处理器、/v1/models 端点
│   └── ws_handler.rs    # WebSocket 代理处理器
├── stats/mod.rs         # 统计收集器 (内存累积 + 定期快照)
├── usage/mod.rs         # 用量提取 (OpenAI/Anthropic/流式)
└── utils/mod.rs         # 工具函数 (SHA256 哈希)
```

---

## 七、新需求开发模板

> 开发新需求时，按以下模板在此文档末尾追加内容：

```
### [需求编号] 需求标题

**需求描述**：
(描述要实现什么功能)

**技术方案**：
- 数据库变更: (新增/修改哪些表)
- API 变更: (新增/修改哪些端点)
- 核心逻辑: (实现思路)
- 前端变更: (Dashboard 修改)

**测试用例**：
- (列出需要编写的测试)

**实现状态**: 📋 待开发 / 🔧 开发中 / ✅ 已完成
```

---

## 用量配额：固定窗口 vs 滑动窗口 + 校准窗口期绑定

### 需求

1. **两种窗口模式**：
   - **固定窗口**：从周期起点开始计算，如"每月1号0点"、"每周一0点"、"每5小时整点（0:00/5:00/10:00/15:00/20:00）"
   - **滑动窗口**：从当前时刻往前推算，如"最近5小时"、"最近7天"、"最近30天"

2. **校准窗口期绑定**：
   - 校准时记录校准时刻和当时的窗口期范围
   - 校准偏移只在对应的窗口期内生效
   - 窗口期切换后（如从5:00-10:00切换到10:00-15:00），旧校准自动失效
   - 滑动窗口：校准记录包含 `calibrated_at`，只有当 `calibrated_at` 在当前滑动窗口内时才生效

3. **用户可自定义窗口大小**：
   - 不再硬编码 `5h`/`weekly`/`monthly`
   - 用户可输入窗口时长（如 3h、12h、2d、7d、30d）
   - 同时选择窗口模式（固定/滑动）

### 数据库变更

#### `provider_quotas` 表

```sql
CREATE TABLE IF NOT EXISTS provider_quotas (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    quota_type TEXT NOT NULL,        -- 保留兼容，但新格式为 "fixed:5h"/"sliding:5h"/"fixed:7d"/"sliding:30d"
    window_mode TEXT NOT NULL DEFAULT 'fixed',  -- 'fixed' 或 'sliding'
    window_size TEXT NOT NULL DEFAULT '5h',     -- 窗口大小，如 '5h'/'12h'/'7d'/'30d'
    limit_count INTEGER NOT NULL,
    is_enabled BOOLEAN NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE,
    UNIQUE(provider_id, quota_type)
);
```

#### `provider_quota_calibrations` 表

```sql
CREATE TABLE IF NOT EXISTS provider_quota_calibrations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    provider_id TEXT NOT NULL,
    quota_type TEXT NOT NULL,
    calibration_offset INTEGER NOT NULL DEFAULT 0,
    calibrated_at TEXT NOT NULL DEFAULT (datetime('now')),
    -- 新增：校准时的窗口期范围，用于判断校准是否仍在有效窗口内
    calibration_window_start TEXT,   -- 校准时所属窗口的起始时间
    calibration_window_end TEXT,     -- 校准时所属窗口的结束时间
    note TEXT,
    FOREIGN KEY (provider_id) REFERENCES providers(id) ON DELETE CASCADE,
    UNIQUE(provider_id, quota_type)
);
```

### 窗口计算逻辑

#### 固定窗口（fixed）

```
window_size = "5h" → 对齐到 0:00/5:00/10:00/15:00/20:00
window_size = "1d" → 对齐到当天 0:00
window_size = "7d" → 对齐到本周一 0:00
window_size = "30d" → 对齐到本月1号 0:00

period_start = 当前时间对齐到窗口起点
period_end = period_start + window_size
```

对齐规则：
- 小时级（Nh）：从当天0:00开始，每N小时一个窗口
- 天级（Nd）：从epoch开始，每N天一个窗口（简化为从最近N天整数倍对齐）
- 实际简化：1d=当天0:00，7d=本周一0:00，30d=本月1号0:00

#### 滑动窗口（sliding）

```
period_start = now - window_size
period_end = now
```

### 校准有效性判断

```rust
fn is_calibration_valid(cal: &CalibrationRow, period_start: &DateTime<Utc>, period_end: &DateTime<Utc>, window_mode: &str) -> bool {
    match window_mode {
        "fixed" => {
            // 固定窗口：校准时的窗口范围必须与当前窗口范围完全一致
            // 即 calibration_window_start == period_start && calibration_window_end == period_end
            cal.calibration_window_start.as_deref() == Some(&period_start_str) &&
            cal.calibration_window_end.as_deref() == Some(&period_end_str)
        }
        "sliding" => {
            // 滑动窗口：校准时刻必须在当前滑动窗口内
            // 即 calibrated_at >= period_start && calibrated_at <= period_end
            cal.calibrated_at >= period_start_str && cal.calibrated_at <= period_end_str
        }
        _ => false
    }
}
```

### API 变更

#### `POST /api/v1/quotas/:provider_id`

```json
{
    "window_mode": "fixed",     // "fixed" 或 "sliding"
    "window_size": "5h",        // Nh/Nd 格式
    "limit_count": 500,
    "is_enabled": true
}
```

- `quota_type` 自动生成为 `{window_mode}:{window_size}`，如 `fixed:5h`、`sliding:30d`
- 向后兼容：仍接受 `quota_type` 字段（"5h"/"weekly"/"monthly"），自动转换为 `fixed:5h`/`fixed:7d`/`fixed:30d`

#### `POST /api/v1/quotas/:provider_id/calibration`

```json
{
    "quota_type": "fixed:5h",
    "calibration_offset": 50,
    "note": "手动校准"
}
```

- 服务端自动记录 `calibration_window_start` 和 `calibration_window_end`

### `QuotaUsageInfo` 响应变更

```json
{
    "provider_id": "minimax",
    "quota_type": "fixed:5h",
    "window_mode": "fixed",
    "window_size": "5h",
    "limit_count": 500,
    "gateway_count": 200,
    "calibration_offset": 50,
    "calibration_valid": true,       // 新增：校准是否在当前窗口内有效
    "calibration_window_start": "2026-05-19 10:00:00",  // 新增
    "calibration_window_end": "2026-05-19 15:00:00",    // 新增
    "estimated_total": 250,
    "remaining": 250,
    "usage_percent": 50.0,
    "period_start": "2026-05-19 10:00:00",
    "period_current": "2026-05-19 12:30:00"
}
```

### 前端变更

1. 配额类型选择改为：窗口模式（固定/滑动）+ 窗口大小（输入框，如 5h/7d/30d）
2. 校准模态框显示当前窗口期范围，提示校准只在此窗口内生效
3. 配额表格显示窗口模式和大小，校准列显示是否有效
4. 向后兼容旧数据（quota_type 为 "5h"/"weekly"/"monthly" 的自动映射）

---

## Mock Provider（模拟器模式）

### 需求背景

虚拟网关在开发/测试场景下，需要一种不消耗真实 LLM Token 的工作模式。开启后，请求会被拦截并返回随机生成的模拟响应，同时正常记录用量日志。

### 技术方案

**数据库变更**：
- `providers` 表新增 `mock_mode BOOLEAN DEFAULT 0` 列

**ProviderRow 新增字段**：
```rust
pub mock_mode: bool,
```

**Proxy 层拦截逻辑** (`proxy/handler.rs`)：
- 在获取 provider 后，检测 `provider.mock_mode`
- 若为 true，**跳过所有上游转发**，直接生成模拟响应
- 模拟响应格式为标准 OpenAI ChatCompletion JSON
- 支持流式（streaming）和非流式两种模式
- 流式模式：分批发送 SSE chunks，模拟打字效果
- 请求仍记录到 `request_logs`，token 数量基于内容估算

**模拟响应生成规则**：
- 内容：基于预定义句库随机拼接，目标长度由 `max_tokens` 控制
- Token：基于 `usage::estimate_completion_tokens()` 估算
- 延迟：流式模式下每 chunk 间隔 15ms，模拟真实打字

**API 变更**：
- `POST /api/v1/providers` 新增 `mock_mode: bool`（可选，默认为 false）
- `PUT /api/v1/providers/:id` 新增 `mock_mode: bool`
- `GET /api/v1/providers/:id` 返回值包含 `mock_mode` 字段

**虚拟网关配置**：
- 每个虚拟网关（A/B/C）默认配置一个 `mock-simulator` 提供商
- `mock_mode: true`，`base_url: http://mock-simulator`（任意值，不被使用）
- 映射模型：`MiniMax-M2.7-highspeed` 和 `astron-code-latest`
- 与真实提供商 `生产环境` 共存，可按需切换

### 实现状态**: ✅ 已完成
