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

/// One unit in signed WAD fixed point: 1e18.
pub const WAD: i128 = 1_000_000_000_000_000_000;

/// Errors returned when an input leaves a function's documented domain.
///
/// Every variant is reachable from a test; none is a catch-all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum LmsrError {
    /// The argument to `exp_wad` was outside the domain where the result fits in a signed WAD.
    #[error("exp argument {0} is outside the representable domain")]
    ExpDomain(i128),
    /// `ln_wad` requires a strictly positive argument.
    #[error("ln argument {0} must be strictly positive")]
    LnDomain(i128),
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
        // DELIBERATELY WRONG. This PR exists to prove `required` blocks a red merge
        // (PHASES.md phase 0). It must never be merged.
        assert_eq!(WAD, 10_i128.pow(17));
    }

    #[test]
    fn errors_render_their_domain() {
        assert_eq!(
            LmsrError::NonPositiveLiquidity(0).to_string(),
            "liquidity parameter b must be strictly positive, got 0"
        );
    }
}
