#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if ! command -v shellcheck >/dev/null; then
  echo "… shellcheck not installed, skipping (just doctor tells you how)" >&2
  exit 0
fi
shellcheck -S warning scripts/*.sh
echo "✓ shellcheck clean"
