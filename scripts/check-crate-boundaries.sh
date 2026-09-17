#!/usr/bin/env bash
# lmsr and protocol are zero-IO (CLAUDE.md section 3). This is what makes them exhaustively
# testable: no clock, no database, no network, no async runtime.
#
# - lmsr: an allowlist. Its non-dev dependencies are exactly those Jay approved on #31 (D1,
#   ADR-0008): alloy-primitives with default features off, and thiserror.
# - protocol: a banned list of IO crates, until Phase 2 decides its own allowlist.
# - both: no direct wall-clock or network use in the source.
#
# Usage: check-crate-boundaries.sh [engine-dir]   (default: this repository's engine/)
set -euo pipefail

engine="${1:-$(dirname "$0")/../engine}"
[[ -f "$engine/Cargo.toml" ]] || { echo "✗ no Cargo.toml in $engine" >&2; exit 2; }

metadata="$(cargo metadata --format-version 1 --no-deps --manifest-path "$engine/Cargo.toml")"

STATUS=0
python3 - "$metadata" <<'PY' || STATUS=1
import json, sys

LMSR_ALLOWED = {"alloy-primitives", "thiserror"}
NO_DEFAULT_FEATURES = {"alloy-primitives"}
PROTOCOL_BANNED = {"tokio", "sqlx", "reqwest", "axum", "hyper", "alloy-provider", "chrono", "rand"}

packages = {p["name"]: p for p in json.loads(sys.argv[1])["packages"]}
problems = []

def runtime_deps(name):
    # Everything except dev-dependencies ships with the crate: normal and build.
    return [d for d in packages[name]["dependencies"] if d["kind"] != "dev"]

if "lmsr" in packages:
    for d in runtime_deps("lmsr"):
        if d["name"] not in LMSR_ALLOWED:
            problems.append(f"lmsr may depend only on {sorted(LMSR_ALLOWED)}, not '{d['name']}' ({d['kind'] or 'normal'})")
        elif d["name"] in NO_DEFAULT_FEATURES and d["uses_default_features"]:
            problems.append(f"lmsr must use '{d['name']}' with default-features = false")
else:
    problems.append("no lmsr crate in the workspace")

if "protocol" in packages:
    for d in runtime_deps("protocol"):
        if d["name"] in PROTOCOL_BANNED:
            problems.append(f"protocol must stay zero-IO but depends on '{d['name']}'")
else:
    problems.append("no protocol crate in the workspace")

for p in problems:
    print(f"✗ {p}", file=sys.stderr)
sys.exit(1 if problems else 0)
PY

# Belt and braces: no direct wall-clock or network in the source either.
for crate in lmsr protocol; do
  if grep -rnE 'std::time::(SystemTime|Instant)|std::net' "$engine/crates/$crate/src" 2>/dev/null; then
    echo "✗ $crate reads the clock or the network directly" >&2
    STATUS=1
  fi
done

(( STATUS == 0 )) && echo "✓ lmsr uses only its allowlist; lmsr and protocol are zero-IO"
exit "$STATUS"
