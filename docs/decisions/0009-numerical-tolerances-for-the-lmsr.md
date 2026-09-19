# ADR-0009: Numerical tolerances for the LMSR, and the margin that protects I15

- **Status:** accepted (Jay, on #50)
- **Date:** 2026-09-19
- **Phase:** 1
- **Invariants touched:** I3, I15 (both rest on the margin below); I2

## Context

`PHASES.md` phase 1 required the Rust LMSR to match the mpmath vectors "within 1 wei in WAD".
With Solady's `lnWad` as the shared numerical method (D1, ADR-0008), `cost` cannot meet that.

`cost = max(qY, qN) + b * ln(1 + exp(-|qY - qN| / b))`. The `ln` term is accurate to about 2 wei
out of 1e18, and it is multiplied by `b / WAD`, which reaches 1e7 at `B_MAX`. The error therefore
scales with `b`, up to roughly 4e7 wei. One base unit is 1e12 wei, so that is about 4e-5 of a base
unit, but it is far more than 1 wei.

## Options

| Option | Cost | What it does |
|---|---|---|
| (a) Derive a bound `E(b)`, assert it, and add it as a margin before rounding to base units | a tolerance that has to be derived and kept honest | keeps the shared numerical method; I15 holds with room to spare |
| (b) A higher-precision internal `ln` for `cost` | a custom routine | **undoes D1**: Rust and Solidity would no longer share one numerical method, which was the whole reason for the Solady port |
| (c) Keep "within 1 wei" and cap `b` near 1 share | none technically | **breaks D2**: Jay set `b` in [1, 1e7] shares, and 1 share is the bottom of that range |

## Decision

**(a).** With three rules that keep it from rotting, all from Jay on #50:

1. **The bound is derived first, from per-step bounds, and only then asserted.** Measuring the
   error and asserting the measurement is circular: a bug that inflated the error would move the
   bound with it and the test would still pass.
2. **Tightness is asserted too.** The measured maximum over the vectors must be at most `E(b) / 2`.
   A bound nothing approaches is not a test. If measurement exceeds the derivation, or the
   derivation exceeds twice the measurement, that is a `needs-jay`, not a constant to tune.
3. **Every money conversion carries the margin, in the vault's direction** (see below).

### Leaf bounds (empirical, from Solady, each asserted by a test)

These two cannot be derived: they are properties of Solady's rational approximations. Both are
measured over the 2,200 `exp`/`ln` cases in `testdata/vectors/lmsr.json` and asserted by tests in
`lmsr` (`exp_wad_matches_reference_vectors`, `ln_wad_matches_reference_vectors`, and the leaf
assertions in `tests/error_bounds.rs`):

| Leaf | Bound | Note |
|---|---|---|
| `exp_wad(x)` for `x <= 0` | within the exact floor and ceiling, so `<= 1` wei | the only range the cost uses |
| `ln_wad(x)` for `x in [WAD, 2 * WAD]` | `<= 2` wei | the only range the cost uses; measured over the whole domain |

### Derivation of `E(b)`

Every quantity is a WAD integer. `/` is truncating division, as in Rust and the EVM.

| Step | Value | Error against exact, in wei |
|---|---|---|
| 1 | `d = abs(qY - qN)`, `m = max(qY, qN)` | 0, exact |
| 2 | `z = -(d * WAD) / b` | `< 1`. Truncation moves `z` **toward zero**, so `exp(z)` and therefore the cost come out slightly high, which favours the vault. |
| 3 | `u = exp_wad(z)`, `u` in `[0, WAD]` | `<= 2`. One from the leaf bound, plus the step-2 error carried through: `d(exp)/dz = exp(z) <= 1` for `z <= 0`, so one wei of `z` moves `u` by at most one wei. |
| 4 | `s = WAD + u`, `s` in `[WAD, 2 * WAD]` | `<= 2`, unchanged. Exact addition: `u <= WAD` so `s <= 2 * WAD` and nothing overflows. |
| 5 | `L = ln_wad(s)`, `L` in `[0, ln2 * WAD]` | `<= 4`. Two from the leaf bound, plus step 4 carried through: `dL/ds = WAD / s <= 1` because `s >= WAD`. |
| 6 | `t = b * L / WAD` | `<= 4 * b / WAD + 1`. Step 5 scaled by `b / WAD`, plus one wei of truncation. |
| 7 | `C = m + t` | unchanged. Exact addition. |

**`E(b) = ceil(4 * b / WAD) + 1` wei.**

At `B_MIN` (1 share) that is 5 wei. At `B_MAX` (1e7 shares) it is 40,000,001 wei, which is
**under 1e9 and more than three orders of magnitude below one base unit (1e12 wei)**. That fact is
what makes the tolerance safe, so it has its own test,
`cost_error_bound_stays_far_below_one_base_unit`.

### Derivation of the price bound

`pSmall = (u * WAD) / (WAD + u)` where `u = exp_wad(-(d * WAD) / b)`, and the larger price is
`WAD - pSmall`, so the two always sum to exactly `WAD`.

| Step | Error in wei |
|---|---|
| `u` | `<= 2`, as above |
| `pSmall` | `<= 3`: `d(pSmall)/du = WAD^2 / (WAD + u)^2 <= 1`, carrying 2, plus one wei of truncation |
| `pLarge = WAD - pSmall` | `<= 3`, exact subtraction |

**`PRICE_ERROR_BOUND = 3` wei**, constant: it does not scale with `b`.

### The margin at the money boundary

Rounding up alone is not enough: the WAD cost can sit up to `E(b)` **below** exact, and then
`ceil` of it could still land below the exact cost in base units. Every conversion that produces
money the vault must hold adds the margin first:

| Conversion | Rule | Why |
|---|---|---|
| cost to base units | `ceil((cost_wad + E(b)) / 1e12)` | converted cost is then never below exact (I15) |
| `cost_to_buy` to base units | `ceil((delta_wad + 2 * E(b)) / 1e12)` | it is a difference of two costs, so it carries two bounds, not one |
| subsidy | `ceil((b * ln2 + E(b)) / 1e12)` | the subsidy is money the vault must hold; without the margin I3 could rest on a value below exact |
| shares to base units | `floor`, no margin | margins only ever favour the vault |

`cost_to_buy`, the subsidy and the conversions land in S5 (#42); this ADR fixes their rule now so
the implementation has nothing to invent.

## Consequences

The money property is unchanged and stronger than a wei-level tolerance: converted cost is always
at or above exact, and the overcharge is bounded. S5 and S6 assert both directions
(`prop_i15_converted_cost_ge_exact_from_vectors` and
`prop_converted_cost_never_exceeds_exact_by_more_than_two_base_units`).

The cost is that `cost` now has a `b`-dependent tolerance that a reader must look up rather than a
flat "1 wei". The tightness assertion is what stops that tolerance drifting into meaninglessness.

`PHASES.md` phase 1 is amended in the same PR: exp, ln and price within their derived constant
bounds, cost within `E(b)`, and I15 guaranteed at the base-unit boundary by the margin.

## Evidence that this kind of checking works

Three defects in the S3 port were caught by tests rather than by review, and all three would have
produced wrong money:

- `FIVE_POW_18` was written from memory as 5^41 rather than 5^18.
- `ruint`'s `checked_shr` returns `None` when nonzero bits are shifted out, which silently zeroed
  every `exp` result.
- `alloy`'s `I256::checked_shl` only rejects shift amounts of 256 or more, and drops high bits.

None of these is visible by reading the code against the Solidity source; each showed up as a
wrong number. That is why the parity run against real Solady **moves into CI in Phase 3**
(`PHASES.md` phase 3 already requires Solidity `expWad`/`lnWad` against `lmsr.json`) rather than
staying the one-off scratch run it was in #51.

### The four log2 mutants

S3 removed Solady's `log2` binary search in favour of `256 - bit_len(x)`, so the four mutants that
survived there no longer exist: the production code no longer contains those comparisons, and
Solady's search remains in the tests as the reference that proves the two agree for every input.
Jay asked that they be revisited once the tightness assertion exists. They cannot be revisited in
production code that no longer has them; if the search is ever restored, the tightness assertion
is the tool that would expose them.

## How this is enforced

`engine/crates/lmsr/tests/error_bounds.rs` asserts each leaf bound, each derived bound, the
tightness ratio, and `E(B_MAX) < 1e9`. `cost_matches_reference_vectors_within_derived_bound` and
`price_matches_reference_vectors_within_derived_bound` run over every vector case.
