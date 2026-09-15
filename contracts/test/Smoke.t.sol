// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";

/// @title Phase 0 smoke test.
/// @notice Proves the Foundry harness, the solc pin, and the three profiles all work before
///         any vault code exists. Phase 3 replaces this with the unit, fuzz, and invariant
///         suites for `TicklineVault`.
contract SmokeTest is Test {
    /// @dev The one thing worth asserting at Phase 0: this project's arithmetic is checked.
    ///      Solidity 0.8 reverts on overflow, and no `unchecked` block enters the vault
    ///      without a comment justifying it (CLAUDE.md section 8).
    function test_arithmetic_reverts_on_overflow() public {
        uint256 max = type(uint256).max;
        vm.expectRevert(stdError.arithmeticError);
        this.addOne(max);
    }

    /// @dev Fuzz harness smoke: runs at the profile's configured `runs` count.
    function testFuzz_additionIsCommutative(uint128 a, uint128 b) public pure {
        assertEq(uint256(a) + uint256(b), uint256(b) + uint256(a));
    }

    function addOne(uint256 x) external pure returns (uint256) {
        return x + 1;
    }
}
