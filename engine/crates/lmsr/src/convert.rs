//! Buying, the subsidy, and the WAD to base-unit boundary (issue #42).
//!
//! Stub for the red commit; the implementation follows.

use alloy_primitives::I256;

use crate::LmsrError;

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

/// Cost of buying `d` shares of `outcome`, in WAD.
///
/// # Errors
///
/// Stub.
pub fn cost_to_buy(
    _q_yes: i128,
    _q_no: i128,
    _b: i128,
    _outcome: Outcome,
    _d: i128,
) -> Result<i128, LmsrError> {
    Ok(0)
}

/// A cost in WAD to USDC base units, rounding up (I15).
///
/// # Errors
///
/// Stub.
pub fn round_cost_up(_wad: I256) -> Result<u128, LmsrError> {
    Ok(0)
}

/// A share count in WAD to base units, rounding down (I15).
///
/// # Errors
///
/// Stub.
pub fn round_shares_down(_wad: I256) -> Result<u128, LmsrError> {
    Ok(0)
}

/// A cost in base units, with the margin that keeps it at or above exact (ADR-0009).
///
/// # Errors
///
/// Stub.
pub fn cost_to_base_units(_cost_wad: i128, _b: i128) -> Result<u128, LmsrError> {
    Ok(0)
}

/// A buy cost in base units, with the double margin a difference of two costs needs (ADR-0009).
///
/// # Errors
///
/// Stub.
pub fn cost_to_buy_base_units(_delta_wad: i128, _b: i128) -> Result<u128, LmsrError> {
    Ok(0)
}

/// The creator's subsidy in base units: `ceil((b * ln2 + E(b)) / 1e12)` (I3, ADR-0009).
///
/// # Errors
///
/// Stub.
pub fn subsidy_base_units(_b: i128) -> Result<u128, LmsrError> {
    Ok(0)
}
