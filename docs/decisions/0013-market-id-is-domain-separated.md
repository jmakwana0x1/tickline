# ADR-0013: the market id is domain separated

- **Status:** accepted (Jay, D3 on #59)
- **Date:** 2026-09-26
- **Phase:** 2
- **Invariants touched:** none directly; I10 and I11 are both keyed by the market id, so a collision breaks them at once

## Context

`PHASES.md` phase 2 proposes

```
MarketId = keccak256(abi.encode(creator, templateId, templateParamsHash, deadline, b, epochLength, salt))
```

That preimage has no domain separation. Two consequences follow, and both are exploitable rather
than merely untidy:

- **Across deployments.** The same parameters produce the same id on a testnet vault and on its
  replacement. A `PositionReceipt` signed for market `X` on the old vault is a valid receipt for
  market `X` on the new one, so a settled or abandoned market can be replayed against a live one.
- **Across chains.** Identical parameters give one id on every chain, so the same argument applies
  between a testnet and mainnet deployment.

The EIP-712 domain protects the *receipt* against both, because it binds `chainId` and
`verifyingContract`. It does not protect the market **id**, which is an ordinary `keccak256`
preimage and is the key every position, commit, and claim is filed under. The x402 escrow binds
`channelId` to its own domain for exactly this reason (`docs/spec-notes.md` §1).

## Decision

The preimage gains a tag, the chain, and the vault:

```
MarketId = keccak256(abi.encode(
  MARKET_ID_TAG,            // bytes32
  uint256 chainId,
  address vault,            // the verifyingContract of the Tickline domain
  address creator,
  bytes32 templateId,
  bytes32 templateParamsHash,
  uint64  deadline,         // unix seconds
  uint128 b,                // WAD, matching lmsr's domain
  uint32  epochLength,      // seconds
  bytes32 salt
))
```

`abi.encode` pads every value to 32 bytes, so the widths bind the call sites rather than the
encoding. They are still worth stating: `b` is a WAD and `uint128` matches the receipt's amounts,
`deadline` is `uint64` seconds, and `epochLength` is `uint32` seconds.

**`MARKET_ID_TAG` is the string as a `bytes32` literal**, right-padded with zeros:

```
"Tickline MarketId v1" = 0x5469636b6c696e65204d61726b65744964207631000000000000000000000000
```

The alternative was `keccak256("Tickline MarketId v1")` =
`0xb9f45e5973771fa38a5985ebb20dd891bcc39d62f9435beac6de57cfe49d204f`. The literal wins on one
argument: a preimage is something a human reads when a hash disagrees, and a `cast abi-encode` dump
shows the literal as text while the keccak shows 32 opaque bytes. The preimage is hashed anyway, so
pre-hashing the tag adds nothing. The string is 20 characters, so nothing is truncated. Recorded
here because it is the kind of choice a future reader would otherwise have to reverse-engineer,
and because reversing it later means regenerating vectors and changing a Solidity constant.

The `v1` in the tag is deliberate. If the preimage ever changes shape, the tag changes with it and
old ids cannot collide with new ones.

### The MVP template

`templateId = keccak256("pyth-threshold-v1")` =
`0x0bb1a797d3ee9dfb409f0f7dfa94ff9b9be705e41cad289f02f39a64cfa6a31e`, and
`templateParamsHash = keccak256(abi.encode(priceId, threshold, direction))` where `priceId` is the
Pyth feed id, `threshold` an `int64` in the feed's exponent, and `direction` a `uint8` (`0` above,
`1` below). Resolution is machine-only (ADR-0005), so the parameters are exactly what the
resolution rule reads.

## Consequences

`MarketId` can only be computed by someone who knows which chain and which vault they are talking
about, which is the point. A client that hardcodes a market id is hardcoding a deployment.

Phase 3's vault computes the same preimage, so `contracts/src/TicklineTypes.sol` holds the tag and
the field order (D4), and #67's cross-stack vectors prove all three stacks agree.

`abi.encodePacked` is banned anywhere near a hash preimage: it does not pad, so two different field
tuples can produce identical bytes. The Solidity lint enforces it from this slice.

## How this is enforced

`engine/crates/protocol/tests/market_id.rs` asserts the committed vectors, one case per field, and
the two cases D3 exists for: identical parameters under a different chain id and a different vault
must give different ids. The `encodePacked` ban has a planted-use test.
