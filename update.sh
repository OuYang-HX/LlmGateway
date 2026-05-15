#!/bin/bash
set -e

PROJECT_DIR="/home/oyhx/code/LlmGateway"
BRANCH="main"

echo "=== LLM Gateway Update Script (Local) ==="
echo ""

cd "$PROJECT_DIR"

# 1. Pull latest code
echo "[1/3] Pulling latest code from origin/$BRANCH..."
git fetch origin "$BRANCH"
git reset --hard "origin/$BRANCH"

# 2. Rebuild
echo "[2/3] Building release binary..."
cargo build --release 2>&1

# 3. Restart service
echo "[3/3] Restarting llm-gateway-local service..."
sudo systemctl restart llm-gateway-local

sleep 1
if sudo systemctl is-active --quiet llm-gateway-local; then
    echo ""
    echo "✅ Update complete! Service is running."
    sudo systemctl status llm-gateway-local --no-pager | head -10
else
    echo ""
    echo "❌ Service failed to start! Check logs:"
    sudo journalctl -u llm-gateway-local -n 20 --no-pager
    exit 1
fi
