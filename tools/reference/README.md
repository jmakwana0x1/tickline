# tools/reference

Independent implementations, used to generate test vectors. **Never imported at runtime.**

The point is disagreement: if the Rust LMSR and a 60-digit `mpmath` implementation of the same
formulas agree to within 1 wei across 5,000 cases including the edges, the Rust is probably
right. If they were the same code, agreement would prove nothing.

| File | Produces | Consumed by |
|---|---|---|
| `lmsr_ref.py` | `testdata/vectors/lmsr.json` | Phase 1 Rust differential tests, Phase 3 Solidity `expWad`/`lnWad` parity |

Run through `just vectors`, which regenerates everything and **fails if the output differs from
what is committed**. Generators are deterministic; a drifting vector file is either a real
change that needs review or a bug in the generator.
