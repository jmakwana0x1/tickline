# ADR-0015: when four implementations disagree, the contract wins

- **Status:** accepted (Jay, on #67)
- **Date:** 2026-09-26
- **Phase:** 2
- **Invariants touched:** none directly; I7 and I12 both depend on our digests matching the escrow's

## Context

Phase 2's point is that Rust, Solidity, and TypeScript produce byte-identical digests. Three
implementations we wrote agreeing proves we are consistent with ourselves, which is worth much less
than it looks: a misreading of the x402 wire format would be copied faithfully into all three.

So S6 adds an opinion we did not write, the official SDK (`@x402/core` and `@x402/evm` at
`2.27.0`), and one that is not an implementation at all: the deployed escrow, read through its
public getters.

**Counting them honestly, there are three independent implementations, not four.** `cast` is
Foundry, Foundry is built on alloy, and `protocol` hashes with `alloy-primitives`, so `cast` and our
Rust are one source wearing two hats. The genuinely independent three are the **EVM** inside the
deployed contract, **viem's** JavaScript keccak in the SDK, and **alloy** in Rust and `cast`
together.

Four sources can disagree, and the moment to rank them is before anything is red.

## Decision

**Precedence, highest first:**

1. **The deployed contract**, read through its getters. It settles disputes with real money.
2. **The scheme spec text**, cited.
3. **The official SDK.**
4. **Our reading.**

If the SDK disagrees with a value read off the contract, the contract wins, the SDK is what is
wrong, and that is a `needs-jay` issue plus a report upstream. It is **never** a silent adjustment
to make S6 green. The same applies to the spec text: prose that contradicts the deployed bytecode
describes an intention, not the thing that will hold our collateral.

**Every vector entry records what confirmed it**, in a `confirmed_by` field:

| Tag | Meaning |
|---|---|
| `deployed-contract` | read from a getter on Base Sepolia, with the getter named |
| `sdk` | reproduced by `@x402/evm` |
| `spec` | quoted from the scheme spec, with no executable oracle |
| `ours` | Tickline's own type. Nothing external has an opinion |

This matters because the SDK does **not** bless everything. It has an opinion on the x402 envelopes
and on client-side voucher construction; it has none on `PositionReceipt` or `MarketId`, which are
ours. Without the tag a reader assumes the SDK validated our receipt type, and it did not.

**The oracle's version is recorded inside the file it produces.** `just vectors` guards against
drift by requiring an empty `git diff`, and that guard is blind to the case that matters: if a
dependency bump changes what the SDK emits, the generator and the committed file move together and
the diff stays empty. So the provenance records the `viem` and `@x402/*` versions, and a test
asserts the recorded versions equal the installed ones. A bump then fails loudly, and its PR has to
say why the values changed.

That is the same failure ADR-0010 exists for: a check that moves with the thing it checks proves
nothing.

## Consequences

Pins, recorded in `docs/STATUS.md`: `@x402/core` and `@x402/evm` at exactly `2.27.0`, `viem` at
exactly `2.56.9` in both `agents/` and `e2e/`. No range operators, per Q7 on #9.

`viem` could not be pinned at `2.27.0` alongside the SDK, although that version exists:
`@x402/evm@2.27.0` declares **`viem ^2.48.11`**, so the two numbers are unrelated despite matching.
Both are recorded here so the range is visible rather than inferred: the declared floor is
**2.48.11** and the pin is the **2.56.9** that resolved.

The pin is the resolved version rather than the floor deliberately. A declared range bound is not a
tested configuration: the SDK's own CI runs against something recent, so pinning its floor would
ship a combination nobody exercises, in exchange for a compatibility margin that an exact pin makes
pointless.

`zod` arrives as a transitive dependency of the SDK. Nothing of ours imports it, and neither it nor
`viem` reaches the engine: both live only in `agents/` and `e2e/`, which are client and test code.

### What actually happened on the first run

Every value agreed, so the ranking has not yet had to be used:

| Value | EVM (deployed contract) | alloy (our Rust, and `cast`) | viem (the SDK) |
|---|---|---|---|
| `channelId` for the fixture config | `0xbb834cd9…ca80` | same | same, via `computeChannelId` |
| voucher digest at 1 USDC | `0x99af2f50…f8ae` | same | same, via `voucherTypes` |
| two-entry claim batch digest | `0x364fdf2c…8e05` | same | same, via `claimBatchTypes` |

Three independent implementations agreeing on a **two-entry** batch digest is the strongest
verification this repository has, because that is the value with both an ordering and a
concatenation risk, and a one-element array would have hidden either.

### What three agreeing implementations did not catch

The first run of the generator produced a channel id no contract had ever returned, because the
generator used the correct direction (the agent pays, the operator receives) while the hand-written
fixtures had them reversed. Reading the escrow again for the generator's inputs fixed the values,
but the lesson is larger and is now `CLAUDE.md` §5: **a value confirmed by the contract is confirmed
as an encoding of the input you gave it, never as the right input.** `getChannelId` would have
confirmed the reversed channel just as happily, and every digest in the file would still have agreed
with every other, because they were all hashes over the same wrong input.

The answer is not another digest. It is a semantic assertion naming the parties
(`the_agent_pays_and_the_operator_receives`, in Rust and in TypeScript), and behaviour where the
escrow enforces it: only `payer` may withdraw, only `receiver` may claim, and `settle` pays the
receiver. A reversed-role test cannot pass those, which is the proof no hash comparison can give.
The earlier values, `0x5bc300d3…`, `0x0d64a8e6…` and `0xca9f200d…`, are recorded in
`docs/spec-notes.md` §1 as the swapped-role reads so nobody finds them in the history and trusts
them.

**`ClaimEntry` carries the derived `channelId`, not the `ChannelConfig`.** That travelled from an
inference, to a value read off `getClaimBatchDigest`, to a second implementation's type definition:
the SDK's `claimBatchTypes` declares `ClaimEntry[]` over `bytes32 channelId`. Three steps, and only
the last two are evidence.

## How this is enforced

`agents/src/vectors/generate.ts` writes `confirmed_by` per entry and the dependency versions into
the provenance. `agents/test/vectors.test.ts` asserts the recorded versions match the installed
tree, and the Rust, Solidity, and TypeScript readers all load the same file.
