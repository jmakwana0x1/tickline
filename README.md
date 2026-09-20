# Tickline

A prediction market priced **per HTTP request** and paid with **x402 batch settlement**.

An AI agent pays a fraction of a cent per request to take a position on a future price. Thousands
of those requests settle onchain as a handful of transactions, and every agent leaves each request
holding a signed receipt that beats the operator in a dispute.

> **Status: Phase 1 of 9 complete.** The pricing math is done, differentially tested against a
> 60-digit reference oracle. Nothing here prices real money yet. [`PHASES.md`](PHASES.md) is the
> build plan and [`docs/STATUS.md`](docs/STATUS.md) is where the build actually is.

---

## The idea

### Machines want to bet, and cannot afford to

A prediction market is a good instrument for a machine. It turns a belief about the future into a
single number, it settles against a fact rather than an opinion, and it needs no taste. An agent
that wants exposure to "is ETH above $4,000 on Friday" does not want a chart or a trading screen.
It wants a price and a way to pay for it.

Onchain markets serve that agent badly. Every trade is a transaction: sign it, broadcast it, wait
for a block, and pay gas priced for humans making occasional decisions rather than machines making
constant small ones. A fifty cent position that costs thirty cents to enter is not a position, it
is a donation. The mechanics of trading swamp the trade.

The obvious fix, moving the market offchain, trades one problem for a worse one. Now an operator
holds the prices, the balances, and the payouts, and the agent holds a promise.

### Paying for a request, not for a transaction

Paying per HTTP request has the same shape of problem, and it recently got an answer. **x402**
revives HTTP's long unused `402 Payment Required`: the server answers an unpaid request with a
challenge describing exactly what payment it accepts, the client retries with a signed payment
attached, and the server serves the response. No account, no invoice, no card.

Its **batch settlement** scheme is the part that matters here. Instead of moving value per
request, the client opens a session once by depositing USDC into an escrow contract, and then
signs a *cumulative ceiling* with each request: "across this session, you may claim up to this
total". The server verifies the signature locally, serves the request, and decides later, in its
own time, how much of that ceiling to actually redeem, in one transaction covering many requests.

Three consequences fall out, and Tickline is built on all three:

- **Payment costs no block.** The signature is checked in microseconds, in the request handler.
- **The price can be decided after the client commits to paying.** The client authorizes a ceiling,
  the server captures the true amount. That is exactly the shape of a market order.
- **Settlement is amortized.** The cost of touching the chain is paid once per batch, not once per
  trade, so a one cent fill is economic.

### Pointing that rail at a market

A fill is an HTTP request. `POST /v1/markets/:id/fills` is priced like any other paid endpoint,
except that its price is the cost of the shares being bought, computed at the instant the request
is handled.

Prices come from a **binary LMSR** market maker. An LMSR always quotes, at any size, without a
counterparty and without an order book, which is what lets a market exist for a question nobody
else has thought about yet. It has one parameter, the liquidity `b`, which sets how far a given
trade moves the price, and one property that makes it safe to run: its worst case loss over every
possible sequence of trades is `b · ln 2`, known before the first trade. The market's creator
posts exactly that as a subsidy, and can lose no more.

Each accepted fill returns a **`PositionReceipt`**, an EIP-712 object signed by the operator
saying: at this nonce, in this epoch, this agent holds this many YES and NO shares in this market,
having paid this much cost and this much fee. That receipt is the agent's leverage, and section
"Who has to be trusted" below is about what it is leverage for.

Periodically the operator does two onchain things: **commits** the epoch's aggregated positions to
`TicklineVault`, pulling in the collateral that backs them, and **redeems** the accumulated
vouchers from escrow. At the deadline the market resolves against a **Pyth** price at a named
timestamp, and every agent claims directly from the vault. The operator cannot stop a claim, and
does not have to be online for one.

One share of the winning outcome pays exactly 1 USDC. A YES share and a NO share together always
pay exactly 1 USDC, which is what makes the vault's solvency check a piece of arithmetic rather
than a risk model.

---

## A session, end to end

An agent that believes ETH will be above $4,000 on Friday does this:

1. **Open a session.** Deposit USDC into the x402 escrow contract for the channel
   `(payer, receiver, token, withdrawDelay, salt)`. The salt is the agent's own, so one wallet can
   run many sessions at once. Tickline requires `withdrawDelay = 3600`: the agent can always walk
   away, after an hour's notice, with everything not yet claimed.
2. **Ask.** `GET /v1/markets/:id/quote` is a paid read, 0.001 USDC, answered with the current
   prices and the cost of a given size.
3. **Fill.** `POST /v1/markets/:id/fills` with a voucher whose ceiling covers the worst case the
   agent will accept. The engine prices the fill against the LMSR, charges the true cost plus a
   100 bps fee, and never more than the signed ceiling. The unused remainder of the ceiling is not
   captured and is not reserved: it stays spendable in the very next request.
4. **Hold the receipt.** The response carries the signed `PositionReceipt`, issued in the same
   database transaction that recorded the money. Shares are never issued before the money moves.
5. **Repeat.** Steps 2 to 4 are a loop the agent can run as fast as it likes. Nothing so far has
   touched a chain.
6. **The operator settles.** Once per epoch, positions are committed to the vault and the
   collateral delta is pulled in. Vouchers are redeemed in batches.
7. **Resolution.** After the deadline, anyone submits the Pyth price update for the market's named
   timestamp and calls `resolve`. Nobody votes, nobody adjudicates. If no valid price exists in the
   window, the market voids and collateral is refunded.
8. **Claim.** The agent calls `claim` and is paid for the winning side. If the committed position
   is smaller than the one in its receipt, it calls `claimWithReceipt` instead: it is made whole
   from the operator's bond, and the operator is slashed.

---

## Three properties, each of which can break

The interesting part of this design is also its attack surface. Each of these is something the
test suite exists to hunt.

**Thousands of fills, a handful of transactions.** Pricing is offchain and only aggregates land
onchain, so the vault must never trust the operator further than the operator's bond reaches. The
vault re-derives what it can and refuses what it cannot.

**The ceiling.** A fill whose price moves between quote and execution must never capture more than
the client signed for, and must never leave the unused remainder stranded. Both failures are
silent, and both are money.

**A lying operator is always slashable.** Every agent holds a signed receipt. If the committed
position is smaller than the receipt, the receipt wins and the bond pays. There is no ordering,
timing, or gas condition under which the receipt loses. That sentence is invariant **I12**, and
a Foundry invariant suite with a deliberately dishonest operator exists to try to falsify it.

---

## Who has to be trusted

| Party | Can | Cannot |
|---|---|---|
| **Operator** | Price fills, choose when to commit and redeem, go offline | Commit a position smaller than a receipt it signed and keep its bond. Take more than the signed ceiling. Stop a claim. Change a resolution. |
| **Market creator** | Choose the question, the deadline, the liquidity `b`, the fee | Lose more than the `b · ln 2` subsidy posted up front |
| **Agent** | Fill, read, hold to resolution, leave the session on an hour's notice | Sell before resolution. Exceed its deposit. |
| **The vault** | Hold collateral, enforce solvency and monotonic positions, resolve, pay, slash | Price a trade. It never runs an LMSR. |

The design principle underneath the whole thing: **lose no funds, pay no one twice, and prefer
halting to drifting.** Where the offchain ledger and the chain disagree, the market halts. Drift is
never reconciled away by adjusting the ledger to match. Every conversion between the engine's
18-decimal math and USDC's 6 decimals rounds costs up and shares down, always toward the vault,
with no exceptions for a single wei.

Sixteen invariants state the rest precisely, with stable IDs that tests reference by name:
[`CLAUDE.md` §4](CLAUDE.md#4-invariants).

---

## What this is not

Deliberately absent, and not coming: selling positions before resolution, order books,
multi-outcome markets, human or committee resolution, a trading UI for people, a token, and
anything cross-chain. Resolution is machine-only, positions are bought and held, and the markets
are binary.

The full list is [`CLAUDE.md` §10](CLAUDE.md#10-out-of-scope-do-not-build-do-not-scaffold).

---

## Start here

| If you want to | Read |
|---|---|
| Understand how work happens here | [`CLAUDE.md`](CLAUDE.md), the working agreement |
| See what gets built, and when | [`PHASES.md`](PHASES.md) |
| Know what is actually done | [`docs/STATUS.md`](docs/STATUS.md) |
| Know what must never break | [`CLAUDE.md` §4](CLAUDE.md#4-invariants), the sixteen invariants |
| Read the x402 findings, with citations | [`docs/spec-notes.md`](docs/spec-notes.md) |
| Know why something is the way it is | [`docs/decisions/`](docs/decisions/) |
| Work on the repo itself | [`docs/GITHUB.md`](docs/GITHUB.md) |

## Getting set up

```bash
scripts/dev-setup.sh     # rust, just, foundry, node, pnpm, sqlx-cli; idempotent
just doctor              # what is installed, and the install line for what is not
just up                  # postgres + anvil + migrations
just test                # every stack
just gate "$(cat .phase)"   # the only definition of "done"
```

`just` on its own lists every command. Nothing in CI runs a command you cannot run locally by
the same name.

## Layout

```
engine/      Rust: the seller. lmsr · protocol · ledger · market · api · settlement · indexer
contracts/   Foundry: TicklineVault. Unit, fuzz, and handler-based invariant suites
agents/      TypeScript: x402 clients and the cross-stack vector generator
e2e/         Declarative scenarios, each asserting final balances to the base unit
tools/       A 60-digit mpmath oracle the Rust math is diffed against
docs/        The spec notes, the status, and every ADR
```

Full annotated tree in [`CLAUDE.md` §3](CLAUDE.md#3-repository-layout).

## How it is tested

Money software earns trust by the kinds of failure it has already survived, so the suite is
built around failure kinds rather than coverage percentages:

- **Differential**: Rust LMSR against 60-digit `mpmath`, rounding toward the vault. Solidity
  against the same committed vectors.
- **Cross-stack**: Rust, Solidity, and TypeScript must produce byte-identical EIP-712 digests
  from one committed vector file.
- **Property**: random fill sequences, random agents, random markets, with invariants asserted
  after every step.
- **Invariant (Foundry)**: handler-based, including a deliberately dishonest operator.
- **Concurrency**: 50 agents × 5 markets firing at once: no voucher accepted twice, no
  reservation leaked.
- **Crash and reorg injection**: fail-points at every outbox transition; reorgs deeper than the
  confirmation depth must halt and alert rather than quietly diverge.
- **Mutation**: surviving mutants on public functions are a gate failure, not a metric.

A flaky test is a bug, never noise to retry. A counterexample becomes a permanent unit test
before its phase can close.

## Licence

MIT.
