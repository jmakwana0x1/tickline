# spec-notes: what the x402 batch-settlement scheme actually says

**Status: researched 2026-09-15, awaiting Jay's sign-off on the open questions in §8.**

`PHASES.md` phase 0 names the risk this file retires: *building against a misread x402 spec*.
Nothing in `engine/crates/protocol` or `contracts/src` may be written before the section it
depends on is filled in here and signed off (`CLAUDE.md` §2: guessing at the wire format is
banned).

Every claim below carries a source URL and the date it was read. Where the documentation is
ambiguous the ambiguity is recorded in §8 rather than resolved by picking the convenient
reading — the convenient reading is the one that loses money.

**Primary sources**, all read 2026-09-15:

| Source | URL |
|---|---|
| Scheme spec (abstract) | `https://github.com/x402-foundation/x402/blob/main/specs/schemes/batch-settlement/scheme_batch_settlement.md` |
| Scheme spec (EVM binding) | `https://github.com/x402-foundation/x402/blob/main/specs/schemes/batch-settlement/scheme_batch_settlement_evm.md` |
| Contract source | `https://github.com/x402-foundation/x402/blob/main/contracts/evm/src/x402BatchSettlement.sol` |
| Docs index | `https://docs.x402.org/llms.txt` |
| V1→V2 migration | `https://docs.x402.org/guides/migration-v1-to-v2.md` |
| Pyth `IPyth.sol` | `https://github.com/pyth-network/pyth-crosschain/blob/main/target_chains/ethereum/sdk/solidity/IPyth.sol` |

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
across chains. The config is **immutable** — changing any parameter means a new channel.

### EIP-712 types — verbatim from the contract (lines 92–107)

**Domain:** `EIP712("x402 Batch Settlement", "1")` — contract constructor, line 185.

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
(`0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003`) on 2026-09-15:

| Type hash | Value | |
|---|---|---|
| `VOUCHER_TYPEHASH` | `0x1e1bd6ff84c3e0d9029a292b212e039c0ca97ec497c55191a4a5874294609a69` | matches `keccak256` of the string above |
| `REFUND_TYPEHASH` | `0xbe23ad087435072cd69b49cc5b92de10ee610fce7ae0ab428307972d764bb216` | matches |
| `CHANNEL_CONFIG_TYPEHASH` | `0x1c9a06ceab9b0ebbd3301dc56c9111bb6d9af421356dc9ccb3b7084c755db308` | matches |

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
| **Deposit** | Client deposits via `eip3009` (`receiveWithAuthorization`, e.g. USDC) or `permit2` fallback. Gasless — sponsored by the facilitator. Channel is created implicitly on first deposit. |
| **Voucher** | Each paid request carries a cumulative voucher. |
| **Claim** | `claim(voucherClaims)` (`msg.sender` must be `receiver` or `receiverAuthorizer`) or `claimWithSignature(claims, signature)` (relay-friendly, anyone submits with a `ClaimBatch` signature from `receiverAuthorizer`). **No token transfer occurs** — accounting only. |
| **Settle** | `settle(receiver, token)` sweeps all claimed-but-unsettled funds in one transfer. **Permissionless.** |
| **Refund** | Cooperative: `refund(config, amount)` by receiver/receiverAuthorizer, or `refundWithSignature(...)`. Returns up to `balance - totalClaimed`. |
| **Withdraw** | Client escape hatch: `initiateWithdraw` → wait `withdrawDelay` → `finalizeWithdraw`. |

### Events

`ChannelCreated(channelId, config)` and `ChannelClosed(channelId, config)`.

> "Indexers must handle `ChannelCreated` firing more than once on the same `channelId` if the
> channel is re-funded after being fully drained."

**Direct consequence for Phase 5:** our indexer cannot treat `ChannelCreated` as a
create-once event.

---

## 2. The "up to" ceiling

**Confirmed: the ceiling is part of batch settlement itself.** `PHASES.md`'s assumption holds.
(`upto` is a *separate* scheme where value moves immediately per request — not what we use.)

> "The scheme supports **dynamic pricing**: the client authorizes a maximum per-request, and the
> server charges the actual cost within that ceiling."

The mechanism:

- The server tracks `chargedCumulativeAmount` — actual accumulated cost for the channel.
- For each request the client sets `voucher.maxClaimableAmount = chargedCumulativeAmount + amount`,
  where `amount` is `PaymentRequirements.amount`, the per-request maximum.
- The server verifies **exactly** that equality, and rejects with
  `invalid_batch_settlement_evm_cumulative_amount_mismatch` plus a corrective 402 otherwise.
- On success: `chargedCumulativeAmount += actualPrice`, where `actualPrice <= amount`.
- The unused remainder is never "released" — it simply never becomes part of the next voucher's
  base. There is no reservation to leak.

**This materially simplifies Tickline's I7.** There is no separate reserve-then-capture
bookkeeping to get wrong; the ceiling is implicit in the next voucher the client signs. What the
engine must do instead is exactly what the spec demands:

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

**Critical client rule** — and a good model for our agents:

> "PAYMENT-RESPONSE extra is untrusted. The client updates local state from its own previous
> state plus `extra.chargedAmount` […] It MUST NOT copy `extra.channelState` into that local
> state."

### Corrective 402

On cumulative mismatch the server returns `accepts[].extra.channelState` **and**
`accepts[].extra.voucherState` (`signedMaxClaimable` + `signature`) so the client can verify its
own prior signature before adopting the server's number. Tickline's API must implement this path
— it is how a desynced agent recovers.

---

## 4. Reference EVM contracts

Deployed to deterministic addresses via CREATE2 on every supported chain:

| Contract | Canonical address |
|---|---|
| `x402BatchSettlement` | `0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003` |
| `ERC3009DepositCollector` | `0x4020806089470a89826cB9fB1f4059150b550004` |
| `Permit2DepositCollector` | `0x4020425FAf3B746C082C2f942b4E5159887B0005` |

- **Builds under Foundry: yes.** `contracts/evm/` in the x402 repo *is* a Foundry project —
  `foundry.toml`, `remappings.txt`, `lib/forge-std`, `lib/openzeppelin-contracts`, `lib/permit2`,
  and a committed gas snapshot.
- **Audited**: three Cantina reports in `contracts/evm/audits/` (Feb, Mar, May 2026).
- Uses `ReentrancyGuardTransient` (EIP-1153), so it "must only be deployed on chains where that
  opcode is supported". Base Sepolia is Cancun-capable; our `foundry.toml` already sets
  `evm_version = "cancun"`.
- **We do not need to vendor or reimplement it** — it is deployed. We integrate against the
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

**Versions to pin: not yet determined** — see Q7.

---

## 6. `x402-rs`

- Repository `https://github.com/x402-rs/x402-rs`, crates.io `x402-rs` **0.12.5**, **Apache-2.0**.
- Publishes `x402-types`, `x402-axum`, `x402-reqwest`, `x402-facilitator-local`,
  `x402-chain-eip155`, `x402-chain-solana`.
- **Full x402 V2 support** in `x402-axum` and `x402-reqwest`.
- **Confirmed: it does not implement batch settlement.** Its roadmap lists `upto` and `deferred`
  as future work; batch settlement is not mentioned. `PHASES.md`'s assertion holds — the seller
  side is ours.
- `x402-types` is worth evaluating for the V2 envelope types rather than hand-rolling them.
  Apache-2.0 is compatible with our MIT. See Q8.

---

## 7. Pyth

- **`MockPyth` exists** and is usable: `target_chains/ethereum/sdk/solidity/MockPyth.sol`,
  `contract MockPyth is AbstractPyth`, constructor `(uint validTimePeriod, uint singleUpdateFeeInWei)`,
  with `createPriceFeedUpdateData(...)` for building fixtures. Good for Phase 3 and Phase 6.
- **`parsePriceFeedUpdatesUnique` — read the guarantee carefully.** Verbatim NatSpec:

  > "Similar to `parsePriceFeedUpdates` but ensures the updates returned are the first updates
  > published in minPublishTime. That is, if there are multiple updates for a given timestamp,
  > this method will return the first update."

  Signature:

  ```solidity
  function parsePriceFeedUpdatesUnique(
      bytes[] calldata updateData,
      bytes32[] calldata priceIds,
      uint64 minPublishTime,
      uint64 maxPublishTime
  ) external payable returns (PythStructs.PriceFeed[] memory priceFeeds);
  ```

  **This is not the guarantee `PHASES.md` assumes.** `PHASES.md` phase 0 asks whether it
  "guarantees the first update at or after a given timestamp". It does not. It guarantees
  uniqueness *at* `minPublishTime` — first update among those sharing one timestamp — while
  still accepting any update in `[minPublishTime, maxPublishTime]`. The **caller supplies
  `updateData`**, so with a wide window a resolver can choose which in-range update to submit.
  This is a resolution-manipulation surface and it is **Q1**, the most important open question
  here.

- `getUpdateFee(updateData)` returns the fee, which must be forwarded as `msg.value`.

---

## 8. Open questions

Each becomes a `needs-jay` issue. Phase 0 does not exit until every one is answered.

| # | Question | Blocks |
|---|---|---|
| **Q1** | **Pyth resolution window.** `parsePriceFeedUpdatesUnique` does not give "first update at or after the deadline" (§7). Setting `minPublishTime == maxPublishTime == deadline` makes resolution deterministic but reverts if no update carries exactly that timestamp — a liveness risk. A window makes it live but lets the submitter pick within it. Which trade, and what window? This decides whether resolution is manipulable. | Phase 3 |
| **Q2** | **Session model.** An x402 channel is `(payer, payerAuthorizer, receiver, receiverAuthorizer, token, withdrawDelay, salt)` and the receiver is fixed per channel. Tickline assumed "agent escrow" ≈ our own concept. Confirm that one agent ↔ one channel ↔ one Tickline session, and that `salt` is how an agent runs concurrent sessions. | Phase 4 |
| **Q3** | **`withdrawDelay` floor vs. epoch length.** The minimum is **15 minutes**. An agent can `initiateWithdraw` at any moment; unclaimed vouchers become unclaimable once `finalizeWithdraw` runs. Our settlement worker must therefore claim within the agent's chosen delay. Do we require a minimum `withdrawDelay` in our 402 (we set `extra.withdrawDelay`), and how does it relate to epoch length and `commitGrace`? | Phases 4, 5 |
| **Q4** | **I7 restatement.** The spec has no reserve/release step (§2), so I7's "outstanding reservations + captured cumulative" wording describes a mechanism that does not exist. Proposed replacement: *for every channel, `chargedCumulativeAmount <= signedMaxClaimable <= balance` at all times, and a fill is committed only after the resource handler succeeds.* Confirm. | Phase 4 |
| **Q5** | **`uint128`, not `uint256`.** Voucher amounts and claim totals are `uint128`. Do Tickline's own `PositionReceipt` amounts follow, or stay `uint256`? Mixing widths at a boundary is where truncation bugs live (I15). | Phase 2 |
| **Q6** | **Facilitator.** The spec assumes a facilitator for `/verify`, `/settle`, `/supported` and gasless deposits. Do we run our own (`x402-facilitator-local`), use a public one, or verify EOA vouchers locally (permitted when mirrored state is fresh)? This is a dependency and a trust decision. | Phases 4, 5 |
| **Q7** | Exact npm versions to pin for `@x402/*`. Not yet determined. | Phase 2 |
| **Q8** | Adopt `x402-types` (Apache-2.0) for V2 envelope types, or hand-roll in `protocol`? | Phase 2 |
| **Q9** | Invariants **I4, I5, I6** remain Claude's reconstruction (ADR-0002), unchanged by this research. | Phase 4 |

---

## 9. Things believed but not yet verified

| Belief | Why it matters | How to verify |
|---|---|---|
| USDC is 6 decimals on Base | every amount conversion (I15) | read the token contract on Base Sepolia |
| Solady's `expWad`/`lnWad` port to Rust unchanged | Phase 1's whole numerical method | port, then diff against `mpmath` at 60 digits |
| ~~`x402BatchSettlement` is deployed at the canonical address on Base Sepolia~~ | — | **verified 2026-09-15**: 22,353 bytes of code at that address, and all three type hashes read back matching (§1) |
| ~~Base Sepolia supports EIP-1153~~ | — | **implied verified**: the contract uses `ReentrancyGuardTransient` and is deployed and callable there |
