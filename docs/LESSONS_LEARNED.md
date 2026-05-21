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