//! `exp_wad` and `ln_wad`: Solady's `FixedPointMathLib.expWad` and `lnWad`, ported.
//!
//! Stub for the red commit of #40; the port follows.

use alloy_primitives::I256;

use crate::LmsrError;

/// Solady returns 0 for any argument at or below this (`x <= ln(0.5e-18) * 1e18`).
pub const EXP_ZERO_AT: i128 = -41_446_531_673_892_822_313;

/// Solady reverts for any argument at or above this: the result would not fit in `int256`.
pub const EXP_OVERFLOW_AT: i128 = 135_305_999_368_893_231_589;

/// `exp(x)` for a signed WAD `x`, as a signed WAD.
///
/// # Errors
///
/// Stub.
pub fn exp_wad(_x: I256) -> Result<I256, LmsrError> {
    Ok(I256::ZERO)
}

/// `ln(x)` for a signed WAD `x`, as a signed WAD.
///
/// # Errors
///
/// Stub.
pub fn ln_wad(_x: I256) -> Result<I256, LmsrError> {
    Ok(I256::ZERO)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(v: i128) -> Result<I256, LmsrError> {
        I256::try_from(v).map_err(|_| LmsrError::ArithmeticOverflow("test constant"))
    }

    #[test]
    fn exp_wad_of_zero_is_one_wad() -> Result<(), LmsrError> {
        assert_eq!(exp_wad(I256::ZERO)?, int(crate::WAD)?);
        Ok(())
    }

    #[test]
    fn exp_wad_returns_zero_below_domain() -> Result<(), LmsrError> {
        for x in [EXP_ZERO_AT, EXP_ZERO_AT - 1, i128::MIN] {
            assert_eq!(exp_wad(int(x)?)?, I256::ZERO, "exp_wad({x})");
        }
        assert_eq!(exp_wad(I256::MIN)?, I256::ZERO);
        // One wei above the cutoff the algorithm runs; exp(-41.4465...) is about 0.5e-18.
        assert!(exp_wad(int(EXP_ZERO_AT + 1)?)? >= I256::ZERO);
        Ok(())
    }

    #[test]
    fn exp_wad_rejects_argument_above_domain() -> Result<(), LmsrError> {
        for x in [int(EXP_OVERFLOW_AT)?, int(EXP_OVERFLOW_AT + 1)?, I256::MAX] {
            assert_eq!(exp_wad(x), Err(LmsrError::ExpOverflow(x)));
        }
        assert!(exp_wad(int(EXP_OVERFLOW_AT - 1)?)? > I256::ZERO);
        Ok(())
    }

    #[test]
    fn ln_wad_of_one_wad_is_zero() -> Result<(), LmsrError> {
        assert_eq!(ln_wad(int(crate::WAD)?)?, I256::ZERO);
        Ok(())
    }

    #[test]
    fn ln_wad_rejects_non_positive_argument() -> Result<(), LmsrError> {
        for x in [I256::ZERO, int(-1)?, I256::MIN] {
            assert_eq!(ln_wad(x), Err(LmsrError::LnUndefined(x)));
        }
        Ok(())
    }
}
