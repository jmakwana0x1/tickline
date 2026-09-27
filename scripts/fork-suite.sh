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
# PIPESTATUS, not $?: through a pipe the latter is the redactor's status, which is always 0, so
# every failing run would have looked like a passing one with odd output.
# Captured without a pipe, so $? is forge's own status: through a pipe it would be the redactor's,
# which is always 0, and every failing run would have looked like a passing one with odd output.
output="$(cd contracts && FOUNDRY_PROFILE=fork forge test --force --json 2>&1)"
status=$?

# Redacted after capture, because the endpoint carries an API key and this output reaches CI logs.
output="$(printf '%s' "$output" | redact)"

python3 - "$EXPECTED_FORK_TESTS" "$status" <<PY
import json, sys

expected, status = int(sys.argv[1]), int(sys.argv[2])
raw = """$output"""

suites = None
for line in raw.splitlines():
    line = line.strip()
    if line.startswith("{"):
        try:
            suites = json.loads(line)
        except json.JSONDecodeError:
            continue
if suites is None:
    print("✗ the fork suite produced no JSON report:", file=sys.stderr)
    print(raw[-2000:], file=sys.stderr)
    sys.exit(1 if status else 2)

results = {
    name: result
    for suite in suites.values()
    for name, result in suite.get("test_results", {}).items()
}
failed = {name: r for name, r in results.items() if r.get("status") != "Success"}

if len(results) != expected:
    print(f"✗ the fork suite ran {len(results)} tests, expected {expected}.", file=sys.stderr)
    print("  A suite that can pass by running nothing is not a suite. Check the fork profile's", file=sys.stderr)
    print("  match_path and no_match_path, and EXPECTED_FORK_TESTS in this script.", file=sys.stderr)
    sys.exit(2)

if failed:
    print(f"✗ the deployment disagrees with the committed vectors ({len(failed)} of {expected}):", file=sys.stderr)
    for name, result in sorted(failed.items()):
        print(f"    {name}: {result.get('reason') or result.get('status')}", file=sys.stderr)
    print("  This is a finding: the escrow may have been redeployed or upgraded. It is a", file=sys.stderr)
    print("  needs-jay issue plus a check of docs/spec-notes.md, never a vector update to go green.", file=sys.stderr)
    sys.exit(1)

print(f"✓ the deployment agrees with the committed vectors ({expected} fork tests)")
PY
