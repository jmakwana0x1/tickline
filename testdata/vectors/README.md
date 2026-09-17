# testdata/vectors

Generated, committed, and diffed in CI. `just vectors` regenerates everything and fails if the
result differs from what is in git.

| File | Generator | Proves |
|---|---|---|
| `lmsr.json` | `tools/reference/lmsr_ref.py` (mpmath, 60 digits) | Rust and Solidity LMSR math match an independent oracle to within 1 wei, rounding toward the vault |
| `eip712.json` | `agents/src/vectors/generate.ts` (viem + the official SDK) | Rust, Solidity, and TypeScript produce byte-identical struct hashes, digests, recovered signers, and market IDs |

Both use **fixed private keys**. That is the point of a vector file, and it is why this
directory is allowlisted in `.gitleaks.toml`. No key here is ever funded on any real chain.

`lmsr.json` exists since Phase 1 (5,163 cases); `eip712.json` arrives in Phase 2.
