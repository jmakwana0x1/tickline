#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
if ! command -v shellcheck >/dev/null; then
  # Never skip silently: CI runners ship shellcheck, so a local skip means `just lint`
  # passes here and fails there, the exact split CLAUDE.md section 6 forbids.
  echo "✗ shellcheck is not installed, and CI runs it. Install it (see 'just doctor')." >&2
  exit 1
fi
shellcheck -S warning scripts/*.sh
echo "✓ shellcheck clean"
