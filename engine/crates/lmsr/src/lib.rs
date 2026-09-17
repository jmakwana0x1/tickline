//! Fixed-point binary LMSR pricing.
//!
//! Phase 1 fills this in. The crate exists from Phase 0 so the harness, the lint
//! profile, and the zero-IO boundary check are proven before any money math lands.
//!
//! # Contract
//!
//! - Signed WAD fixed point, 1e18. No floats anywhere (`clippy::float_arithmetic` is denied).
//! - Zero IO: no clock, no database, no network, no async runtime.
//! - Conversions to USDC base units round **costs up and shares down**, always (I15).

#![forbid(unsafe_code)]

pub mod fixed;

pub use alloy_primitives::I256;
pub use fixed::{exp_wad, ln_wad};

/// One unit in signed WAD fixed point: 1e18.
pub const WAD: i128 = 1_000_000_000_000_000_000;

/// Errors returned when an input leaves a function's documented domain.
///
/// Every variant is reachable from a test; none is a catch-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LmsrError {
    /// `exp_wad`'s result would not fit in a signed 256-bit WAD: the argument is at or above
    /// [`fixed::EXP_OVERFLOW_AT`].
    #[error("exp argument {0} is at or above the representable domain")]
    ExpOverflow(I256),
    /// `ln_wad` is undefined for arguments that are not strictly positive.
    #[error("ln argument {0} must be strictly positive")]
    LnUndefined(I256),
    /// A checked 256-bit operation overflowed. The ported algorithms are designed so this never
    /// happens inside their documented domains; if it does, the result is refused rather than
    /// wrapped the way the EVM would wrap it.
    #[error("256-bit arithmetic overflow in {0}")]
    ArithmeticOverflow(&'static str),
    /// The liquidity parameter `b` must be strictly positive.
    #[error("liquidity parameter b must be strictly positive, got {0}")]
    NonPositiveLiquidity(i128),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 0 smoke test: the Rust harness runs and the crate's constants are sane.
    /// Phase 1 replaces this file with the real unit, property, and differential suites.
    #[test]
    fn wad_is_one_times_ten_to_the_eighteen() {
        assert_eq!(WAD, 10_i128.pow(18));
    }

    #[test]
    fn errors_render_their_domain() {
        assert_eq!(
            LmsrError::NonPositiveLiquidity(0).to_string(),
            "liquidity parameter b must be strictly positive, got 0"
        );
    }
}
