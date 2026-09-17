# ADR-0005: Pyth resolution rule

- **Status:** accepted
- **Date:** 2026-09-15
- **Phase:** 3
- **Invariants touched:** I1 (solvency, via the void path), I11 (no double claim, extended to void refunds)
- **Decided by:** Jay, on issue #6

## Context

A Tickline market resolves against a Pyth price at its deadline. Everything about that resolution
must be a function of the deadline and the feed, never of who submits the transaction or when.

An earlier reading of this claimed `parsePriceFeedUpdatesUnique` did not guarantee "the first
update at or after the deadline", and that a resolver could therefore choose among in-window
updates. **That reading was wrong.** It came from the `IPyth.sol` NatSpec, which describes the
uniqueness condition loosely, without reading the implementation.

Pinned to `pyth-crosschain` commit `807ff575a9090cee99b9e1a30dc23edf3522fe1b`:

`contracts/pyth/Pyth.sol` lines 612 to 631: `parsePriceFeedUpdatesUnique` passes
`checkUniqueness = true`. Lines 240 to 246: an update is accepted only when:

```solidity
publishTime >= context.minAllowedPublishTime &&
publishTime <= context.maxAllowedPublishTime &&
(!context.checkUniqueness ||
    context.minAllowedPublishTime > prevPublishTime)
```

`prevPublishTime` is the publish time of that feed's immediately preceding update, and it is part
of the signed Merkle payload (lines 224 to 229), so a submitter cannot forge it. With
`minPublishTime = deadline`, the only update that satisfies the condition is the one whose
predecessor was published *before* the deadline. That is the first update at or after the deadline, and
there is exactly one such update per feed.

**Submitter discretion is zero, at any window size.** The window exists only to bound liveness
when a feed has a gap.

## Decision

### 1. The call

```solidity
pyth.parsePriceFeedUpdatesUnique{value: fee}(
    updateData,
    priceIds,          // [market.feedId]
    deadline,          // minPublishTime
    deadline + RESOLVE_WINDOW
);
```

`RESOLVE_WINDOW` is **60 seconds** and is a **vault constant, not a per-market parameter**. It is
not a security parameter. A market creator has nothing to gain by tuning it, and exposing it
would only create a knob that looks like one.

`fee` comes from `pyth.getUpdateFee(updateData)` and is forwarded as `msg.value`.

### 2. Resolution is permissionless from the deadline

Anyone may call `resolve` the moment `block.timestamp > deadline`. No grace period. Historical
updates are fetchable by timestamp from Hermes, so a late resolver is not disadvantaged and an
early one gains nothing: the same update is the only valid one whenever it is submitted.

This is a deliberate departure from `PHASES.md`, which specified "permissionless after deadline +
final grace". The grace period existed to protect against a timing race that the uniqueness check
proves cannot happen.

### 3. Outcome

**YES iff the expo-normalized price >= threshold.** The confidence interval is **ignored**:
`price.conf` plays no part in resolution. A confidence-weighted rule would make the outcome depend
on a second, noisier quantity for no gain in a binary market.

Normalization: a Pyth price is `price` (`int64`) with `expo` (`int32`, negative in practice).
Normalizing to signed WAD:

| Case | Operation | Rounds? |
|---|---|---|
| `expo >= -18` | `price * 10^(18 + expo)` | No, exact |
| `expo < -18` | `price / 10^(-(18 + expo))`, **floor division toward negative infinity** | Yes |

**Rounding direction: down.** The only case that rounds is `expo < -18`, and it rounds the
observed price *down*, so a value that would land exactly on the threshold after truncation
resolves **NO**. This is the conservative direction for a "price >= threshold" question: it never
manufactures a YES out of discarded precision.

In practice every major Pyth feed publishes `expo = -8`, so the rounding branch is unreachable for
the feeds Tickline will use. It is implemented and tested anyway, because "unreachable in practice"
is how truncation bugs arrive (I15).

Overflow: `price * 10^(18 + expo)` must be checked. `int64` times `10^10` (the `expo = -8` case)
fits comfortably in `int256`; the multiplication is nonetheless checked, and a market whose feed
reports an expo that would overflow cannot be created.

### 4. The void path

A feed gap is possible: no update may exist in `[deadline, deadline + 60]`. If so, **no `resolve`
call can ever succeed**. The qualifying update does not exist and never will, because
`prevPublishTime` is immutable history.

Therefore: if a market is still unresolved at `deadline + VOID_TIMEOUT` (**24 hours**), anyone may
call `voidMarket(marketId)`.

On void:
- every receipt holder may reclaim `costPaid + feesPaid` from **market collateral**;
- the creator receives the remaining subsidy once the claim window closes;
- the **operator bond is untouched**. A feed gap is not operator fault, and slashing for it would
  punish the honest case.

Void is terminal and mutually exclusive with resolution. `voidMarket` reverts on a resolved
market; `resolve` reverts on a voided one.

## Consequences

Resolution becomes a pure function of `(feedId, deadline)`. Two independent resolvers submitting
different `updateData` produce the same outcome or both revert, which is what makes
`invariant_resolution_is_final_and_unique` provable rather than merely tested.

The void path is new surface: a third terminal state, a second refund route, and an interaction
with I11 (a voided market must not permit both a void refund and a claim). It is the price of not
having markets that can be permanently stuck, and it is worth paying.

The 24-hour timeout is a judgement call. Too short risks voiding a market that a slow feed would
have resolved; too long traps funds. 24 hours is far beyond any observed Pyth gap while still
bounding the trap.

The confidence interval being ignored is a real limitation to state plainly: a market resolving
during extreme feed uncertainty resolves on the point estimate anyway. For a binary
threshold market that is the honest behaviour; the alternative is a rule nobody can predict in
advance.

## How this is enforced

Required Phase 3 tests, each named here so a reviewer can `grep` for them:

- `test_resolve_accepts_first_update_at_or_after_deadline`
- `test_resolve_rejects_later_update_in_window`: `prevPublishTime >= deadline` reverts
- `test_resolve_rejects_update_before_deadline`
- `test_resolve_rejects_update_after_window`
- `testFuzz_resolve_outcome_matches_threshold`: includes the equality case and the full expo range
- `test_void_only_after_timeout_and_only_if_unresolved`
- `invariant_resolution_is_final_and_unique`: no call sequence produces two outcomes, or resolves
  a voided market, or voids a resolved one

`MockPyth.createPriceFeedUpdateData` takes `prevPublishTime` as an argument
(`sdk/solidity/MockPyth.sol` lines 281 to 290), so every case above is constructible on anvil with no
network access.

Phase 8 adds a fork test resolving against a real historical Hermes update.

`RESOLVE_WINDOW` and `VOID_TIMEOUT` are vault constants, and changing either is an ABI-visible
change that needs a new ADR.
