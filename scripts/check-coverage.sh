#!/usr/bin/env bash
# check-coverage.sh <crate> <min-percent> [--report <llvm-cov.json>]
#
# Fails unless the crate's own line coverage is at least <min-percent>. Fails closed: a missing
# or unparseable report, zero measured lines, or a target it does not know how to measure are all
# failures, never a pass (issue #37).
#
# Without --report, it measures with `cargo llvm-cov` (pinned in scripts/tool-versions.sh) by
# running the crate's tests. With --report, it only evaluates an existing JSON report, which is
# how scripts/test-check-coverage.sh exercises it.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1

target="${1:?usage: check-coverage.sh <crate> <min-percent> [--report file]}"
min="${2:?usage: check-coverage.sh <crate> <min-percent> [--report file]}"
shift 2
report=""
while (( $# )); do
  case "$1" in
    --report) report="${2:?--report needs a file}"; shift 2 ;;
    *) echo "✗ unknown argument: $1" >&2; exit 2 ;;
  esac
done

fail() { echo "✗ coverage for '$target': $*" >&2; exit 1; }

[[ "$min" =~ ^[0-9]+$ ]] && (( min <= 100 )) || fail "threshold must be an integer percentage, got '$min'"

# Only workspace crates can be measured today. Solidity coverage (TicklineVault, gate 3) lands
# with Phase 3; until then asking for it is a failure, not a silent pass.
if [[ ! -f "engine/crates/$target/Cargo.toml" ]]; then
  fail "no coverage source for this target (not an engine crate)"
fi

if [[ -z "$report" ]]; then
  command -v cargo-llvm-cov >/dev/null || fail "cargo-llvm-cov is not installed (see 'just doctor')"
  report="$(mktemp)"
  trap 'rm -f "$report"' EXIT
  cargo llvm-cov --manifest-path engine/Cargo.toml -p "$target" --all-features \
    --json --summary-only --output-path "$report" >&2 \
    || fail "cargo llvm-cov failed"
fi

[[ -s "$report" ]] || fail "report '$report' is missing or empty"

# Count only this crate's own sources. A crate's test run also instruments the workspace
# crates it depends on, and their lines must not dilute or inflate its number.
if ! totals="$(jq -er --arg dir "/crates/$target/src/" '
    [.data[0].files[] | select(.filename | contains($dir)) | .summary.lines]
    | "\(map(.covered) | add // 0) \(map(.count) | add // 0)"' "$report" 2>/dev/null)"; then
  fail "report '$report' is not a readable llvm-cov JSON summary"
fi
read -r covered count <<< "$totals"

[[ "$covered" =~ ^[0-9]+$ && "$count" =~ ^[0-9]+$ ]] || fail "unexpected line counts: '$totals'"
(( count > 0 )) || fail "no lines measured under crates/$target/src/"

# Integer comparison, so no rounding decides a pass: covered/count >= min/100.
pct="$(awk -v c="$covered" -v n="$count" 'BEGIN { printf "%.2f", 100 * c / n }')"
if (( covered * 100 < min * count )); then
  fail "${pct}% of ${count} lines, below the ${min}% threshold"
fi
echo "✓ coverage for '$target': ${pct}% of ${count} lines (threshold ${min}%)"
