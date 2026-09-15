#!/usr/bin/env bash
# check-coverage.sh <target> <min-percent>
# Placeholder until Phase 1 wires cargo-llvm-cov / forge coverage into the gate.
set -euo pipefail
TARGET="${1:?target}" MIN="${2:?min percent}"
echo "… coverage gate for '$TARGET' (>= ${MIN}%) lands with the phase that owns it"
