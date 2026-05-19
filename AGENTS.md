# LlmGateway 项目 Agent 守则

## 必须遵守

### 代码修改后必须编译重启
- **每次修改代码后，必须执行 `bash /home/oyhx/github/LlmGateway/deploy.sh update`**
- 该命令会自动编译（`cargo build --release`）+ 重启服务（`systemctl restart llm-gateway`）
- Linux 进程加载二进制文件后，内存中运行的是旧版本，仅修改源码不会影响运行中的服务
- 只有 restart 后才会加载新版本

### 服务信息
- **本地网关**: `http://127.0.0.1:49127`
- **外网**: `http://47.117.247.155:49127`（frp 转发）
- **管理命令**: `bash /home/oyhx/github/LlmGateway/deploy.sh {update|status|restart|logs}`

### Git 配置
- **用户名**: OuYang-HX
- **邮箱**: OuYang-HX@outlook.com
- **SSH Key**: `~/.ssh/id_ed25519_github`
- **远程方式**: `git@github.com:OuYang-HX/LlmGateway.git`（SSH 方式，完全免密）

### 经验教训
- **文档位置**: `/home/oyhx/github/LlmGateway/LESSONS_LEARNED.md`
- 每次开发完成后，必须对照经验教训检查是否犯了已知错误
- 遇到新问题并解决后，必须将经验教训补充到该文档
- 开发过程中遇到没有思路的问题，先查阅该文档寻找启发

### 需求设计与技术方案
- **文档位置**: `/home/oyhx/github/LlmGateway/DESIGN.md`
- 每次开发新需求时，必须先在 DESIGN.md 中梳理需求设计与技术方案
- 文档中包含完整的数据库设计、API 端点、核心功能实现和测试覆盖
- 新需求按文档末尾的模板追加

### 测试驱动开发
- **测试目录**: `/home/oyhx/github/LlmGateway/tests/`
- 新需求必须先总结为测试用例
- 每次修改代码后，必须运行测试确保全部通过
- 只有需求发生变化时，才可以修改测试用例
- 运行测试命令：`cargo test`
