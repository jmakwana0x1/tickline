#!/usr/bin/env bash
# The fork suite, with the three outcomes kept apart (#67, Jay's conditions on #80).
#
# An unreachable endpoint and a changed type hash are different findings. Conflating them means the
# first bad night on Base Sepolia produces an issue claiming the escrow was redeployed, so the exit
# code says which happened:
#
#   0   agreed. The deployment matches the committed vectors.
#   1   disagreed, or a test failed. This is the finding: the escrow may have been redeployed.
#   75  could not reach the endpoint. Not a finding. 75 is EX_TEMPFAIL, by convention.
#   2   misconfigured: no secret, or the suite ran no tests.
#
# The caller decides what to do with 75. `phase-gate` fails on it, because a phase should not close
# without the suite having actually run; nightly warns and moves on.
#
# An absent secret fails and never skips: a suite that quietly does nothing is worse than a red one.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 2

# Bump this when the suite gains a test. Asserting the exact count, not merely a non-zero one, is
# what catches the `no_match_path` inheritance bug: a profile that selects a directory the default
# profile excludes matches nothing and exits 0, which reads as success.
EXPECTED_FORK_TESTS=5


# A developer keeps the endpoint in .env, CI passes it as a secret. The environment wins, so CI is
# never surprised by a file, and the value is read rather than sourced: `.env` is not a script, and
# executing it to get one variable would run whatever else is in there.
# `+set` and not `:-`: a variable that is set but empty means CI passed a missing secret, and that
# must fail rather than quietly fall back to a developer's file.
if [[ -z "${BASE_SEPOLIA_RPC_URL+set}" && -f .env ]]; then
  from_file="$(sed -n 's/^[[:space:]]*BASE_SEPOLIA_RPC_URL[[:space:]]*=[[:space:]]*//p' .env | tail -1)"
  from_file="${from_file%\"}"; from_file="${from_file#\"}"
  from_file="${from_file%\'}"; from_file="${from_file#\'}"
  BASE_SEPOLIA_RPC_URL="$from_file"
fi

if [[ -z "${BASE_SEPOLIA_RPC_URL:-}" ]]; then
  echo "✗ BASE_SEPOLIA_RPC_URL is not set, in the environment or in .env." >&2
  echo "  The fork suite fails rather than skips (CLAUDE.md §2), because a suite that quietly" >&2
  echo "  does nothing is worse than a red one. Put a keyed endpoint in .env locally, and the" >&2
  echo "  repository secret of the same name in CI." >&2
  exit 2
fi
export BASE_SEPOLIA_RPC_URL

# The endpoint carries an API key, so it never reaches a log. Errors below are redacted through this.
redact() { sed -e "s|${BASE_SEPOLIA_RPC_URL}|<BASE_SEPOLIA_RPC_URL>|g" -e 's|\(https\?://[^/ ]*\)/[^ ]*|\1/<redacted>|g'; }

# Reachability first, so an outage is never reported as a disagreement. One cheap call, not a fork.
probe="$(curl -sS --max-time 20 -X POST -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
  "$BASE_SEPOLIA_RPC_URL" 2>&1)" || {
  echo "✗ could not reach BASE_SEPOLIA_RPC_URL:" >&2
  printf '%s\n' "$probe" | redact >&2
  echo "  This is an outage, not a finding: the escrow has not been shown to disagree." >&2
  exit 75
}
case "$probe" in
  *'"result":"0x14a34"'*) ;;  # 84532, Base Sepolia
  *)
    echo "✗ the endpoint answered, but not as Base Sepolia (84532):" >&2
    printf '%s\n' "$probe" | redact >&2
    exit 2
    ;;
esac

# --force is not optional: forge caches which tests it found per profile, so without it a fork run
# after a default-profile run reports "No tests found" while `--list` shows every one of them.
# Captured without a pipe, so $? is forge's own status: through a pipe it would be the redactor's,
# which is always 0, and every failing run would have looked like a passing one with odd output.
output="$(cd contracts && FOUNDRY_PROFILE=fork forge test --force --json 2>&1)"
status=$?

# Redacted after capture, because the endpoint carries an API key and this output reaches CI logs.
output="$(printf '%s' "$output" | redact)"

# Through the environment, never through the parser's source. Interpolating the report into an
# unquoted heredoc made forge's output part of the program, so an escaped quote in a revert reason
# stopped being JSON before it was parsed, and only ever on a failing run (#82).
FORK_REPORT="$output" python3 scripts/parse-fork-report.py "$EXPECTED_FORK_TESTS" "$status"
