#!/usr/bin/env bash
# Self-test for scripts/uncovered-lines.sh (issue #90).
# Evaluates hand-written lcov reports, so it needs neither cargo-llvm-cov nor a build.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
probe="$PWD/scripts/uncovered-lines.sh"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
failures=0

pass() { printf '  \033[32m✓\033[0m %s\n' "$1"; }
fail() { printf '  \033[31m✗\033[0m %s\n' "$1"; failures=$((failures + 1)); }

# <file> then DA lines, in lcov's shape: SF:<path>, DA:<line>,<hits>, end_of_record.
expect_out() { # <label> <report> <crate> <want-stdout>
  local label="$1" report="$2" crate="$3" want="$4" got status=0
  got="$(bash "$probe" "$crate" --report "$report" 2>/dev/null)" || status=$?
  if (( status != 0 )); then
    fail "$label (exit $status)"
  elif [[ "$got" == "$want" ]]; then
    pass "$label"
  else
    fail "$label (got '$got', want '$want')"
  fi
}

# A rejection must be the script's own, so the accepted statuses are named: 1 is a refused report,
# 2 is a usage error. 127 (no such script) would otherwise satisfy every negative test here and the
# suite would pass with nothing implemented.
expect_fail() { # <label> <want-status> <args...>
  local label="$1" want="$2" status=0
  shift 2
  bash "$probe" "$@" >/dev/null 2>&1 || status=$?
  if (( status == want )); then pass "$label"; else fail "$label (exit $status, wanted $want)"; fi
}

echo "test-uncovered-lines:"

cat > "$tmp/mixed.lcov" <<'LCOV'
SF:/w/engine/crates/protocol/src/signature.rs
DA:10,3
DA:11,0
DA:12,0
end_of_record
SF:/w/engine/crates/protocol/src/envelope.rs
DA:9,0
DA:10,0
end_of_record
SF:/w/engine/crates/lmsr/src/lib.rs
DA:1,0
end_of_record
LCOV

# Numeric ordering matters: a plain lexicographic sort puts line 10 before line 9.
expect_out "extracts the uncovered lines from a report" "$tmp/mixed.lcov" protocol \
'/w/engine/crates/protocol/src/envelope.rs:9
/w/engine/crates/protocol/src/envelope.rs:10
/w/engine/crates/protocol/src/signature.rs:11
/w/engine/crates/protocol/src/signature.rs:12'
expect_out "sorts by file then by line number" "$tmp/mixed.lcov" protocol \
'/w/engine/crates/protocol/src/envelope.rs:9
/w/engine/crates/protocol/src/envelope.rs:10
/w/engine/crates/protocol/src/signature.rs:11
/w/engine/crates/protocol/src/signature.rs:12'
expect_out "ignores files outside the crate's own src" "$tmp/mixed.lcov" lmsr \
'/w/engine/crates/lmsr/src/lib.rs:1'

cat > "$tmp/full.lcov" <<'LCOV'
SF:/w/engine/crates/protocol/src/lib.rs
DA:1,1
DA:2,7
end_of_record
LCOV
# Covered lines in a file that also has uncovered ones must not be printed either, which the
# mixed report above already shows: signature.rs:10 is covered and absent from the expected output.
expect_out "prints nothing when every line is covered" "$tmp/full.lcov" protocol ''
expect_out "ignores covered lines in the same file" "$tmp/mixed.lcov" protocol \
'/w/engine/crates/protocol/src/envelope.rs:9
/w/engine/crates/protocol/src/envelope.rs:10
/w/engine/crates/protocol/src/signature.rs:11
/w/engine/crates/protocol/src/signature.rs:12'

# Fails closed (#37): "no lines measured" and "nothing uncovered" must not look alike.
cat > "$tmp/other.lcov" <<'LCOV'
SF:/w/engine/crates/api/src/lib.rs
DA:1,0
end_of_record
LCOV
expect_fail "fails when the report names no lines for the crate" 1 protocol --report "$tmp/other.lcov"
expect_fail "fails when the report is missing" 1 protocol --report "$tmp/nope.lcov"
expect_fail "rejects an unknown argument" 2 protocol --report "$tmp/mixed.lcov" --verbose

(( failures == 0 )) || { echo "✗ $failures uncovered-lines test(s) failed" >&2; exit 1; }
echo "✓ uncovered-lines reports what a coverage report leaves out"
