#!/usr/bin/env bash
# Self-test for scripts/check-coverage.sh (issue #37).
# Evaluates hand-written llvm-cov JSON reports, so it needs neither cargo-llvm-cov nor a build.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
check="$PWD/scripts/check-coverage.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0

# <file> <covered> <count> for lmsr, plus a noisy file from another crate that must be ignored.
report() {
  cat > "$1" <<JSON
{"data":[{"files":[
  {"filename":"/w/engine/crates/lmsr/src/lib.rs","summary":{"lines":{"count":$3,"covered":$2}}},
  {"filename":"/w/engine/crates/api/src/lib.rs","summary":{"lines":{"count":1000,"covered":0}}}
]}]}
JSON
}

expect() { # <label> <want: pass|fail> <args...>
  local label="$1" want="$2" status=0
  shift 2
  bash "$check" "$@" >/dev/null 2>&1 || status=$?
  if { [[ "$want" == pass ]] && (( status == 0 )); } || { [[ "$want" == fail ]] && (( status != 0 )); }; then
    printf '  \033[32m✓\033[0m %s\n' "$label"
  else
    printf '  \033[31m✗\033[0m %s (exit %s)\n' "$label" "$status"
    failures=$((failures + 1))
  fi
}

echo "test-check-coverage:"
report "$tmp/below.json" 94 100
expect "check-coverage fails below threshold" fail lmsr 95 --report "$tmp/below.json"
report "$tmp/at.json" 95 100
expect "check-coverage passes at or above threshold" pass lmsr 95 --report "$tmp/at.json"
report "$tmp/above.json" 100 100
expect "check-coverage passes at or above threshold (100%)" pass lmsr 95 --report "$tmp/above.json"
expect "check-coverage fails when the report is missing or unparseable (missing)" fail lmsr 95 --report "$tmp/nope.json"
echo '{"data": [' > "$tmp/broken.json"
expect "check-coverage fails when the report is missing or unparseable (unparseable)" fail lmsr 95 --report "$tmp/broken.json"
report "$tmp/other.json" 0 0
expect "check-coverage fails when the crate has no measured lines" fail lmsr 95 --report "$tmp/other.json"
expect "check-coverage ignores other crates' files" pass lmsr 95 --report "$tmp/at.json"
expect "check-coverage rejects a non-integer threshold" fail lmsr 95.5 --report "$tmp/at.json"
expect "check-coverage fails on a target it cannot measure" fail TicklineVault 95 --report "$tmp/at.json"

(( failures == 0 )) || exit 1
