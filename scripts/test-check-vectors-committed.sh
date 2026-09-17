#!/usr/bin/env bash
# Self-test for scripts/check-vectors-committed.sh (issue #37).
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
repo="$(mktemp -d)"
trap 'rm -rf "$repo"' EXIT
mkdir -p "$repo/scripts" "$repo/testdata/vectors"
cp scripts/check-vectors-committed.sh "$repo/scripts/"
git -C "$repo" init -q
echo '{"cases": 1}' > "$repo/testdata/vectors/committed.json"
git -C "$repo" add -A
git -C "$repo" -c user.name=t -c user.email=t@t commit -q -m base
failures=0

expect() { # <label> <want: pass|fail> <file>
  local status=0
  bash "$repo/scripts/check-vectors-committed.sh" "$3" >/dev/null 2>&1 || status=$?
  if { [[ "$2" == pass ]] && (( status == 0 )); } || { [[ "$2" == fail ]] && (( status != 0 )); }; then
    printf '  \033[32m✓\033[0m %s\n' "$1"
  else
    printf '  \033[31m✗\033[0m %s (exit %s)\n' "$1" "$status"
    failures=$((failures + 1))
  fi
}

echo "test-check-vectors-committed:"
expect "check-vectors-committed passes on an unchanged committed file" pass testdata/vectors/committed.json
echo '{"cases": 1}' > "$repo/testdata/vectors/new.json"
expect "check-vectors-committed fails on a generated file that was never committed" fail testdata/vectors/new.json
expect "check-vectors-committed fails when the file does not exist" fail testdata/vectors/absent.json
echo '{"cases": 2}' > "$repo/testdata/vectors/committed.json"
expect "check-vectors-committed fails when the committed file drifted" fail testdata/vectors/committed.json

(( failures == 0 )) || exit 1
