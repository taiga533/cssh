#!/bin/bash
# cssh E2E test script
#
# Spins up a Docker SSH environment and verifies cssh end-to-end behavior.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
SSH_PORT=2222
SSH_USER=testuser
SSH_HOST=localhost
PASSED=0
FAILED=0
TOKEN="e2e-test-token-$(date +%s)"

# Color output
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
    echo "--- Cleanup ---"
    docker compose -f "$SCRIPT_DIR/compose.yml" down -v 2>/dev/null || true
    # Stop agent process if running
    kill "$AGENT_PID" 2>/dev/null || true
}
trap cleanup EXIT

echo "=== cssh E2E Tests ==="

# Build
echo "--- Build ---"
cargo build --workspace --manifest-path "$PROJECT_DIR/Cargo.toml"

BINARY_DIR="$PROJECT_DIR/target/debug"
CSSH_AGENT="$BINARY_DIR/cssh-agent"
CEXEC="$BINARY_DIR/cexec"

# Start Docker environment
echo "--- Starting Docker SSH environment ---"
docker compose -f "$SCRIPT_DIR/compose.yml" up -d --build --wait

# Generate and deploy SSH keys
echo "--- SSH key setup ---"
SSH_KEY="$SCRIPT_DIR/.test_key"
rm -f "$SSH_KEY" "$SSH_KEY.pub"
ssh-keygen -t ed25519 -f "$SSH_KEY" -N "" -q

docker compose -f "$SCRIPT_DIR/compose.yml" exec -T sshd bash -c \
    "cat > /home/testuser/.ssh/authorized_keys && chown testuser:testuser /home/testuser/.ssh/authorized_keys && chmod 600 /home/testuser/.ssh/authorized_keys" \
    < "$SSH_KEY.pub"

SSH_COMMON_OPTS="-i $SSH_KEY -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o LogLevel=ERROR"
SSH_OPTS="-p $SSH_PORT $SSH_COMMON_OPTS"
SCP_OPTS="-P $SSH_PORT $SSH_COMMON_OPTS"

# Verify SSH connectivity
echo "--- Verifying SSH connection ---"
if ssh $SSH_OPTS "$SSH_USER@$SSH_HOST" echo "ssh ok" 2>/dev/null | grep -q "ssh ok"; then
    log_pass "SSH connection"
else
    log_fail "SSH connection" "cannot connect"
    exit 1
fi

# Deploy cexec binary to remote (pre-requisite for all tests)
echo "--- Deploying cexec to remote ---"
ssh $SSH_OPTS "$SSH_USER@$SSH_HOST" "mkdir -p ~/.cssh" 2>/dev/null
scp $SCP_OPTS "$CEXEC" "$SSH_USER@$SSH_HOST:~/.cssh/cexec" 2>/dev/null
ssh $SSH_OPTS "$SSH_USER@$SSH_HOST" "chmod +x ~/.cssh/cexec" 2>/dev/null

# Test 1: Agent standalone startup
echo ""
echo "--- Test 1: Agent standalone startup ---"
AGENT_PID=""
$CSSH_AGENT 0 > /tmp/cssh_agent_port.txt 2>/dev/null &
AGENT_PID=$!
sleep 0.5

AGENT_PORT=$(head -1 /tmp/cssh_agent_port.txt)
if [ -n "$AGENT_PORT" ] && [ "$AGENT_PORT" -gt 0 ] 2>/dev/null; then
    log_pass "Agent startup (port: $AGENT_PORT)"
else
    log_fail "Agent startup" "failed to get port number"
fi

kill "$AGENT_PID" 2>/dev/null || true
wait "$AGENT_PID" 2>/dev/null || true
AGENT_PID=""

# Helper: write agent_port file on remote and run cexec via Unix domain socket forwarding.
# The remote (Linux container) uses UDS format: socket|<path>|<token>
# The -R flag forwards the remote UDS path to the local agent TCP port.
# Usage: run_cexec_remote <agent_port> <token> <command...>
run_cexec_remote() {
    local port="$1"
    local token="$2"
    shift 2
    local sock_path="/tmp/cssh-agent-${port}.sock"
    local connection_info="socket|${sock_path}|${token}"
    ssh $SSH_OPTS \
        -R "${sock_path}:127.0.0.1:${port}" \
        "$SSH_USER@$SSH_HOST" \
        "mkdir -p ~/.cssh && (umask 077 && printf '%s\n' '${connection_info}' > ~/.cssh/agent_port) && ~/.cssh/cexec $*"
}

# Test 2: Command execution via SSH reverse port forwarding (UDS)
echo ""
echo "--- Test 2: Command execution via reverse port forwarding (UDS) ---"
$CSSH_AGENT 0 "$TOKEN" > /tmp/cssh_agent_port2.txt 2>/dev/null &
AGENT_PID=$!
sleep 0.5

AGENT_PORT=$(head -1 /tmp/cssh_agent_port2.txt)

if [ -n "$AGENT_PORT" ] && [ "$AGENT_PORT" -gt 0 ] 2>/dev/null; then
    RESULT=$(run_cexec_remote "$AGENT_PORT" "$TOKEN" echo e2e-success 2>/dev/null)

    if [ "$RESULT" = "e2e-success" ]; then
        log_pass "Command execution via reverse port forwarding (UDS)"
    else
        log_fail "Command execution via reverse port forwarding (UDS)" "output: '$RESULT'"
    fi
else
    log_fail "Agent startup (test 2)" "failed to get port"
fi

kill "$AGENT_PID" 2>/dev/null || true
wait "$AGENT_PID" 2>/dev/null || true
AGENT_PID=""

# Test 3: Exit code propagation
echo ""
echo "--- Test 3: Exit code propagation ---"
$CSSH_AGENT 0 "$TOKEN" > /tmp/cssh_agent_port3.txt 2>/dev/null &
AGENT_PID=$!
sleep 0.5

AGENT_PORT=$(head -1 /tmp/cssh_agent_port3.txt)

if [ -n "$AGENT_PORT" ] && [ "$AGENT_PORT" -gt 0 ] 2>/dev/null; then
    EXIT_CODE=0
    run_cexec_remote "$AGENT_PORT" "$TOKEN" false 2>/dev/null || EXIT_CODE=$?

    if [ "$EXIT_CODE" -ne 0 ]; then
        log_pass "Exit code propagation (exit_code=$EXIT_CODE)"
    else
        log_fail "Exit code propagation" "expected: non-zero, actual: $EXIT_CODE"
    fi
else
    log_fail "Agent startup (test 3)" "failed to get port"
fi

kill "$AGENT_PID" 2>/dev/null || true
wait "$AGENT_PID" 2>/dev/null || true
AGENT_PID=""

# Test 4: stderr propagation
echo ""
echo "--- Test 4: stderr propagation ---"
$CSSH_AGENT 0 "$TOKEN" > /tmp/cssh_agent_port4.txt 2>/dev/null &
AGENT_PID=$!
sleep 0.5

AGENT_PORT=$(head -1 /tmp/cssh_agent_port4.txt)

if [ -n "$AGENT_PORT" ] && [ "$AGENT_PORT" -gt 0 ] 2>/dev/null; then
    STDERR_RESULT=$(run_cexec_remote "$AGENT_PORT" "$TOKEN" "bash -c 'echo err_test >&2'" 2>&1 >/dev/null)

    if echo "$STDERR_RESULT" | grep -q "err_test"; then
        log_pass "stderr propagation"
    else
        log_fail "stderr propagation" "stderr does not contain 'err_test': '$STDERR_RESULT'"
    fi
else
    log_fail "Agent startup (test 4)" "failed to get port"
fi

kill "$AGENT_PID" 2>/dev/null || true
wait "$AGENT_PID" 2>/dev/null || true
AGENT_PID=""

# Test 5: Authentication failure with wrong token
echo ""
echo "--- Test 5: Authentication failure with wrong token ---"
$CSSH_AGENT 0 "$TOKEN" > /tmp/cssh_agent_port5.txt 2>/dev/null &
AGENT_PID=$!
sleep 0.5

AGENT_PORT=$(head -1 /tmp/cssh_agent_port5.txt)

if [ -n "$AGENT_PORT" ] && [ "$AGENT_PORT" -gt 0 ] 2>/dev/null; then
    EXIT_CODE=0
    run_cexec_remote "$AGENT_PORT" "wrong-token" echo hello 2>/dev/null || EXIT_CODE=$?

    if [ "$EXIT_CODE" -ne 0 ]; then
        log_pass "Authentication failure with wrong token (exit_code=$EXIT_CODE)"
    else
        log_fail "Authentication failure with wrong token" "expected: non-zero, actual: $EXIT_CODE"
    fi
else
    log_fail "Agent startup (test 5)" "failed to get port"
fi

kill "$AGENT_PID" 2>/dev/null || true
wait "$AGENT_PID" 2>/dev/null || true
AGENT_PID=""

# Results summary
echo ""
echo "=== Test Results ==="
echo -e "Passed: ${GREEN}${PASSED}${NC}  Failed: ${RED}${FAILED}${NC}"

if [ "$FAILED" -gt 0 ]; then
    exit 1
fi
