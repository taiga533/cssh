#!/bin/bash
# cssh E2Eテストスクリプト
#
# Docker SSH環境を立ち上げ、csshの一連の動作を検証する。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SSH_PORT=2222
SSH_USER=testuser
SSH_HOST=localhost
SSH_PASS=testpass
PASSED=0
FAILED=0

# カラー出力
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    PASSED=$((PASSED + 1))
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1: $2"
    FAILED=$((FAILED + 1))
}

cleanup() {
    echo "--- クリーンアップ ---"
    docker compose -f "$SCRIPT_DIR/docker-compose.yml" down -v 2>/dev/null || true
    # エージェントプロセスがあれば停止
    kill "$AGENT_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo "=== cssh E2Eテスト ==="

# ビルド
echo "--- ビルド ---"
cargo build --workspace --manifest-path "$PROJECT_DIR/Cargo.toml"

BINARY_DIR="$PROJECT_DIR/target/debug"
CSSH_AGENT="$BINARY_DIR/cssh-agent"
CEXEC="$BINARY_DIR/cexec"
CSSH="$BINARY_DIR/cssh"

# Docker環境起動
echo "--- Docker SSH環境起動 ---"
docker compose -f "$SCRIPT_DIR/docker-compose.yml" up -d --build --wait

# SSH鍵生成・配置
echo "--- SSH鍵設定 ---"
SSH_KEY="$SCRIPT_DIR/.test_key"
rm -f "$SSH_KEY" "$SSH_KEY.pub"
ssh-keygen -t ed25519 -f "$SSH_KEY" -N "" -q

# sshpassで公開鍵をリモートに配置
docker compose -f "$SCRIPT_DIR/docker-compose.yml" exec -T sshd bash -c \
    "cat > /home/testuser/.ssh/authorized_keys && chown testuser:testuser /home/testuser/.ssh/authorized_keys && chmod 600 /home/testuser/.ssh/authorized_keys" \
    < "$SSH_KEY.pub"

SSH_COMMON_OPTS="-i $SSH_KEY -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"
SSH_OPTS="-p $SSH_PORT $SSH_COMMON_OPTS"
SCP_OPTS="-P $SSH_PORT $SSH_COMMON_OPTS"

# SSH接続確認
echo "--- SSH接続確認 ---"
if ssh $SSH_OPTS "$SSH_USER@$SSH_HOST" echo "ssh ok" 2>/dev/null | grep -q "ssh ok"; then
    log_pass "SSH接続"
else
    log_fail "SSH接続" "接続できません"
    exit 1
fi

# テスト1: エージェント単体起動・通信テスト
echo ""
echo "--- テスト1: エージェント単体起動 ---"
AGENT_PID=""
$CSSH_AGENT 0 > /tmp/cssh_agent_port.txt 2>/dev/null &
AGENT_PID=$!
sleep 0.5

AGENT_PORT=$(head -1 /tmp/cssh_agent_port.txt)
if [ -n "$AGENT_PORT" ] && [ "$AGENT_PORT" -gt 0 ] 2>/dev/null; then
    log_pass "エージェント起動（ポート: $AGENT_PORT）"
else
    log_fail "エージェント起動" "ポート番号が取得できません"
fi

kill "$AGENT_PID" 2>/dev/null || true
wait "$AGENT_PID" 2>/dev/null || true
AGENT_PID=""

# テスト2: リモートバイナリ配置
echo ""
echo "--- テスト2: リモートバイナリ配置 ---"
# cexecを手動でSCP
scp $SCP_OPTS "$CEXEC" "$SSH_USER@$SSH_HOST:~/.cssh/cexec" 2>/dev/null || {
    # ディレクトリ作成してから再試行
    ssh $SSH_OPTS "$SSH_USER@$SSH_HOST" "mkdir -p ~/.cssh" 2>/dev/null
    scp $SCP_OPTS "$CEXEC" "$SSH_USER@$SSH_HOST:~/.cssh/cexec" 2>/dev/null
}
ssh $SSH_OPTS "$SSH_USER@$SSH_HOST" "chmod +x ~/.cssh/cexec" 2>/dev/null

REMOTE_CHECK=$(ssh $SSH_OPTS "$SSH_USER@$SSH_HOST" "test -x ~/.cssh/cexec && echo exists" 2>/dev/null)
if [ "$REMOTE_CHECK" = "exists" ]; then
    log_pass "リモートバイナリ配置"
else
    log_fail "リモートバイナリ配置" "バイナリが見つかりません"
fi

# テスト3: SSH経由でエージェント↔cexec通信（逆ポートフォワーディング）
echo ""
echo "--- テスト3: SSH逆ポートフォワーディング経由のコマンド実行 ---"
$CSSH_AGENT 0 > /tmp/cssh_agent_port2.txt 2>/dev/null &
AGENT_PID=$!
sleep 0.5

AGENT_PORT=$(head -1 /tmp/cssh_agent_port2.txt)

if [ -n "$AGENT_PORT" ] && [ "$AGENT_PORT" -gt 0 ] 2>/dev/null; then
    # SSH逆ポートフォワーディングでリモートからcexec実行
    RESULT=$(ssh $SSH_OPTS \
        -R "$AGENT_PORT:127.0.0.1:$AGENT_PORT" \
        "$SSH_USER@$SSH_HOST" \
        "CSSH_PORT=$AGENT_PORT ~/.cssh/cexec echo e2e-success" 2>/dev/null)

    if [ "$RESULT" = "e2e-success" ]; then
        log_pass "逆ポートフォワーディング経由のコマンド実行"
    else
        log_fail "逆ポートフォワーディング経由のコマンド実行" "出力: '$RESULT'"
    fi
else
    log_fail "エージェント起動（テスト3）" "ポート取得失敗"
fi

kill "$AGENT_PID" 2>/dev/null || true
wait "$AGENT_PID" 2>/dev/null || true
AGENT_PID=""

# テスト4: 終了コードの伝搬
echo ""
echo "--- テスト4: 終了コード伝搬 ---"
$CSSH_AGENT 0 > /tmp/cssh_agent_port3.txt 2>/dev/null &
AGENT_PID=$!
sleep 0.5

AGENT_PORT=$(head -1 /tmp/cssh_agent_port3.txt)

if [ -n "$AGENT_PORT" ] && [ "$AGENT_PORT" -gt 0 ] 2>/dev/null; then
    EXIT_CODE=0
    ssh $SSH_OPTS \
        -R "$AGENT_PORT:127.0.0.1:$AGENT_PORT" \
        "$SSH_USER@$SSH_HOST" \
        "CSSH_PORT=$AGENT_PORT ~/.cssh/cexec false" 2>/dev/null || EXIT_CODE=$?

    if [ "$EXIT_CODE" -ne 0 ]; then
        log_pass "終了コード伝搬（exit_code=$EXIT_CODE）"
    else
        log_fail "終了コード伝搬" "期待: 非0、実際: $EXIT_CODE"
    fi
else
    log_fail "エージェント起動（テスト4）" "ポート取得失敗"
fi

kill "$AGENT_PID" 2>/dev/null || true
wait "$AGENT_PID" 2>/dev/null || true
AGENT_PID=""

# テスト5: stderr伝搬
echo ""
echo "--- テスト5: stderr伝搬 ---"
$CSSH_AGENT 0 > /tmp/cssh_agent_port4.txt 2>/dev/null &
AGENT_PID=$!
sleep 0.5

AGENT_PORT=$(head -1 /tmp/cssh_agent_port4.txt)

if [ -n "$AGENT_PORT" ] && [ "$AGENT_PORT" -gt 0 ] 2>/dev/null; then
    STDERR_RESULT=$(ssh $SSH_OPTS \
        -R "$AGENT_PORT:127.0.0.1:$AGENT_PORT" \
        "$SSH_USER@$SSH_HOST" \
        "CSSH_PORT=$AGENT_PORT ~/.cssh/cexec bash -c 'echo err_test >&2'" 2>&1 >/dev/null)

    if echo "$STDERR_RESULT" | grep -q "err_test"; then
        log_pass "stderr伝搬"
    else
        log_fail "stderr伝搬" "stderrに'err_test'が含まれません: '$STDERR_RESULT'"
    fi
else
    log_fail "エージェント起動（テスト5）" "ポート取得失敗"
fi

kill "$AGENT_PID" 2>/dev/null || true
wait "$AGENT_PID" 2>/dev/null || true
AGENT_PID=""

# 結果サマリー
echo ""
echo "=== テスト結果 ==="
echo -e "合格: ${GREEN}${PASSED}${NC}  失敗: ${RED}${FAILED}${NC}"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
