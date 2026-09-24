#!/usr/bin/env bash
# testdata/vectors/ is allowlisted in .gitleaks.toml, because fixed keys are the point of a vector
# file. That makes the directory a blind spot: a real private key pasted into a vector file is
# invisible to the scan (#72). gitleaks cannot tell a 32-byte hash from a 32-byte key by syntax,
# but a field *name* can be classified, so this check says: in a vector file, a key-ish field may
# hold only a key that .gitleaks.toml already allowlists.
#
# Stub for the red commit; the implementation follows.
#
# Usage: check-vector-secrets.sh [vectors-dir] [gitleaks-config]
set -euo pipefail
echo "✓ stub"
