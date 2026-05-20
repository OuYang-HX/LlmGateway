# LlmGateway 开发流程文档

## 一、整体架构

```
用户/客户端
    │
    ▼ (外网: 47.117.247.155:49127)
┌──────────────────────────────────────────────────────┐
│  阿里云 frps (47.117.247.155)                         │
│  frp 隧道转发 → 本机 127.0.0.1:49127                  │
└──────────────────────────────────────────────────────┘
    │
    ▼ (本机: 127.0.0.1:49127 / 局域网: 192.168.50.188:49127)
┌──────────────────────────────────────────────────────┐
│                  生产环境 (49127)                      │
│  路径: /home/oyhx/github/LlmGateway                  │
│  分支: main                                           │
│  服务: systemd (llm-gateway.service, 开机自启)         │
│  数据库: llm_gateway.db                               │
│  服务商: MiniMax Coding Plan / 讯飞星辰 Coding Plan    │
│  职责: 对接真实 LLM 服务商，统计所有请求               │
└──────────────────────────────────────────────────────┘
    │ ▲
    │ │ (生产网关作为"服务商")
    │ ▼
┌────────────────┐ ┌────────────────┐ ┌────────────────┐
│  虚拟网关 A     │ │  虚拟网关 B     │ │  虚拟网关 C     │
│  端口 49129     │ │  端口 49130     │ │  端口 49131     │
│  路径: -virtual-A│ │  路径: -virtual-B│ │  路径: -virtual-C│
│  数据库: _virtual│ │  数据库: _virtual│ │  数据库: _virtual│
└───────┬────────┘ └───────┬────────┘ └───────┬────────┘
        │                  │                  │
        └──────────┬───────┘──────────────────┘
                   ▼
        ┌──────────────────────┐
        │  开发环境 (49128)      │
        │  路径: LlmGateway-dev │
        │  分支: dev            │
        │  数据库: _dev.db      │
        │  服务商: 3个虚拟网关   │
        │  职责: 开发调试        │
        └──────────────────────┘
```

**核心设计**：开发环境的所有 LLM 请求都经过虚拟网关 → 生产网关 → 真实服务商，确保生产网关能统计到所有请求。

---

## 二、环境清单

| 环境 | 路径 | 端口 | 数据库 | 分支 | 部署命令 | 进程管理 |
|------|------|------|--------|------|----------|----------|
| 生产 | `/home/oyhx/github/LlmGateway` | 49127 | `llm_gateway.db` | `main` | `deploy.sh update` | systemd (开机自启) |
| 开发 | `/home/oyhx/github/LlmGateway-dev` | 49128 | `llm_gateway_dev.db` | `dev` | `deploy-dev.sh update` | nohup 后台进程 |
| 虚拟A | `/home/oyhx/github/LlmGateway-virtual-A` | 49129 | `llm_gateway_virtual.db` | `main` | `deploy-virtual.sh update` | nohup 后台进程 |
| 虚拟B | `/home/oyhx/github/LlmGateway-virtual-B` | 49130 | `llm_gateway_virtual.db` | `main` | `deploy-virtual.sh update` | nohup 后台进程 |
| 虚拟C | `/home/oyhx/github/LlmGateway-virtual-C` | 49131 | `llm_gateway_virtual.db` | `main` | `deploy-virtual.sh update` | nohup 后台进程 |

### 部署命令详解

```bash
# 生产环境
bash /home/oyhx/github/LlmGateway/deploy.sh update   # 编译+重启
bash /home/oyhx/github/LlmGateway/deploy.sh status   # 查看状态
bash /home/oyhx/github/LlmGateway/deploy.sh restart  # 重启
bash /home/oyhx/github/LlmGateway/deploy.sh logs     # 查看日志

# 开发环境
bash /home/oyhx/github/LlmGateway-dev/deploy-dev.sh update   # 编译+重启
bash /home/oyhx/github/LlmGateway-dev/deploy-dev.sh status   # 查看状态
bash /home/oyhx/github/LlmGateway-dev/deploy-dev.sh logs     # 查看日志 (/tmp/llm-gateway-dev.log)
bash /home/oyhx/github/LlmGateway-dev/deploy-dev.sh stop     # 停止

# 虚拟网关 (A/B/C 同理)
bash /home/oyhx/github/LlmGateway-virtual-A/deploy-virtual.sh update  # 编译+重启
bash /home/oyhx/github/LlmGateway-virtual-A/deploy-virtual.sh status  # 查看状态
bash /home/oyhx/github/LlmGateway-virtual-A/deploy-virtual.sh logs    # 查看日志 (/tmp/llm-gateway-virtual-A.log)
bash /home/oyhx/github/LlmGateway-virtual-A/deploy-virtual.sh stop    # 停止
```

---

## 三、网络访问

| 访问方式 | 地址 | 说明 |
|----------|------|------|
| 本机 | `http://127.0.0.1:49127` | 直接访问生产网关 |
| 局域网 | `http://192.168.50.188:49127` | 局域网设备访问 |
| 外网 | `http://47.117.247.155:49127` | 通过 frp 隧道访问 |
| 开发环境 | `http://127.0.0.1:49128` | 仅本机可访问 |
| 虚拟网关 | `http://127.0.0.1:49129~49131` | 仅本机可访问 |

### frp 配置

- **frpc**（本机）: `/opt/frp_0.67.0_linux_amd64/frpc.toml`
- **frps**（阿里云）: `/opt/frp_0.67.0_linux_amd64/frps.toml`
- **frpc 连接**: `47.117.247.155:7000`，token: `Oy2.71828`
- **frpc SSH 映射**: `localhost:22` → `47.117.247.155:27182`
- **frpc 网关映射**: `localhost:49127` → `47.117.247.155:49127`

---

## 四、服务商与 API Key 配置

### 生产环境服务商

| 服务商 ID | 名称 | base_url | 认证方式 |
|-----------|------|----------|----------|
| MiniMax-CodingPlan | MiniMax Coding Plan | `https://api.minimaxi.com/v1` | API Key |
| astroncodingplan | 讯飞星辰 Coding Plan | `https://maas-coding-api.cn-huabei-1.xf-yun.com/v2` | API Key |

### 生产环境 API Keys

| 名称 | 前缀 | 用途 |
|------|------|------|
| mi15-pi | lgk-dcd08a28 | 小米手机 pi 客户端 |
| my-feishu-agent | lgk-e9529445 | 飞书机器人 |
| opencode | lgk-ec899d43 | OpenCode 终端 |
| openclaw | lgk-da50470e | OpenClaw |
| qwenpaw | lgk-b40e8e75 | QwenPaw |
| hermes | lgk-0e57716c | Hermes |
| pi-debian | lgk-04dbd369 | 本机 pi agent |

### 开发环境链路

开发环境不直接对接真实服务商，而是通过虚拟网关链路：

```
开发环境 (49128)
  ├── 服务商: virtual-a → http://127.0.0.1:49129/v1
  ├── 服务商: virtual-b → http://127.0.0.1:49130/v1
  └── 服务商: virtual-c → http://127.0.0.1:49131/v1

虚拟网关 A/B/C (49129~49131)
  └── 服务商: 各自指向 http://127.0.0.1:49127/v1 (生产网关)

生产环境 (49127)
  ├── 服务商: MiniMax Coding Plan → https://api.minimaxi.com/v1
  └── 服务商: 讯飞星辰 Coding Plan → https://maas-coding-api.cn-huabei-1.xf-yun.com/v2
```

**开发环境 API Key**: `lgk-c3990c5b...`（dev-test-key）

---

## 五、Git 工作流

### 仓库关系

| 仓库 | 远程 | 分支 | push 目标 | 用途 |
|------|------|------|-----------|------|
| LlmGateway | `origin` → github | `main` | origin (main) | 生产代码，只通过 pull 更新 |
| LlmGateway-dev | `origin` (no-push) + `dev-origin` → github | `dev` | dev-origin (dev) | 开发代码，自由 push |

### 开发流程

```
1. 在 LlmGateway-dev (dev 分支) 上开发
2. 修改代码后: bash deploy-dev.sh update
3. 测试通过后: git add -A && git commit && git push dev-origin dev
4. 合并到生产:
   cd /home/oyhx/github/LlmGateway
   git pull origin main        # 先拉取最新
   git merge dev               # 合并开发分支
   bash deploy.sh update       # 编译+重启生产
   git push origin main        # 推送到远程
```

### Git 配置

- **用户名**: OuYang-HX
- **邮箱**: OuYang-HX@outlook.com
- **SSH Key**: `~/.ssh/id_ed25519_github`
- **远程方式**: `git@github.com:OuYang-HX/LlmGateway.git`（SSH 方式，完全免密）

---

## 六、开发规范

### 代码修改后必须编译重启

Rust 是编译型语言，修改源码后必须重新编译并重启服务才能生效：

```bash
# 开发环境
bash /home/oyhx/github/LlmGateway-dev/deploy-dev.sh update

# 生产环境
bash /home/oyhx/github/LlmGateway/deploy.sh update
```

### TDD 开发流程

1. **先写测试**：在 `tests/` 目录下编写测试用例
2. **运行测试**：`cargo test` 确保测试失败（红）
3. **实现功能**：编写最小代码使测试通过（绿）
4. **重构**：优化代码，确保测试仍然通过
5. **提交**：测试全部通过后提交

### 文档要求

- **需求设计**: `/home/oyhx/github/LlmGateway/DESIGN.md`
- **经验教训**: `/home/oyhx/github/LlmGateway/LESSONS_LEARNED.md`
- 新需求必须先在 DESIGN.md 中梳理
- 遇到问题解决后必须补充到 LESSONS_LEARNED.md

---

## 七、阿里云服务器

| 项目 | 值 |
|------|-----|
| IP | `47.117.247.155` |
| 用户 | `root` |
| 密码 | `Oy271828` |
| SSH 端口 | `22` (直连) / `27182` (frp 映射) |
| 用途 | frps 中转、外网访问网关 |

---

## 八、常用操作速查

### 重启所有服务

```bash
# 生产环境
sudo systemctl restart llm-gateway

# 开发环境 + 虚拟网关
for port in 49128 49129 49130 49131; do
  pids=$(ss -tlnp | grep $port | grep -oP 'pid=\K[0-9]+')
  for pid in $pids; do kill -9 $pid 2>/dev/null; done
done

cd /home/oyhx/github/LlmGateway-dev && nohup ./target/release/llm-gateway > /tmp/llm-gateway-dev.log 2>&1 &
for letter in A B C; do
  cd /home/oyhx/github/LlmGateway-virtual-$letter && nohup ./target/release/llm-gateway > /tmp/llm-gateway-virtual-$letter.log 2>&1 &
done
```

### 查看所有服务状态

```bash
ss -tlnp | grep -E '49127|49128|49129|49130|49131'
```

### 端到端测试

```bash
# 测试完整链路: 开发 → 虚拟网关 → 生产 → 真实LLM
curl -s http://127.0.0.1:49128/v1/chat/completions \
  -H "Authorization: Bearer lgk-c3990c5bb438405e865f2057fa2495fb" \
  -H "Content-Type: application/json" \
  -d '{"model":"MiniMax-M2.7-highspeed","messages":[{"role":"user","content":"hi"}],"max_tokens":5}'
```

### 查看生产日志

```bash
# 最近的请求日志
curl -s "http://127.0.0.1:49127/api/v1/logs?page=1&page_size=5" | python3 -m json.tool

# 服务日志
sudo journalctl -u llm-gateway --no-pager -n 50
```

---

## 九、注意事项

1. **生产环境只通过 `git pull` 更新**，不要在生产仓库上直接开发
2. **测试删除类 API 时先备份数据库**，SQLite DELETE 不可逆
3. **虚拟网关不是 systemd 服务**，机器重启后需要手动启动
4. **开发环境数据库从生产复制**，包含生产 API Key，注意安全
5. **frp 隧道只映射了 49127**（生产端口），开发环境端口不对外暴露
