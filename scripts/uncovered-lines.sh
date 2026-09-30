#!/usr/bin/env bash
# uncovered-lines.sh <crate> [--report <file.lcov>]
#
# Prints every line of <crate>'s own sources that a coverage run did not execute, one
# "<file>:<line>" per line, sorted by file then numerically by line. Companion to
# check-coverage.sh (#37), which reports one percentage: this one names the lines behind it, so two
# machines that disagree about a percentage can be diffed rather than guessed about (#89).
#
# Fails closed. A missing report, a report naming no lines for this crate, and a usage error are all
# failures. "Nothing uncovered" prints nothing and exits 0, and the two must never look alike.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1

crate="${1:?usage: uncovered-lines.sh <crate> [--report <file.lcov>]}"
shift
report=""
own_report=""
while (( $# )); do
  case "$1" in
    --report) report="${2:?--report needs a file}"; shift 2 ;;
    *) echo "✗ unknown argument: $1" >&2; exit 2 ;;
  esac
done

fail() { echo "✗ uncovered lines for '$crate': $*" >&2; exit 1; }

if [[ -z "$report" ]]; then
  [[ -f "engine/crates/$crate/Cargo.toml" ]] || fail "not an engine crate"
  command -v cargo-llvm-cov >/dev/null || fail "cargo-llvm-cov is not installed (see 'just doctor')"
  report="$(mktemp)"
  own_report="$report"   # only a report this script made is ours to delete
  # The same measurement check-coverage.sh makes, in lcov form so it carries per-line hit counts.
  cargo llvm-cov --manifest-path engine/Cargo.toml -p "$crate" --all-features \
    --lcov --output-path "$report" >&2 || fail "cargo llvm-cov failed"
fi

[[ -s "$report" ]] || fail "report '$report' is missing or empty"

# lcov: SF:<path> opens a file's records, DA:<line>,<hits> is one line's hit count. Count every
# DA under this crate's own src/, so "no lines measured" is told apart from "all lines covered".
measured="$(mktemp)"
uncovered="$(mktemp)"
# Never "${report}": with --report that is the caller's file, and deleting it would destroy the
# input we were asked to read.
trap 'rm -f "$measured" "$uncovered" "${own_report:-}"' EXIT
awk -v dir="/crates/$crate/src/" -v out="$uncovered" '
  /^SF:/ { keep = (index(substr($0, 4), dir) > 0); file = substr($0, 4); next }
  keep && /^DA:/ {
    split(substr($0, 4), a, ",")
    measured++
    if (a[2] == 0) print file ":" a[1] > out
  }
  END { print measured + 0 }
' "$report" > "$measured"

read -r count < "$measured"
(( count > 0 )) || fail "the report names no lines under crates/$crate/src/"

# -k1,1 then -k2,2n: line 10 sorts after line 9, which a lexicographic sort gets wrong.
sort -t: -k1,1 -k2,2n "$uncovered"
