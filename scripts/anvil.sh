#!/usr/bin/env bash
# Start/stop a background anvil with a deterministic chain (CLAUDE.md section 2: determinism).
set -euo pipefail
cd "$(dirname "$0")/.."

PID_FILE=".anvil.pid"
LOG_FILE=".anvil.log"
PORT="${ANVIL_PORT:-8545}"

case "${1:-start}" in
  start)
    if [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null; then
      echo "anvil already running (pid $(cat "$PID_FILE"))"; exit 0
    fi
    anvil --port "$PORT" --chain-id 31337 --block-time 1 \
          --mnemonic "test test test test test test test test test test test junk" \
          > "$LOG_FILE" 2>&1 &
    echo $! > "$PID_FILE"
    for _ in {1..50}; do
      cast block-number --rpc-url "http://localhost:$PORT" >/dev/null 2>&1 && break
      sleep 0.2
    done
    echo "anvil up on :$PORT (pid $(cat "$PID_FILE"))"
    ;;
  stop)
    [[ -f "$PID_FILE" ]] || { echo "anvil not running"; exit 0; }
    kill "$(cat "$PID_FILE")" 2>/dev/null || true
    rm -f "$PID_FILE"
    echo "anvil stopped"
    ;;
  *) echo "usage: anvil.sh start|stop" >&2; exit 2 ;;
esac
