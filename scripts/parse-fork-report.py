#!/usr/bin/env python3
"""Turns forge's fork report into one of the fork suite's outcomes (#67, fixed in #82).

Usage: FORK_REPORT=<forge output> parse-fork-report.py <expected-test-count> <forge-exit-status>

Exit codes match `scripts/fork-suite.sh`: 0 agreed, 1 disagreed, 2 misconfigured.

**The report arrives through the environment, never through this file's source.** It used to be
interpolated into an unquoted heredoc, which made forge's output part of the program text: Python
then processed the escapes in a triple-quoted literal, so a revert reason carrying `\\"` stopped
being valid JSON before `json.loads` ever saw it, and a `\"\"\"` anywhere ended the literal outright.

That failed only when a test failed, which is the one time the output matters: a real disagreement
was reported as "no JSON report" with a truncated tail instead of naming the test. Jay found it
reviewing #80. A separate file is also what lets the self-test drive this directly, with a canned
report and no network.
"""

import json
import os
import sys


def main() -> int:
    if len(sys.argv) != 3:
        print(f"usage: {sys.argv[0]} <expected-test-count> <forge-exit-status>", file=sys.stderr)
        return 2
    expected, status = int(sys.argv[1]), int(sys.argv[2])
    raw = os.environ.get("FORK_REPORT")
    if raw is None:
        print("✗ FORK_REPORT is not set", file=sys.stderr)
        return 2

    suites = last_json_object(raw)
    if suites is None:
        print("✗ the fork suite produced no JSON report:", file=sys.stderr)
        print(raw[-2000:], file=sys.stderr)
        return 1 if status else 2

    results = {
        name: result
        for suite in suites.values()
        if isinstance(suite, dict)
        for name, result in suite.get("test_results", {}).items()
    }
    failed = {name: r for name, r in results.items() if r.get("status") != "Success"}

    if len(results) != expected:
        print(f"✗ the fork suite ran {len(results)} tests, expected {expected}.", file=sys.stderr)
        print("  A suite that can pass by running nothing is not a suite. Check the fork profile's",
              file=sys.stderr)
        print("  match_path and no_match_path, and EXPECTED_FORK_TESTS in fork-suite.sh.",
              file=sys.stderr)
        return 2

    if failed:
        print(f"✗ the deployment disagrees with the committed vectors ({len(failed)} of {expected}):",
              file=sys.stderr)
        for name, result in sorted(failed.items()):
            print(f"    {name}: {result.get('reason') or result.get('status')}", file=sys.stderr)
        print("  This is a finding: the escrow may have been redeployed or upgraded. It is a",
              file=sys.stderr)
        print("  needs-jay issue plus a check of docs/spec-notes.md, never a vector update to go green.",
              file=sys.stderr)
        return 1

    print(f"✓ the deployment agrees with the committed vectors ({expected} fork tests)")
    return 0


def last_json_object(raw: str) -> dict | None:
    """The last line that parses as a JSON object.

    forge prints its report as one line, after any warnings. Parsing line by line rather than the
    whole output means a warning above it cannot make the report unreadable.
    """
    found = None
    for line in raw.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            parsed = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict):
            found = parsed
    return found


if __name__ == "__main__":
    sys.exit(main())
