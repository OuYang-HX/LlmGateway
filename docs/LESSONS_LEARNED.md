# LlmGateway 开发经验教训

> 开发过程中持续补充，每次开发完成后对照检查。

## 编译部署类

### 1. 代码修改后必须编译重启
- **错误**：修改代码后没有执行 `deploy.sh update`，导致运行中的服务仍是旧版本
- **原因**：Linux 进程加载二进制文件后，内存中运行的是旧版本，源码修改不影响运行中的服务
- **正确做法**：每次修改代码后，必须执行 `bash deploy.sh update` 或 `bash deploy-dev.sh update`

## 测试类

### 1. 测试驱动开发流程
- **规则**：新需求必须总结为测试用例，修改代码后必须保证测试通过
- **例外**：只有需求发生变化时才可以修改测试用例
- **运行测试**：`cargo test`

### 2. 修改后端逻辑后必须同步更新测试
- **场景**：修改 `take_snapshot()` 增加 `elapsed < 1s` 跳过逻辑后，原有测试中 `record_usage` 后立即 `take_snapshot`（elapsed ≈ 0）的用例全部失败
- **正确做法**：
  1. 修改后端逻辑后，先跑一遍测试看哪些失败
  2. 在 `record_usage` 和 `take_snapshot` 之间加 `tokio::time::sleep(1.1s)` 使 elapsed >= 1.0
  3. 新增专门测试验证短 elapsed 快照被跳过的行为

## 前端开发类

### 1. 内嵌 JS 中变量不能重复声明
- **错误**：在 `dashboard.html` 的 `<script>` 中声明了 `let allProviders=[]`，但原有代码已有同名变量声明，导致 `let` 重复声明，整个 JS 解析失败
- **后果**：JS 解析失败后，页面所有交互全部失效
- **根因**：`let`/`const` 不允许同一作用域内重复声明，且 JS 是整体解析的
- **正确做法**：
  1. 新增变量前，先用 `grep` 检查是否已存在同名变量声明
  2. 如果已存在，直接复用，不要重新声明
  3. 修改后用 `new Function(scriptContent)` 验证 JS 语法正确性

### 2. 修改 dashboard.html 后必须验证 JS 语法
- **正确做法**：
  ```bash
  node -e "const fs=require('fs'); const html=fs.readFileSync('src/dashboard.html','utf8'); const m=html.match(/<script>([\s\S]*?)<\/script>/); new Function(m[1]); console.log('OK')"
  ```
- **特别注意**：Git 合并/暂存恢复时可能留下冲突标记，检查：
  ```bash
  grep -n '<<<<<<\|======\|>>>>>>' src/dashboard.html | grep -v '// ===\|<!-- ===\|/\* ==='
  ```

### 3. 新增功能引入新变量时，检查与现有代码的冲突
- 新增 `let`/`const`/`function` 前，`grep -n` 检查同名标识符是否已存在
- 变量命名加模块前缀可降低冲突风险（如 `trOutputChart` 而非 `outputChart`）

## 数据库/SQL类

### 1. 新增数据库列需要同步更新三处代码
- **必须同步更新的位置**：
  1. **迁移代码**：`run_migrations()` 中添加 `ALTER TABLE ... ADD COLUMN`
  2. **结构体**：`ProviderRow` 添加对应字段
  3. **手动解析函数**：`row_to_provider()` 中添加字段解析
- **遗漏后果**：编译错误 `missing field`
- **正确做法**：新增列后，全局搜索相关结构体和解析函数，逐一更新

## API/网络类

### 1. 速率计算中 elapsed 时间过短会导致速率虚高
- **根因**：`elapsed` 可能只有 0.056s，此时 `39 tokens / 0.056s = 694 tokens/s` 就是虚假高速率
- **修复**：`take_snapshot()` 中，当 `elapsed < 1s` 或 `request_count == 0` 时跳过快照
- **教训**：速率 = 数量 / 时间 的公式中，分母必须足够大才有统计意义

### 2. 步进滑动窗口（rolling）的正确理解
- **正确理解**：窗口期基于**订阅时间步进**，刷新时刻只是触发窗口向前移动一步
- **核心公式**：
  1. N = 从订阅时间到当前，经过的步进对齐刷新次数
  2. `period_end = subscription_start + N * step_duration`
  3. `period_start = period_end - window_size`
- **关键要点**：
  - 订阅开始时间是服务商级别属性，不是配额级别
  - 窗口期始终基于订阅时间偏移，不会对齐到整点
  - rolling 模式**必须设置订阅开始时间**

### 3. rsync 对 Rust 项目不可靠，同步源码用 cp
- **根因**：rsync 默认以"文件大小 + 修改时间"判断是否更新，可能跳过内容不同的同名文件
- **修复**：对 Rust 项目使用 `cp -av` 或 `cp --remove-destination`，强制覆盖目标

### 4. Rust 函数签名变更后，必须找到所有调用点并更新
- 修改函数签名后，`cargo build --tests 2>&1 | grep "E0061"` 找所有缺失参数的调用
- Python 批量替换时注意转义：若原文本是 `"choices.0"`，替换字符串也要是 `"choices.0"`，不是 `"choices\.0"`

### 5. 测试中引用 Vec 而非 json! 数组时，小心 as_array().unwrap() panic
- 从 HTTP API 返回的 JSON 形状不一定是你期望的，测试中用 `.unwrap()` 前先 assert 字段存在性

### 6. axum-test 的 `.json()` 方法是链式 builder 模式
- **错误用法**：`server.post("/path", &body)` — 编译错误
- **正确用法**：`server.post("/path").json(&body).await`

### 7. 三个虚拟网关各自独立数据库
- 每个虚拟网关（A=49129, B=49130, C=49131）有独立 SQLite DB
- 新增列后，每个虚拟网关的 DB 需要手动执行 `ALTER TABLE`
- 每个虚拟网关有独立的 API Key，不同网关的 API Key 不能混用

## 数据库迁移类

### 1. SQLite 重建表迁移必须关闭外键
- **错误**：迁移代码中 `ALTER TABLE ... RENAME TO` 在外键启用时失败，错误被 `let _` 吞掉
- **后果**：数据库中 `model_mappings` 表被 DROP 但 RENAME 失败，导致 `no such table` 错误
- **正确做法**：
  1. 迁移前 `PRAGMA foreign_keys=OFF`
  2. 检查是否需要迁移（避免每次启动都重建表）
  3. `CREATE TABLE new → INSERT → DROP old → RENAME → PRAGMA ON`
  4. 添加索引

### 2. 函数签名变更后必须检查路由和前端
- **场景**：`remove_model_mapping` 增加参数后，`main.rs` 路由和 `dashboard.html` API 路径也需要同步更新
- **编译器无法检查**：API 路由路径和前端 JS 不受编译器保护
- **正确做法**：修改函数签名后，`grep -rn` 搜索所有调用点，包括路由定义和前端代码

### 3. Provider struct 变更后必须检查测试中的 ProviderRow 初始化
- **场景**：`ProviderRow` 新增 `group_id` 字段后，`auth_stats_test.rs` 中的 `ProviderRow { ... }` 初始化缺少字段导致编译失败
- **Rust 会报错**：但只在 `cargo test` 或 `cargo check --tests` 时，`cargo check` 默认不检查测试
- **正确做法**：修改 struct 后运行 `cargo check --tests`，搜索所有 `ProviderRow {` 初始化

### 4. Anthropic SSE 格式与 OpenAI 不同
- **event: 前缀**：Anthropic SSE 每个数据块前面有 `event: content_block_delta\n` 行
- **用量字段**：`input_tokens/output_tokens` 而非 `prompt_tokens/completion_tokens`
- **内容路径**：`delta.text` 而非 `choices[0].delta.content`
- **认证方式**：`x-api-key: <key>` 而非 `Authorization: Bearer <key>`

### 5. 同一服务商多格式对接 = 两个 Provider + group_id
- **两个 Provider**：因为 base_url 和认证方式不同，本质上是不同的上游端点
- **group_id 聚合**：统计和配额按 group_id 维度，而非单个 provider_id
- **实现策略**：request_logs 写入 effective_provider_id（group_id 或自身 id），现有统计/配额查询无需修改
## 客户端接入类

### 1. Claude Code 默认用 `x-api-key` 头做认证,不是 `Authorization: Bearer`
- **根因**:Anthropic SDK 在 `ANTHROPIC_API_KEY` 被设置时,客户端请求使用 `x-api-key: <key>` 头;同时还会注入 `anthropic-version: 2023-06-01`。
- **症状**:网关返回 `401 Missing API key`,因为 `extract_api_key` 只解析 `Authorization` 头。
- **修复**:`src/proxy/handler.rs::extract_api_key()` 增加 `x-api-key` 优先解析,保留 `Authorization` 兼容。
- **教训**:`ANTHROPIC_API_KEY`(用 `x-api-key`)和 `ANTHROPIC_AUTH_TOKEN`(用 `Authorization`)是 Anthropic 客户端的两种互斥认证方式,服务端必须两者都支持。

### 2. `ANTHROPIC_BASE_URL` 不要带 `/v1` 后缀
- **根因**:Anthropic SDK 内部会拼上 `/v1/messages`,如果 base_url 已经带 `/v1`,实际请求路径变成 `/v1/v1/messages`。
- **症状**:`curl` 测试 `/v1/v1/messages` 返回 404 page not found(没有进入任何 axum 路由,因 `/v1/*path` 是单段 wildcard,不能跨段匹配 `v1/messages`)。
- **正确配置**:
  ```bash
  export ANTHROPIC_BASE_URL=http://<gateway-host>:49127      # 不要带 /v1
  export ANTHROPIC_API_KEY=lgk-<gateway-key>
  ```
- **常见错误**:很多教程(包括 LLM 网关的 README)写成 `http://host:port/v1`,这是 OpenAI 兼容网关的写法,Anthropic 客户端不适用。

### 3. Claude Code 默认 first-party 模型需注册到统一模型表
- **现象**:Claude Code 不带 `--model` 时使用内置默认模型(如 `claude-opus-4-7`),如果网关的统一模型表里没有这个名字,即使网关能处理 Anthropic 协议,Claude Code 也会收到 404。
- **诊断**:`[API:timing] dispatching to firstParty model=claude-opus-4-7[1m]` 之后看到 `404 Model 'claude-opus-4-7' not found`。
- **修复**:在 Dashboard → 服务商模型 里创建 `claude-opus-4-7` 统一模型,映射到 Anthropic 格式的 provider(如 `MiniMax-anthropic`)。
- **可用统一模型清单**(2026-06 验证):`claude-haiku-4-5`、`claude-opus-4-7`、`MiniMax-M2.7-highspeed`、`MiniMax-M3`、`astron-code-latest`。
- **自定义默认模型**:用 `ANTHROPIC_MODEL=claude-haiku-4-5` 覆盖 Claude Code 的默认模型。

### 4. Claude Code 在 streaming 失败时会自动 fallback 到 non-streaming
- **现象**:日志报 `Stream completed without receiving message_start event - triggering non-streaming fallback`,但用户仍能收到响应。
- **可能原因**:MiniMax 等代理的 Anthropic 兼容端点会发出非标准事件(如 `event: ping`),Claude Code SDK 解析时丢弃整个流。
- **影响**:非流式响应延迟略高(几秒),功能完全正常,可暂时不修。
- **修复方向**:在 `src/proxy/handler.rs` streaming 路径中,过滤掉非标准 `event:` 类型的事件(只转发 SDK 期望的 `message_start`/`content_block_*`/`message_delta`/`message_stop`/`ping`)。

### 5. axum 0.7 `*path` 是单段 wildcard,不能用 `/v1/*path` 匹配多段路径
- **错误写法**:`.route("/v1/*path", ...)` 只能匹配 `/v1/<单段>`,如 `/v1/messages`,不能匹配 `/v1/v1/messages`。
- **catch-all 写法**:`.route("/{*path}", ...)` 可以匹配多段(把 `*path` 放在 path 末尾,且前面用 `/{` 语法)。
- **兜底方案**:`.fallback(handler)` 处理所有未匹配的请求。
- **经验**:如果想让网关兼容"用户把 base_url 末尾加 `/v1`"的常见错误配置,要么用 catch-all,要么在 README 明确说明正确的 base_url 写法。

## Dashboard / 诊断端点类

### 1. Dashboard "测试连接"功能必须与正式代理路径协议一致
- **症状**:Provider `api_type=anthropic` + `base_url=https://api.minimaxi.com/anthropic/v1` 时,dashboard 报 `HTTP 404: 404 page not found`,但真实代理路径(`/v1/messages`)工作正常,request_logs 全 200
- **根因**:`test_model_connection` 写死 OpenAI 格式 `/chat/completions` 路径,不管 `api_type` 是什么
- **修复**:抽 `build_test_request(provider, model_id) -> TestRequest` 纯函数,按 `api_type` 选路径(anthropic→`/messages`+`anthropic-version`,其他→`/chat/completions`)
- **教训**:**任何 health check / test connection / diagnostic 端点必须与正式路径保持协议一致**。否则误导运维(以为 provider 挂了实际正常)
- **相关 commit**: `fix: dashboard test connection supports anthropic api_type`

### 2. Claude Code 升级会换默认 first-party 模型,网关需防御性注册
- **现象**:Claude Code 2.1.150 内部版本升级,默认 first-party 模型从 `claude-opus-4-7` 改为 `claude-opus-4-8`。网关统一模型表没注册新名字,客户端请求 404
- **防御**:
  1. 用 `ANTHROPIC_MODEL=<已注册 ID>` 显式覆盖默认,生产配置里固化
  2. 网关预置一组 Claude Code 常用 first-party 模型名,统一映射到现有 provider
- **监控**:定期 `claude --version` 抓新版本,在 dashboard /v1/models 页面检查是否覆盖当前 Claude Code 默认

### 3. `rand::thread_rng()` + 关键字断言 = 偶发 flaky 测试
- **症状**:`test_generate_mock_content_realistic_words` 在 `cargo test` 并发跑(多文件并行)时约 1/3 概率失败,单独跑稳定通过
- **根因**:`generate_mock_content` 用 `rand::thread_rng()`,多测试共享全局 RNG 状态,生成的 mock 内容可能不含断言期待的关键字
- **待修**:
  - 改 `generate_mock_content` 接受 `&mut impl Rng`,测试用确定性 seed
  - 或测试加 `serial_test::serial` 注解
  - 或放宽断言,允许同义替换
- **教训**:**生产代码用全局随机数 + 测试断言依赖特定 token 出现 = 反模式**。要么注入 RNG,要么断言不依赖随机性

## API/协议兼容类

### 1. 非标准 LLM 实现把 thinking 用 `<think>` 标签混杂在 content 字段
- **症状**:`MiniMax-M3` 走 OpenAI 协议时,响应里 thinking 内容**用 `<think>...</think>` markdown 标签直接放在 `message.content`**,违反 OpenAI 2025+ reasoning 字段标准(应放 `reasoning_content`)
- **影响**:pi agent / Claude Code 等客户端把整段 content 渲染给用户,thinking 占用终端大量空间,看起来"截断"
- **修复**:网关层加 opt-in provider 配置 `strip_thinking_tags_in_response`(默认 `false`)
  - non-streaming 路径在 `proxy_request` 拿到 response body 后,用 `strip_thinking_from_openai_response_body()` 处理(纯函数,可单测)
  - 日志仍记录**原始** body(让 dashboard 审计完整 thinking),只清理给客户端的响应
  - 复用 `handler::strip_thinking_tags` 函数
- **关键设计**:**opt-in** 保持"透明转发"原则;只对 `api_type=openai` 生效(Anthropic 协议响应是结构化 block,SDK 能处理)
- **未来 streaming 路径**:TODO 留,需要 buffer + 状态机处理跨 chunk 的 `<think>` 标签边界
- **相关 commit**: `feat: opt-in strip thinking tags from openai response body`
