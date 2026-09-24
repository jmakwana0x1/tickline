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

## The code is the contract, not the message

Each rejection carries a flat, stable code, and that is what crosses a stack boundary:

| Code | Rule |
|---|---|
| `SIG_LENGTH` | not 65 bytes |
| `SIG_COMPACT` | 64 bytes, EIP-2098 |
| `SIG_RECOVERY_ID` | `v` outside {27, 28} |
| `SIG_R_ZERO`, `SIG_S_ZERO` | a zero scalar |
| `SIG_R_ABOVE_ORDER`, `SIG_S_ABOVE_ORDER` | a scalar at or above `n` |
| `SIG_HIGH_S` | `s` above `n/2` |
| `SIG_UNRECOVERABLE` | no signer for this signature and digest |
| `SIG_WRONG_SIGNER` | a valid signature belonging to someone else |

The English message is for a human reading logs. It cannot be the cross-stack contract, because a
Solidity custom error carries no message at all and a TypeScript client would invent its own
wording, so a vector file that pinned the wording could never be satisfied by all three stacks.

The codes are flat, one per rejection, so `Scalar` stays a Rust implementation detail:
`SIG_R_ZERO` and `SIG_S_ZERO` are separate codes rather than one code with a field, which would
force Solidity to encode which component failed.

**The prefixes are reserved now, before anything carries them.** One namespace per signed object
family, fixed while this ADR is being written rather than retrofitted later:

| Prefix | Family | Slice |
|---|---|---|
| `SIG_` | signature validation | #62 |
| `RCPT_` | `PositionReceipt` | #64 |
| `MID_` | `MarketId` | #65 |
| `ENV_` | 402 challenge and payment envelopes | #66 |

Each family publishes its own list, `ALL_ERROR_CODES` collects them, and one registry test asserts
that every code carries exactly one reserved prefix and that no code is used by two families.
Adding a namespace after Solidity and TypeScript both carry the codes would be a breaking change
in three stacks at once, which is the whole reason to spend the paragraph now.

From #67 the vector file carries the code, `contracts/src/TicklineTypes.sol` declares one custom
error per code, and the TypeScript client surfaces the code on its error. Changing a code is a
breaking change for every stack, so it takes its own ADR.

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
own error variant with a test that triggers it. `every_rejection_carries_its_stable_code` asserts
the table above, that the published list is exactly what the variants produce, and that no two
rejections share a code. From #67 the same cases are vectors that all three stacks run, and from
Phase 3 the vault runs them too.

## Update, 2026-09-24

Made at Jay's direction on #63. ADR-0001 says accepted ADRs are not edited. This section is added
below the original, which is left as it was, so the record of what was believed earlier on
2026-09-24 survives.

**Two more prefixes are reserved: `X402_` and `POLICY_`.** The full list is now `SIG_`, `RCPT_`,
`MID_`, `ENV_`, `X402_`, `POLICY_`.

They are reserved together because they separate two things that are easy to collapse and
expensive to confuse:

| Prefix | Means | Example |
|---|---|---|
| `X402_` | **malformed as an x402 object.** Any stack, and the escrow itself, would refuse it | `X402_WITHDRAW_DELAY_WIDTH`: a `withdrawDelay` that does not fit `uint40`, so it cannot be the config the contract hashed |
| `POLICY_` | **a valid object Tickline declines to serve.** The protocol is satisfied; we are not | `POLICY_WITHDRAW_DELAY_BELOW_FLOOR`: the escrow accepts any delay between 15 minutes and 30 days, and the 3600 floor is our decision from #8 |

Calling a policy refusal `X402_` tells a client the payment protocol rejected its channel, which is
false, and sends it to the wrong place to fix it. Phase 4 fills `POLICY_` further: a channel with a
pending withdrawal, a fill after the deadline, a fill below one base unit of shares, a cost above
the client's signed ceiling. None of those fit an existing family either, and by then three stacks
carry the codes.

**A family is scoped to the stacks that can produce it.** This is new, and it changes what the
registry asserts:

| Family | Rust | Solidity | TypeScript |
|---|---|---|---|
| `SIG_` | yes | yes | yes |
| `X402_` | yes | yes | yes |
| `POLICY_` | yes | no | yes |

The vault never enforces the 3600 floor, so no Solidity custom error exists for a `POLICY_` code,
and requiring one would fail Phase 3 against a rule nobody intended. `RCPT_`, `MID_` and `ENV_`
declare their reach when they are first used.

The registry test therefore checks reach per family rather than assuming every code is universal,
and #67's vector file records each family's reach so the Solidity and TypeScript sides know which
codes they are obliged to carry.
