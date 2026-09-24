# spec-notes: what the x402 batch-settlement scheme actually says

**Status: researched 2026-09-15. Every open question in §8 answered by Jay on 2026-09-17 (#6, #7, #8, #9).**

`PHASES.md` phase 0 names the risk this file retires: *building against a misread x402 spec*.
Nothing in `engine/crates/protocol` or `contracts/src` may be written before the section it
depends on is filled in here and signed off (`CLAUDE.md` §2: guessing at the wire format is
banned).

Every claim below carries a source URL and the date it was read. Where the documentation is
ambiguous the ambiguity is recorded in §8 rather than resolved by picking the convenient
reading. The convenient reading is the one that loses money.

**Primary sources**, read 2026-09-15 and re-checked 2026-09-17. Source files are pinned by commit:
x402 at `84ffb6412a1f2f45a62971c5549eff794281c099` (the only commit that has touched `x402BatchSettlement.sol`), Pyth at `807ff575a9090cee99b9e1a30dc23edf3522fe1b`.

| Source | URL |
|---|---|
| Scheme spec (abstract) | `https://github.com/x402-foundation/x402/blob/main/specs/schemes/batch-settlement/scheme_batch_settlement.md` |
| Scheme spec (EVM binding) | `https://github.com/x402-foundation/x402/blob/main/specs/schemes/batch-settlement/scheme_batch_settlement_evm.md` |
| Contract source | `https://github.com/x402-foundation/x402/blob/84ffb6412a1f2f45a62971c5549eff794281c099/contracts/evm/src/x402BatchSettlement.sol` |
| Docs index | `https://docs.x402.org/llms.txt` |
| V1→V2 migration | `https://docs.x402.org/guides/migration-v1-to-v2.md` |
| Pyth `IPyth.sol` | `https://github.com/pyth-network/pyth-crosschain/blob/807ff575a9090cee99b9e1a30dc23edf3522fe1b/target_chains/ethereum/sdk/solidity/IPyth.sol` |
| Pyth `Pyth.sol` | `https://github.com/pyth-network/pyth-crosschain/blob/807ff575a9090cee99b9e1a30dc23edf3522fe1b/target_chains/ethereum/contracts/contracts/pyth/Pyth.sol` |
| Pyth `MockPyth.sol` | `https://github.com/pyth-network/pyth-crosschain/blob/807ff575a9090cee99b9e1a30dc23edf3522fe1b/target_chains/ethereum/sdk/solidity/MockPyth.sol` |

**Lesson from this research, twice over: read the code, not the comment.** The `IPyth.sol` NatSpec
understates what `parsePriceFeedUpdatesUnique` guarantees (§7), and the `x402BatchSettlement`
header comment (line 18) says `channelId = keccak256(abi.encode(channelConfig))` while the code
(line 445) computes the EIP-712 hash. Where a comment and the code disagree, this file cites the
code.

---

## 1. Batch settlement: the scheme

It is a **stateless unidirectional payment channel**, not a generic escrow. This matters for
Tickline's mental model: there is one channel per `(payer, payerAuthorizer, receiver,
receiverAuthorizer, token, withdrawDelay, salt)` tuple, and Tickline's "agent session" *is* an
x402 channel.

> "Clients deposit funds into onchain channels once and sign off-chain **cumulative vouchers**
> per request. Servers verify vouchers with fast signature checks and claim them onchain
> periodically in batches […] A single claim transaction can cover many channels at once and
> only updates onchain accounting; claimed funds are later transferred to the receiver via a
> separate settle operation."

### Channel identity

```solidity
struct ChannelConfig {
    address payer;              // Client wallet (EOA or smart wallet)
    address payerAuthorizer;    // EOA for voucher signing, or address(0) for EIP-1271 via payer
    address receiver;           // Server's payment destination
    address receiverAuthorizer; // Authorizes claims and refunds via EIP-712 signatures
    address token;              // ERC-20 payment token
    uint40  withdrawDelay;      // Seconds before timed withdrawal completes (15 min – 30 days)
    bytes32 salt;               // Differentiates channels with identical parameters
}
```

`channelId = EIP712Hash(ChannelConfig)` under the `x402 Batch Settlement` domain. The hash binds
config to `chainId` and the deployed contract address, so the same config yields different IDs
across chains. The config is **immutable**: changing any parameter means a new channel.

### EIP-712 types, verbatim from the contract (lines 92 to 107)

**Domain:** `EIP712("x402 Batch Settlement", "1")`, contract constructor, line 185.

```
ChannelConfig(address payer,address payerAuthorizer,address receiver,address receiverAuthorizer,address token,uint40 withdrawDelay,bytes32 salt)

Voucher(bytes32 channelId,uint128 maxClaimableAmount)

Refund(bytes32 channelId,uint256 nonce,uint128 amount)

ClaimEntry(bytes32 channelId,uint128 maxClaimableAmount,uint128 totalClaimed)

ClaimBatch(ClaimEntry[] claims)ClaimEntry(bytes32 channelId,uint128 maxClaimableAmount,uint128 totalClaimed)
```

**The voucher is two fields.** No amount-per-request, no expiry, no nonce, no deadline. Note
`maxClaimableAmount` is **`uint128`**, not `uint256`.

**Verified against the deployed bytecode**, not just the source, on Base Sepolia
(`0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003`). The first three on 2026-09-15, the last two on
2026-09-24: **all five are public constants with generated getters**, so every type hash can be
read straight off the deployment rather than recomputed.

| Getter | Value | |
|---|---|---|
| `CHANNEL_CONFIG_TYPEHASH()` | `0x1c9a06ceab9b0ebbd3301dc56c9111bb6d9af421356dc9ccb3b7084c755db308` | matches `keccak256` of the string above |
| `VOUCHER_TYPEHASH()` | `0x1e1bd6ff84c3e0d9029a292b212e039c0ca97ec497c55191a4a5874294609a69` | matches |
| `REFUND_TYPEHASH()` | `0xbe23ad087435072cd69b49cc5b92de10ee610fce7ae0ab428307972d764bb216` | matches |
| `CLAIM_ENTRY_TYPEHASH()` | `0x30b9a0367528fca13c3e8ec6134c3113f1887ebe7a1ad61de0955aac288b0cf5` | matches |
| `CLAIM_BATCH_TYPEHASH()` | `0x42bd57c010b870c2e96203c66341f28a6a19b4d56a74533a8eb5f706b9fd6d57` | matches |

Reading a constant proves the contract's own opinion of its type hash. Recomputing proves only
that two implementations of `keccak256` agree, which is why #67's fork suite reads the getters.

**`Refund` is deliberately not in `testdata/vectors/eip712.json`.** Four of these five types are:
`ChannelConfig`, `Voucher`, `ClaimEntry`, `ClaimBatch`. Nothing between Phase 2 and Phase 5 signs a
refund, and adding the fifth later is additive: a type string, a vector group, and no change to
anything existing. **#73** is the Phase 5 slice that adds it, and the research is already done, so
a reader who counts four types here and five on the contract is looking at a decision rather than
an omission.

### Three digest getters, and the wire struct

| Getter | Returns |
|---|---|
| `getChannelId(ChannelConfig)` | the channel id, the config's digest under this domain |
| `getVoucherDigest(bytes32,uint128)` | the digest a payer signs |
| `getClaimBatchDigest(VoucherClaim[])` | the digest a receiver authorizer signs over a whole batch |
| `getRefundDigest(bytes32,uint256,uint128)` | the refund digest, for #73 |

`getClaimBatchDigest` takes a wire struct that is **not** the signed type, and the difference
matters when Phase 3 and Phase 5 build it:

```solidity
VoucherClaim {
    Voucher { ChannelConfig config; uint128 maxClaimableAmount } voucher;
    bytes    signature;
    uint128  totalClaimed;
}
```

Two things follow. It carries the **`ChannelConfig`** and derives the `channelId` itself, so the
wire struct nests one level deeper than the signed `Voucher`, while the `ClaimEntry` that is
hashed is one level flatter. And the third field is **`totalClaimed`**, the cumulative total the
`ClaimEntry` type string names, **never a per-batch delta**: the contract reads
`voucherClaims[i].totalClaimed` when it builds the entry hash, and vouchers supersede each other
precisely because that number only rises. Calling it an "amount" anywhere in our code or docs
invites a delta into the slot where the running total belongs, which is how I4 and I13 fail
quietly.

Confirmed on 2026-09-24 against the deployment, with a two-entry batch rather than one, because a
single-element array hides an ordering or concatenation mistake:

| Value | Read from the contract |
|---|---|
| `getChannelId` for the fixture config | `0x5bc300d3a7e3ae87ac56379e964001d3f03d366f76fe8b4347a95f5e02bdbbab` |
| `getVoucherDigest(channelId, 1000000)` | `0x0d64a8e669f88cd5e7d086129c5a8c02760ccb5f14d2f8c41496bab83e4e9cea` |
| `getClaimBatchDigest` over two entries | `0xca9f200d7e8c61c6c37fafdbcf046e8c258d7b6e17c0e72b425ea768bcf62a81` |

Each equals what `engine/crates/protocol` computes, and the fixtures are in
`testdata/vectors/eip712-primitives.json` with the getter each came from.

Reproduce with:

```bash
cast call 0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003 "VOUCHER_TYPEHASH()(bytes32)" \
  --rpc-url https://sepolia.base.org
cast keccak "Voucher(bytes32 channelId,uint128 maxClaimableAmount)"
```

These three values are the seed for `testdata/vectors/eip712.json` in Phase 2.

> "The cumulative model makes nonces unnecessary. As `totalClaimed` only increases, […] old
> vouchers are naturally superseded."

### Channel state, onchain

Two values: `balance` (deposited minus withdrawals and refunds) and `totalClaimed`. Available
escrow is `balance - totalClaimed`.

### Lifecycle

| Phase | Mechanism |
|---|---|
| **Deposit** | Client deposits via `eip3009` (`receiveWithAuthorization`, e.g. USDC) or `permit2` fallback. Gasless, sponsored by the facilitator (Tickline uses none; see §3). Channel is created implicitly on first deposit. |
| **Voucher** | Each paid request carries a cumulative voucher. |
| **Claim** | `claim(voucherClaims)` (`msg.sender` must be `receiver` or `receiverAuthorizer`) or `claimWithSignature(claims, signature)` (relay-friendly, anyone submits with a `ClaimBatch` signature from `receiverAuthorizer`). **No token transfer occurs**; accounting only. |
| **Settle** | `settle(receiver, token)` sweeps all claimed-but-unsettled funds in one transfer. **Permissionless.** |
| **Refund** | Cooperative: `refund(config, amount)` by receiver/receiverAuthorizer, or `refundWithSignature(...)`. Returns up to `balance - totalClaimed`. |
| **Withdraw** | Client escape hatch: `initiateWithdraw` → wait `withdrawDelay` → `finalizeWithdraw`. |

### Withdrawal bounds and mechanics

`MIN_WITHDRAW_DELAY = 15 minutes` and `MAX_WITHDRAW_DELAY = 30 days` (lines 85 to 86), enforced
at deposit time (line 210). Tickline advertises and requires `withdrawDelay = 3600` (#8).

`initiateWithdraw` (lines 313 to 344) computes available escrow from onchain `totalClaimed`
only. Its NatSpec, lines 320 to 323:

> Available liquidity is `ch.balance - ch.totalClaimed` using **only** on-chain `totalClaimed`.
> Payer-signed vouchers that are not yet claimed do **not** reserve funds; receivers relying on
> voucher value must treat inclusion of `claim` during the withdrawal window as an operational
> requirement.

`finalizeWithdraw` caps the payout at `balance - totalClaimed` as of finalization (lines 370 to
371). So a claim that lands before `finalizeWithdraw` wins, and vouchers accepted between
`initiateWithdraw` and our detection of it are still collectable if we claim in time. This is why
I7 forbids accepting vouchers on a channel with a pending withdrawal and sets a claim deadline.

### Events

Eight, all at lines 129 to 150 of the pinned source:

```solidity
event ChannelCreated(bytes32 indexed channelId, ChannelConfig config);
event ChannelClosed(bytes32 indexed channelId, ChannelConfig config);
event Deposited(bytes32 indexed channelId, address indexed sender, uint128 amount, uint128 newBalance);
event Claimed(bytes32 indexed channelId, address indexed sender, uint128 claimAmount, uint128 newTotalClaimed);
event Settled(address indexed receiver, address indexed token, address indexed sender, uint128 amount);
event Refunded(bytes32 indexed channelId, address indexed sender, uint128 amount);
event WithdrawInitiated(bytes32 indexed channelId, uint128 amount, uint40 finalizeAfter);
event WithdrawFinalized(bytes32 indexed channelId, address indexed sender, uint128 amount);
```

Public state readable by the indexer: `channels(channelId)` (line 113) and
`pendingWithdrawals(channelId)` (line 121).

**`WithdrawInitiated` is a priority event for Tickline** (#8): the indexer surfaces it, the session
actor stops accepting vouchers on that channel, and the settlement worker claims immediately.

An earlier version of this section listed only `ChannelCreated` and `ChannelClosed`, copied from
the spec prose. Jay caught it; the list above is from the contract.

> "Indexers must handle `ChannelCreated` firing more than once on the same `channelId` if the
> channel is re-funded after being fully drained."

**Direct consequence for Phase 5:** our indexer cannot treat `ChannelCreated` as a
create-once event.

---

## 2. The "up to" ceiling

**Confirmed: the ceiling is part of batch settlement itself.** `PHASES.md`'s assumption holds.
(`upto` is a *separate* scheme where value moves immediately per request, and not what we use.)

> "The scheme supports **dynamic pricing**: the client authorizes a maximum per-request, and the
> server charges the actual cost within that ceiling."

The mechanism:

- The server tracks `chargedCumulativeAmount`, the actual accumulated cost for the channel.
- For each request the client sets `voucher.maxClaimableAmount = chargedCumulativeAmount + amount`,
  where `amount` is `PaymentRequirements.amount`, the per-request maximum.
- The server verifies **exactly** that equality, and rejects with
  `invalid_batch_settlement_evm_cumulative_amount_mismatch` plus a corrective 402 otherwise.
- On success: `chargedCumulativeAmount += actualPrice`, where `actualPrice <= amount`.
- The unused remainder is never "released"; it simply never becomes part of the next voucher's
  base. There is no reservation to leak.

**This materially simplifies Tickline's I7**, which was rewritten on this basis (#7, `CLAUDE.md`
§4). There is no separate reserve-then-capture bookkeeping to get wrong; the ceiling is implicit
in the next voucher the client signs. What the engine must do instead is exactly what the spec
demands:

> "The server must serialize request processing per channel and must not update voucher state
> until the resource handler has succeeded."

That is our session actor, and the spec independently requires it. On failure: "State unchanged,
client can retry the same voucher."

### Who eats the difference

Nobody. The server claims at most `maxClaimableAmount`; over-claiming within that bound is
possible and is explicitly a **trust** matter, not a protocol one:

> "Clients bear risk up to the signed `maxClaimableAmount`; the receiver authorizer determines
> actual `totalClaimed` onchain within that bound. Over-claiming is a trust violation, not a
> protocol violation."

### Vouchers never expire

> "Vouchers carry no expiry field. A voucher remains claimable as long as
> `balance - totalClaimed > 0`; `finalizeWithdraw` and `refundWithSignature` close the claim
> window by draining available escrow."

---

## 3. x402 V2 envelope

| Thing | V1 | **V2** |
|---|---|---|
| Request header | `X-PAYMENT` | **`PAYMENT-SIGNATURE`** |
| Response header | `X-PAYMENT-RESPONSE` | **`PAYMENT-RESPONSE`** |
| Network id | `base-sepolia` | **CAIP-2: `eip155:84532`** |
| Envelope version | `x402Version: 1` | **`x402Version: 2`** |

Base mainnet is `eip155:8453`; Base Sepolia `eip155:84532`.

**Tickline runs no facilitator (Q6).** The spec describes `/verify`, `/settle` and `/supported` on
a facilitator, and sponsored deposits through one. Tickline does all of it in the engine:
vouchers are verified locally, deposits go through the canonical `ERC3009DepositCollector`
under an outbox intent, and claims are sent with the `receiverAuthorizer` key. The wire format
toward clients is unchanged, which is what lets the official TS client work as-is.

### 402 challenge body

```json
{
  "scheme": "batch-settlement",
  "network": "eip155:8453",
  "amount": "100000",
  "asset": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
  "payTo": "0xServerReceiverAddress",
  "maxTimeoutSeconds": 3600,
  "extra": {
    "receiverAuthorizer": "0xReceiverAuthorizerAddress",
    "withdrawDelay": 900,
    "name": "USDC",
    "version": "2",
    "minDeposit": "1000000"
  }
}
```

`extra.receiverAuthorizer`, `extra.withdrawDelay`, `extra.name`, `extra.version` are **required**;
`assetTransferMethod`, `minDeposit`, `channelState`, `voucherState` are optional.

### Payment payload

Three types: `deposit`, `voucher`, `refund`. Each carries the full `channelConfig` plus
`voucher: { channelId, maxClaimableAmount, signature }`.

### Payment response

Voucher-only response has `transaction: ""` and `amount: ""`, with the real figure in
`extra.chargedAmount` and a snapshot in `extra.channelState`.

**Critical client rule**, and a good model for our agents:

> "PAYMENT-RESPONSE extra is untrusted. The client updates local state from its own previous
> state plus `extra.chargedAmount` […] It MUST NOT copy `extra.channelState` into that local
> state."

### Corrective 402

On cumulative mismatch the server returns `accepts[].extra.channelState` **and**
`accepts[].extra.voucherState` (`signedMaxClaimable` + `signature`) so the client can verify its
own prior signature before adopting the server's number. Tickline's API must implement this path,
because it is how a desynced agent recovers.

---

## 4. Reference EVM contracts

Deployed to deterministic addresses via CREATE2 on every supported chain:

| Contract | Canonical address |
|---|---|
| `x402BatchSettlement` | `0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003` |
| `ERC3009DepositCollector` | `0x4020806089470a89826cB9fB1f4059150b550004` |
| `Permit2DepositCollector` | `0x4020425FAf3B746C082C2f942b4E5159887B0005` |

- **Builds under Foundry: yes.** `contracts/evm/` in the x402 repo *is* a Foundry project:
  `foundry.toml`, `remappings.txt`, `lib/forge-std`, `lib/openzeppelin-contracts`, `lib/permit2`,
  and a committed gas snapshot.
- **Audited**: three Cantina reports in `contracts/evm/audits/` (Feb, Mar, May 2026).
- Uses `ReentrancyGuardTransient` (EIP-1153), so it "must only be deployed on chains where that
  opcode is supported". Base Sepolia is Cancun-capable; our `foundry.toml` already sets
  `evm_version = "cancun"`.
- **We do not need to vendor or reimplement it**: it is deployed. We integrate against the
  canonical address and use a local deployment only for anvil tests.

---

## 5. Official TypeScript SDK

Packages named on the batch-settlement docs page:

| Package | Role |
|---|---|
| `@x402/core/server` | server core |
| `@x402/evm/batch-settlement/server` | server-side batch settlement |
| `@x402/evm/batch-settlement/client` | client-side batch settlement |
| `@x402/fetch` | client fetch wrapper |
| `@x402/express` | server middleware |
| `@x402/evm` | EVM binding |

Client examples also use `viem`. Go and Python clients exist, so the scheme is not TS-only.

**Versions:** exact, no `^` or `~`, pinned in the Phase 2 PR that first adds `@x402/*` and recorded
in `docs/STATUS.md` (Q7).

---

## 6. `x402-rs`

- Repository `https://github.com/x402-rs/x402-rs`, crates.io `x402-rs` **0.12.5**, **Apache-2.0**.
- Publishes `x402-types`, `x402-axum`, `x402-reqwest`, `x402-facilitator-local`,
  `x402-chain-eip155`, `x402-chain-solana`.
- **Full x402 V2 support** in `x402-axum` and `x402-reqwest`.
- **Confirmed: it does not implement batch settlement.** Its roadmap lists `upto` and `deferred`
  as future work; batch settlement is not mentioned. `PHASES.md`'s assertion holds: the seller
  side is ours.
- **Not adopted (Q8).** `protocol` writes its own V2 envelope types: they are a handful of
  structs, the batch-settlement `extra` fields are not in `x402-types`, and `protocol` stays
  zero-IO with minimal dependencies. The guard against drift is the Phase 2 cross-stack vector
  file generated by the official TS SDK, not a shared crate.

---

## 7. Pyth

- **`MockPyth` exists** and is usable: `target_chains/ethereum/sdk/solidity/MockPyth.sol`,
  `contract MockPyth is AbstractPyth`, constructor `(uint validTimePeriod, uint singleUpdateFeeInWei)`,
  with `createPriceFeedUpdateData(...)` for building fixtures. Good for Phase 3 and Phase 6.
- **`parsePriceFeedUpdatesUnique` gives exactly the guarantee `PHASES.md` assumed.** The
  `IPyth.sol` NatSpec is misleading; the implementation is stronger than it reads.

  Pinned to `pyth-crosschain` commit **`807ff575a9090cee99b9e1a30dc23edf3522fe1b`**, read
  2026-09-15.

  `target_chains/ethereum/contracts/contracts/pyth/Pyth.sol` **lines 612 to 631**:
  `parsePriceFeedUpdatesUnique` forwards to `parsePriceFeedUpdatesWithConfig` with
  `checkUniqueness = true`:

  ```solidity
  (priceFeeds, ) = parsePriceFeedUpdatesWithConfig(
      updateData, priceIds, minPublishTime, maxPublishTime,
      true,   // checkUniqueness
      false, false
  );
  ```

  Same file, **lines 240 to 246**: an update is accepted only when:

  ```solidity
  publishTime >= context.minAllowedPublishTime &&
  publishTime <= context.maxAllowedPublishTime &&
  (!context.checkUniqueness ||
      context.minAllowedPublishTime > prevPublishTime)
  ```

  `prevPublishTime` is the publish time of the immediately preceding update for that feed, and it
  is **part of the signed Merkle payload** (`extractPriceInfoFromMerkleProof`, lines 224 to 229), so a
  submitter cannot forge it. So with `minPublishTime = deadline`, the only update that passes is
  the one whose predecessor was published *before* the deadline: **the first update at or after
  the deadline**.

  The caller supplies `updateData`, but **exactly one update per feed can satisfy the check, at
  any window size**. Submitter discretion is zero. The window bounds liveness during feed gaps
  and nothing else.

  `target_chains/ethereum/sdk/solidity/MockPyth.sol` **line 133** applies the same condition
  (`prevPublishTime < minAllowedPublishTime`), and `createPriceFeedUpdateData` takes
  `prevPublishTime` as its eighth argument (**lines 281 to 290**), so every case is testable on
  anvil without a network.

  **Resolution rule decided: see ADR-0005.**

- `getUpdateFee(updateData)` returns the fee, which must be forwarded as `msg.value`.

---

## 8. Open questions

All nine were answered by Jay on 2026-09-17. The answers are recorded here so later phases
build against the decision, not the question.

| # | Question | Decision | Where |
|---|---|---|---|
| Q1 | Pyth resolution window | `parsePriceFeedUpdatesUnique(updateData, [feedId], deadline, deadline + 60)`. The uniqueness check admits exactly one update per feed, so there is no manipulation surface; the 60 s window only bounds liveness and is a vault constant. My original reading was wrong: I took the `IPyth.sol` NatSpec literally and did not read `Pyth.sol`. Resolution is permissionless from the deadline, ignores the confidence interval, and has a void path at `deadline + 24h`. | #6, ADR-0005, §7 |
| Q2 | Session model | One x402 channel is one session; agents run concurrent sessions by varying `salt`. Positions and receipts are keyed by `(market, payer)`, never by `channelId`. Only EOA `payerAuthorizer` values are accepted; `payerAuthorizer == address(0)` (EIP-1271) is rejected with a typed error, so voucher verification is a pure signature check with no RPC call. | #9 |
| Q3 | `withdrawDelay` vs epochs | No coupling exists: epoch commits go to `TicklineVault`, claims go to `x402BatchSettlement`. The 402 advertises `extra.withdrawDelay = 3600`, and the engine rejects vouchers on any channel with `withdrawDelay < 3600`. `WithdrawInitiated` is a priority event: the session actor stops accepting vouchers on that channel and the settlement worker claims it immediately under an outbox intent. An alert fires if the claim is not confirmed by `finalizeAfter - 1800`. | #8, §1 |
| Q4 | I7 restatement | Rewritten in `CLAUDE.md` §4, with two additions beyond the proposal: no pending withdrawal, and `balance`/`totalClaimed` read at confirmation depth. **I16 (money before shares)** added alongside it. | #7 |
| Q5 | `uint128` vs `uint256` | Every amount and share count in `PositionReceipt` is `uint128` (`u128` in Rust): `yesShares`, `noShares`, `costPaid`, `feesPaid`. No width conversion at the escrow boundary. LMSR math stays signed WAD inside `lmsr`; narrowing to `u128` happens only at the `lmsr` boundary, checked, with I15 rounding. | #9 |
| Q6 | Facilitator | None: not our own, not `x402-facilitator-local`, not a public one. See §3. Running or depending on a facilitator is out of scope. | #9 |
| Q7 | npm pins | Exact versions, no `^` or `~`, pinned in the Phase 2 PR that first adds `@x402/*`. | #9 |
| Q8 | `x402-types` | Not adopted; `protocol` writes its own V2 envelope types. See §6. | #9 |
| Q9 | I4, I5, I6 | Confirmed as written. ADR-0002 updated. | #7 |

---

## 9. Things believed but not yet verified

| Belief | Why it matters | How to verify |
|---|---|---|
| USDC is 6 decimals on Base | every amount conversion (I15) | read the token contract on Base Sepolia |
| Solady's `expWad`/`lnWad` port to Rust unchanged | Phase 1's whole numerical method | port, then diff against `mpmath` at 60 digits |
| ~~`x402BatchSettlement` is deployed at the canonical address on Base Sepolia~~ | n/a | **verified 2026-09-15**: 22,353 bytes of code at that address, and all three type hashes read back matching (§1) |
| ~~Base Sepolia supports EIP-1153~~ | n/a | **implied verified**: the contract uses `ReentrancyGuardTransient` and is deployed and callable there |

---

## Appendix: the deployment's selectors

Recovered from the runtime bytecode of `0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003` on Base
Sepolia on 2026-09-24 and resolved against the openchain signature database. All 25 resolved.

This is committed because it is evidence about the deployment rather than about the spec, and
because recovering it cost real time: `getClaimBatchDigest`'s argument tuple could not be guessed,
and four plausible shapes all reverted identically. #67's fork suite asserts each selector it
calls, so a redeployed escrow with a changed signature fails by name instead of as a bare revert.

| Selector | Signature |
|---|---|
| `0x8e4a68ac` | `CHANNEL_CONFIG_TYPEHASH()` |
| `0x7a7ebd7b` | `channels(bytes32)` |
| `0x29237b0c` | `claim((((address,address,address,address,address,uint40,bytes32),uint128),bytes,uint128)[])` |
| `0x53ab0fae` | `CLAIM_BATCH_TYPEHASH()` |
| `0xba94ec0d` | `CLAIM_ENTRY_TYPEHASH()` |
| `0xe43ce1f2` | `claimWithSignature((((address,address,address,address,address,uint40,bytes32),uint128),bytes,uint128)[],bytes)` |
| `0x140f1e75` | `deposit((address,address,address,address,address,uint40,bytes32),uint128,address,bytes)` |
| `0x84b0196e` | `eip712Domain()` |
| `0xe88377b1` | `finalizeWithdraw((address,address,address,address,address,uint40,bytes32))` |
| `0x5e5e0b87` | `getChannelId((address,address,address,address,address,uint40,bytes32))` |
| `0x488ccc3b` | `getClaimBatchDigest((((address,address,address,address,address,uint40,bytes32),uint128),bytes,uint128)[])` |
| `0xe25cf189` | `getRefundDigest(bytes32,uint256,uint128)` |
| `0x862bb199` | `getVoucherDigest(bytes32,uint128)` |
| `0xcf5cf3dc` | `initiateWithdraw((address,address,address,address,address,uint40,bytes32),uint128)` |
| `0x1fc3277d` | `MAX_WITHDRAW_DELAY()` |
| `0xae159439` | `MIN_WITHDRAW_DELAY()` |
| `0xac9650d8` | `multicall(bytes[])` |
| `0xb7f06ebe` | `pendingWithdrawals(bytes32)` |
| `0x21ff6389` | `receivers(address,address)` |
| `0xdce4bfae` | `refund((address,address,address,address,address,uint40,bytes32),uint128)` |
| `0x1e69ef40` | `REFUND_TYPEHASH()` |
| `0xf0dc792e` | `refundNonce(bytes32)` |
| `0xb77433e9` | `refundWithSignature((address,address,address,address,address,uint40,bytes32),uint128,uint256,bytes)` |
| `0x9db32a8f` | `settle(address,address)` |
| `0x94739e87` | `VOUCHER_TYPEHASH()` |
