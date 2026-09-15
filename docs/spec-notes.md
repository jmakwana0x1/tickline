# spec-notes: what the x402 batch-settlement scheme actually says

**Status: not started. This file blocks Phase 0's exit.**

`PHASES.md` phase 0 names the risk this file retires: *building against a misread x402 spec*.
Nothing in `engine/crates/protocol`, and nothing in `contracts/src`, may be written before the
section it depends on is filled in here and signed off by Jay (`CLAUDE.md` §2: guessing at the
wire format is banned).

## How to fill this in

Every claim below carries **a URL and the date it was read**. A claim without a citation is a
question, not a finding. Where the documentation is ambiguous, record the ambiguity as an open
question rather than picking the reading that is convenient — the convenient reading is the one
that loses money.

Where behaviour can be checked rather than read, check it: the reference contracts either build
under Foundry or they do not, and `parsePriceFeedUpdatesUnique` either guarantees the first
update at-or-after a timestamp or it does not.

---

## 1. Batch settlement: the scheme

_Source:_
_Read on:_

- Voucher EIP-712 typed data — exact field names, types, and **order** (order changes the hash):
- Escrow contract interface:
- Deposit semantics:
- Redemption semantics:
- Refund semantics:
- Withdrawal semantics:
- Session lifecycle, start to close:

## 2. The "up to" ceiling

The mechanism Tickline's pricing depends on. What exactly does the client sign per request, how
does the seller capture actual usage, and how does the cumulative amount advance when
`actual < ceiling`?

_Source:_
_Read on:_

- What the client signs per request:
- How the seller captures actual usage:
- How the cumulative advances when actual < ceiling:
- **Who eats the difference, and when is the remainder released:**
- What stops a seller from capturing the ceiling every time:

## 3. x402 V2 envelope

_Source:_
_Read on:_

- 402 challenge body format:
- Payment header names (exact casing):
- CAIP network identifiers used:
- Version negotiation, if any:

## 4. Reference EVM escrow contracts

_Source:_
_Read on:_

- Repository and commit to pin:
- Builds under Foundry? (checked, not assumed):
- Licence:
- Vendor or submodule:

## 5. Official TypeScript SDK

_Source:_
_Read on:_

- Package name(s) for batch settlement, client side:
- Version to pin:
- Does it sign vouchers itself or expect a signer callback:

## 6. `x402-rs`

_Source:_
_Read on:_

- V2 envelope types worth reusing:
- Licence and version:
- Confirmed: it does **not** implement batch settlement. The seller side is ours.

## 7. Pyth

_Source:_
_Read on:_

- `MockPyth` availability and package:
- `parsePriceFeedUpdatesUnique` (or its current equivalent) — **does it guarantee the first
  update at or after a given timestamp?** Resolution correctness rests on this:
- Update fee mechanics:
- Price feed IDs for the demo market:
- Confidence interval: does resolution use it, and how:

---

## Open questions

Each becomes a `needs-jay` issue and is listed in `docs/STATUS.md`. Phase 0 does not exit until
every one has Jay's answer.

| # | Question | Blocks |
|---|---|---|
|  |  |  |

---

## Things believed but not yet verified

Anything that reached the code from memory, intuition, or a blog post rather than from the
sources above belongs here until it is checked. This list should be empty when Phase 0 closes.

| Belief | Why it matters | How to verify |
|---|---|---|
| USDC is 6 decimals on Base | every amount conversion (I15) | read the token contract on Base Sepolia |
| Solady's `expWad`/`lnWad` are portable to Rust unchanged | Phase 1's whole numerical method | port, then diff against `mpmath` at 60 digits |
