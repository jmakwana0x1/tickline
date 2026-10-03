// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {FixedPointMath} from "../src/FixedPointMath.sol";

/// @title FixedPointMath against the Phase 1 vectors (#103)
/// @notice The vault does not price trades. This suite exists so it can never disagree with the
///         engine that does: `engine/crates/lmsr` and this library are two ports of one method
///         (Solady `9fe23ffd`, ADR-0008), and `testdata/vectors/lmsr.json` is the third opinion,
///         computed independently in Python with 60-digit `mpmath`.
/// @dev The tolerances are not this suite's to choose. They are the ones
///      `engine/crates/lmsr/tests/exp_ln_vectors.rs` asserts, restated here so a reader can see
///      they are the same numbers: `expWad` within `max(floor / 1e19, 1)` of exact for positive
///      arguments and exact for the rest, `lnWad` within one wei. ADR-0009 owns them.
///
///      `floor` in the vector file is the exact value's floor, and `inexact` says whether the exact
///      value sits above it, so the admissible exact interval is `[floor, floor + 1]` when inexact
///      and the single point `floor` when not.
contract FixedPointMathDiffTest is Test {
    string internal vectors;

    /// @dev Read from the vector file rather than written here: the file is the independent source.
    int256 internal expOverflowAt;
    int256 internal expZeroAt;

    /// @dev `expWad`'s relative tolerance divisor, from the Rust suite: 1e19.
    int256 internal constant EXP_RELATIVE = 10_000_000_000_000_000_000;

    function setUp() public {
        vectors = vm.readFile("../testdata/vectors/lmsr.json");
        expOverflowAt = vm.parseJsonInt(vectors, ".constants.exp_overflow_at");
        expZeroAt = vm.parseJsonInt(vectors, ".constants.exp_zero_at");
    }

    /// The exact value's admissible interval for one case.
    function _exact(string memory at) private view returns (int256 lo, int256 hi) {
        lo = vm.parseJsonInt(vectors, string.concat(at, ".floor"));
        hi = vm.parseJsonBool(vectors, string.concat(at, ".inexact")) ? lo + 1 : lo;
    }

    function test_expWad_matches_every_committed_vector() public {
        uint256 checked;
        for (uint256 i = 0;; i++) {
            string memory at = string.concat(".exp_wad[", vm.toString(i), "]");
            if (!vm.keyExists(vectors, string.concat(at, ".x"))) break;
            int256 x = vm.parseJsonInt(vectors, string.concat(at, ".x"));

            // A case with no `floor` is a case the reference refused, and the only reason it does
            // is an argument at or above the overflow point. Asserting the reason, not just the
            // revert, is what makes this a check rather than a coincidence.
            if (!vm.keyExists(vectors, string.concat(at, ".floor"))) {
                assertGe(x, expOverflowAt, "a refused case that is inside the domain");
                vm.expectRevert(abi.encodeWithSelector(FixedPointMath.ExpOverflow.selector, x));
                FixedPointMath.expWad(x);
                checked++;
                continue;
            }

            int256 got = FixedPointMath.expWad(x);
            (int256 lo, int256 hi) = _exact(at);
            if (x > 0) {
                int256 tolerance = lo / EXP_RELATIVE;
                if (tolerance < 1) tolerance = 1;
                lo -= tolerance;
                hi += tolerance;
            }
            assertLe(lo, got, string.concat("expWad below tolerance at ", at));
            assertGe(hi, got, string.concat("expWad above tolerance at ", at));
            checked++;
        }
        assertGt(checked, 1000, "too few expWad cases checked");
    }

    function test_lnWad_matches_every_committed_vector() public {
        uint256 checked;
        for (uint256 i = 0;; i++) {
            string memory at = string.concat(".ln_wad[", vm.toString(i), "]");
            if (!vm.keyExists(vectors, string.concat(at, ".x"))) break;
            int256 x = vm.parseJsonInt(vectors, string.concat(at, ".x"));

            if (!vm.keyExists(vectors, string.concat(at, ".floor"))) {
                assertLe(x, 0, "a refused case that is inside the domain");
                vm.expectRevert(abi.encodeWithSelector(FixedPointMath.LnUndefined.selector, x));
                FixedPointMath.lnWad(x);
                checked++;
                continue;
            }

            int256 got = FixedPointMath.lnWad(x);
            (int256 lo, int256 hi) = _exact(at);
            assertLe(lo - 1, got, string.concat("lnWad below tolerance at ", at));
            assertGe(hi + 1, got, string.concat("lnWad above tolerance at ", at));
            checked++;
        }
        assertGt(checked, 1000, "too few lnWad cases checked");
    }

    /// The vector file is Phase 1's, read and never regenerated here. Both assertions matter: the
    /// numbers must come from the Python oracle rather than from either port, and the Solady commit
    /// the oracle recorded must be the one this library claims to be a port of (ADR-0008). If those
    /// two ever disagree, "three implementations agree" is one implementation counted twice.
    function test_the_vector_file_is_the_one_phase_1_committed() public view {
        assertEq(
            vm.parseJsonString(vectors, ".provenance.generator"),
            "tools/reference/lmsr_ref.py",
            "the vectors must come from the Python oracle, not from either port"
        );
        assertEq(
            vm.parseJsonString(vectors, ".provenance.solady_commit"),
            "9fe23ffdcd395c4169396064226ed27206b222c3",
            "this library is a port of the commit the oracle recorded"
        );
    }

    /// Solady returns zero below the point where the result would be under half a wei, and this is
    /// the accepted edge of that rule rather than only the first rejected value.
    function test_expWad_is_zero_at_and_below_the_zero_point() public pure {
        assertEq(FixedPointMath.expWad(-41_446_531_673_892_822_314), 0, "below");
        assertEq(FixedPointMath.expWad(-41_446_531_673_892_822_313), 0, "at");
    }

    function test_lnWad_of_one_wad_is_zero() public pure {
        assertEq(FixedPointMath.lnWad(1_000_000_000_000_000_000), 0, "ln(1) is 0");
    }
}
