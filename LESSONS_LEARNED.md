# LlmGateway 开发经验教训

## 编译部署类

### 1. 代码修改后必须编译重启
- **错误**：修改代码后没有执行 `deploy.sh update`，导致运行中的服务仍是旧版本
- **原因**：Linux 进程加载二进制文件后，内存中运行的是旧版本，源码修改不影响运行中的服务
- **正确做法**：每次修改代码后，必须执行 `bash /home/oyhx/github/LlmGateway/deploy.sh update`

## 测试类

### 1. 测试驱动开发流程
- **规则**：新需求必须总结为测试用例，修改代码后必须保证测试通过
- **例外**：只有需求发生变化时才可以修改测试用例
- **运行测试**：`cargo test`

---

（暂无，开发过程中持续补充）

---

## 前端开发类

### 1. 内嵌 JS 中变量不能重复声明
- **错误**：在 `dashboard.html` 的 `<script>` 中，新增的代码声明了 `let allProviders=[]`，但原有代码中已有同名变量声明，导致 `let` 重复声明，整个 JS 解析失败
- **后果**：JS 解析失败后，页面所有交互（按钮、标签页、弹窗等）全部失效，用户看到的是完全无响应的页面
- **根因**：`let`/`const` 不允许同一作用域内重复声明，且 JS 是整体解析的——一个语法错误会导致整个 `<script>` 块无法执行
- **正确做法**：
  1. 新增变量前，先用 `grep` 检查是否已存在同名变量声明
  2. 如果已存在，直接复用，不要重新声明
  3. 修改后用 `new Function(scriptContent)` 验证 JS 语法正确性

### 2. 修改 dashboard.html 后必须验证 JS 语法
- **错误**：修改 HTML 中内嵌的 JS 代码后，没有验证语法就直接部署
- **后果**：JS 语法错误导致整个页面交互失效，用户无法操作任何功能
- **正确做法**：每次修改 dashboard.html 中的 JS 后，用以下方式验证：
  ```bash
  # 提取 script 内容并用 Node.js 验证
  node -e "const fs=require('fs'); const html=fs.readFileSync('src/dashboard.html','utf8'); const m=html.match(/<script>([\s\S]*?)<\/script>/); new Function(m[1]); console.log('OK')"
  ```
  或者部署后用浏览器开发者工具检查 Console 是否有红色报错
- **特别注意**：Git 合并/暂存恢复时可能留下 `<<<<<<< Updated upstream` 等冲突标记，这些标记在 HTML 中不可见但会破坏 JS 解析。修改后务必检查冲突标记：
  ```bash
  grep -n '<<<<<<\|======\|>>>>>>' src/dashboard.html | grep -v '// ===\|<!-- ===\|/\* ==='
  ```

### 3. 新增功能引入新变量时，检查与现有代码的冲突
- **场景**：在 HTML 内嵌 JS 中添加新功能模块时，容易与已有变量/函数名冲突
- **正确做法**：
  1. 新增 `let`/`const`/`function` 前，`grep -n` 检查同名标识符是否已存在
  2. 特别注意：不同功能模块的变量可能分散在文件不同位置，不能只看附近代码
  3. 变量命名加模块前缀可降低冲突风险（如 `trOutputChart` 而非 `outputChart`）

## 数据库/SQL类

### 1. 新增数据库列需要同步更新三处代码
- **场景**：给 providers 表新增 `chart_color` 列
- **必须同步更新的位置**：
  1. **迁移代码**：`run_migrations()` 中添加 `ALTER TABLE ... ADD COLUMN`
  2. **结构体**：`ProviderRow` 添加对应字段
  3. **手动解析函数**：`row_to_provider()` 中添加字段解析（本项目未用 sqlx 自动映射，而是手动解析）
- **遗漏后果**：编译错误 `missing field`
- **正确做法**：新增列后，全局搜索相关结构体和解析函数，逐一更新

## API/网络类

### 1. 速率计算中 elapsed 时间过短会导致速率虚高
- **现象**：Token 速率曲线显示 Output 峰值 694 tokens/s，体感远没有这么高
- **根因**：`take_snapshot()` 每 10 秒执行一次，但 `elapsed` 是从上次窗口重置到当前的墙钟时间。当请求刚完成（token 加入窗口）后紧接着快照被拍下，`elapsed` 可能只有 0.056s，此时 `39 tokens / 0.056s = 694 tokens/s` 就是虚假高速率
- **修复**：
  1. **后端**：`take_snapshot()` 中，当 `elapsed < 1s` 或 `request_count == 0` 时跳过快照，不写入数据库
  2. **前端**：对速率数据做 3 点滑动平均平滑处理，过滤 `elapsed < 1s` 或 `request_count == 0` 的数据点
- **教训**：速率 = 数量 / 时间 的公式中，分母（时间）必须足够大才有统计意义。极短时间内的速率是噪声，不是信号

### 2. 修改后端逻辑后必须同步更新测试
- **场景**：修改 `take_snapshot()` 增加 `elapsed < 1s` 跳过逻辑后，原有测试中 `record_usage` 后立即 `take_snapshot`（elapsed ≈ 0）的用例全部失败
- **正确做法**：
  1. 修改后端逻辑后，先跑一遍测试看哪些失败
  2. 在 `record_usage` 和 `take_snapshot` 之间加 `tokio::time::sleep(1.1s)` 使 elapsed >= 1.0
  3. 新增专门测试验证短 elapsed 快照被跳过的行为

### 4. 步进滑动窗口（rolling）的正确理解
- **错误理解1**：窗口期每时每刻在滑动（now - size ~ now）
- **错误理解2**：刷新后窗口对齐到整点（如 06:00:00~11:00:00）
- **正确理解**：窗口期基于**订阅时间步进**，刷新时刻（整点/08:00）只是触发窗口向前移动一步
- **官方说明对照**：
  - 5h流控，1h步长，订阅时间 03:47:22 → 初始窗口 22:47:22~03:47:22，04:00刷新后变为 23:47:22~04:47:22，10:00刷新后变为 05:47:22~10:47:22
  - 周流控，1d步长，订阅时间 03:47:22 → 初始窗口 7天前03:47:22~今天03:47:22，08:00刷新后窗口基于03:47:22步进1天
- **核心公式**：
  1. N = 从订阅时间到当前，经过的步进对齐刷新次数
  2. `period_end = subscription_start + N * step_duration`
  3. `period_start = period_end - window_size`
  4. next_refresh = 下一个步进对齐点（整点/08:00等），与窗口期计算无关
- **关键要点**：
  - 订阅开始时间（subscription_start）是服务商级别属性，不是配额级别
  - 窗口期始终基于订阅时间偏移（如 03:47:22），不会对齐到整点（如 04:00:00）
  - 刷新时刻决定"何时步进"，订阅时间决定"步进到哪里"
  - rolling 模式**必须设置订阅开始时间**

### 5. rsync 对 Rust 项目不可靠，同步源码用 cp
- **问题**：对 Rust 项目（src/、Cargo.toml）执行 `rsync -av --delete`，部分文件内容未同步到目标目录，但 rsync 报告成功
- **根因**：rsync 默认以"文件大小 + 修改时间"判断是否更新。如果目标目录已存在同名文件且大小相同，即使内容不同，rsync 也可能跳过
- **修复**：对 Rust 项目使用 `cp -av` 或 `cp --remove-destination`，强制覆盖目标
- **验证**：同步后检查目标文件的 `wc -c` 或 `cargo build --release` 确认编译成功

### 6. Rust 函数签名变更后，必须找到所有调用点并更新
- **场景**：给 `create_provider()` 添加第 25 个参数 `mock_mode: bool`，导致所有测试中调用此函数的地方全部编译失败
- **常见模式**：
  - `db.create_provider()` 签名变化 → 更新所有 tests 中的调用
  - `db.update_provider()` 签名变化 → 更新所有 tests 中的调用
  - 任何 pub async fn 签名变化 → 所有调用点需同步
- **Python 批量修复的坑**：用 Python 字符串替换修复 `.bind()` 调用时，`"choices\.0"` 中的转义点号可能被错误处理。如果原文件使用普通 `"choices.0"`，Python 替换后变成 `"choices\\.0"`，Rust 编译器报 `unknown character escape: \.`
- **正确做法**：
  1. 修改函数签名后，`cargo build --tests 2>&1 | grep "E0061"` 找所有缺失参数的调用
  2. Python 批量替换时注意转义：若原文本是 `"choices.0"`，替换字符串也要是 `"choices.0"`，不是 `"choices\.0"`
  3. 如果不确定，先 `git checkout` 恢复原文件，再用 Python 精确匹配原文本

### 7. 测试中引用 Vec 而非 json! 数组时，小心 as_array().unwrap() panic
- **场景**：`serde_json::Value` 的 `["mappings"]` 字段实际是对象而非数组，`as_array().unwrap()` panic
- **修复**：先检查字段类型，或用 `get("mappings").and_then(|v| v.as_array())`
- **教训**：从 HTTP API 返回的 JSON 形状不一定是你期望的，测试中用 `.unwrap()` 前先 assert 字段存在性

### 8. axum-test 的 `.json()` 方法是链式 builder 模式
- **错误用法**：`server.post("/path", &body)` — 编译错误"this method takes 1 argument"
- **正确用法**：`server.post("/path").json(&body).await`
- **模式**：`method(path)` 返回 builder，builder 提供 `.json(val)` 添加 body，`.await` 执行请求

### 9. 三个虚拟网关各自独立数据库
- **架构**：开发机运行三个虚拟网关（A=49129, B=49130, C=49131），每个有独立 SQLite DB
- **每个 DB 需要单独迁移**：新增列后，每个虚拟网关的 DB 需要手动执行 `ALTER TABLE`
- **API Key 也独立**：每个虚拟网关有自己的 API Key 数据库，不同网关的 API Key 不能混用
- **Mock Provider 配置**：每个虚拟网关需要单独添加 mock provider 才能启用模拟模式
