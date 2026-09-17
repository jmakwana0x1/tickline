#!/usr/bin/env bash
# check-vectors-committed.sh <vector-file>
# A regenerated vector file must be tracked and unchanged (CLAUDE.md section 3). `git diff`
# alone says nothing about an untracked file, so a generator writing a new, uncommitted file
# would otherwise pass as "no drift".
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
file="${1:?usage: check-vectors-committed.sh <vector-file>}"

if ! git ls-files --error-unmatch -- "$file" >/dev/null 2>&1; then
  echo "✗ $file is not committed. Commit the generated file, then rerun." >&2
  exit 1
fi
if ! git diff --quiet -- "$file"; then
  echo "✗ $file drifted from the committed version; commit it or fix the generator." >&2
  git diff --stat -- "$file" >&2
  exit 1
fi
echo "✓ $file matches the committed version"
