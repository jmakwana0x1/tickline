#!/usr/bin/env bash
# Fails on U+2014 (em-dash) in tracked text files, in commit messages on a range, and in the
# PR title and body (issue #18). Squash merges use the PR title and body as the commit message on
# main, so those are checked too.
#
# Usage: check-no-em-dash.sh [--repo DIR] [--range BASE..HEAD]
# Env:   EM_DASH_RANGE  commit range, if --range is not given (CI sets it on pull requests)
#        PR_TITLE, PR_BODY  checked when set
# Without a range, commits on origin/main..HEAD are checked if origin/main exists.
set -euo pipefail

repo="$(dirname "$0")/.."
range="${EM_DASH_RANGE:-}"
while (( $# )); do
  case "$1" in
    --repo)  repo="$2"; shift 2 ;;
    --range) range="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

DASH=$'—'
status=0
report() { echo "✗ $*" >&2; status=1; }

if hits="$(git -C "$repo" grep -I -n -F "$DASH" 2>/dev/null)"; then
  report "U+2014 (em-dash) in tracked files:"
  printf '%s\n' "$hits" | sed 's/^/    /' >&2
fi

if [[ -z "$range" ]] && git -C "$repo" rev-parse --verify -q origin/main >/dev/null; then
  range="origin/main..HEAD"
fi
if [[ -n "$range" ]]; then
  while IFS= read -r sha; do
    [[ -z "$sha" ]] && continue
    if git -C "$repo" log -1 --format=%B "$sha" | grep -q -F "$DASH"; then
      report "U+2014 in the message of commit $(git -C "$repo" log -1 --format='%h %s' "$sha")"
    fi
  done <<< "$(git -C "$repo" rev-list "$range")"
fi

if [[ "${PR_TITLE:-}" == *"$DASH"* ]]; then report "U+2014 in the PR title"; fi
if [[ "${PR_BODY:-}" == *"$DASH"* ]];  then report "U+2014 in the PR body (it becomes the squash commit message)"; fi

if (( status )); then
  echo "  Use a colon, comma, parentheses, or a new sentence instead." >&2
  exit 1
fi
echo "✓ no em-dashes${range:+ (commits: $range)}"
