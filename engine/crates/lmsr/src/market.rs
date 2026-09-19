//! Binary LMSR cost and price, in the log-sum-exp form (issue #41).
//!
//! Stub for the red commit; the implementation follows.

use crate::LmsrError;

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
/// # Errors
///
/// Stub.
pub fn cost_error_bound(_b: i128) -> Result<i128, LmsrError> {
    Ok(0)
}

/// `C(qY, qN) = max(qY, qN) + b * ln(1 + exp(-|qY - qN| / b))`, in WAD.
///
/// # Errors
///
/// Stub.
pub fn cost(_q_yes: i128, _q_no: i128, _b: i128) -> Result<i128, LmsrError> {
    Ok(0)
}

/// `pY = 1 / (1 + exp((qN - qY) / b))` and `pN = 1 - pY`, in WAD.
///
/// # Errors
///
/// Stub.
pub fn prices(_q_yes: i128, _q_no: i128, _b: i128) -> Result<Prices, LmsrError> {
    Ok(Prices { yes: 0, no: 0 })
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
            let exact = i128::try_from(
                i256_mul_div(b, LN2_WAD, WAD).ok_or(LmsrError::ArithmeticOverflow("test"))?,
            )
            .map_err(|_| LmsrError::ArithmeticOverflow("test"))?;
            let bound = cost_error_bound(b)?;
            assert!(
                (c - exact).abs() <= bound + 1,
                "cost(0, 0, {b}) = {c}, expected about {exact} (bound {bound})"
            );
        }
        Ok(())
    }

    /// `b * ln2` needs 256 bits at `B_MAX`; this keeps the test honest without a float.
    fn i256_mul_div(a: i128, b: i128, d: i128) -> Option<i128> {
        let wide = i256::I256::try_from(a)
            .ok()?
            .checked_mul(i256::I256::try_from(b).ok()?)?;
        i128::try_from(wide.checked_div(i256::I256::try_from(d).ok()?)?).ok()
    }
    use alloy_primitives as i256;

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
