#!/usr/bin/env bash
# Self-test for scripts/fork-suite.sh (#67, Jay's condition 2 on #80).
#
# The script exists to keep three outcomes apart, so the three have to be proven apart. The cases
# that need the network are skipped by the caller, never by this script: what is checked here is the
# classification, which is what would otherwise turn an outage into a false accusation.
#
# Cleanup deletes only the directories this script created with mktemp, recorded at creation and
# checked to sit under the temp directory.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
suite="$PWD/scripts/fork-suite.sh"
tmp_base="$(cd "${TMPDIR:-/tmp}" && pwd -P)"
created=()
cleanup() {
  local dir
  for dir in "${created[@]}"; do
    case "$dir" in
      "$tmp_base"/tmp.*) rm -rf -- "$dir" ;;
      *) echo "refusing to delete unexpected path: $dir" >&2 ;;
    esac
  done
}
trap cleanup EXIT
failures=0

expect() { # <label> <want-exit> <env-assignment...>
  local label="$1" want="$2"
  shift 2
  local status=0
  env "$@" bash "$suite" >/dev/null 2>&1 || status=$?
  if (( status == want )); then
    printf '  \033[32m✓\033[0m %s (exit %s)\n' "$label" "$status"
  else
    printf '  \033[31m✗\033[0m %s (exit %s, wanted %s)\n' "$label" "$status" "$want"
    failures=$((failures + 1))
  fi
}

echo "test-fork-suite:"

# 4. An absent secret fails, never skips.
expect "an absent secret is a misconfiguration, not a skip" 2 BASE_SEPOLIA_RPC_URL=

# 2. An outage is not a finding. A port nothing listens on is the cheapest unreachable endpoint.
expect "an unreachable endpoint is 75, not a disagreement" 75 \
  BASE_SEPOLIA_RPC_URL=http://127.0.0.1:1/

# An endpoint that answers, but not as Base Sepolia, is a misconfiguration rather than a finding: it
# would otherwise report every type hash as changed.
dir="$(cd "$(mktemp -d)" && pwd -P)"
created+=("$dir")
cat > "$dir/wrong-chain.py" <<'PY'
import http.server, json

class Handler(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        self.rfile.read(int(self.headers.get("content-length", 0)))
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "result": "0x1"}).encode()
        self.send_response(200)
        self.send_header("content-type", "application/json")
        self.send_header("content-length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, *_):
        pass

http.server.HTTPServer(("127.0.0.1", 8599), Handler).serve_forever()
PY
python3 "$dir/wrong-chain.py" &
server=$!
sleep 1
expect "an endpoint on the wrong chain is a misconfiguration" 2 BASE_SEPOLIA_RPC_URL=http://127.0.0.1:8599/
kill "$server" 2>/dev/null || true

(( failures == 0 )) || exit 1
