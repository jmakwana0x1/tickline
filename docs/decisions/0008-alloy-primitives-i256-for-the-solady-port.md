# ADR-0008: `alloy-primitives` `I256` for the Solady port

- **Status:** accepted (Jay, D1 on #31)
- **Date:** 2026-09-17
- **Phase:** 1
- **Invariants touched:** none directly; I2, I3 and I15 all rest on the arithmetic this chooses

## Context

`PHASES.md` phase 1 asks for Solady's `expWad` and `lnWad` to be ported to Rust, so the engine and
the vault share one numerical method. At Solady `9fe23ffdcd395c4169396064226ed27206b222c3`
(`src/utils/FixedPointMathLib.sol`) both are signed 256-bit arithmetic:

- `expWad` moves `x` to a 2^96 basis (line 226), shifts it a further 96 bits (line 231, about
  2^199) and carries `p` in a 2^192 basis (lines 242 to 244). That overflows `i128` even for
  `x <= 0`, the only range the log-sum-exp cost needs.
- Its last step multiplies as **unsigned** 256-bit, `uint256(r) * 38228...5667 >> (195 - k)`. The
  product reaches about 2^255.4, which fits `U256` but not `I256`.
- `lnWad` uses `mul`, `sdiv`, `sar`, `shl`, `shr`, `add`, `sub`, `xor`, `and` and `byte` in
  assembly.

So the port needs signed and unsigned 256-bit integers with checked multiply, truncating signed
division (EVM `sdiv`), arithmetic right shift (EVM `sar`), and left shift.

`CLAUDE.md` §3 described `lmsr` as "zero IO, zero deps", while the crate already depended on
`thiserror`.

## Options

| Option | Cost | What it does |
|---|---|---|
| `ruint` directly | unsigned only | sign handling would be hand-rolled, which is the hardest part to prove |
| hand-rolled signed 256-bit type | the most code to prove | no dependency |
| an `i128`-safe exp/ln algorithm | none | Rust and Solidity stop sharing one method |
| **`alloy-primitives` `I256` and `U256`** | a dependency with a transitive tree | signed 256-bit arithmetic built on `ruint`, and the type family `protocol` adopts in Phase 2 anyway |

## Decision

`lmsr` depends on **`alloy-primitives = { version = "=1.7.3", default-features = false }`**, and on
`thiserror`, and on nothing else. `CLAUDE.md` §3 now says so, and
`scripts/check-crate-boundaries.sh` enforces it as an allowlist: any other non-dev dependency,
including a build-dependency, fails, and so does `alloy-primitives` with default features.

The dependency itself is added in S3 (#40), where it is first used.

### Verified, not assumed

A scratch crate depending on exactly `alloy-primitives =1.7.3` with `default-features = false`
compiled and passed these checks on 2026-09-17:

| Capability the port needs | Checked with | Result |
|---|---|---|
| checked signed multiply | `(-3) * 4 = -12`; `I256::MAX * 2` and `I256::MIN * -1` return `None` | yes |
| checked signed divide, truncating toward zero like `sdiv` | `-7 / 2 = -3`, `7 / -2 = -3`; divide by zero and `MIN / -1` return `None` | yes |
| arithmetic shift right like `sar` | `(-5).asr(1) = -3`, `5.asr(1) = 2`, `(-1).asr(255) = -1`, `MIN.asr(255) = -1` | yes |
| checked shift left | `(-3).checked_shl(4) = -48`; `3 << 78` | yes |
| checked add and subtract, ordering | `5 - 8 = -3`; `MAX + 1` returns `None`; `-1 < 0` | yes |
| unsigned 256-bit multiply and shift for `expWad`'s scale step | `2^94 * 38228...5667` fits; `U256::MAX * 2` returns `None` | yes |
| raw signed/unsigned conversion, bit length, byte access | `I256::from_raw(x.into_raw()) == x`; `bit_len`; `byte(0)` | yes |

With default features off, the normal-dependency tree has 28 crates besides the probe. None of
them does IO: `ruint`, `bytes`, `const-hex`, `derive_more`, `sha3`/`keccak`, `itoa` and their
build-time proc-macro helpers.

## Consequences

The port can mirror Solady operation for operation, which is what makes Rust-versus-Solidity
parity checkable in Phase 3.

The transitive tree is larger than "zero deps" suggested. The allowlist bounds what `lmsr` declares
itself; it does not audit the tree. Upgrading `alloy-primitives` is its own PR with its own gate
run (`CLAUDE.md` §5).

**For S3:** the workspace denies `clippy::arithmetic_side_effects`. The port should use the
`checked_*` methods and turn `None` into a typed error rather than use operators, which also makes
every overflow the proofs rule out a visible, tested branch.

## How this is enforced

`scripts/check-crate-boundaries.sh` (in `just deps-check`, run by the required `deps` job) and its
self-test `scripts/test-check-crate-boundaries.sh`, which proves it rejects an unlisted dependency,
`alloy-primitives` with default features, and an unlisted build-dependency.
