#!/usr/bin/env python3
"""Reference LMSR, computed at 60 decimal digits with mpmath.

This is the oracle Phase 1's Rust implementation is diffed against. It is deliberately the
*naive, obvious* transcription of the formulas: readability here is worth more than speed,
because its only job is to be independently correct.

Phase 1 implements the generator. Phase 0 ships the entry point and its contract so
``just vectors`` exists, fails loudly, and cannot silently produce nothing.

Contract:

* signed WAD fixed point, 1e18, integers only on the way in and out;
* at least 5,000 cases, including the edges: ``q = (0, 0)``, symmetric ``q``, large imbalance,
  minimum and maximum ``b``, and inputs just outside the domain of ``exp``/``ln``;
* every case records the exact rational result rounded **toward the vault** (I15), so the Rust
  side is compared against the value it is required to produce, not a convenient one.
"""

from __future__ import annotations

import argparse
import sys

WAD = 10**18


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", required=True, help="path to write the vector JSON")
    parser.add_argument("--cases", type=int, default=5000, help="minimum number of cases")
    args = parser.parse_args()

    print(
        f"lmsr_ref: generator lands in Phase 1 (PHASES.md).\n"
        f"  would write {args.cases} cases to {args.out}\n"
        f"  refusing to write an empty vector file: a passing differential test against\n"
        f"  zero cases is worse than no differential test at all.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    raise SystemExit(main())
