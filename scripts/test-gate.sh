#!/usr/bin/env bash
# Self-test for scripts/gate.sh (issue #17).
#
# The gate is the only definition of done, so it must never report green when a step fails.
# This runs the real gate.sh in a sandbox where every tool it calls is a stub, first cleanly,
# then once per tool invocation with a failure planted at that invocation. Each planted run must
# exit non-zero AND must stop at the failing command: if anything runs after it, the gate
# swallowed the failure (for example `cd engine && cargo test && cd ..` under `set -e`).
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1

STUBBED_TOOLS=(just cargo forge pnpm git)

sandbox="$(mktemp -d)"
trap 'rm -rf "$sandbox"' EXIT
repo="$sandbox/repo"
mkdir -p "$repo/scripts" "$repo/engine" "$repo/contracts" "$sandbox/bin"
cp scripts/gate.sh "$repo/scripts/gate.sh"
# .phase = 9 so that `gate.sh 9` exercises every gate function, 0 through 9.
echo 9 > "$repo/.phase"

export STUB_LOG="$sandbox/calls.log"
stub() { # <path>
  cat > "$1" <<'STUB'
#!/bin/sh
n=$(( $(wc -l < "$STUB_LOG") + 1 ))
echo "$0 $*" >> "$STUB_LOG"
[ "$n" = "${FAIL_AT:-0}" ] && exit 1
exit 0
STUB
  chmod +x "$1"
}
for tool in "${STUBBED_TOOLS[@]}"; do stub "$sandbox/bin/$tool"; done
# Scripts the gate runs with `bash scripts/<name>.sh` are stubbed in place.
for s in $(grep -o 'scripts/[A-Za-z0-9_-]*\.sh' scripts/gate.sh | sort -u); do
  [[ "$s" == "scripts/gate.sh" ]] || stub "$repo/$s"
done

run_gate() { # <fail-at>; prints the exit status
  : > "$STUB_LOG"
  local status=0
  FAIL_AT="$1" PATH="$sandbox/bin:$PATH" bash "$repo/scripts/gate.sh" 9 >"$sandbox/out.log" 2>&1 || status=$?
  echo "$status"
}

failures=0
fail() { printf '  \033[31m✗\033[0m %s\n' "$*"; failures=$((failures + 1)); }

echo "test-gate: a clean run of every gate function exits 0"
status="$(run_gate 0)"
total="$(wc -l < "$STUB_LOG")"
if [[ "$status" != 0 ]]; then
  fail "clean run exited $status; a tool gate.sh calls may be missing from STUBBED_TOOLS"
  sed 's/^/      /' "$sandbox/out.log" | tail -15
elif (( total == 0 )); then
  fail "clean run invoked no tools, so the planted-failure test below would prove nothing"
else
  printf '  \033[32m✓\033[0m exit 0 after %s tool invocations\n' "$total"
fi

echo "test-gate: a failure planted at each external command exits non-zero"
for (( k = 1; k <= total; k++ )); do
  status="$(run_gate "$k")"
  ran="$(wc -l < "$STUB_LOG")"
  failed_cmd="$(sed -n "${k}p" "$STUB_LOG" | sed "s|$sandbox/||")"
  if [[ "$status" == 0 ]]; then
    fail "planted failure at #$k ($failed_cmd): gate exited 0"
  elif (( ran != k )); then
    fail "planted failure at #$k ($failed_cmd): gate kept going for $((ran - k)) more command(s)"
  fi
done
(( failures == 0 )) && printf '  \033[32m✓\033[0m all %s planted failures stopped the gate\n' "$total"

if (( failures )); then
  echo "✗ gate.sh can report green, or keep running, after a failed step ($failures case(s))" >&2
  exit 1
fi
echo "✓ gate.sh stops at every failing step"
