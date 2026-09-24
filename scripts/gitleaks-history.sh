#!/usr/bin/env bash
# Scan the full git history for secrets, the same way the CI gitleaks job does (#72).
#
# The pre-commit hook scans staged files only, so a secret already committed, or one revived by a
# rebase, is invisible locally until CI says so. That is what happened on #62: the branch was
# green in the working tree and red in CI, and had to be rewritten. This closes the gap.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1

if ! command -v gitleaks >/dev/null 2>&1; then
  cat >&2 <<'MSG'
✗ gitleaks is not installed.
  Download a release for your platform from https://github.com/gitleaks/gitleaks/releases
  and put it on PATH. `just doctor` reports it alongside the rest of the toolchain.
MSG
  exit 2
fi

# `detect` walks every commit reachable from the repository's refs, which is what the CI job does
# with fetch-depth: 0. --redact keeps a finding from printing the secret it found into a terminal
# that may be shared or recorded.
gitleaks detect --no-banner --redact
