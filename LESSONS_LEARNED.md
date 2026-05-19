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
