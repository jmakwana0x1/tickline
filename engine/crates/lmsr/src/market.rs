//! Binary LMSR cost and price, in the log-sum-exp form (issue #41).
//!
//! `C(qY, qN) = max(qY, qN) + b * ln(1 + exp(-|qY - qN| / b))`, never the naive form, so nothing
//! overflows however far the quantities diverge (`PHASES.md` phase 1).
//!
//! Every value here is a signed WAD integer. The public surface is `i128`, which holds every
//! value the D2 bounds allow (`Q_MAX` is 1e30, `i128` reaches 1.7e38); `I256` stays inside, where
//! the Solady port needs it.
//!
//! Error bounds are derived in ADR-0009, step by step, and asserted against the vectors. The
//! steps below are numbered to match that derivation.

use alloy_primitives::I256;

use crate::{
    exp_wad,
    fixed::{add, div, int, mul, sub},
    ln_wad, LmsrError, WAD,
};

/// Smallest liquidity parameter: 1 share, in WAD (D2, #31).
pub const B_MIN: i128 = 1_000_000_000_000_000_000;

/// Largest liquidity parameter: 1e7 shares, in WAD (D2, #31).
pub const B_MAX: i128 = 10_000_000_000_000_000_000_000_000;

/// Largest quantity of either outcome: 1e12 shares, in WAD (D2, #31).
pub const Q_MAX: i128 = 1_000_000_000_000_000_000_000_000_000_000;

/// Derived error bound for [`prices`], in wei. Constant: it does not scale with `b` (ADR-0009).
pub const PRICE_ERROR_BOUND: i128 = 3;

/// The two prices, which always sum to exactly one WAD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prices {
    /// Probability of YES, in WAD.
    pub yes: i128,
    /// Probability of NO, in WAD.
    pub no: i128,
}

/// Derived error bound for [`cost`] at liquidity `b`: `ceil(4 * b / WAD) + 1` wei (ADR-0009).
///
/// At `B_MIN` this is 5 wei and at `B_MAX` 40,000,001 wei, which is over a thousand times smaller
/// than one USDC base unit (1e12 wei). That headroom is what lets the base-unit conversion in S5
/// add this bound as a margin and still never overcharge by a whole base unit.
///
/// # Errors
///
/// The same domain errors as [`cost`] for `b`.
pub fn cost_error_bound(b: i128) -> Result<i128, LmsrError> {
    check_liquidity(b)?;
    // ceil(4 * b / WAD) + 1, in integers.
    let scaled = b
        .checked_mul(4)
        .and_then(|v| v.checked_add(WAD - 1))
        .ok_or(LmsrError::ArithmeticOverflow("error bound"))?;
    scaled
        .checked_div(WAD)
        .and_then(|v| v.checked_add(1))
        .ok_or(LmsrError::ArithmeticOverflow("error bound"))
}

/// `C(qY, qN) = max(qY, qN) + b * ln(1 + exp(-|qY - qN| / b))`, in WAD.
///
/// Accurate to within [`cost_error_bound`] of the exact value. The truncations inside all push the
/// result upward, so the error favours the vault, and the base-unit conversion in S5 adds the
/// bound as a margin before rounding up (I15, ADR-0009).
///
/// # Errors
///
/// [`LmsrError::NonPositiveLiquidity`], [`LmsrError::LiquidityBelowMin`] or
/// [`LmsrError::LiquidityAboveMax`] when `b` is outside `[B_MIN, B_MAX]`;
/// [`LmsrError::QuantityNegative`] or [`LmsrError::QuantityAboveMax`] when a quantity is outside
/// `[0, Q_MAX]`.
pub fn cost(q_yes: i128, q_no: i128, b: i128) -> Result<i128, LmsrError> {
    check_domain(q_yes, q_no, b)?;

    // Step 1: the split into an exact integer part and a tail is what keeps this from overflowing.
    let spread = q_yes
        .checked_sub(q_no)
        .map(i128::abs)
        .ok_or(LmsrError::ArithmeticOverflow("spread"))?;
    let larger = q_yes.max(q_no);

    // Steps 2 to 5.
    let log_term = log_one_plus_exp(spread, b)?;
    // Step 6: t = b * L / WAD, truncating.
    let tail = scale_by_liquidity(log_term, b)?;
    // Step 7: exact.
    larger
        .checked_add(tail)
        .ok_or(LmsrError::ArithmeticOverflow("cost"))
}

/// `pY = 1 / (1 + exp((qN - qY) / b))` and `pN = 1 - pY`, in WAD.
///
/// The smaller price is computed directly and the larger derived from it, so the two always sum to
/// exactly one WAD and neither can overflow. Accurate to within [`PRICE_ERROR_BOUND`].
///
/// # Errors
///
/// The same domain errors as [`cost`].
pub fn prices(q_yes: i128, q_no: i128, b: i128) -> Result<Prices, LmsrError> {
    check_domain(q_yes, q_no, b)?;
    let spread = q_yes
        .checked_sub(q_no)
        .map(i128::abs)
        .ok_or(LmsrError::ArithmeticOverflow("spread"))?;

    // The outcome with fewer shares is the cheaper one, so compute that price directly: its
    // formula has no cancellation, and one WAD minus it is exact.
    let u = exp_of_negative_ratio(spread, b)?;
    let denominator = add(int(WAD)?, u)?;
    let smaller_wide = div(mul(u, int(WAD)?)?, denominator)?;
    let smaller =
        i128::try_from(smaller_wide).map_err(|_| LmsrError::ArithmeticOverflow("price"))?;
    let larger = WAD
        .checked_sub(smaller)
        .ok_or(LmsrError::ArithmeticOverflow("price"))?;

    Ok(if q_yes >= q_no {
        Prices {
            yes: larger,
            no: smaller,
        }
    } else {
        Prices {
            yes: smaller,
            no: larger,
        }
    })
}

// ------------------------------------------------------------------------------- derivation steps

/// Step 2: `z = -(spread * WAD) / b`, truncating, so `|z|` is never larger than exact.
///
/// Truncating toward zero makes `exp(z)` and therefore the cost come out slightly high, which
/// favours the vault.
pub(crate) fn exponent_argument(spread: i128, b: i128) -> Result<I256, LmsrError> {
    // spread * WAD reaches 1e48, past i128, which is why this step is 256-bit.
    let numerator = mul(int(spread)?, int(WAD)?)?;
    let magnitude = div(numerator, int(b)?)?;
    sub(I256::ZERO, magnitude)
}

/// Step 3: `u = exp_wad(z)`, in `[0, WAD]` because `z <= 0`.
pub(crate) fn exp_of_negative_ratio(spread: i128, b: i128) -> Result<I256, LmsrError> {
    exp_wad(exponent_argument(spread, b)?)
}

/// Steps 3 to 5: `L = ln_wad(WAD + exp_wad(z))`, in `[0, ln2 * WAD]`.
///
/// Step 4's addition is exact and cannot overflow: `u <= WAD`, so `s <= 2 * WAD`.
pub(crate) fn log_one_plus_exp(spread: i128, b: i128) -> Result<I256, LmsrError> {
    let u = exp_of_negative_ratio(spread, b)?;
    ln_wad(add(int(WAD)?, u)?)
}

/// Step 6: `t = b * L / WAD`, truncating.
pub(crate) fn scale_by_liquidity(log_term: I256, b: i128) -> Result<i128, LmsrError> {
    let wide = div(mul(int(b)?, log_term)?, int(WAD)?)?;
    i128::try_from(wide).map_err(|_| LmsrError::ArithmeticOverflow("tail"))
}

// ------------------------------------------------------------------------------- domain

fn check_liquidity(b: i128) -> Result<(), LmsrError> {
    if b <= 0 {
        return Err(LmsrError::NonPositiveLiquidity(b));
    }
    if b < B_MIN {
        return Err(LmsrError::LiquidityBelowMin(b));
    }
    if b > B_MAX {
        return Err(LmsrError::LiquidityAboveMax(b));
    }
    Ok(())
}

fn check_domain(q_yes: i128, q_no: i128, b: i128) -> Result<(), LmsrError> {
    check_liquidity(b)?;
    for q in [q_yes, q_no] {
        if q < 0 {
            return Err(LmsrError::QuantityNegative(q));
        }
    }
    for q in [q_yes, q_no] {
        if q > Q_MAX {
            return Err(LmsrError::QuantityAboveMax(q));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LN2_WAD: i128 = 693_147_180_559_945_309; // floor(ln2 * 1e18)
    const WAD: i128 = crate::WAD;

    #[test]
    fn cost_at_origin_is_b_ln2() -> Result<(), LmsrError> {
        for b in [B_MIN, 100 * WAD, B_MAX] {
            let c = cost(0, 0, b)?;
            // C(0, 0) = b * ln 2, within the derived bound.
            let exact =
                i256_mul_div(b, LN2_WAD, WAD).ok_or(LmsrError::ArithmeticOverflow("test"))?;
            let bound = cost_error_bound(b)?;
            let difference = c
                .checked_sub(exact)
                .map(i128::abs)
                .ok_or(LmsrError::ArithmeticOverflow("test"))?;
            assert!(
                difference <= bound,
                "cost(0, 0, {b}) = {c}, expected about {exact} (bound {bound})"
            );
        }
        Ok(())
    }

    /// `b * ln2` needs 256 bits at `B_MAX`; this keeps the test honest without a float.
    fn i256_mul_div(a: i128, b: i128, d: i128) -> Option<i128> {
        i128::try_from(div(mul(int(a).ok()?, int(b).ok()?).ok()?, int(d).ok()?).ok()?).ok()
    }

    #[test]
    fn price_at_origin_is_half_wad() -> Result<(), LmsrError> {
        for b in [B_MIN, B_MAX] {
            let p = prices(0, 0, b)?;
            assert_eq!(p.yes, WAD / 2, "yes price at balance, b = {b}");
            assert_eq!(p.no, WAD / 2, "no price at balance, b = {b}");
        }
        Ok(())
    }

    #[test]
    fn prices_always_sum_to_one_wad() -> Result<(), LmsrError> {
        for (q_yes, q_no) in [
            (0, 0),
            (WAD, 0),
            (0, WAD),
            (Q_MAX, 1),
            (1, Q_MAX),
            (Q_MAX, Q_MAX),
        ] {
            let p = prices(q_yes, q_no, B_MIN)?;
            assert_eq!(p.yes + p.no, WAD, "prices({q_yes}, {q_no})");
        }
        Ok(())
    }

    #[test]
    fn cost_is_symmetric_in_outcomes() -> Result<(), LmsrError> {
        for (q_yes, q_no, b) in [
            (0, WAD, B_MIN),
            (5 * WAD, 3 * WAD, 2 * WAD),
            (Q_MAX, 7, B_MAX),
        ] {
            assert_eq!(
                cost(q_yes, q_no, b)?,
                cost(q_no, q_yes, b)?,
                "cost symmetry"
            );
            let straight = prices(q_yes, q_no, b)?;
            let swapped = prices(q_no, q_yes, b)?;
            assert_eq!(straight.yes, swapped.no, "price symmetry");
        }
        Ok(())
    }

    /// The other truncation edge: a buy moves the price by about `d * WAD / (4 * b)` wei, so at
    /// the largest liquidity a one-wei buy does not move it at all. `PHASES.md` says buying YES
    /// "strictly raises" the price; that holds above this resolution, and the property suite
    /// splits the two cases.
    #[test]
    fn buying_below_the_price_resolution_leaves_the_price_unchanged() -> Result<(), LmsrError> {
        let before = prices(0, 0, B_MAX)?.yes;
        assert_eq!(
            prices(1, 0, B_MAX)?.yes,
            before,
            "one wei at B_MAX moves nothing"
        );
        // Eight shares' worth of liquidity does move it.
        let meaningful = B_MAX / WAD * 8 * WAD;
        assert!(prices(meaningful, 0, B_MAX)?.yes > before);
        // At the smallest liquidity even one wei lands.
        assert!(prices(1, 0, B_MIN)?.yes > prices(0, 0, B_MIN)?.yes);
        Ok(())
    }

    #[test]
    fn price_saturates_under_large_imbalance_without_overflow() -> Result<(), LmsrError> {
        // exp(-1e12) underflows Solady's domain, so the price pins at the extremes.
        let p = prices(Q_MAX, 0, B_MIN)?;
        assert_eq!(p.yes, WAD, "yes price under maximum imbalance");
        assert_eq!(p.no, 0, "no price under maximum imbalance");
        let q = prices(0, Q_MAX, B_MIN)?;
        assert_eq!(q.yes, 0);
        assert_eq!(q.no, WAD);
        Ok(())
    }

    #[test]
    fn cost_under_large_imbalance_is_the_larger_quantity() -> Result<(), LmsrError> {
        // C = max(q) + b * ln(1 + exp(-huge)), and the tail is far below one wei.
        assert_eq!(cost(Q_MAX, 0, B_MIN)?, Q_MAX);
        assert_eq!(cost(0, Q_MAX, B_MIN)?, Q_MAX);
        Ok(())
    }

    #[test]
    fn rejects_non_positive_liquidity() {
        for b in [0, -1, -B_MIN, i128::MIN] {
            assert_eq!(cost(0, 0, b), Err(LmsrError::NonPositiveLiquidity(b)));
            assert_eq!(prices(0, 0, b), Err(LmsrError::NonPositiveLiquidity(b)));
        }
    }

    #[test]
    fn b_below_min_is_rejected() {
        for b in [1, B_MIN - 1] {
            assert_eq!(cost(0, 0, b), Err(LmsrError::LiquidityBelowMin(b)));
            assert_eq!(prices(0, 0, b), Err(LmsrError::LiquidityBelowMin(b)));
        }
    }

    #[test]
    fn b_at_min_is_accepted() -> Result<(), LmsrError> {
        assert!(cost(0, 0, B_MIN)? > 0);
        assert!(prices(0, 0, B_MIN)?.yes > 0);
        Ok(())
    }

    #[test]
    fn b_at_max_is_accepted() -> Result<(), LmsrError> {
        assert!(cost(0, 0, B_MAX)? > 0);
        assert!(prices(0, 0, B_MAX)?.yes > 0);
        Ok(())
    }

    #[test]
    fn b_above_max_is_rejected() {
        for b in [B_MAX + 1, i128::MAX] {
            assert_eq!(cost(0, 0, b), Err(LmsrError::LiquidityAboveMax(b)));
            assert_eq!(prices(0, 0, b), Err(LmsrError::LiquidityAboveMax(b)));
        }
    }

    #[test]
    fn q_at_max_is_accepted() -> Result<(), LmsrError> {
        assert!(cost(Q_MAX, Q_MAX, B_MIN)? >= Q_MAX);
        assert!(prices(Q_MAX, Q_MAX, B_MIN)?.yes > 0);
        Ok(())
    }

    #[test]
    fn q_above_max_is_rejected() {
        for (q_yes, q_no) in [(Q_MAX + 1, 0), (0, Q_MAX + 1), (i128::MAX, 0)] {
            let offender = if q_yes > Q_MAX { q_yes } else { q_no };
            assert_eq!(
                cost(q_yes, q_no, B_MIN),
                Err(LmsrError::QuantityAboveMax(offender))
            );
            assert_eq!(
                prices(q_yes, q_no, B_MIN),
                Err(LmsrError::QuantityAboveMax(offender))
            );
        }
    }

    #[test]
    fn negative_quantities_are_rejected() {
        for (q_yes, q_no) in [(-1, 0), (0, -1), (i128::MIN, 0)] {
            let offender = if q_yes < 0 { q_yes } else { q_no };
            assert_eq!(
                cost(q_yes, q_no, B_MIN),
                Err(LmsrError::QuantityNegative(offender))
            );
            assert_eq!(
                prices(q_yes, q_no, B_MIN),
                Err(LmsrError::QuantityNegative(offender))
            );
        }
    }

    // ------------------------------------------------- per-step bounds (ADR-0009's derivation)

    /// Step 2: truncation moves the exponent toward zero by less than one wei, never away.
    #[test]
    fn exponent_argument_truncates_toward_zero_by_less_than_one_wei() -> Result<(), LmsrError> {
        for (spread, b) in [
            (0, B_MIN),
            (1, B_MIN),
            (WAD, 3 * WAD),
            (Q_MAX, B_MAX),
            (7, B_MAX),
        ] {
            let z = exponent_argument(spread, b)?;
            assert!(z <= I256::ZERO, "the exponent is never positive");
            // |z| * b <= spread * WAD < (|z| + 1) * b: a truncated quotient, so |z| is never
            // larger than exact, which makes exp(z) and the cost come out high.
            let magnitude = sub(I256::ZERO, z)?;
            let exact_numerator = mul(int(spread)?, int(WAD)?)?;
            let b_wide = int(b)?;
            assert!(
                mul(magnitude, b_wide)? <= exact_numerator,
                "|z| is above exact"
            );
            assert!(
                exact_numerator < mul(add(magnitude, I256::ONE)?, b_wide)?,
                "|z| is more than one wei below exact"
            );
        }
        Ok(())
    }

    /// Steps 3 and 4: `u` stays in `[0, WAD]`, so `WAD + u` is exact and cannot overflow.
    #[test]
    fn one_wad_plus_exp_is_exact_and_cannot_overflow() -> Result<(), LmsrError> {
        let wad = int(WAD)?;
        let two_wad = mul(wad, int(2)?)?;
        for (spread, b) in [
            (0, B_MIN),
            (1, B_MIN),
            (WAD / 2, WAD),
            (Q_MAX, B_MAX),
            (Q_MAX, B_MIN),
        ] {
            let u = exp_of_negative_ratio(spread, b)?;
            assert!(
                u >= I256::ZERO && u <= wad,
                "exp(z) must lie in [0, WAD], got {u}"
            );
            let s = add(wad, u)?;
            assert!(
                s >= wad && s <= two_wad,
                "1 + exp(z) must lie in [1, 2] WAD, got {s}"
            );
        }
        Ok(())
    }

    /// Step 6: `b * L / WAD` truncates by less than one wei, and never rounds up.
    #[test]
    fn tail_scaling_truncates_by_less_than_one_wei() -> Result<(), LmsrError> {
        let wad = int(WAD)?;
        for (spread, b) in [(0, B_MIN), (0, B_MAX), (WAD, 5 * WAD), (Q_MAX / 2, B_MAX)] {
            let log_term = log_one_plus_exp(spread, b)?;
            let tail = int(scale_by_liquidity(log_term, b)?)?;
            let exact = mul(int(b)?, log_term)?;
            assert!(mul(tail, wad)? <= exact, "the tail is above exact");
            assert!(
                exact < mul(add(tail, I256::ONE)?, wad)?,
                "the tail is more than one wei below exact"
            );
        }
        Ok(())
    }

    // ---------------------------------------------------------------- derived bound (ADR-0009)

    #[test]
    fn cost_error_bound_matches_the_derivation() -> Result<(), LmsrError> {
        // E(b) = ceil(4 * b / WAD) + 1
        assert_eq!(cost_error_bound(B_MIN)?, 5);
        assert_eq!(cost_error_bound(2 * WAD)?, 9);
        assert_eq!(cost_error_bound(B_MIN + 1)?, 6, "the ceiling rounds up");
        assert_eq!(cost_error_bound(B_MAX)?, 40_000_001);
        Ok(())
    }

    #[test]
    fn cost_error_bound_stays_far_below_one_base_unit() -> Result<(), LmsrError> {
        // One base unit is 1e12 wei. The worst bound must stay orders of magnitude below it,
        // which is what makes the margin in ADR-0009 safe.
        assert!(
            cost_error_bound(B_MAX)? < 1_000_000_000,
            "E(B_MAX) must be under 1e9 wei"
        );
        assert!(cost_error_bound(B_MAX)? * 1_000 < 1_000_000_000_000);
        Ok(())
    }

    #[test]
    fn cost_error_bound_rejects_liquidity_outside_the_domain() {
        assert_eq!(cost_error_bound(0), Err(LmsrError::NonPositiveLiquidity(0)));
        assert_eq!(
            cost_error_bound(B_MAX + 1),
            Err(LmsrError::LiquidityAboveMax(B_MAX + 1))
        );
    }
}
