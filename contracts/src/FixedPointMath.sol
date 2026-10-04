// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @title FixedPointMath
/// @notice `expWad` and `lnWad` on signed WAD (1e18) fixed point.
/// @dev Solady's `FixedPointMathLib`, at commit `9fe23ffdcd395c4169396064226ed27206b222c3`, MIT
///      licensed, which credits Remco Bloemen (<https://2pi.com/22/exp-ln>), also MIT.
///
///      This is the third port of one method, and that is the point. `engine/crates/lmsr` ports the
///      same commit to Rust (ADR-0008) and `tools/reference/lmsr_ref.py` computes the same functions
///      independently in 60-digit `mpmath`. The vault does not price trades, so nothing here is
///      called on a trading path; it exists so the vault can never disagree with the engine that
///      does. `test/FixedPointMath.diff.t.sol` checks all three against each other.
///
///      Two deliberate departures from Solady, both of which the Rust port already made:
///
///      1. No assembly. Solady's `lnWad` finds `floor(log2(x))` with a binary search and a byte
///         table; `_msb` here is the same cascade written as Solidity, and the Rust port proved the
///         integer is identical for every input. Nothing here is on a hot path, so the readable form
///         is the right trade.
///      2. Checked arithmetic. Where the EVM wraps silently, Solidity 0.8 reverts. Inside the
///         documented domains these algorithms do not overflow, so a revert from arithmetic marks a
///         bug rather than a normal result, and 2,200 committed vectors are what says so.
///
///      What is *not* departed from: `>>` on a signed value is an arithmetic shift, rounding toward
///      negative infinity, which is what Solady's `sar` does. Replacing any of these with division
///      would round toward zero instead and change results for negative intermediates.
library FixedPointMath {
    /// @dev `x` was at or above `EXP_OVERFLOW_AT`, where the result does not fit `int256`.
    error ExpOverflow(int256 x);
    /// @dev `ln(x)` is undefined for `x <= 0`.
    error LnUndefined(int256 x);

    /// @notice At or below this, `exp(x)` is under half a wei and the result is zero.
    int256 internal constant EXP_ZERO_AT = -41_446_531_673_892_822_313;
    /// @notice At or above this, `exp(x)` does not fit in `int256`.
    int256 internal constant EXP_OVERFLOW_AT = 135_305_999_368_893_231_589;

    /// @dev `5 ** 18`. Dividing by it after `<< 78` converts a WAD to a 2^96 basis.
    int256 private constant FIVE_POW_18 = 3_814_697_265_625;
    /// @dev `ln(2) * 2^96`.
    int256 private constant LN2_X96 = 54_916_777_467_707_473_351_141_471_128;

    int256 private constant EXP_Y0 = 1_346_386_616_545_796_478_920_950_773_328;
    int256 private constant EXP_Y1 = 57_155_421_227_552_351_082_224_309_758_442;
    int256 private constant EXP_P0 = 94_201_549_194_550_492_254_356_042_504_812;
    int256 private constant EXP_P1 = 28_719_021_644_029_726_153_956_944_680_412_240;
    int256 private constant EXP_P2 = 4_385_272_521_454_847_904_659_076_985_693_276;
    int256 private constant EXP_Q0 = 2_855_989_394_907_223_263_936_484_059_900;
    int256 private constant EXP_Q1 = 50_020_603_652_535_783_019_961_831_881_945;
    int256 private constant EXP_Q2 = 533_845_033_583_426_703_283_633_433_725_380;
    int256 private constant EXP_Q3 = 3_604_857_256_930_695_427_073_651_918_091_429;
    int256 private constant EXP_Q4 = 14_423_608_567_350_463_180_887_372_962_807_573;
    int256 private constant EXP_Q5 = 26_449_188_498_355_588_339_934_803_723_976_023;
    /// @dev `2^k` and the base conversion folded into one unsigned multiply.
    uint256 private constant EXP_SCALE =
        3_822_833_074_963_236_453_042_738_258_902_158_003_155_416_615_667;

    int256 private constant LN_P0 = 3_273_285_459_638_523_848_632_254_066_296;
    int256 private constant LN_P1 = 24_828_157_081_833_163_892_658_089_445_524;
    int256 private constant LN_P2 = 43_456_485_725_739_037_958_740_375_743_393;
    int256 private constant LN_P3 = 11_111_509_109_440_967_052_023_855_526_967;
    int256 private constant LN_P4 = 45_023_709_667_254_063_763_336_534_515_857;
    int256 private constant LN_P5 = 14_706_773_417_378_608_786_704_636_184_526;
    int256 private constant LN_P6 = 795_164_235_651_350_426_258_249_787_498;
    int256 private constant LN_Q0 = 5_573_035_233_440_673_466_300_451_813_936;
    int256 private constant LN_Q1 = 71_694_874_799_317_883_764_090_561_454_958;
    int256 private constant LN_Q2 = 283_447_036_172_924_575_727_196_451_306_956;
    int256 private constant LN_Q3 = 401_686_690_394_027_663_651_624_208_769_553;
    int256 private constant LN_Q4 = 204_048_457_590_392_012_362_485_061_816_622;
    int256 private constant LN_Q5 = 31_853_899_698_501_571_402_653_359_427_138;
    int256 private constant LN_Q6 = 909_429_971_244_387_300_277_376_558_375;
    /// @dev `s * 5**18 * 2**96`.
    int256 private constant LN_SCALE = 1_677_202_110_996_718_588_342_820_967_067_443_963_516_166;
    /// @dev `ln(2) * 5**18 * 2**192`.
    int256 private constant LN_LN2 =
        16_597_577_552_685_614_221_487_285_958_193_947_469_193_820_559_219_878_177_908_093_499_208_371;
    /// @dev `ln(2**96 / 10**18) * 5**18 * 2**192`.
    int256 private constant LN_OFFSET =
        600_920_179_829_731_861_736_702_779_321_621_459_595_472_258_049_074_101_567_377_883_020_018_308;

    /// @notice `exp(x)` for a signed WAD `x`, as a signed WAD.
    /// @param x The exponent, in WAD.
    /// @return `exp(x)` in WAD. Zero at or below `EXP_ZERO_AT`.
    /// @dev Reverts `ExpOverflow` at or above `EXP_OVERFLOW_AT`. An approximation, monotonically
    ///      increasing; `test/FixedPointMath.diff.t.sol` pins its accuracy against the vectors.
    function expWad(int256 x) internal pure returns (int256) {
        if (x <= EXP_ZERO_AT) return 0;
        if (x >= EXP_OVERFLOW_AT) revert ExpOverflow(x);

        // WAD to a 2^96 basis: x * 2^78 / 5^18.
        x = (x << 78) / FIVE_POW_18;

        // k = round(x / ln 2), then x' = x - k * ln 2, so exp(x) = exp(x') * 2^k.
        int256 k = (((x << 96) / LN2_X96) + (int256(1) << 95)) >> 96;
        x = x - k * LN2_X96;

        // (6, 7)-term rational approximation.
        int256 y = x + EXP_Y0;
        y = ((y * x) >> 96) + EXP_Y1;
        int256 p = y + x - EXP_P0;
        p = ((p * y) >> 96) + EXP_P1;
        p = p * x + (EXP_P2 << 96);

        int256 q = x - EXP_Q0;
        q = ((q * x) >> 96) + EXP_Q1;
        q = ((q * x) >> 96) - EXP_Q2;
        q = ((q * x) >> 96) + EXP_Q3;
        q = ((q * x) >> 96) - EXP_Q4;
        q = ((q * x) >> 96) + EXP_Q5;

        int256 r = p / q;

        // r * 2^k * 1e18 / 2^96, as one unsigned multiply and a shift by 195 - k. `x` is below
        // EXP_OVERFLOW_AT, which bounds k at 195, so the shift count cannot go negative. Solady
        // relies on the same bound and the Rust port only checks it because `usize::try_from`
        // forces the question; a check here would be a branch no input can reach.
        // forge-lint: disable-next-line(unsafe-typecast)
        return int256((uint256(r) * EXP_SCALE) >> uint256(195 - k));
    }

    /// @notice `ln(x)` for a positive signed WAD `x`, as a signed WAD.
    /// @param x The argument, in WAD. Must be positive.
    /// @return `ln(x)` in WAD.
    /// @dev Reverts `LnUndefined` for `x <= 0`.
    function lnWad(int256 x) internal pure returns (int256) {
        if (x <= 0) revert LnUndefined(x);

        // r = 255 - floor(log2(x)). Solady finds the same integer with a binary search and a byte
        // table; the Rust port proved the two agree for every input.
        // forge-lint: disable-next-line(unsafe-typecast)
        uint256 r = 255 - _msb(uint256(x));

        // Reduce to [1, 2) in the 2^96 basis: (x << r) >> 159. The shift loses no bits.
        // forge-lint: disable-next-line(unsafe-typecast)
        int256 z = int256((uint256(x) << r) >> 159);

        // (8, 8)-term rational approximation.
        int256 p = ((LN_P0 + z) * z) >> 96;
        p = ((LN_P1 + p) * z) >> 96;
        p = (((LN_P2 + p) * z) >> 96) - LN_P3;
        p = ((p * z) >> 96) - LN_P4;
        p = ((p * z) >> 96) - LN_P5;
        p = p * z - (LN_P6 << 96);

        int256 q = LN_Q0 + z;
        q = LN_Q1 + ((z * q) >> 96);
        q = LN_Q2 + ((z * q) >> 96);
        q = LN_Q3 + ((z * q) >> 96);
        q = LN_Q4 + ((z * q) >> 96);
        q = LN_Q5 + ((z * q) >> 96);
        q = LN_Q6 + ((z * q) >> 96);

        // Scale, add k * ln 2 and ln(2^96 / 1e18), then back to WAD.
        p = p / q;
        // forge-lint: disable-next-line(divide-before-multiply)
        p = LN_SCALE * p;
        // forge-lint: disable-next-line(unsafe-typecast)
        p = LN_LN2 * (159 - int256(r)) + p;
        p = LN_OFFSET + p;
        return p >> 174;
    }

    /// @dev `floor(log2(x))` for `x > 0`: the index of the highest set bit. The same cascade Solady
    ///      runs in assembly, which is why the eight steps are 128, 64, 32, 16, 8, 4, 2, 1 and not a
    ///      loop: each halves the remaining range exactly once.
    function _msb(uint256 x) private pure returns (uint256 bit) {
        if (x >= 1 << 128) {
            x >>= 128;
            bit += 128;
        }
        if (x >= 1 << 64) {
            x >>= 64;
            bit += 64;
        }
        if (x >= 1 << 32) {
            x >>= 32;
            bit += 32;
        }
        if (x >= 1 << 16) {
            x >>= 16;
            bit += 16;
        }
        if (x >= 1 << 8) {
            x >>= 8;
            bit += 8;
        }
        if (x >= 1 << 4) {
            x >>= 4;
            bit += 4;
        }
        if (x >= 1 << 2) {
            x >>= 2;
            bit += 2;
        }
        if (x >= 1 << 1) bit += 1;
    }
}
