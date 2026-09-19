//! Buying, the subsidy, and the WAD to base-unit boundary (issue #42).
//!
//! One share pays 1 USDC at resolution, so share and USDC base units are both 1e-6 and a WAD
//! value converts by dividing by 1e12 (D3, #31).
//!
//! **Rounding is the money rule here (I15):** costs round up, shares round down, always. On top of
//! that, every conversion that produces money the vault must hold adds the error bound from
//! ADR-0009 as a margin first, because the WAD value can sit up to that far *below* exact:
//!
//! | Conversion | Margin | Why |
//! |---|---|---|
//! | [`cost_to_base_units`] | `E(b)` | one cost, one bound |
//! | [`cost_to_buy_base_units`] | `2 * E(b)` | a difference of two costs carries two bounds |
//! | [`subsidy_base_units`] | `E(b)` | the vault must hold it, so I3 cannot rest on a low value |
//! | [`round_shares_down`] | none | a margin only ever favours the vault |

use alloy_primitives::I256;

use crate::{
    cost,
    fixed::{add, div, int, sub},
    market::cost_error_bound,
    LmsrError, Q_MAX,
};

/// WAD to USDC or share base units: both have six decimals (D3, #31).
pub const BASE_UNIT_SCALE: i128 = 1_000_000_000_000;

/// Which outcome a buy is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The YES side.
    Yes,
    /// The NO side.
    No,
}

/// Cost of buying `d` shares of `outcome`, in WAD: `C(after) - C(before)`.
///
/// Accurate to within `2 * cost_error_bound(b)`, since it is a difference of two costs.
///
/// # Errors
///
/// [`LmsrError::QuantityNotPositive`] when `d <= 0`; [`LmsrError::QuantityAboveMax`] when the
/// resulting quantity would pass `Q_MAX`; plus every domain error [`cost`] returns.
pub fn cost_to_buy(
    q_yes: i128,
    q_no: i128,
    b: i128,
    outcome: Outcome,
    d: i128,
) -> Result<i128, LmsrError> {
    let before = cost(q_yes, q_no, b)?;
    if d <= 0 {
        return Err(LmsrError::QuantityNotPositive(d));
    }
    let bought = match outcome {
        Outcome::Yes => q_yes,
        Outcome::No => q_no,
    };
    let after_bought = bought
        .checked_add(d)
        .ok_or(LmsrError::ArithmeticOverflow("buy"))?;
    if after_bought > Q_MAX {
        // The resulting quantity is the offending value: the starting one may be perfectly legal.
        return Err(LmsrError::QuantityAboveMax(after_bought));
    }
    let after = match outcome {
        Outcome::Yes => cost(after_bought, q_no, b)?,
        Outcome::No => cost(q_yes, after_bought, b)?,
    };
    after
        .checked_sub(before)
        .ok_or(LmsrError::ArithmeticOverflow("buy"))
}

/// A cost in WAD to USDC base units, rounding **up** (I15).
///
/// # Errors
///
/// [`LmsrError::AmountNegative`] for a negative value; [`LmsrError::AmountAboveU128`] when the
/// result would not fit in `u128`.
pub fn round_cost_up(wad: I256) -> Result<u128, LmsrError> {
    let scale = int(BASE_UNIT_SCALE)?;
    // ceil(wad / scale) = (wad + scale - 1) / scale for a non-negative value.
    let rounded = div(add(non_negative(wad)?, sub(scale, I256::ONE)?)?, scale)?;
    to_u128(rounded, wad)
}

/// A share count in WAD to base units, rounding **down** (I15).
///
/// # Errors
///
/// The same as [`round_cost_up`].
pub fn round_shares_down(wad: I256) -> Result<u128, LmsrError> {
    let rounded = div(non_negative(wad)?, int(BASE_UNIT_SCALE)?)?;
    to_u128(rounded, wad)
}

/// A cost in base units, carrying the margin that keeps it at or above exact (ADR-0009).
///
/// # Errors
///
/// The domain errors of [`cost_error_bound`], plus those of [`round_cost_up`].
pub fn cost_to_base_units(cost_wad: i128, b: i128) -> Result<u128, LmsrError> {
    round_cost_up(with_margin(cost_wad, cost_error_bound(b)?)?)
}

/// A buy cost in base units, carrying the double margin a difference of two costs needs.
///
/// # Errors
///
/// The same as [`cost_to_base_units`].
pub fn cost_to_buy_base_units(delta_wad: i128, b: i128) -> Result<u128, LmsrError> {
    let margin = cost_error_bound(b)?
        .checked_mul(2)
        .ok_or(LmsrError::ArithmeticOverflow("margin"))?;
    round_cost_up(with_margin(delta_wad, margin)?)
}

/// The creator's subsidy in base units: `ceil((b * ln2 + E(b)) / 1e12)` (I3, ADR-0009).
///
/// `b * ln2` is `C(0, 0)`, the most the market maker can ever lose, so this is the worst case the
/// creator funds up front. The margin keeps it from being funded short.
///
/// # Errors
///
/// The domain errors of [`cost`] for `b`.
pub fn subsidy_base_units(b: i128) -> Result<u128, LmsrError> {
    cost_to_base_units(cost(0, 0, b)?, b)
}

/// Adds the margin, in WAD, widening to 256 bits so the sum cannot overflow.
fn with_margin(wad: i128, margin: i128) -> Result<I256, LmsrError> {
    add(int(wad)?, int(margin)?)
}

fn non_negative(wad: I256) -> Result<I256, LmsrError> {
    if wad.is_negative() {
        return Err(LmsrError::AmountNegative(wad));
    }
    Ok(wad)
}

/// Narrows a rounded base-unit value, naming the original in the error.
fn to_u128(rounded: I256, original: I256) -> Result<u128, LmsrError> {
    u128::try_from(rounded).map_err(|_| LmsrError::AmountAboveU128(original))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{market::B_MIN, WAD};

    #[test]
    fn cost_conversion_rounds_up() -> Result<(), LmsrError> {
        assert_eq!(round_cost_up(int(0)?)?, 0);
        assert_eq!(round_cost_up(int(1)?)?, 1, "a single wei costs a base unit");
        assert_eq!(round_cost_up(int(BASE_UNIT_SCALE - 1)?)?, 1);
        assert_eq!(round_cost_up(int(BASE_UNIT_SCALE)?)?, 1);
        assert_eq!(round_cost_up(int(BASE_UNIT_SCALE + 1)?)?, 2);
        assert_eq!(round_cost_up(int(WAD)?)?, 1_000_000);
        Ok(())
    }

    #[test]
    fn share_conversion_rounds_down() -> Result<(), LmsrError> {
        assert_eq!(round_shares_down(int(0)?)?, 0);
        assert_eq!(round_shares_down(int(1)?)?, 0, "a single wei is no share");
        assert_eq!(round_shares_down(int(BASE_UNIT_SCALE - 1)?)?, 0);
        assert_eq!(round_shares_down(int(BASE_UNIT_SCALE)?)?, 1);
        assert_eq!(round_shares_down(int(BASE_UNIT_SCALE + 1)?)?, 1);
        assert_eq!(round_shares_down(int(WAD)?)?, 1_000_000);
        Ok(())
    }

    #[test]
    fn conversions_reject_negative_amounts() -> Result<(), LmsrError> {
        for wad in [int(-1)?, int(-WAD)?, I256::MIN] {
            assert_eq!(round_cost_up(wad), Err(LmsrError::AmountNegative(wad)));
            assert_eq!(round_shares_down(wad), Err(LmsrError::AmountNegative(wad)));
        }
        Ok(())
    }

    #[test]
    fn the_margin_never_lowers_a_cost() -> Result<(), LmsrError> {
        // Adding the margin can only push the conversion up, never down.
        for (wad, b) in [(0, B_MIN), (1, B_MIN), (WAD, B_MIN), (WAD, 1_000 * WAD)] {
            assert!(cost_to_base_units(wad, b)? >= round_cost_up(int(wad)?)?);
            assert!(cost_to_buy_base_units(wad, b)? >= cost_to_base_units(wad, b)?);
        }
        Ok(())
    }

    #[test]
    fn buying_zero_or_fewer_shares_is_rejected() {
        for d in [0, -1, i128::MIN] {
            assert_eq!(
                cost_to_buy(0, 0, B_MIN, Outcome::Yes, d),
                Err(LmsrError::QuantityNotPositive(d))
            );
        }
    }

    #[test]
    fn buying_past_the_quantity_cap_is_rejected() {
        assert_eq!(
            cost_to_buy(Q_MAX, 0, B_MIN, Outcome::Yes, 1),
            Err(LmsrError::QuantityAboveMax(Q_MAX + 1)),
            "the resulting quantity is the offending one"
        );
        assert_eq!(
            cost_to_buy(0, 0, B_MIN, Outcome::No, Q_MAX + 1),
            Err(LmsrError::QuantityAboveMax(Q_MAX + 1))
        );
    }

    /// One wei of shares costs less than one wei of WAD, so the WAD figure truncates to zero.
    /// The charge is positive where it matters: in base units, where the margin and the round-up
    /// make any buy cost at least one base unit (I15).
    #[test]
    fn buying_always_costs_at_least_one_base_unit() -> Result<(), LmsrError> {
        for (outcome, d) in [(Outcome::Yes, 1), (Outcome::No, 1), (Outcome::Yes, WAD)] {
            let wad = cost_to_buy(0, 0, B_MIN, outcome, d)?;
            assert!(wad >= 0, "a buy never costs less than nothing");
            assert!(
                cost_to_buy_base_units(wad, B_MIN)? >= 1,
                "buying {d} of {outcome:?} must cost at least one base unit"
            );
        }
        assert!(
            cost_to_buy(0, 0, B_MIN, Outcome::Yes, WAD)? > 0,
            "a whole share shows up in WAD"
        );
        Ok(())
    }

    #[test]
    fn buying_is_symmetric_between_outcomes() -> Result<(), LmsrError> {
        let yes = cost_to_buy(3 * WAD, WAD, 2 * WAD, Outcome::Yes, WAD)?;
        let no = cost_to_buy(WAD, 3 * WAD, 2 * WAD, Outcome::No, WAD)?;
        assert_eq!(yes, no);
        Ok(())
    }

    #[test]
    fn subsidy_covers_the_cost_at_the_origin() -> Result<(), LmsrError> {
        // I3: the subsidy is at least C(0, 0) converted upward.
        for b in [B_MIN, 1_000 * WAD, crate::market::B_MAX] {
            let at_origin = cost(0, 0, b)?;
            assert!(subsidy_base_units(b)? >= round_cost_up(int(at_origin)?)?);
        }
        Ok(())
    }

    #[test]
    fn subsidy_rejects_liquidity_outside_the_domain() {
        assert_eq!(
            subsidy_base_units(0),
            Err(LmsrError::NonPositiveLiquidity(0))
        );
        assert_eq!(
            subsidy_base_units(crate::market::B_MAX + 1),
            Err(LmsrError::LiquidityAboveMax(crate::market::B_MAX + 1))
        );
    }
}
