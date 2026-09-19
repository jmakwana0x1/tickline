//! `exp_wad` and `ln_wad`: Solady's `FixedPointMathLib.expWad` and `lnWad`, ported.
//!
//! Source: Solady `src/utils/FixedPointMathLib.sol` at commit
//! `9fe23ffdcd395c4169396064226ed27206b222c3`, MIT licensed, which credits Remco Bloemen
//! (<https://2π.com/22/exp-ln>), also MIT. Both are rational approximations; see
//! `tests/exp_ln_vectors.rs` for their measured accuracy against exact values.
//!
//! The port follows Solady operation for operation so the engine and the vault share one
//! numerical method (ADR-0008), with one deliberate exception: `lnWad` finds
//! `floor(log2(x))` with a binary search and a byte table, and this port reads it from
//! `bit_len` instead. The integer is the same for every input (proven in the tests), so every
//! output is too, and the port loses a search whose boundary comparisons mutation testing could
//! not distinguish. Where the EVM wraps silently, this code uses checked arithmetic and
//! returns [`LmsrError::ArithmeticOverflow`] instead; inside the documented domains the algorithms
//! never overflow, so that error marks a bug, never a normal result.
//!
//! EVM operation mapping:
//!
//! | EVM / Solidity | Here |
//! |---|---|
//! | `mul`, `add`, `sub` (signed use) | [`mul`], [`add`], [`sub`] (checked) |
//! | `sdiv`, signed `/` (truncates toward zero) | [`div`] (checked) |
//! | `sar`, signed `>>` (rounds toward negative infinity) | [`sar`] |
//! | signed `<<` | [`shl`] (checked) |
//! | `shl`, `shr` on unsigned words | `U256` operations, [`shr_word`] |
//! | `lnWad`'s `log2` binary search and byte table | `256 - bit_len(x)`, proven identical in the tests |

use alloy_primitives::{uint, I256, U256};

use crate::LmsrError;

/// Solady returns 0 for any argument at or below this (`x <= ln(0.5e-18) * 1e18`).
pub const EXP_ZERO_AT: i128 = -41_446_531_673_892_822_313;

/// Solady reverts for any argument at or above this: the result would not fit in `int256`.
pub const EXP_OVERFLOW_AT: i128 = 135_305_999_368_893_231_589;

// ---------------------------------------------------------------------------------- expWad

/// `5 ** 18`: dividing by it after `<< 78` converts a WAD to a 2^96 basis.
/// Solady writes this as the expression `5 ** 18`; `five_pow_18_is_five_to_the_eighteenth` pins it.
const FIVE_POW_18: i128 = 3_814_697_265_625;
/// `ln(2) * 2^96`.
const LN2_X96: i128 = 54_916_777_467_707_473_351_141_471_128;
const EXP_Y0: i128 = 1_346_386_616_545_796_478_920_950_773_328;
const EXP_Y1: i128 = 57_155_421_227_552_351_082_224_309_758_442;
const EXP_P0: i128 = 94_201_549_194_550_492_254_356_042_504_812;
const EXP_P1: i128 = 28_719_021_644_029_726_153_956_944_680_412_240;
/// Shifted left by 96 before use.
const EXP_P2: i128 = 4_385_272_521_454_847_904_659_076_985_693_276;
const EXP_Q0: i128 = 2_855_989_394_907_223_263_936_484_059_900;
const EXP_Q1: i128 = 50_020_603_652_535_783_019_961_831_881_945;
const EXP_Q2: i128 = 533_845_033_583_426_703_283_633_433_725_380;
const EXP_Q3: i128 = 3_604_857_256_930_695_427_073_651_918_091_429;
const EXP_Q4: i128 = 14_423_608_567_350_463_180_887_372_962_807_573;
const EXP_Q5: i128 = 26_449_188_498_355_588_339_934_803_723_976_023;
/// Scale factor, `2^k` and base conversion folded into one unsigned multiply.
const EXP_SCALE: U256 = uint!(3822833074963236453042738258902158003155416615667_U256);

// ---------------------------------------------------------------------------------- lnWad

const LN_P0: i128 = 3_273_285_459_638_523_848_632_254_066_296;
const LN_P1: i128 = 24_828_157_081_833_163_892_658_089_445_524;
const LN_P2: i128 = 43_456_485_725_739_037_958_740_375_743_393;
const LN_P3: i128 = 11_111_509_109_440_967_052_023_855_526_967;
const LN_P4: i128 = 45_023_709_667_254_063_763_336_534_515_857;
const LN_P5: i128 = 14_706_773_417_378_608_786_704_636_184_526;
/// Shifted left by 96 before use.
const LN_P6: i128 = 795_164_235_651_350_426_258_249_787_498;
const LN_Q0: i128 = 5_573_035_233_440_673_466_300_451_813_936;
const LN_Q1: i128 = 71_694_874_799_317_883_764_090_561_454_958;
const LN_Q2: i128 = 283_447_036_172_924_575_727_196_451_306_956;
const LN_Q3: i128 = 401_686_690_394_027_663_651_624_208_769_553;
const LN_Q4: i128 = 204_048_457_590_392_012_362_485_061_816_622;
const LN_Q5: i128 = 31_853_899_698_501_571_402_653_359_427_138;
const LN_Q6: i128 = 909_429_971_244_387_300_277_376_558_375;
/// `s * 5**18 * 2**96`.
const LN_SCALE: I256 = I256::from_raw(uint!(1677202110996718588342820967067443963516166_U256));
/// `ln(2) * 5**18 * 2**192`.
const LN_LN2: I256 = I256::from_raw(uint!(
    16597577552685614221487285958193947469193820559219878177908093499208371_U256
));
/// `ln(2**96 / 10**18) * 5**18 * 2**192`.
const LN_OFFSET: I256 = I256::from_raw(uint!(
    600920179829731861736702779321621459595472258049074101567377883020018308_U256
));

/// `exp(x)` for a signed WAD `x`, as a signed WAD. Solady's `expWad`.
///
/// Domain: returns `0` for `x <= EXP_ZERO_AT` (the result is below half a wei), and is defined
/// up to `EXP_OVERFLOW_AT` (exclusive). An approximation, monotonically increasing.
///
/// # Errors
///
/// [`LmsrError::ExpOverflow`] when `x >= EXP_OVERFLOW_AT`; [`LmsrError::ArithmeticOverflow`]
/// never occurs inside the domain.
pub fn exp_wad(x: I256) -> Result<I256, LmsrError> {
    if x <= int(EXP_ZERO_AT)? {
        return Ok(I256::ZERO);
    }
    if x >= int(EXP_OVERFLOW_AT)? {
        return Err(LmsrError::ExpOverflow(x));
    }

    // Convert from WAD to a 2^96 basis: x * 2^78 / 5^18.
    let x = div(shl(x, 78)?, int(FIVE_POW_18)?)?;

    // k = round(x / ln 2); x' = x - k * ln 2, so exp(x) = exp(x') * 2^k.
    let ln2 = int(LN2_X96)?;
    let k = sar(add(div(shl(x, 96)?, ln2)?, shl(I256::ONE, 95)?)?, 96);
    let x = sub(x, mul(k, ln2)?)?;

    // (6, 7)-term rational approximation.
    let mut y = add(x, int(EXP_Y0)?)?;
    y = add(sar(mul(y, x)?, 96), int(EXP_Y1)?)?;
    let mut p = sub(add(y, x)?, int(EXP_P0)?)?;
    p = add(sar(mul(p, y)?, 96), int(EXP_P1)?)?;
    p = add(mul(p, x)?, shl(int(EXP_P2)?, 96)?)?;

    let mut q = sub(x, int(EXP_Q0)?)?;
    q = add(sar(mul(q, x)?, 96), int(EXP_Q1)?)?;
    q = sub(sar(mul(q, x)?, 96), int(EXP_Q2)?)?;
    q = add(sar(mul(q, x)?, 96), int(EXP_Q3)?)?;
    q = sub(sar(mul(q, x)?, 96), int(EXP_Q4)?)?;
    q = add(sar(mul(q, x)?, 96), int(EXP_Q5)?)?;

    let r = div(p, q)?;

    // r * scale * 2^k * 1e18 / 2^96, as one unsigned multiply and a shift by 195 - k.
    let shift = sub(int(195)?, k)?;
    let shift = usize::try_from(shift).map_err(|_| LmsrError::ArithmeticOverflow("exp shift"))?;
    let scaled = r
        .into_raw()
        .checked_mul(EXP_SCALE)
        .ok_or(LmsrError::ArithmeticOverflow("exp scale"))?;
    // A shift of 256 or more yields 0, as in Solidity. `ruint`'s `>>` already does that;
    // `checked_shr` would not do: it also refuses any shift that drops nonzero bits.
    let out = shr_word(scaled, shift);
    let out = I256::from_raw(out);
    if out.is_negative() {
        return Err(LmsrError::ArithmeticOverflow("exp result"));
    }
    Ok(out)
}

/// `ln(x)` for a signed WAD `x`, as a signed WAD. Solady's `lnWad`.
///
/// Domain: `x > 0`, up to `I256::MAX`. An approximation, monotonically increasing.
///
/// # Errors
///
/// [`LmsrError::LnUndefined`] when `x <= 0`; [`LmsrError::ArithmeticOverflow`] never occurs
/// inside the domain.
pub fn ln_wad(x: I256) -> Result<I256, LmsrError> {
    if x <= I256::ZERO {
        return Err(LmsrError::LnUndefined(x));
    }
    let xu = x.into_raw();

    // r = 255 - floor(log2(x)) = 256 - bit_len(x). Solady computes the same integer with a
    // binary search and a byte-table lookup; `ln_log2_step_equals_solady_search_everywhere`
    // proves the two agree for every possible input (the search depends only on the bit length,
    // the lookup only on the top byte, and the test covers every top byte at every shift).
    let r = 256_usize
        .checked_sub(xu.bit_len())
        .ok_or(LmsrError::ArithmeticOverflow("ln log2"))?;

    // Reduce x to [1, 2) * 2^96: x = (x << r) >> 159. The shift loses no bits by construction.
    let shifted = xu
        .checked_shl(r)
        .ok_or(LmsrError::ArithmeticOverflow("ln normalize"))?;
    let normalized = shr_word(shifted, 159);
    let x = I256::from_raw(normalized);

    // (8, 8)-term rational approximation.
    let mut p = sar(mul(add(int(LN_P0)?, x)?, x)?, 96);
    p = sar(mul(add(int(LN_P1)?, p)?, x)?, 96);
    p = sub(sar(mul(add(int(LN_P2)?, p)?, x)?, 96), int(LN_P3)?)?;
    p = sub(sar(mul(p, x)?, 96), int(LN_P4)?)?;
    p = sub(sar(mul(p, x)?, 96), int(LN_P5)?)?;
    p = sub(mul(p, x)?, shl(int(LN_P6)?, 96)?)?;

    let mut q = add(int(LN_Q0)?, x)?;
    q = add(int(LN_Q1)?, sar(mul(x, q)?, 96))?;
    q = add(int(LN_Q2)?, sar(mul(x, q)?, 96))?;
    q = add(int(LN_Q3)?, sar(mul(x, q)?, 96))?;
    q = add(int(LN_Q4)?, sar(mul(x, q)?, 96))?;
    q = add(int(LN_Q5)?, sar(mul(x, q)?, 96))?;
    q = add(int(LN_Q6)?, sar(mul(x, q)?, 96))?;

    // Finalization: scale, add k * ln 2 and ln(2^96 / 1e18), convert back to WAD.
    p = div(p, q)?;
    p = mul(LN_SCALE, p)?;
    let k = sub(
        int(159)?,
        int(i128::try_from(r).map_err(|_| LmsrError::ArithmeticOverflow("ln k"))?)?,
    )?;
    p = add(mul(LN_LN2, k)?, p)?;
    p = add(LN_OFFSET, p)?;
    Ok(sar(p, 174))
}

// ---------------------------------------------------------------------------------- checked ops

pub(crate) fn int(v: i128) -> Result<I256, LmsrError> {
    I256::try_from(v).map_err(|_| LmsrError::ArithmeticOverflow("constant"))
}

pub(crate) fn mul(a: I256, b: I256) -> Result<I256, LmsrError> {
    a.checked_mul(b).ok_or(LmsrError::ArithmeticOverflow("mul"))
}

pub(crate) fn add(a: I256, b: I256) -> Result<I256, LmsrError> {
    a.checked_add(b).ok_or(LmsrError::ArithmeticOverflow("add"))
}

pub(crate) fn sub(a: I256, b: I256) -> Result<I256, LmsrError> {
    a.checked_sub(b).ok_or(LmsrError::ArithmeticOverflow("sub"))
}

/// Signed division truncating toward zero, like EVM `sdiv`.
pub(crate) fn div(a: I256, b: I256) -> Result<I256, LmsrError> {
    a.checked_div(b).ok_or(LmsrError::ArithmeticOverflow("div"))
}

/// Signed left shift, `a * 2^bits`, refusing to lose bits or flip the sign.
///
/// Written as a checked multiply: `I256::checked_shl` only rejects shift amounts of 256 or more
/// and would silently drop high bits.
fn shl(a: I256, bits: usize) -> Result<I256, LmsrError> {
    let factor = U256::ONE
        .checked_shl(bits)
        .map(I256::from_raw)
        .filter(|f| f.is_positive())
        .ok_or(LmsrError::ArithmeticOverflow("shl"))?;
    a.checked_mul(factor)
        .ok_or(LmsrError::ArithmeticOverflow("shl"))
}

/// Logical right shift of an unsigned word, like EVM `shr`: shifts of 256 or more give 0.
///
/// `ruint`'s `checked_shr` is not this: it also refuses a shift that drops nonzero bits.
fn shr_word(v: U256, bits: usize) -> U256 {
    v.wrapping_shr(bits)
}

/// Arithmetic right shift, rounding toward negative infinity, like EVM `sar`.
fn sar(a: I256, bits: usize) -> I256 {
    a.asr(bits)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn checked_operations_report_overflow_as_typed_errors() {
        let overflow = |op| Err(LmsrError::ArithmeticOverflow(op));
        assert_eq!(mul(I256::MAX, I256::MAX), overflow("mul"));
        assert_eq!(add(I256::MAX, I256::ONE), overflow("add"));
        assert_eq!(sub(I256::MIN, I256::ONE), overflow("sub"));
        assert_eq!(div(I256::ONE, I256::ZERO), overflow("div"));
        assert_eq!(div(I256::MIN, I256::MINUS_ONE), overflow("div"));
        assert_eq!(shl(I256::MAX, 1), overflow("shl"));
        assert_eq!(shl(I256::ONE, 255), overflow("shl"));
        assert_eq!(shl(I256::ONE, 256), overflow("shl"));
    }

    /// `ln_wad` at the edges of its `log2` binary search, where each step's `>` must not be
    /// `>=`. A wrong `r` there still gives an answer within the vector tolerance, so these are
    /// pinned to Solady's exact outputs, produced by running Solady `9fe23ffd` under forge.
    #[test]
    fn ln_wad_matches_solady_exactly_at_log2_search_boundaries() -> Result<(), LmsrError> {
        let cases: [(u128, i128); 5] = [
            (u128::MAX, 47_276_307_437_780_177_293),
            (u128::from(u64::MAX), 2_914_887_881_943_677_490),
            (u128::from(u32::MAX), -19_265_821_896_207_403_055),
            (u128::from(u16::MAX), -30_356_192_043_839_176_368),
            (u128::from(u8::MAX), -35_905_268_128_734_396_167),
        ];
        for (x, solady) in cases {
            let x = I256::try_from(x).map_err(|_| LmsrError::ArithmeticOverflow("test input"))?;
            assert_eq!(ln_wad(x)?, int(solady)?, "ln_wad({x})");
        }
        Ok(())
    }

    /// Solady `lnWad`'s computation of `r = 255 - floor(log2(x))`, kept verbatim as the
    /// reference for the `bit_len` form used above. EVM `byte(i, v)` counts from the most
    /// significant byte; `ruint`'s `byte` counts from the least, hence `31 - i`.
    fn solady_log2_r(x: U256) -> usize {
        const SEQ: U256 = uint!(0x8421084210842108cc6318c6db6d54be_U256);
        const TABLE: U256 =
            uint!(0xf8f9f9faf9fdfafbf9fdfcfdfafbfcfef9fafdfafcfcfbfefafafcfbffffffff_U256);
        let step = |r: usize, limit: U256, bit: usize| -> usize {
            if shr_word(x, r) > limit {
                r | bit
            } else {
                r
            }
        };
        let mut r = 0;
        r = step(r, U256::from(u128::MAX), 128);
        r = step(r, U256::from(u64::MAX), 64);
        r = step(r, U256::from(u32::MAX), 32);
        r = step(r, U256::from(u16::MAX), 16);
        r = step(r, U256::from(u8::MAX), 8);
        let top = usize::try_from(shr_word(x, r)).unwrap_or(usize::MAX);
        let index = usize::try_from(shr_word(SEQ, top) & U256::from(0x1f_u8)).unwrap_or(0);
        r ^ usize::from(TABLE.byte(31_usize.saturating_sub(index)))
    }

    /// Solady's search depends only on `x`'s bit length and its lookup only on the top byte, so
    /// checking every top byte at every shift, plus every 16-bit value, covers every input.
    #[test]
    fn ln_log2_step_equals_solady_search_everywhere() {
        let bit_len_r = |x: U256| 256_usize.saturating_sub(x.bit_len());
        for top in 1_u16..256 {
            for shift in 0_usize..248 {
                let x = U256::from(top) << shift;
                assert_eq!(solady_log2_r(x), bit_len_r(x), "x = {top} << {shift}");
            }
        }
        for v in 1_u32..65_536 {
            let x = U256::from(v);
            assert_eq!(solady_log2_r(x), bit_len_r(x), "x = {v}");
        }
    }

    #[test]
    fn shr_word_matches_evm_shr_including_oversized_shifts() {
        let v = U256::from(0b1011_u8);
        assert_eq!(shr_word(v, 0), v);
        assert_eq!(shr_word(v, 1), U256::from(0b101_u8));
        assert_eq!(shr_word(v, 4), U256::ZERO);
        assert_eq!(shr_word(U256::MAX, 255), U256::ONE);
        assert_eq!(shr_word(U256::MAX, 256), U256::ZERO);
        assert_eq!(shr_word(U256::MAX, 1000), U256::ZERO);
    }

    #[test]
    fn five_pow_18_is_five_to_the_eighteenth() {
        assert_eq!(Some(FIVE_POW_18), 5_i128.checked_pow(18));
    }

    #[test]
    fn checked_operations_match_evm_semantics_in_range() -> Result<(), LmsrError> {
        assert_eq!(div(int(-7)?, int(2)?)?, int(-3)?); // sdiv truncates toward zero
        assert_eq!(sar(int(-7)?, 1), int(-4)?); // sar rounds toward negative infinity
        assert_eq!(shl(int(-3)?, 4)?, int(-48)?);
        assert_eq!(mul(int(-3)?, int(4)?)?, int(-12)?);
        Ok(())
    }
}
