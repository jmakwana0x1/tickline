#!/usr/bin/env bash
# `abi.encodePacked` is banned in our Solidity (ADR-0013, Jay's D3 on #59).
#
# It does not pad, so two different field tuples can encode to identical bytes:
# encodePacked(uint8(1), uint16(2)) and encodePacked(uint16(258)) are the same three bytes. In a
# hash preimage that is a collision, and every preimage in this system keys money: the market id,
# the EIP-712 struct hashes, the claim batch root.
#
# There is no exception for the EIP-712 `0x1901` framing. `bytes.concat(hex"1901", separator,
# structHash)` does the same job with fixed-width inputs and no ambiguity, and Solidity has had it
# since 0.8.4. We pin 0.8.28.
#
# `lib/` is excluded: forge-std is vendored and uses encodePacked for string building, which is not
# a hash preimage and not ours to change.
#
# Usage: check-no-encode-packed.sh [contracts-dir]
set -euo pipefail
dir="${1:-$(cd "$(dirname "$0")/.." && pwd -P)/contracts}"
[[ -d "$dir" ]] || { echo "✗ no such directory: $dir" >&2; exit 2; }

found=0
while IFS= read -r -d '' file; do
  if grep -n 'abi\.encodePacked' "$file"; then
    echo "✗ $file uses abi.encodePacked; use abi.encode, or bytes.concat for fixed-width framing" >&2
    found=1
  fi
done < <(find "$dir" -name '*.sol' -not -path '*/lib/*' -print0)

(( found == 0 )) || exit 1
echo "✓ no abi.encodePacked in our Solidity"
