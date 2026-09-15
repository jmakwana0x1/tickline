#!/usr/bin/env bash
# Fails if any test is ignored, skipped, or focused.
# CLAUDE.md section 2: weakening a test is a needs-jay conversation, not a commit.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1

declare -a FOUND=()

scan() { # <label> <pattern> <path...>
  local label="$1" pattern="$2"; shift 2
  local hits
  hits="$(grep -rnE "$pattern" "$@" \
    --include='*.rs' --include='*.sol' --include='*.ts' --include='*.tsx' \
    --exclude-dir=target --exclude-dir=node_modules --exclude-dir=lib --exclude-dir=out \
    2>/dev/null | grep -v 'check-no-skipped-tests' || true)"
  [[ -n "$hits" ]] && FOUND+=("$label"$'\n'"$hits")
  return 0
}

scan "rust: #[ignore]"        '^\s*#\[ignore'                       engine
scan "rust: todo/unimplemented in tests" '^\s*(todo!|unimplemented!)\(\)' engine
scan "solidity: vm.skip"      'vm\.skip\s*\(\s*true'                contracts
scan "vitest: skip/only"      '\b(it|test|describe)\.(skip|only|todo)\b' agents e2e dashboard

if (( ${#FOUND[@]} )); then
  echo "✗ disabled or focused tests found:" >&2
  printf '%s\n\n' "${FOUND[@]}" >&2
  echo "Re-enable them, or open a needs-jay issue. Do not merge around this." >&2
  exit 1
fi
echo "✓ no ignored, skipped, or focused tests"
