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
# Comments are not code: the ban is explained in doc comments that name the thing they ban, so only
# real uses count. `lib/` is vendored (forge-std builds strings with it) and `out/` and `cache/` are
# build products.
#
# Usage: check-no-encode-packed.sh [contracts-dir]
set -euo pipefail
dir="${1:-$(cd "$(dirname "$0")/.." && pwd -P)/contracts}"
[[ -d "$dir" ]] || { echo "✗ no such directory: $dir" >&2; exit 2; }

python3 - "$dir" <<'PY'
import re
import sys
from pathlib import Path

root = Path(sys.argv[1])
SKIP = {"lib", "out", "cache", "broadcast", "node_modules"}

# Strip line comments and block comments, then look for a real call.
LINE_COMMENT = re.compile(r"//.*$", re.M)
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.S)
CALL = re.compile(r"\babi\.encodePacked\s*\(")

problems = []
for path in sorted(root.rglob("*.sol")):
    if SKIP & set(path.relative_to(root).parts):
        continue
    source = path.read_text()
    code = LINE_COMMENT.sub("", BLOCK_COMMENT.sub("", source))
    if CALL.search(code):
        # Report the line numbers from the original, so the message points at real source.
        for number, line in enumerate(source.splitlines(), start=1):
            if CALL.search(LINE_COMMENT.sub("", line)):
                problems.append(f"{path}:{number}: {line.strip()}")

for problem in problems:
    print(f"✗ {problem}", file=sys.stderr)
if problems:
    print("use abi.encode, or bytes.concat for fixed-width framing", file=sys.stderr)
    sys.exit(1)
print("✓ no abi.encodePacked in our Solidity")
PY
