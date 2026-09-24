#!/usr/bin/env bash
# lmsr and protocol are zero-IO (CLAUDE.md section 3). This is what makes them exhaustively
# testable: no clock, no database, no network, no async runtime.
#
# - lmsr: an allowlist. Its non-dev dependencies are exactly those Jay approved on #31 (D1,
#   ADR-0008): alloy-primitives with default features off, and thiserror.
# - protocol: an allowlist too, from #59 (D1, ADR-0011). alloy-primitives and k256 must have
#   default features off; k256 is used directly because ADR-0012 inspects s before deciding.
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

ALLOWED = {
    # ADR-0008, Jay's D1 on #31.
    "lmsr": {"alloy-primitives", "thiserror"},
    # ADR-0011, Jay's D1 on #59. serde_json and base64 arrive with the envelopes (S5, #66);
    # the list permits them before the manifest carries them.
    "protocol": {"alloy-primitives", "k256", "serde", "serde_json", "base64", "thiserror"},
}
# Default features drag in std, rand, and in k256's case a signer surface ADR-0012 refuses.
NO_DEFAULT_FEATURES = {"alloy-primitives", "k256"}

packages = {p["name"]: p for p in json.loads(sys.argv[1])["packages"]}
problems = []

def runtime_deps(name):
    # Everything except dev-dependencies ships with the crate: normal and build.
    return [d for d in packages[name]["dependencies"] if d["kind"] != "dev"]

for crate, allowed in ALLOWED.items():
    if crate not in packages:
        problems.append(f"no {crate} crate in the workspace")
        continue
    for d in runtime_deps(crate):
        if d["name"] not in allowed:
            problems.append(f"{crate} may depend only on {sorted(allowed)}, not '{d['name']}' ({d['kind'] or 'normal'})")
        elif d["name"] in NO_DEFAULT_FEATURES and d["uses_default_features"]:
            problems.append(f"{crate} must use '{d['name']}' with default-features = false")

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

(( STATUS == 0 )) && echo "✓ lmsr and protocol use only their allowlists, and neither reads the clock or the network"
exit "$STATUS"
