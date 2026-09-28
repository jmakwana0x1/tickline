#!/usr/bin/env bash
# Every text file type the evm-private-key rule does not cover, and the one
# this repository writes most (#81, found by Jay).
#
# Its path filter is (rs|ts|tsx|sol|json|ya?ml|env|sh|toml), with no md, which is why spec-notes.md
# can carry eleven 64-hex values and need no allowlist entry. The cost is that a real key pasted into
# an ADR, a STATUS gate log or a README is invisible, and gitleaks' default rules only catch a bare
# hex when a keyword sits beside it.
#
# Same answer as testdata/vectors: cover the gap rather than trust the path. A 64-hex literal is
# flagged when a key-ish word sits within three lines of it. Measured before it was written: 11 hex
# literals across 30 tracked markdown files, and none is flagged at any window from zero to three
# lines, because every legitimate one sits beside hash, digest, typehash, channelId or feed id.
#
# Stub for the red commit; the implementation follows.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 2
echo "✓ stub"
