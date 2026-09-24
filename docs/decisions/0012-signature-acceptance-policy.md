# ADR-0012: one signature is one encoding, and anything else is rejected

- **Status:** accepted (Jay, D2 on #59)
- **Date:** 2026-09-24
- **Phase:** 2
- **Invariants touched:** I6 (receipts verify against the operator key), I7 (a voucher is accepted only if valid), I12 (a receipt always wins a dispute)

## Context

Every dispute in Tickline is settled by a signed object. A `PositionReceipt` is what makes a lying
operator slashable (I12), and a voucher is what makes a charge legitimate (I7). The vault, the
engine, and the client therefore have to agree, byte for byte, on which signatures are valid.

secp256k1 signatures admit more than one encoding of the same authorization:

- for every valid `(r, s)` there is a second valid `(r, n - s)` recovering the same signer, the
  **malleability** that EIP-2 addressed onchain;
- `v` is written as 27/28 by Ethereum tooling and as 0/1 by the recovery id itself;
- EIP-2098 packs `y`-parity into the top bit of `s`, giving a 64-byte form of the same signature.

The x402 batch-settlement scheme does not state an acceptance policy, so we must. If we do not,
one voucher has several valid encodings, and "have I already seen this signature?" stops having an
answer.

## Decision

A signature is **65 bytes, `r || s || v`**, and every other form is rejected with a typed error.
Rejection, never normalization.

| Rule | Rejected because |
|---|---|
| exactly 65 bytes | any other length is not the form we pinned |
| 64 bytes is its own error (EIP-2098 compact) | it is a real encoding of a real signature, and it is still not ours |
| `v` in {27, 28} | `v` of 0 or 1 is rejected with a typed error, not silently fixed |
| `s <= n/2` | the high-s twin is the same authorization under a second encoding |
| `r` and `s` non-zero | a zero scalar is not a signature |
| `r` and `s` below the curve order `n` | above `n` is not a scalar |
| verification recovers and compares to the **expected** signer | "the recovered address is not zero" is never the check |

**Why reject `v` of 0 or 1 rather than normalize.** The official x402 SDK signs through viem, which
emits 27 and 28, so nothing legitimate is lost. Normalizing would silently accept input we never
pinned in a vector, and the first time we saw a 0/1 signature in production we would not know
whether it came from a client we support or from something else.

**Why this policy is not only the engine's.** A policy the engine enforces and the vault does not
is worthless, because the vault settles the disputes. Two obligations follow, and they are part of
this decision rather than consequences of it:

1. **Every rejection case is a cross-stack vector** (#67). `testdata/vectors/eip712.json` carries
   the invalid inputs with their expected rejection reason, and Rust, Solidity, and TypeScript
   must all reject them.
2. **The Phase 3 vault enforces the identical policy**, through the same
   `contracts/src/TicklineTypes.sol` the vectors test (D4), and its acceptance tests load the same
   rejection vectors.

## Consequences

A client that signs with a 0/1 `v`, or sends a compact signature, gets a 4xx with a named error
rather than a silent success. That is a support cost, and it is the point: the set of accepted
encodings is exactly the set we have test vectors for.

`ecrecover` in Solidity returns the zero address on failure, which is why the EVM habit is to
check for it. Our Solidity library will compare against the expected signer instead, so the check
is the same sentence in all three stacks.

The engine never signs a voucher and never needs a signing key in `protocol`. The crate verifies
only, which is why ADR-0011's dependency list has no randomness in it.

## How this is enforced

`engine/crates/protocol/tests/signature.rs` names one test per rule, and every rejection has its
own error variant with a test that triggers it. From #67 the same cases are vectors that all three
stacks run, and from Phase 3 the vault runs them too.
