# LlmGateway 开发指南

> 开发环境的完整操作手册：环境架构、部署命令、Git 工作流、开发规范。

---

## 一、多环境架构

```
客户端 → 开发环境 (49128) → 虚拟网关 A/B/C (49129~49131) → 生产环境 (49127) → 真实LLM服务商
```

**核心设计**：开发环境的所有 LLM 请求都经过虚拟网关 → 生产网关 → 真实服务商，确保生产网关能统计到所有请求。

---

## 二、环境清单

| 环境 | 路径 | 端口 | 数据库 | 分支 | 进程管理 | 部署命令 |
|------|------|------|--------|------|----------|----------|
| 生产 | `/home/oyhx/github/gateway/LlmGateway` | 49127 | `llm_gateway.db` | `main` | systemd (开机自启) | `deploy.sh update` |
| 开发 | `/home/oyhx/github/gateway/LlmGateway-dev` | 49128 | `llm_gateway_dev.db` | `dev` | nohup 后台进程 | `deploy-dev.sh update` |
| 虚拟A | `/home/oyhx/github/gateway/LlmGateway-virtual-A` | 49129 | `llm_gateway_virtual.db` | `main` | nohup 后台进程 | `deploy-virtual.sh update` |
| 虚拟B | `/home/oyhx/github/gateway/LlmGateway-virtual-B` | 49130 | `llm_gateway_virtual.db` | `main` | nohup 后台进程 | `deploy-virtual.sh update` |
| 虚拟C | `/home/oyhx/github/gateway/LlmGateway-virtual-C` | 49131 | `llm_gateway_virtual.db` | `main` | nohup 后台进程 | `deploy-virtual.sh update` |

### 运行时目录

生产环境的二进制、配置、数据库已与代码仓库解耦：

```
~/.local/llm-gateway/
├── bin/llm-gateway       # 二进制
├── config.toml           # 配置
└── llm_gateway.db        # 数据库
```

---

## 三、部署命令

### 开发环境
```bash
bash deploy-dev.sh update   # 编译 + 重启（最常用）
bash deploy-dev.sh status   # 查看状态
bash deploy-dev.sh logs     # 查看日志
bash deploy-dev.sh stop     # 停止
```

### 虚拟网关 (A/B/C 同理)
```bash
bash deploy-virtual.sh update   # 编译 + 重启
bash deploy-virtual.sh status   # 查看状态
bash deploy-virtual.sh logs     # 查看日志
bash deploy-virtual.sh stop     # 停止
```

### 生产环境
```bash
bash deploy.sh update   # 编译 + 重启
bash deploy.sh status   # 查看状态
bash deploy.sh restart  # 重启
bash deploy.sh logs     # 查看日志 (journalctl)
```

---

## 四、网络访问

| 访问方式 | 地址 | 说明 |
|----------|------|------|
| 本机生产 | `http://127.0.0.1:49127` | 直接访问生产网关 |
| 局域网 | `http://192.168.50.188:49127` | 局域网设备访问 |
| 外网 | `http://47.117.247.155:49127` | 通过 frp 隧道访问 |
| 开发环境 | `http://127.0.0.1:49128` | 仅本机 |
| 虚拟网关 | `http://127.0.0.1:49129~49131` | 仅本机 |

### frp 配置
- **frpc**（本机）: `/opt/frp_0.67.0_linux_amd64/frpc.toml`
- **frps**（阿里云）: `/opt/frp_0.67.0_linux_amd64/frps.toml`
- **frpc 连接**: `47.117.247.155:7000`，token: `Oy2.71828`
- **frpc SSH 映射**: `localhost:22` → `47.117.247.155:27182`
- **frpc 网关映射**: `localhost:49127` → `47.117.247.155:49127`

---

## 五、服务商与 API Key 配置

### 生产环境服务商
| 服务商 ID | 名称 | base_url | 认证方式 |
|-----------|------|----------|----------|
| MiniMax-CodingPlan | MiniMax Coding Plan | `https://api.minimaxi.com/v1` | API Key |
| astroncodingplan | 讯飞星辰 Coding Plan | `https://maas-coding-api.cn-huabei-1.xf-yun.com/v2` | API Key |

### 开发环境链路
```
开发环境 (49128) → virtual-a/b/c (49129~49131) → 生产网关 (49127) → 真实服务商
```

### API Key
| 环境 | Key | 用途 |
|------|-----|------|
| 开发环境 | `lgk-c3990c5bb438405e865f2057fa2495fb` | 开发测试 |
| 虚拟网关 A | `lgk-57c37635397e47978df962385dad2589` | 向上游认证 |
| 虚拟网关 B | `lgk-c8d8ed9763a84e1b95f4df5b04521f45` | 向上游认证 |
| 虚拟网关 C | `lgk-1c69172a144b4668b61f8fdfb1ed0227` | 向上游认证 |

---

## 六、Git 工作流

| 仓库 | 远程 | 分支 | push 目标 | 用途 |
|------|------|------|-----------|------|
| LlmGateway | `origin` → github | `main` | origin (main) | 生产代码 |
| LlmGateway-dev | `dev-origin` → github | `dev` | dev-origin (dev) | 开发代码 |

### 开发流程
```
1. 在 LlmGateway-dev (dev 分支) 上开发
2. 修改代码后: bash deploy-dev.sh update
3. 测试通过后: git add -A && git commit && git push dev-origin dev
4. 合并到生产:
   cd /home/oyhx/github/gateway/LlmGateway
   git pull origin main && git merge dev
   bash deploy.sh update && git push origin main
```

### Git 配置
- **用户名**: OuYang-HX
- **邮箱**: OuYang-HX@outlook.com
- **SSH Key**: `~/.ssh/id_ed25519_github`
- **远程方式**: `git@github.com:OuYang-HX/LlmGateway.git`（SSH 方式，完全免密）

---

## 七、注意事项

1. **生产环境只通过 `git pull` 更新**，不要在生产仓库上直接开发
2. **测试删除类 API 时先备份数据库**，SQLite DELETE 不可逆
3. **虚拟网关不是 systemd 服务**，机器重启后需要手动启动
4. **开发环境数据库从生产复制**，包含生产 API Key，注意安全
5. **frp 隧道只映射了 49127**（生产端口），开发环境端口不对外暴露
6. **SQLite 文件锁定** — 进程持有连接时文件被锁定，先停止进程再复制或覆盖数据库
7. **模型映射配置** — 新增服务商模型时，需同时在 models 和 model_mappings 表中配置
8. **开发环境 API Key** — 每次重建开发数据库后需要重新创建，旧 Key 会失效
