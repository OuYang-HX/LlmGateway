# LlmGateway 开发环境架构

## 概述

本文档描述 LlmGateway 项目的多层级开发测试架构，适用于日常开发调试、API 测试、自动化回归等场景。

---

## 架构拓扑

```
┌─────────────────────────────────────────────────────────────┐
│                        开发工具                               │
│          (pi-agent / curl / IDE / 自动化脚本 / etc.)          │
│                       端口: 任意                              │
└────────────────────────────┬────────────────────────────────┘
                             │ HTTP 请求 (API Key 认证)
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                       开发环境                                │
│                  LlmGateway-dev (49128)                      │
│                                                             │
│  职责: 日常开发调试的唯一入口                                 │
│  服务商配置: virtual-a, virtual-b, virtual-c                 │
│             (指向三个虚拟网关，实现负载均衡)                  │
│  数据库: llm_gateway_dev.db (独立，不影响生产)               │
└────────────────────────────┬────────────────────────────────┘
                             │ 负载均衡转发 (轮询 / 权重)
                             ▼
        ┌────────────────────┼────────────────────┐
        │                    │                    │
        ▼                    ▼                    ▼
┌───────────────┐    ┌───────────────┐    ┌───────────────┐
│   虚拟网关 A   │    │   虚拟网关 B   │    │   虚拟网关 C   │
│ 49129 / VA    │    │ 49130 / VB    │    │ 49131 / VC    │
└───────┬───────┘    └───────┬───────┘    └───────┬───────┘
        │                    │                    │
        └────────────────────┼────────────────────┘
                             │ HTTP 转发 (透传，不做统计)
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                       生产环境                                │
│                 LlmGateway (49127)                           │
│                                                             │
│  职责: 所有请求的统计中心                                    │
│  服务商: MiniMax-CodingPlan, 讯飞星辰 Coding Plan 等        │
│  数据库: llm_gateway.db (所有真实请求的日志)                 │
│  访问: http://127.0.0.1:49127 或 http://47.117.247.155:49127│
└────────────────────────────┬────────────────────────────────┘
                             │ 真实 LLM API 调用
                             ▼
              ┌──────────────┴──────────────┐
              │        真实 LLM 服务商         │
              │  MiniMax / 讯飞星辰 / etc.   │
              └─────────────────────────────┘
```

---

## 环境清单

| 环境 | 路径 | 端口 | 用途 | 数据库 |
|------|------|------|------|--------|
| 生产 | `/home/oyhx/github/LlmGateway` | 49127 | 对外服务，只接收 pull 更新 | `llm_gateway.db` |
| 开发 | `/home/oyhx/github/LlmGateway-dev` | 49128 | 日常开发调试入口 | `llm_gateway_dev.db` |
| 虚拟 A | `/home/oyhx/github/LlmGateway-virtual-A` | 49129 | 负载均衡节点 | `llm_gateway_virtual.db` |
| 虚拟 B | `/home/oyhx/github/LlmGateway-virtual-B` | 49130 | 负载均衡节点 | `llm_gateway_virtual.db` |
| 虚拟 C | `/home/oyhx/github/LlmGateway-virtual-C` | 49131 | 负载均衡节点 | `llm_gateway_virtual.db` |

---

## 启动与停止

### 开发环境

```bash
cd /home/oyhx/github/LlmGateway-dev

# 编译 + 重启 (最常用)
bash deploy-dev.sh update

# 查看状态
bash deploy-dev.sh status

# 查看日志
bash deploy-dev.sh logs

# 停止
bash deploy-dev.sh stop
```

### 虚拟网关 (A / B / C)

```bash
# 以虚拟网关 A 为例 (B、C 替换端口和目录即可)
cd /home/oyhx/github/LlmGateway-virtual-A
bash deploy-virtual.sh update   # 编译 + 重启
bash deploy-virtual.sh status   # 查看状态
bash deploy-virtual.sh logs     # 查看日志
bash deploy-virtual.sh stop      # 停止
```

### 一次性重启所有环境

```bash
# 停止所有
for port in 49128 49129 49130 49131; do
  pid=$(ss -tlnp | grep ":$port " | grep -oP 'pid=\K[0-9]+')
  [ -n "$pid" ] && kill $pid
done
sleep 1

# 启动虚拟网关 (49129-49131)
for letter in A B C; do
  port=$((49128 + 10#$(echo $letter | tr ABC 012)))
  cd /home/oyhx/github/LlmGateway-virtual-$letter
  export LLM_GW_SERVER_PORT=$port
  export LLM_GW_DATABASE_URL="sqlite:llm_gateway_virtual.db"
  nohup ./target/release/llm-gateway > /tmp/llm-gateway-virtual-$letter.log 2>&1 &
done

# 启动开发环境 (49128)
cd /home/oyhx/github/LlmGateway-dev
nohup ./target/release/llm-gateway > /tmp/llm-gateway-dev.log 2>&1 &

sleep 2
ss -tlnp | grep -E '49128|49129|49130|49131'
```

---

## 数据库说明

### 开发数据库 (`llm_gateway_dev.db`)

- 从生产数据库复制而来，保持初始数据一致
- 服务商配置为 `virtual-a/b/c`，指向三个虚拟网关
- 开发调试产生的请求会被虚拟网关转发到生产环境，最终由生产环境统计
- **重要**: 开发数据库独立，不影响生产数据库

### 虚拟网关数据库 (`llm_gateway_virtual.db`)

- 初始化为空数据库 (通过 SQLite schema 创建)
- 包含服务商 `virtual-a/b/c`，指向生产环境作为上游
- 不存储任何请求统计，只做透传

### 生产数据库 (`llm_gateway.db`)

- 所有真实请求的最终统计中心
- 虚拟网关的请求在生产环境中显示为来自生产环境的 API Key

---

## API Key 配置

### 开发环境 API Key

在开发环境的 Dashboard 中生成，示例:

```
lgk-c3990c5bb438405e865f2057fa2495fb
```

### 虚拟网关 API Key

每个虚拟网关有独立的 API Key，用于向上游 (生产环境) 认证:

| 网关 | API Key |
|------|---------|
| 虚拟网关 A | `lgk-57c37635397e47978df962385dad2589` |
| 虚拟网关 B | `lgk-c8d8ed9763a84e1b95f4df5b04521f45` |
| 虚拟网关 C | `lgk-1c69172a144b4668b61f8fdfb1ed0227` |

这些 Key 在虚拟网关配置时已自动写入各自的数据库，无需手动管理。

---

## 请求示例

### 通过开发环境调用 (完整链路)

```bash
curl http://127.0.0.1:49128/v1/chat/completions \
  -H "Authorization: Bearer lgk-c3990c5bb438405e865f2057fa2495fb" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "MiniMax-M2.7-highspeed",
    "messages": [{"role": "user", "content": "say hi"}],
    "max_tokens": 10
  }'
```

### 直接调用虚拟网关 (跳过负载均衡，直接指定某节点)

```bash
# 直接调用虚拟网关 A
curl http://127.0.0.1:49129/v1/chat/completions \
  -H "Authorization: Bearer lgk-57c37635397e47978df962385dad2589" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "MiniMax-M2.7-highspeed",
    "messages": [{"role": "user", "content": "say hi"}],
    "max_tokens": 10
  }'
```

### 直接调用生产环境 (绕过所有网关)

```bash
curl http://127.0.0.1:49127/v1/chat/completions \
  -H "Authorization: Bearer lgk-04dbd369643b49d58022cf110d626492" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "MiniMax-M2.7-highspeed",
    "messages": [{"role": "user", "content": "say hi"}],
    "max_tokens": 10
  }'
```

---

## 开发流程

### 每次开发前的准备

```bash
# 1. 确保所有环境运行中
ss -tlnp | grep -E '49128|49129|49130|49131'

# 2. 如有进程未启动，按上文"一次性重启所有环境"启动

# 3. 在开发环境进行开发调试
cd /home/oyhx/github/LlmGateway-dev
# ... 修改代码 ...

# 4. 编译并重启开发环境
bash deploy-dev.sh update

# 5. 测试
curl http://127.0.0.1:49128/...
```

### 提交代码

```bash
cd /home/oyhx/github/LlmGateway-dev
git add -A
git commit -m "描述"
git push dev-origin dev   # 推送到 dev 分支
```

### 生产环境更新

```bash
cd /home/oyhx/github/LlmGateway
git pull origin main      # 从 main 分支拉取更新
bash deploy.sh update     # 编译并重启
```

---

## 注意事项

1. **不要在生产环境直接开发** — 所有新功能在开发环境调试完成后，再合并到 main 分支更新生产环境

2. **测试删除类 API 时先备份** — 使用 `cp llm_gateway.db llm_gateway.db.bak` 备份数据库

3. **进程管理** — 杀掉进程时使用 `kill <pid>` 而非 `pkill -f`（避免误杀其他同名进程），确认 PID 与端口对应关系后再杀

4. **数据库文件锁定** — SQLite 在进程持有连接时文件会被锁定，无法直接覆盖。先停止进程再复制或覆盖数据库文件

5. **模型映射配置** — 新增服务商模型时，需要同时在开发环境的 `models` 表和 `model_mappings` 表中配置映射关系，虚拟网关也需要相同的映射配置

6. **开发环境 API Key** — 每次重建开发数据库后需要重新创建开发环境 API Key，旧 Key 会失效

---

## 目录结构

```
/home/oyhx/
├── github/
│   ├── LlmGateway/              # 生产环境
│   │   ├── src/
│   │   ├── Cargo.toml
│   │   ├── deploy.sh            # 生产部署脚本
│   │   ├── config.toml
│   │   └── llm_gateway.db       # 生产数据库
│   │
│   ├── LlmGateway-dev/          # 开发环境
│   │   ├── src/
│   │   ├── Cargo.toml
│   │   ├── deploy-dev.sh        # 开发部署脚本
│   │   ├── config.toml
│   │   ├── llm_gateway_dev.db   # 开发数据库
│   │   └── DEVELOPER_GUIDE.md   # 本文档
│   │
│   ├── LlmGateway-virtual-A/    # 虚拟网关 A (49129)
│   ├── LlmGateway-virtual-B/    # 虚拟网关 B (49130)
│   └── LlmGateway-virtual-C/    # 虚拟网关 C (49131)
│       ├── deploy-virtual.sh    # 通用虚拟网关部署脚本
│       └── llm_gateway_virtual.db
```
