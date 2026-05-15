# 项目记忆文件

## LlmGateway 项目总结

### 项目地址
- **GitHub**: https://github.com/OuYang-HX/LlmGateway
- **本机代码**: `/home/oyhx/code/LlmGateway`
- **本机数据库**: `/home/oyhx/code/LlmGateway/llm_gateway.db`
- **阿里云代码**: `/root/LlmGateway`

### 服务架构

```
用户 → http://47.117.247.155:49127 (阿里云 frps)
                                ↓ frp 隧道
                        → 本机 127.0.0.1:49127 (frpc)
                                ↓
                        → 127.0.0.1:49127 (llm-gateway)
```

### 服务端口
- **本地网关**: `http://127.0.0.1:49127`（本机直接访问）
- **局域网**: `http://192.168.50.188:49127`（局域网其他设备）
- **外网**: `http://47.117.247.155:49127`（frp 转发）

### 服务管理命令
```bash
# 本机
sudo systemctl restart llm-gateway-local   # 重启服务
sudo systemctl status llm-gateway-local    # 查看状态
bash /home/oyhx/code/LlmGateway/update.sh  # 一键更新（拉代码+编译+重启）

# 阿里云（已停用）
systemctl stop llm-gateway
```

### frp 配置
- **frpc**（本机）: `/opt/frp_0.67.0_linux_amd64/frpc.toml`
- **frps**（阿里云）: `/opt/frp_0.67.0_linux_amd64/frps.toml`
- **frpc 连接**: `47.117.247.155:7000`，token: `Oy2.71828`
- **frpc SSH 映射**: `localhost:22` → `47.117.247.155:27182`
- **frpc 网关映射**: `localhost:49127` → `47.117.247.155:49127`

### 阿里云 SSH 信息
- **IP**: `47.117.247.155`
- **用户**: `root`
- **密码**: `Oy271828`
- **SSH 端口**: `22`

### 当前数据
- **服务商**: `MiniMax-CodingPlan`（base_url: `https://api.minimaxi.com/v1`）
- **API Key**: `lgk-04dbd369643b49d58022cf110d626492`（test-key）
- **已测试通过的模型**: `MiniMax-M2.7-highspeed`、`astron-code-latest`

### 核心功能
- 服务商管理（含 API Key / 动态 Token 认证）
- **模型 ID 管理**（每个服务商可配置允许的模型 ID）
- **模型连接测试**（每个模型有测试按钮，失败自动标记不可用）
- **全部测试连接**（一键测试所有模型）
- 请求日志与统计分析
- 按模型校验，拒绝未测试/失败模型的使用

### Git 配置
- **用户名**: OuYang-HX
- **邮箱**: oyhx@debian.oyhx.net
- **SSH Key**: `~/.ssh/id_ed25519_github`
- **远程方式**: `git@github.com:OuYang-HX/LlmGateway.git`（SSH 方式，完全免密）

### 本机已配置的工具
- Rust + Cargo（已配置清华 crates.io sparse 镜像）
- frpc（已配置开机自启）
- llm-gateway-local（systemd 服务，开机自启）
