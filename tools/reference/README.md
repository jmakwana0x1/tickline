# tools/reference

Independent implementations, used to generate test vectors. **Never imported at runtime.**

The point is disagreement: if the Rust LMSR and a 60-digit `mpmath` implementation of the same
formulas agree to within 1 wei across 5,000 cases including the edges, the Rust is probably
right. If they were the same code, agreement would prove nothing.

| File | Produces | Consumed by |
|---|---|---|
| `lmsr_ref.py` | `testdata/vectors/lmsr.json` | Phase 1 Rust differential tests, Phase 3 Solidity `expWad`/`lnWad` parity |

Run through `just vectors-lmsr` (or `just vectors` for every file), which regenerates the file
with `uv` at the pinned Python minor version and **fails if the output differs from what is
committed, or was never committed**.

## How `lmsr_ref.py` earns trust

- **Floors, not decimals.** Each value is stored as its exact `floor` plus an `inexact` flag, so a
  consumer can check rounding direction exactly (I15).
- **Precision.** At least 60 significant digits (`PHASES.md`) *and* at least 30 digits below the
  wei. The second rule exists because `exp_wad`'s domain reaches ~1e76 wei, which 60 significant
  digits alone cannot floor.
- **Stability check.** Every value is recomputed with 20 more digits; the generator refuses to write
  one whose floor or integrality moves.
- **No false integers.** Exact integer parts (`max(qY, qN)`, the larger price) are kept in integer
  arithmetic, because at any fixed precision `1 - tiny` rounds to exactly 1.
- **Provenance** in the file: mpmath version, Python minor version, precision, seed, the PRNG, the
  Solady commit whose domain limits are used, and the git blob hash of this generator (a blob hash,
  unlike a commit hash, survives squash merges).
- **Determinism.** SplitMix64 with a fixed seed, never Python's `random`, and a fixed file layout. Generators are deterministic; a drifting vector file is either a real
change that needs review or a bug in the generator.
