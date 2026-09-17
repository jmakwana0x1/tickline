#!/usr/bin/env bash
# Self-test for scripts/check-no-stray-files.sh (issue #13).
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
check="$PWD/scripts/check-no-stray-files.sh"
failures=0

expect() { # <label> <want-exit: 0|nonzero> <file-to-add or "">
  local label="$1" want="$2" name="$3" repo status=0
  repo="$(mktemp -d)"
  git -C "$repo" init -q
  mkdir -p "$repo/docs" && echo ok > "$repo/docs/fine-name_1.md"
  [[ -n "$name" ]] && echo x > "$repo/$name"
  git -C "$repo" add -A
  bash "$check" "$repo" >/dev/null 2>&1 || status=$?
  rm -rf "$repo"
  if { [[ "$want" == 0 ]] && [[ "$status" == 0 ]]; } || { [[ "$want" != 0 ]] && [[ "$status" != 0 ]]; }; then
    printf '  \033[32m✓\033[0m %s\n' "$label"
  else
    printf '  \033[31m✗\033[0m %s (exit %s)\n' "$label" "$status"
    failures=$((failures + 1))
  fi
}

echo "test-no-stray-files:"
expect "check-no-stray-files rejects a file named '='" nonzero "="
expect "check-no-stray-files rejects a filename with shell metacharacters" nonzero 'out$(x).log'
expect "check-no-stray-files rejects a filename with a space" nonzero "two words.md"
expect "check-no-stray-files accepts ordinary names" 0 ""

status=0; bash "$check" >/dev/null 2>&1 || status=$?
if [[ "$status" == 0 ]]; then
  printf '  \033[32m✓\033[0m check-no-stray-files passes on the current tree\n'
else
  printf '  \033[31m✗\033[0m check-no-stray-files passes on the current tree\n'
  failures=$((failures + 1))
fi

(( failures == 0 )) || exit 1
