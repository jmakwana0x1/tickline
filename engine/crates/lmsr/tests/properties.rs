//! The property suite (issue #43). Every invariant this crate owns, asserted over generated
//! inputs drawn from the D2 bounds, edges included.
//!
//! `PROPTEST_CASES` sets the number of cases, so `just deep` raises it without touching the code.
//!
//! Two properties from `PHASES.md` are split rather than restated, because fixed point makes the
//! original wording false at the wei level (Jay's direction on #43):
//!
//! - **cost to buy**: a buy smaller than one wei of cost truncates to zero in WAD, so the
//!   statement splits into non-negativity, monotonicity in `d`, and the money statement after
//!   conversion. A named unit test in `convert.rs` documents the edge.
//! - **price**: a buy moves the price by about `d * WAD / (4 * b)` wei, so below that resolution
//!   the price does not move. The property is that it never falls, and strictness is asserted
//!   where the move is large enough to land.

// Shared by several test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use lmsr::{
    cost, cost_error_bound, cost_to_base_units, cost_to_buy, cost_to_buy_base_units, prices,
    round_cost_up, round_shares_down, subsidy_base_units, LmsrError, Outcome, BASE_UNIT_SCALE,
    B_MAX, B_MIN, I256, Q_MAX, WAD,
};
use proptest::prelude::*;

/// `floor(ln2 * 1e18)`, the coefficient in I2's upper bound.
const LN2_WAD: i128 = 693_147_180_559_945_309;

/// Liquidity across the whole D2 range, with both edges always in the mix.
fn liquidity() -> impl Strategy<Value = i128> {
    prop_oneof![
        1 => Just(B_MIN),
        1 => Just(B_MAX),
        4 => B_MIN..=B_MAX,
        4 => (18_u32..=25).prop_flat_map(|e| {
            let decade = 10_i128.saturating_pow(e);
            (decade..=decade.saturating_mul(9).min(B_MAX)).prop_map(move |v| v.clamp(B_MIN, B_MAX))
        }),
    ]
}

/// A quantity across `[0, Q_MAX]`, with both edges always in the mix.
fn quantity() -> impl Strategy<Value = i128> {
    prop_oneof![
        1 => Just(0_i128),
        1 => Just(Q_MAX),
        4 => 0_i128..=Q_MAX,
        4 => (0_u32..=30).prop_flat_map(|e| {
            let decade = 10_i128.saturating_pow(e);
            (decade..=decade.saturating_mul(9).min(Q_MAX)).prop_map(|v| v.min(Q_MAX))
        }),
    ]
}

/// A market state: two quantities and a liquidity.
fn state() -> impl Strategy<Value = (i128, i128, i128)> {
    (quantity(), quantity(), liquidity())
}

/// A state and a buy that fits: `d` is drawn from the headroom left under `Q_MAX`, rather than
/// drawn freely and then rejected. Rejection does not scale: proptest's global reject budget is
/// fixed, so at high `PROPTEST_CASES` the run aborts before finishing.
fn state_and_buy() -> impl Strategy<Value = (i128, i128, i128, i128)> {
    state().prop_flat_map(|(q_yes, q_no, b)| {
        let base = q_yes.min(Q_MAX.saturating_sub(1));
        let headroom = Q_MAX.saturating_sub(base);
        (Just(base), Just(q_no), Just(b), 1_i128..=headroom)
    })
}

/// A state and two buys that both fit, for the split and monotonicity properties.
fn state_and_two_buys() -> impl Strategy<Value = (i128, i128, i128, i128, i128)> {
    state().prop_flat_map(|(q_yes, q_no, b)| {
        // Two below the cap, so there is always room for both buys.
        let base = q_yes.min(Q_MAX.saturating_sub(2));
        let headroom = Q_MAX.saturating_sub(base);
        (
            Just(base),
            Just(q_no),
            Just(b),
            1_i128..=headroom.saturating_sub(1),
        )
            .prop_flat_map(move |(q_yes, q_no, b, first)| {
                let left = headroom.saturating_sub(first).max(1);
                (Just(q_yes), Just(q_no), Just(b), Just(first), 1_i128..=left)
            })
    })
}

/// A state whose prices have not saturated: the spread stays within a few multiples of `b`, so
/// both prices are strictly between 0 and one WAD. Beyond about 40 * b the exponential underflows
/// and the prices pin at the extremes, which is correct but says nothing about price movement.
fn unsaturated_state() -> impl Strategy<Value = (i128, i128, i128)> {
    liquidity().prop_flat_map(|b| {
        let reach = b.saturating_mul(20);
        let lowest = reach.checked_neg().unwrap_or(i128::MIN);
        (0_i128..=Q_MAX.saturating_div(2), lowest..=reach, Just(b)).prop_map(
            move |(q_yes, spread, b)| (q_yes, q_yes.saturating_add(spread).clamp(0, Q_MAX), b),
        )
    })
}

/// Shares to pay out, in base units: what the vault owes if that outcome wins.
fn payout_base_units(q: i128) -> Result<u128, LmsrError> {
    round_shares_down(I256::try_from(q).map_err(|_| LmsrError::ArithmeticOverflow("payout"))?)
}

proptest! {
    // Counterexamples are printed, not written to a file beside the source: `PHASES.md` says a
    // counterexample becomes a permanent named test, which a regressions file would quietly
    // replace.
    #![proptest_config(ProptestConfig { failure_persistence: None, ..ProptestConfig::default() })]

    #[test]
    fn prop_prices_sum_to_one_wad_within_one_wei((q_yes, q_no, b) in state()) {
        let p = prices(q_yes, q_no, b)?;
        // Stronger than the phase plan asks: the two sum to exactly one WAD, because the larger
        // price is derived from the smaller by subtraction.
        prop_assert_eq!(p.yes.checked_add(p.no), Some(WAD));
    }

    #[test]
    fn prop_buying_yes_never_lowers_price_yes((q_yes, q_no, b, d) in state_and_buy()) {
        let before = prices(q_yes, q_no, b)?.yes;
        let after = prices(q_yes.saturating_add(d), q_no, b)?.yes;
        prop_assert!(after >= before, "price fell from {} to {}", before, after);
    }

    /// A buy moves the price by about `d * WAD / (4 * b)` wei. Above that resolution, with room
    /// to spare, the move has to land.
    #[test]
    fn prop_buying_a_meaningful_size_strictly_raises_price_yes((q_yes, q_no, b) in unsaturated_state()) {
        // Eight shares' worth of liquidity: twice the size that moves the price by one wei.
        let d = b.saturating_mul(8).saturating_div(WAD).saturating_mul(WAD);
        prop_assume!(d > 0 && q_yes.checked_add(d).is_some_and(|q| q <= Q_MAX));
        // Away from saturation, where both prices have pinned at the extremes.
        let before = prices(q_yes, q_no, b)?.yes;
        prop_assume!(before > 0 && before < WAD);
        let after = prices(q_yes.saturating_add(d), q_no, b)?.yes;
        prop_assert!(after > before, "price did not move: {} to {}", before, after);
    }

    #[test]
    fn prop_cost_to_buy_is_non_negative((q_yes, q_no, b, d) in state_and_buy()) {
        prop_assert!(cost_to_buy(q_yes, q_no, b, Outcome::Yes, d)? >= 0);
    }

    #[test]
    fn prop_cost_to_buy_is_monotonic_in_d((q_yes, q_no, b, d1, extra) in state_and_two_buys()) {
        let d2 = d1.saturating_add(extra);
        let small = cost_to_buy(q_yes, q_no, b, Outcome::Yes, d1)?;
        let large = cost_to_buy(q_yes, q_no, b, Outcome::Yes, d2)?;
        prop_assert!(small <= large, "buying more cost less: {} then {}", small, large);
    }

    #[test]
    fn prop_cost_to_buy_base_units_is_at_least_one_base_unit((q_yes, q_no, b, d) in state_and_buy()) {
        let wad = cost_to_buy(q_yes, q_no, b, Outcome::Yes, d)?;
        prop_assert!(cost_to_buy_base_units(wad, b)? >= 1);
    }

    #[test]
    fn prop_cost_to_buy_equals_cost_difference((q_yes, q_no, b, d) in state_and_buy()) {
        let difference = cost(q_yes.saturating_add(d), q_no, b)?.checked_sub(cost(q_yes, q_no, b)?);
        prop_assert_eq!(Some(cost_to_buy(q_yes, q_no, b, Outcome::Yes, d)?), difference);
    }

    #[test]
    fn prop_split_buys_cost_at_least_one_buy((q_yes, q_no, b, d1, d2) in state_and_two_buys()) {
        let total = d1.saturating_add(d2);
        let split_first = cost_to_buy(q_yes, q_no, b, Outcome::Yes, d1)?;
        let split_second = cost_to_buy(q_yes.saturating_add(d1), q_no, b, Outcome::Yes, d2)?;
        let single = cost_to_buy(q_yes, q_no, b, Outcome::Yes, total)?;
        prop_assert!(split_first.checked_add(split_second).ok_or(LmsrError::ArithmeticOverflow("split"))? >= single);

        // And in money, where each leg pays its own rounding and margin.
        let split_charge = cost_to_buy_base_units(split_first, b)?
            .checked_add(cost_to_buy_base_units(split_second, b)?)
            .ok_or(LmsrError::ArithmeticOverflow("split charge"))?;
        prop_assert!(split_charge >= cost_to_buy_base_units(single, b)?);
    }

    /// I2: `max(qY, qN) <= C(q) <= max(qY, qN) + b * ln2`, within the derived bound.
    #[test]
    fn prop_i2_cost_between_max_q_and_max_q_plus_b_ln2((q_yes, q_no, b) in state()) {
        let c = cost(q_yes, q_no, b)?;
        let larger = q_yes.max(q_no);
        let bound = cost_error_bound(b)?;
        prop_assert!(c >= larger.checked_sub(bound).ok_or(LmsrError::ArithmeticOverflow("i2"))?,
            "cost {} is below max(q) {}", c, larger);
        let wad_wide = I256::try_from(WAD)?;
        let bound_wide = I256::try_from(bound)?;
        let subsidy_wad = I256::try_from(b)?
            .checked_mul(I256::try_from(LN2_WAD)?)
            .and_then(|v| v.checked_div(wad_wide))
            .ok_or(LmsrError::ArithmeticOverflow("i2"))?;
        let ceiling = I256::try_from(larger)?
            .checked_add(subsidy_wad)
            .and_then(|v| v.checked_add(bound_wide))
            .ok_or(LmsrError::ArithmeticOverflow("i2"))?;
        prop_assert!(I256::try_from(c)? <= ceiling, "cost {} is above max(q) + b ln2", c);
    }

    /// I3: the creator's subsidy plus everything collected covers the worst payout, after any
    /// sequence of buys.
    #[test]
    fn prop_i3_subsidy_plus_collected_covers_max_payout(
        b in liquidity(),
        buys in prop::collection::vec((any::<bool>(), 1_i128..=10_i128.saturating_pow(24)), 1..12),
    ) {
        let subsidy = subsidy_base_units(b)?;
        let (mut q_yes, mut q_no, mut collected) = (0_i128, 0_i128, 0_u128);
        for (yes, d) in buys {
            let outcome = if yes { Outcome::Yes } else { Outcome::No };
            let held = if yes { q_yes } else { q_no };
            if held.saturating_add(d) > Q_MAX {
                continue;
            }
            let wad = cost_to_buy(q_yes, q_no, b, outcome, d)?;
            collected = collected
                .checked_add(cost_to_buy_base_units(wad, b)?)
                .ok_or(LmsrError::ArithmeticOverflow("collected"))?;
            if yes {
                q_yes = q_yes.saturating_add(d);
            } else {
                q_no = q_no.saturating_add(d);
            }
        }
        let worst_payout = payout_base_units(q_yes.max(q_no))?;
        let funds = subsidy.checked_add(collected).ok_or(LmsrError::ArithmeticOverflow("funds"))?;
        prop_assert!(funds >= worst_payout,
            "subsidy {} plus collected {} is below the worst payout {}", subsidy, collected, worst_payout);
    }

    /// I15: a converted cost is never below the exact value, a converted share count never above.
    #[test]
    fn prop_i15_converted_cost_ge_exact_and_shares_le_exact((q_yes, q_no, b) in state()) {
        let scale = I256::try_from(BASE_UNIT_SCALE)?;
        let cost_wad = cost(q_yes, q_no, b)?;

        let charged = I256::try_from(cost_to_base_units(cost_wad, b)?)?
            .checked_mul(scale)
            .ok_or(LmsrError::ArithmeticOverflow("i15"))?;
        prop_assert!(charged >= I256::try_from(cost_wad)?, "the conversion undercharged");

        // Without the margin the round-up alone still never undercharges.
        let rounded = I256::try_from(round_cost_up(I256::try_from(cost_wad)?)?)?
            .checked_mul(scale)
            .ok_or(LmsrError::ArithmeticOverflow("i15"))?;
        prop_assert!(rounded >= I256::try_from(cost_wad)?);

        let shares = I256::try_from(round_shares_down(I256::try_from(q_yes)?)?)?
            .checked_mul(scale)
            .ok_or(LmsrError::ArithmeticOverflow("i15"))?;
        prop_assert!(shares <= I256::try_from(q_yes)?, "the conversion granted shares that were not bought");
    }
}
