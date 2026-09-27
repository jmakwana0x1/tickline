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

if [[ -z "${BASE_SEPOLIA_RPC_URL:-}" ]]; then
  echo "✗ BASE_SEPOLIA_RPC_URL is not set. The fork suite fails rather than skips (CLAUDE.md §2)." >&2
  exit 2
fi

# Reachability first, so an outage is never reported as a disagreement. One cheap call, not a fork.
probe="$(curl -sS --max-time 20 -X POST -H 'content-type: application/json' \
  --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' \
  "$BASE_SEPOLIA_RPC_URL" 2>&1)" || {
  echo "✗ could not reach BASE_SEPOLIA_RPC_URL: $probe" >&2
  echo "  This is an outage, not a finding: the escrow has not been shown to disagree." >&2
  exit 75
}
case "$probe" in
  *'"result":"0x14a34"'*) ;;  # 84532, Base Sepolia
  *) echo "✗ the endpoint answered, but not as Base Sepolia (84532): $probe" >&2; exit 2 ;;
esac

# --force is not optional: forge caches which tests it found per profile, so without it a fork run
# after a default-profile run reports "No tests found" while `--list` shows every one of them.
output="$(cd contracts && FOUNDRY_PROFILE=fork forge test --force --json 2>&1)"
status=$?

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
