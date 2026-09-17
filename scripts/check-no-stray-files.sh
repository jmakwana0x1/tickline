#!/usr/bin/env bash
# Fails if a tracked path has a segment outside [A-Za-z0-9._-] (issue #13).
# Redirect accidents (`=`, `-`, `?`, `*`, quotes, spaces) produce exactly such names, and
# `git add -A` will commit them through an otherwise green gate. Contents are gitleaks' job.
# Usage: check-no-stray-files.sh [repo-dir]
set -euo pipefail
dir="${1:-$(dirname "$0")/..}"

bad="$(git -C "$dir" ls-files -z \
  | tr '\0' '\n' \
  | awk -F/ '{ for (i = 1; i <= NF; i++) if ($i !~ /^[A-Za-z0-9._-]+$/) { print; next } }')"

if [[ -n "$bad" ]]; then
  echo "✗ tracked paths with unexpected characters (likely a shell accident):" >&2
  while IFS= read -r path; do printf '    %q\n' "$path" >&2; done <<< "$bad"
  echo "  Remove them, or rename to [A-Za-z0-9._-] if they are intended." >&2
  exit 1
fi
echo "✓ no stray file names"
