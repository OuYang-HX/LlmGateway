#!/bin/bash
# ============================================================
# LLM Gateway - 部署管理脚本
# 用法: bash deploy.sh [命令]
#
# 命令:
#   setup      首次部署（安装 Rust + 编译 + 创建服务 + 启动）
#   update     编译 + 重启（日常更新，最常用）
#   build      仅编译
#   start      启动服务
#   stop       停止服务
#   restart    重启服务
#   status     查看服务状态 + 端口监听
#   logs [N]   查看最近 N 条日志（默认 50，持续跟踪）
#   uninstall  卸载服务（保留数据库）
# ============================================================

set -e

# ---- 可配置项 ----
SERVICE_NAME="llm-gateway"
SERVICE_DESC="LLM Gateway - 大模型网关"
DEFAULT_PORT=49127
DEFAULT_HOST="0.0.0.0"

# ---- 自动检测 ----
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_PATH="$PROJECT_DIR/target/release/llm-gateway"
CONFIG_FILE="$PROJECT_DIR/config.toml"
DB_FILE="$PROJECT_DIR/llm_gateway.db"
SERVICE_FILE="/etc/systemd/system/${SERVICE_NAME}.service"

# ---- 颜色 ----
R='\033[0;31m' G='\033[0;32m' Y='\033[1;33m' B='\033[0;34m' NC='\033[0m'
info()  { echo -e "${B}[INFO]${NC} $1"; }
ok()    { echo -e "${G}[OK]${NC} $1"; }
warn()  { echo -e "${Y}[WARN]${NC} $1"; }
err()   { echo -e "${R}[ERROR]${NC} $1"; }

# ============================================================
# 核心功能
# ============================================================

# ---- 安装 Rust ----
install_rust() {
    if command -v cargo &>/dev/null; then
        info "Rust 已安装: $(rustc --version)"
        return
    fi
    err "未安装 Rust 工具链"
    info "正在安装 Rust（使用国内镜像加速）..."
    export RUSTUP_DIST_SERVER=https://mirrors.tuna.tsinghua.edu.cn/rustup
    export RUSTUP_UPDATE_ROOT=https://mirrors.tuna.tsinghua.edu.cn/rustup/rustup
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    ok "Rust 安装完成: $(rustc --version)"
}

# ---- 配置 crates.io 镜像 ----
configure_cargo_mirror() {
    CARGO_CONFIG="$HOME/.cargo/config.toml"
    if [ -f "$CARGO_CONFIG" ] && grep -q 'sparse+' "$CARGO_CONFIG"; then
        return
    fi
    info "配置 crates.io 清华镜像..."
    mkdir -p "$HOME/.cargo"
    cat > "$CARGO_CONFIG" << 'EOF'
[source.crates-io]
replace-with = "tuna"

[source.tuna]
registry = "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/"
EOF
    ok "crates.io 镜像已配置"
}

# ---- 确保配置文件存在 ----
ensure_config() {
    if [ -f "$CONFIG_FILE" ]; then
        return
    fi
    warn "未找到 config.toml"
    if [ -f "$PROJECT_DIR/config.example.toml" ]; then
        cp "$PROJECT_DIR/config.example.toml" "$CONFIG_FILE"
    else
        cat > "$CONFIG_FILE" << EOF
[server]
host = "${DEFAULT_HOST}"
port = ${DEFAULT_PORT}

[database]
url = "sqlite:${DB_FILE}?mode=rwc"
EOF
    fi
    ok "配置文件已创建: config.toml"
    warn "请根据需要修改配置后再次运行"
}

# ---- 编译 ----
do_build() {
    cd "$PROJECT_DIR"
    info "编译 release 版本（编译期间服务不受影响）..."
    if cargo build --release 2>&1; then
        ok "编译完成: $(du -h "$BINARY_PATH" | cut -f1)"
    else
        err "编译失败"
        exit 1
    fi
}

# ---- 创建 systemd 服务 ----
create_service() {
    if [ -f "$SERVICE_FILE" ]; then
        info "服务文件已存在"
        # 检查是否需要更新（路径变化等）
        return
    fi

    info "创建 systemd 服务..."
    CURRENT_USER="$(whoami)"

    sudo tee "$SERVICE_FILE" > /dev/null << EOF
[Unit]
Description=${SERVICE_DESC}
After=network.target

[Service]
Type=simple
User=${CURRENT_USER}
WorkingDirectory=${PROJECT_DIR}
ExecStart=${BINARY_PATH}
Restart=on-failure
RestartSec=5

# 安全：限制写入路径
NoNewPrivileges=true
ProtectSystem=strict
ReadWritePaths=${PROJECT_DIR}

# 日志
StandardOutput=journal
StandardError=journal
SyslogIdentifier=${SERVICE_NAME}

[Install]
WantedBy=multi-user.target
EOF

    sudo systemctl daemon-reload
    sudo systemctl enable "$SERVICE_NAME"
    ok "服务已创建并设为开机自启"
}

# ---- 服务操作 ----
svc_is_active() {
    sudo systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null
}

do_start() {
    info "启动服务..."
    sudo systemctl start "$SERVICE_NAME"
    sleep 1
    if svc_is_active; then
        ok "服务已启动"
    else
        err "服务启动失败！"
        sudo journalctl -u "$SERVICE_NAME" -n 20 --no-pager
        exit 1
    fi
}

do_stop() {
    if ! svc_is_active; then
        info "服务未在运行"
        return
    fi
    info "停止服务..."
    sudo systemctl stop "$SERVICE_NAME"
    ok "服务已停止"
}

do_restart() {
    info "重启服务..."
    sudo systemctl restart "$SERVICE_NAME"
    sleep 1
    if svc_is_active; then
        ok "服务已重启"
    else
        err "服务重启失败！"
        sudo journalctl -u "$SERVICE_NAME" -n 20 --no-pager
        exit 1
    fi
}

do_status() {
    echo ""
    if svc_is_active; then
        ok "服务运行中"
    else
        warn "服务未运行"
    fi
    echo ""
    sudo systemctl status "$SERVICE_NAME" --no-pager 2>/dev/null || true
    echo ""
    # 端口检测
    PORT=$(grep -oP 'port\s*=\s*\K\d+' "$CONFIG_FILE" 2>/dev/null || echo "$DEFAULT_PORT")
    if ss -tlnp 2>/dev/null | grep -q ":${PORT} "; then
        ok "端口 ${PORT} 正在监听"
    else
        warn "端口 ${PORT} 未在监听"
    fi
}

do_logs() {
    local lines="${1:-50}"
    sudo journalctl -u "$SERVICE_NAME" -n "$lines" --no-pager -f
}

# ============================================================
# 复合命令
# ============================================================

# ---- 首次部署 ----
do_setup() {
    echo ""
    echo "========================================="
    echo "  LLM Gateway 首次部署"
    echo "========================================="
    echo ""

    install_rust
    configure_cargo_mirror
    ensure_config
    do_build
    create_service
    do_start

    # 显示结果
    PORT=$(grep -oP 'port\s*=\s*\K\d+' "$CONFIG_FILE" 2>/dev/null || echo "$DEFAULT_PORT")
    LAN_IP="$(hostname -I 2>/dev/null | awk '{print $1}')"
    echo ""
    echo "========================================="
    ok "部署完成！"
    echo ""
    info "访问地址:"
    echo "  本机:   http://127.0.0.1:${PORT}"
    [ -n "$LAN_IP" ] && echo "  局域网: http://${LAN_IP}:${PORT}"
    echo ""
    info "日常命令:"
    echo "  bash deploy.sh update   # 编译+重启（最常用）"
    echo "  bash deploy.sh status   # 查看状态"
    echo "  bash deploy.sh logs     # 查看日志"
    echo "========================================="
}

# ---- 日常更新（最常用） ----
do_update() {
    echo ""
    echo "=== LLM Gateway 更新 ==="
    echo ""

    # 如果有远程仓库，先拉代码
    cd "$PROJECT_DIR"
    if git remote | grep -q origin 2>/dev/null; then
        BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "main")
        info "[1/3] 拉取最新代码 (origin/$BRANCH)..."
        git fetch origin "$BRANCH"
        LOCAL=$(git rev-parse HEAD)
        REMOTE=$(git rev-parse "origin/$BRANCH" 2>/dev/null || echo "$LOCAL")
        if [ "$LOCAL" != "$REMOTE" ]; then
            # 检查是否有未提交的本地修改
            if ! git diff --quiet HEAD 2>/dev/null; then
                warn "检测到本地未提交的修改，暂存后拉取..."
                git stash
                git reset --hard "origin/$BRANCH"
                git stash pop 2>/dev/null || true
            else
                git reset --hard "origin/$BRANCH"
            fi
            ok "代码已更新"
        else
            info "远程代码无变化"
        fi
    else
        info "[1/3] 无远程仓库，跳过拉取"
    fi

    # 编译（期间服务不受影响）
    info "[2/3] 编译..."
    do_build

    # 重启（此时才切换到新版本）
    info "[3/3] 重启服务..."
    do_restart

    echo ""
    ok "更新完成！"
}

# ---- 卸载 ----
do_uninstall() {
    echo ""
    warn "即将卸载 ${SERVICE_DESC}"
    warn "数据库将保留: ${DB_FILE}"
    echo ""
    read -rp "确认卸载？(y/N): " confirm
    [ "$confirm" = "y" ] || [ "$confirm" = "Y" ] || { info "已取消"; exit 0; }

    do_stop
    if [ -f "$SERVICE_FILE" ]; then
        sudo systemctl disable "$SERVICE_NAME" 2>/dev/null || true
        sudo rm -f "$SERVICE_FILE"
        sudo systemctl daemon-reload
        ok "服务文件已删除"
    fi
    ok "卸载完成（数据库保留）"
}

# ---- 帮助 ----
show_help() {
    cat << EOF

LLM Gateway 部署管理脚本

用法: bash deploy.sh <命令>

命令:
  setup      首次部署（安装 Rust + 编译 + 创建服务 + 启动）
  update     编译 + 重启（日常更新最常用）
  build      仅编译
  start      启动服务
  stop       停止服务
  restart    重启服务
  status     查看服务状态 + 端口监听
  logs [N]   查看最近 N 条日志（默认 50，持续跟踪）
  uninstall  卸载服务（保留数据库）

快速开始:
  git clone <repo-url> && cd LlmGateway
  bash deploy.sh setup

日常更新:
  bash deploy.sh update

EOF
}

# ============================================================
# 入口
# ============================================================

COMMAND="${1:-help}"

case "$COMMAND" in
    setup)     do_setup ;;
    update)    do_update ;;
    build)     do_build ;;
    start)     do_start ;;
    stop)      do_stop ;;
    restart)do_restart ;;
    status)    do_status ;;
    logs)      do_logs "${2:-50}" ;;
    uninstall) do_uninstall ;;
    help|--help|-h) show_help ;;
    *)
        err "未知命令: $COMMAND"
        show_help
        exit 1
        ;;
esac