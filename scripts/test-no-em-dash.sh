#!/usr/bin/env bash
# Self-test for scripts/check-no-em-dash.sh (issue #18).
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
check="$PWD/scripts/check-no-em-dash.sh"
# U+2014 built from its UTF-8 bytes, so this file never contains the character it bans.
DASH="$(printf '\342\200\224')"
failures=0

scratch() { # creates a repo with one clean commit, prints its path
  local r
  r="$(mktemp -d)"
  git -C "$r" init -q
  git -C "$r" -c user.name=t -c user.email=t@t commit -q --allow-empty -m "base"
  echo "clean text" > "$r/a.md"
  git -C "$r" add a.md
  git -C "$r" -c user.name=t -c user.email=t@t commit -q -m "clean commit"
  echo "$r"
}

expect() { # <label> <want: pass|fail> <command...>
  local label="$1" want="$2" status=0
  shift 2
  "$@" >/dev/null 2>&1 || status=$?
  if { [[ "$want" == pass ]] && (( status == 0 )); } || { [[ "$want" == fail ]] && (( status != 0 )); }; then
    printf '  \033[32m✓\033[0m %s\n' "$label"
  else
    printf '  \033[31m✗\033[0m %s (exit %s)\n' "$label" "$status"
    failures=$((failures + 1))
  fi
}

echo "test-no-em-dash:"

r="$(scratch)"
expect "check-no-em-dash passes on a clean repo and range" pass \
  env -u PR_TITLE -u PR_BODY bash "$check" --repo "$r" --range HEAD~1..HEAD

echo "one ${DASH} two" > "$r/b.md"; git -C "$r" add b.md
expect "check-no-em-dash fails on a tracked file containing U+2014" fail \
  env -u PR_TITLE -u PR_BODY bash "$check" --repo "$r" --range HEAD~1..HEAD
rm -rf "$r"

r="$(scratch)"
git -C "$r" -c user.name=t -c user.email=t@t commit -q --allow-empty -m "fix ${DASH} something"
expect "check-no-em-dash fails on a commit message in the PR range containing U+2014" fail \
  env -u PR_TITLE -u PR_BODY bash "$check" --repo "$r" --range HEAD~1..HEAD
expect "check-no-em-dash ignores commits outside the range" pass \
  env -u PR_TITLE -u PR_BODY bash "$check" --repo "$r" --range HEAD~2..HEAD~1
expect "check-no-em-dash fails on a PR title containing U+2014" fail \
  env PR_TITLE="a ${DASH} b" -u PR_BODY bash "$check" --repo "$r" --range HEAD~2..HEAD~1
expect "check-no-em-dash fails on a PR body containing U+2014" fail \
  env -u PR_TITLE PR_BODY="a ${DASH} b" bash "$check" --repo "$r" --range HEAD~2..HEAD~1
rm -rf "$r"

expect "check-no-em-dash passes on the current tree" pass \
  env -u PR_TITLE -u PR_BODY bash "$check" --range HEAD..HEAD

(( failures == 0 )) || exit 1
