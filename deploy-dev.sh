#!/bin/bash
# LlmGateway 开发环境部署脚本
# 端口: 49128 (与生产 49127 区分)
# 数据库: llm_gateway_dev.db

set -e
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"

export LLM_GW_SERVER_PORT=49128
export LLM_GW_DATABASE_URL="sqlite:llm_gateway_dev.db"

CMD="${1:-update}"

case "$CMD" in
    update)
        echo "[1/3] 编译..."
        cargo build --release 2>&1 | tail -3
        echo "[OK] 编译完成"
        echo "[2/3] 停止旧进程..."
        pkill -f "llm-gateway.*49128" 2>/dev/null || true
        sleep 1
        echo "[3/3] 启动开发服务..."
        nohup ./target/release/llm-gateway > /tmp/llm-gateway-dev.log 2>&1 &
        sleep 1
        if ss -tlnp | grep -q 49128; then
            echo "[OK] 开发服务已启动 (端口 49128)"
        else
            echo "[FAIL] 启动失败，查看日志: /tmp/llm-gateway-dev.log"
            exit 1
        fi
        ;;
    status)
        if ss -tlnp | grep -q 49128; then
            echo "[OK] 开发服务运行中 (端口 49128)"
            ps aux | grep "llm-gateway.*49128" | grep -v grep
        else
            echo "[STOP] 开发服务未运行"
        fi
        ;;
    logs)
        tail -50 /tmp/llm-gateway-dev.log
        ;;
    stop)
        pkill -f "llm-gateway.*49128" 2>/dev/null || true
        echo "[OK] 开发服务已停止"
        ;;
    *)
        echo "用法: bash $0 {update|status|logs|stop}"
        ;;
esac
