// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {FixedPointMath} from "../src/FixedPointMath.sol";
import {Test} from "forge-std/Test.sol";

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
/// @dev `expectRevert` needs a call frame to watch, and an internal library call is inlined into
///      its caller, so a reverting one is invisible to it. Every case in this suite therefore goes
///      through this harness, including the ones that are expected to return a value: a revert case
///      and a value case must not travel different paths, or the suite proves the paths rather than
///      the library.
contract MathHarness {
    function expWad(int256 x) external pure returns (int256) {
        return FixedPointMath.expWad(x);
    }

    function lnWad(int256 x) external pure returns (int256) {
        return FixedPointMath.lnWad(x);
    }
}

contract FixedPointMathDiffTest is Test {
    MathHarness internal math;
    string internal vectors;

    /// @dev Read from the vector file rather than written here: the file is the independent source.
    int256 internal expOverflowAt;
    int256 internal expZeroAt;

    /// @dev `expWad`'s relative tolerance divisor, from the Rust suite: 1e19.
    int256 internal constant EXP_RELATIVE = 10_000_000_000_000_000_000;

    function setUp() public {
        math = new MathHarness();
        vectors = vm.readFile("../testdata/vectors/lmsr.json");
        expOverflowAt = vm.parseJsonInt(vectors, ".constants.exp_overflow_at");
        expZeroAt = vm.parseJsonInt(vectors, ".constants.exp_zero_at");
    }

    /// The exact value's admissible interval for one case.
    function _exact(string memory at) private view returns (int256 lo, int256 hi) {
        lo = vm.parseJsonInt(vectors, string.concat(at, ".floor"));
        hi = vm.parseJsonBool(vectors, string.concat(at, ".inexact")) ? lo + 1 : lo;
    }

    /// @dev **Why this samples rather than sweeps, and where the sweep lives.**
    ///
    /// Each `parseJson*` call materialises the whole 800 KB vector file in the calling frame, and
    /// EVM memory never shrinks inside a frame, so the cost of reading N cases in one test grows
    /// with N times the file. 1,100 cases times three reads is gigabytes: `forge test` is killed by
    /// the OOM killer on a 3 GB development box and survives on a 16 GB runner. A suite that passes
    /// in CI and dies locally is the exact failure `CLAUDE.md` section 6 calls a bug in the gate,
    /// so it is not shipped.
    ///
    /// The exhaustive sweep already exists and is cheap where it lives:
    /// `engine/crates/lmsr/tests/exp_ln_vectors.rs` checks all 1,100 cases of each function against
    /// this same file, with these same tolerances. What that cannot tell you is whether the
    /// *Solidity* port agrees, which is this suite's job.
    ///
    /// So the selection is a fixed stride over the committed order, which is not a choice this
    /// suite can bias: `STRIDE` is prime and the sampled indices are a function of nothing but the
    /// file's length. Alongside it, every documented boundary is exercised by value below, which is
    /// where a port is most likely to differ.
    uint256 internal constant STRIDE = 13;

    function _path(string memory group, uint256 i) private pure returns (string memory) {
        return string.concat(".", group, "[", vm.toString(i), "].x");
    }

    /// How many cases a group holds, found by doubling and then bisecting rather than by probing
    /// every index: 1,100 probes cost about ten seconds, and about twenty do the same job.
    function _count(string memory group) private view returns (uint256) {
        require(vm.keyExistsJson(vectors, _path(group, 0)), "the group is empty");
        uint256 hi = 1;
        while (vm.keyExistsJson(vectors, _path(group, hi))) hi *= 2;
        uint256 lo = hi / 2;
        while (lo + 1 < hi) {
            uint256 mid = (lo + hi) / 2;
            if (vm.keyExistsJson(vectors, _path(group, mid))) lo = mid;
            else hi = mid;
        }
        return lo + 1;
    }

    function test_expWad_matches_the_committed_vectors_at_every_stride() public {
        uint256 n = _count("exp_wad");
        assertGt(n, 1000, "the exp_wad group shrank");
        uint256 checked;
        for (uint256 i = 0; i < n; i += STRIDE) {
            string memory at = string.concat(".exp_wad[", vm.toString(i), "]");
            int256 x = vm.parseJsonInt(vectors, string.concat(at, ".x"));

            // Which cases the reference refused is derived from the documented domain, not read
            // from the file. If the two ever disagree, the `.floor` read below reverts on a missing
            // key and names the case, so the derivation is checked rather than trusted. Reading a
            // flag instead would make this suite agree with the file about where the domain ends,
            // which is one of the things it is here to test.
            if (x >= expOverflowAt) {
                vm.expectRevert(abi.encodeWithSelector(FixedPointMath.ExpOverflow.selector, x));
                math.expWad(x);
                checked++;
                continue;
            }

            int256 got = math.expWad(x);
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
        assertEq(checked, (n + STRIDE - 1) / STRIDE, "a sampled case was skipped");
    }

    function test_lnWad_matches_the_committed_vectors_at_every_stride() public {
        uint256 n = _count("ln_wad");
        assertGt(n, 1000, "the ln_wad group shrank");
        uint256 checked;
        for (uint256 i = 0; i < n; i += STRIDE) {
            string memory at = string.concat(".ln_wad[", vm.toString(i), "]");
            int256 x = vm.parseJsonInt(vectors, string.concat(at, ".x"));

            if (x <= 0) {
                vm.expectRevert(abi.encodeWithSelector(FixedPointMath.LnUndefined.selector, x));
                math.lnWad(x);
                checked++;
                continue;
            }

            int256 got = math.lnWad(x);
            (int256 lo, int256 hi) = _exact(at);
            assertLe(lo - 1, got, string.concat("lnWad below tolerance at ", at));
            assertGe(hi + 1, got, string.concat("lnWad above tolerance at ", at));
            checked++;
        }
        assertEq(checked, (n + STRIDE - 1) / STRIDE, "a sampled case was skipped");
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
    /// Both sides of the zero point. The accepted edge is the one that says the boundary sits
    /// where it is thought to, rather than one wei off (CLAUDE.md section 5).
    function test_expWad_is_zero_at_and_below_the_zero_point() public view {
        assertEq(math.expWad(expZeroAt - 1), 0, "below the zero point");
        assertEq(math.expWad(expZeroAt), 0, "at the zero point");
        assertGt(math.expWad(expZeroAt + 1), 0, "one wei above it is not zero");
    }

    /// Both sides of the overflow point.
    function test_expWad_overflows_at_the_overflow_point_and_not_below_it() public {
        vm.expectRevert(abi.encodeWithSelector(FixedPointMath.ExpOverflow.selector, expOverflowAt));
        math.expWad(expOverflowAt);
        assertGt(math.expWad(expOverflowAt - 1), 0, "one wei below it still returns");
    }

    function test_lnWad_of_one_wad_is_zero() public view {
        assertEq(math.lnWad(1_000_000_000_000_000_000), 0, "ln(1) is 0");
    }

    /// Both sides of `lnWad`'s domain edge: zero is refused, one wei is not.
    function test_lnWad_refuses_zero_and_accepts_one_wei() public {
        vm.expectRevert(abi.encodeWithSelector(FixedPointMath.LnUndefined.selector, int256(0)));
        math.lnWad(0);
        vm.expectRevert(abi.encodeWithSelector(FixedPointMath.LnUndefined.selector, int256(-1)));
        math.lnWad(-1);
        assertLt(math.lnWad(1), 0, "ln of one wei is negative");
    }
}
