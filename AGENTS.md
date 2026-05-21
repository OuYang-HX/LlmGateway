# LlmGateway 项目 Agent 守则

## 必须遵守

### 1. 代码修改后必须编译重启
- 执行 `bash deploy-dev.sh update`（开发环境）或 `bash deploy.sh update`（生产环境）
- Linux 进程加载二进制后内存中运行旧版本，源码修改不影响运行中的服务

### 2. 测试驱动开发
- 新需求必须先总结为测试用例（`tests/` 目录）
- 修改代码后必须 `cargo test` 确保全部通过
- 只有需求变化时才可以修改测试用例

### 3. 需求先设计后实现
- 新需求必须先在 `docs/SPEC.md` 末尾按模板梳理技术方案
- 包含：数据库变更、API 变更、核心逻辑、前端变更、测试用例

### 4. 对照经验教训
- 开发完成后对照 `docs/LESSONS_LEARNED.md` 检查已知错误
- 解决新问题后必须补充到该文档

### 5. 生产环境不直接开发
- 生产代码只通过 `git pull` 更新
- 所有开发在开发环境（LlmGateway-dev, dev 分支）进行