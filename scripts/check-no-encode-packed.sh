#!/usr/bin/env bash
# `abi.encodePacked` is banned in our Solidity (ADR-0013, Jay's D3 on #59). It does not pad, so two
# different field tuples can encode to identical bytes, and every preimage in this system keys
# money.
#
# Stub for the red commit; the implementation follows.
#
# Usage: check-no-encode-packed.sh [contracts-dir]
set -euo pipefail
echo "✓ stub"
