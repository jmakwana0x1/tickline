# ADR-0011: `protocol` depends on `alloy-primitives` and `k256`, and on nothing that signs for us

- **Status:** accepted (Jay, D1 on #59)
- **Date:** 2026-09-24
- **Phase:** 2
- **Invariants touched:** none directly; I6, I7 and I12 all rest on the hashing and recovery this chooses

## Context

`engine/crates/protocol` has to hash EIP-712 structs exactly as the EVM does and recover the
signer of a secp256k1 signature, while staying zero-IO (`CLAUDE.md` §3): no clock, no network, no
database, no async runtime. That is what makes it exhaustively testable, and it is what lets the
same code be diffed against Solidity and TypeScript in S6.

Two capabilities are needed, and neither can be hand-rolled responsibly near money: keccak256, and
ECDSA public key recovery on secp256k1.

`CLAUDE.md` §3 already gives `lmsr` an allowlist, enforced by `scripts/check-crate-boundaries.sh`.
`protocol` had only a banned list of IO crates, which says what it may not do and nothing about
what it may.

## Options

| Option | Cost | What it does |
|---|---|---|
| `alloy-signer` / `alloy-consensus` | a large surface for two functions, and a signer abstraction we do not want | recovery and hashing, with normalization decided for us |
| `secp256k1` (libsecp256k1 bindings) | a C toolchain in every build, including CI and any future cross-compile | the implementation geth and reth use |
| `ethers` | unmaintained, and larger still | everything |
| **`alloy-primitives` plus `k256`** | one new direct dependency | keccak256 from the crate `lmsr` already pins, recovery from a pure-Rust, no_std-capable curve implementation |

## Decision

`protocol` may depend on exactly these, and on nothing else:

| Crate | Features | Why |
|---|---|---|
| `alloy-primitives` | `default-features = false`, `tiny-keccak` | `Address`, `B256`, `U256`, `keccak256` |
| `k256` | `default-features = false`, `ecdsa` | `recover_from_prehash`, scalar validation |
| `serde` | derive | envelope types (S5) |
| `serde_json` | | envelope encoding (S5) and the vector loader (S6) |
| `base64` | | payment header payloads (S5) |
| `thiserror` | | typed errors, as everywhere else |

No `alloy-signer`, no `ethers`, and nothing that pulls a C toolchain.

**`k256` is used directly rather than through a signer wrapper.** ADR-0012 has to inspect `s`
before deciding whether to accept a signature, and a wrapper that normalizes `s` for you makes
that policy impossible to express: a high-s signature would silently become a valid one.

`CLAUDE.md` §3 carries the list, and `scripts/check-crate-boundaries.sh` enforces it as an
allowlist, exactly as it does for `lmsr`. Each dependency is added to the manifest in the slice
that first uses it, so the manifest never carries a dependency no code needs.

### Verified, not assumed

A scratch crate depending on exactly `alloy-primitives =1.7.3` (`default-features = false`,
`features = ["tiny-keccak"]`) and `k256 0.13` (`default-features = false`, `features = ["ecdsa"]`)
compiled and produced these results on 2026-09-24:

| Claim | Checked with | Result |
|---|---|---|
| keccak256 agrees with the EVM | `keccak256("Tickline")` against `cast keccak "Tickline"` | identical, `0xefe8dcab…c4b7` |
| recovery yields the right address | the `cast wallet sign` fixture for anvil account 0, recovered and hashed to an address | `0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266` |
| high-s is detectable before deciding | `Signature::normalize_s()` returns `Some` only for high-s | yes, `None` for the low-s fixture, `Some` for its negation |
| `r` or `s` of zero is rejected | `Signature::from_slice` with each scalar zeroed | rejected |
| `r` or `s` at the curve order is rejected | `from_slice` with each scalar set to `n` | rejected |
| `n - 1` is accepted | `from_slice` with both scalars at `n - 1` | accepted |
| no IO in the tree | `cargo tree --edges normal` | 45 crates, none doing IO |

The tree includes `rand_core`, which `elliptic-curve` and `signature` declare. It is a transitive
dependency, not a declared one, and nothing in `protocol` draws randomness: the crate has no
signing path at all, only verification. The allowlist bounds what `protocol` declares; it does not
audit the tree.

## Consequences

`tiny-keccak` is enabled on `alloy-primitives` for the whole workspace, because cargo unifies
features across a build. `lmsr` therefore compiles a keccak implementation it does not call. That
is a few kilobytes and no behaviour change, and it is the price of one pinned version of
`alloy-primitives` across the workspace rather than two.

Upgrading either crate is its own PR with its own gate run (`CLAUDE.md` §5).

## How this is enforced

`scripts/check-crate-boundaries.sh` (in `just deps-check`, run by the required `deps` job) and its
self-test `scripts/test-check-crate-boundaries.sh`, which proves the allowlist rejects an unlisted
dependency, an unlisted build-dependency, and `alloy-primitives` or `k256` with default features.
