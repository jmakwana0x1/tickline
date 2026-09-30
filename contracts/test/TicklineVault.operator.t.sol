// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {MockUSDC} from "./mocks/MockUSDC.sol";
import {TicklineVault} from "../src/TicklineVault.sol";

/// @title TicklineVault: operator registration and bond (#95)
/// @notice The bond is what every guarantee in CLAUDE.md section 4 is paid from: I12 says a
///         dishonest commit is always slashable, and "always" means the bond is there and its
///         accounting is exact. So this suite is about arithmetic and authority, not features.
/// @dev Key rotation is deliberately absent (#95): honouring a receipt signed by a retired key
///      changes how I12 is enforced and needs an ADR first.
contract TicklineVaultOperatorTest is Test {
    TicklineVault internal vault;
    MockUSDC internal usdc;

    // Named, deterministic addresses: a failure names the party rather than a hex blob.
    address internal operator = makeAddr("operator");
    address internal signingKey = makeAddr("signingKey");
    address internal stranger = makeAddr("stranger");

    /// @dev 1,000 USDC in base units. Written out rather than rebuilt from decimals(): a constant
    ///      is pinned only by something independent of the expression (CLAUDE.md section 5).
    uint128 internal constant MIN_BOND = 1_000_000_000;

    event OperatorRegistered(address indexed operator, address indexed signingKey, uint128 bond);
    event BondIncreased(address indexed operator, uint128 added, uint128 total);
    event BondWithdrawn(address indexed operator, uint128 removed, uint128 total);

    function setUp() public {
        usdc = new MockUSDC();
        vault = new TicklineVault(address(usdc), MIN_BOND);
        usdc.mint(operator, 10 * uint256(MIN_BOND));
        vm.prank(operator);
        usdc.approve(address(vault), type(uint256).max);
    }

    function test_register_stores_the_signing_key_and_bond() public {
        vm.prank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        (address key, uint128 bond) = vault.operators(operator);
        assertEq(key, signingKey, "signing key");
        assertEq(bond, MIN_BOND, "bond");
    }

    function test_register_pulls_the_bond_in_usdc() public {
        uint256 before = usdc.balanceOf(operator);
        vm.prank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        assertEq(usdc.balanceOf(operator), before - MIN_BOND, "operator debited");
        assertEq(usdc.balanceOf(address(vault)), MIN_BOND, "vault credited");
    }

    function test_register_emits_operator_registered() public {
        vm.expectEmit(true, true, false, true, address(vault));
        emit OperatorRegistered(operator, signingKey, MIN_BOND);
        vm.prank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
    }

    function test_registering_twice_reverts() public {
        vm.startPrank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        vm.expectRevert(TicklineVault.AlreadyRegistered.selector);
        vault.registerOperator(signingKey, MIN_BOND);
        vm.stopPrank();
    }

    function test_a_zero_signing_key_is_rejected() public {
        vm.prank(operator);
        vm.expectRevert(TicklineVault.ZeroSigningKey.selector);
        vault.registerOperator(address(0), MIN_BOND);
    }

    /// The minimum is a condition on creating markets (#96), not on existing as an operator.
    function test_a_zero_bond_registers() public {
        vm.prank(operator);
        vault.registerOperator(signingKey, 0);
        (address key, uint128 bond) = vault.operators(operator);
        assertEq(key, signingKey, "signing key");
        assertEq(bond, 0, "bond");
    }

    function test_bond_can_be_raised() public {
        vm.startPrank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        vault.increaseBond(MIN_BOND);
        vm.stopPrank();
        (, uint128 bond) = vault.operators(operator);
        assertEq(bond, 2 * MIN_BOND, "bond");
    }

    function test_raising_the_bond_pulls_only_the_difference() public {
        vm.startPrank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        uint256 before = usdc.balanceOf(operator);
        vault.increaseBond(MIN_BOND);
        vm.stopPrank();
        assertEq(usdc.balanceOf(operator), before - MIN_BOND, "only the increase is pulled");
    }

    function test_raising_the_bond_emits_bond_increased() public {
        vm.startPrank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        vm.expectEmit(true, false, false, true, address(vault));
        emit BondIncreased(operator, MIN_BOND, 2 * MIN_BOND);
        vault.increaseBond(MIN_BOND);
        vm.stopPrank();
    }

    function test_withdraw_returns_usdc_to_the_operator() public {
        vm.startPrank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        uint256 before = usdc.balanceOf(operator);
        vault.withdrawBond(MIN_BOND / 2);
        vm.stopPrank();
        assertEq(usdc.balanceOf(operator), before + MIN_BOND / 2, "operator credited");
        (, uint128 bond) = vault.operators(operator);
        assertEq(bond, MIN_BOND / 2, "bond reduced");
    }

    function test_withdraw_emits_bond_withdrawn() public {
        vm.startPrank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        vm.expectEmit(true, false, false, true, address(vault));
        emit BondWithdrawn(operator, MIN_BOND, 0);
        vault.withdrawBond(MIN_BOND);
        vm.stopPrank();
    }

    function test_withdrawing_more_than_the_bond_reverts() public {
        vm.startPrank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        vm.expectRevert(TicklineVault.BondTooLow.selector);
        vault.withdrawBond(MIN_BOND + 1);
        vm.stopPrank();
    }

    /// The accepted boundary, not only the first rejected one (CLAUDE.md section 5).
    function test_withdrawing_the_entire_bond_is_allowed_with_no_live_markets() public {
        vm.startPrank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        vault.withdrawBond(MIN_BOND);
        vm.stopPrank();
        (, uint128 bond) = vault.operators(operator);
        assertEq(bond, 0, "bond");
    }

    function test_an_unregistered_operator_cannot_raise_the_bond() public {
        vm.prank(stranger);
        vm.expectRevert(TicklineVault.NotRegistered.selector);
        vault.increaseBond(MIN_BOND);
    }

    function test_an_unregistered_operator_cannot_withdraw() public {
        vm.prank(stranger);
        vm.expectRevert(TicklineVault.NotRegistered.selector);
        vault.withdrawBond(1);
    }

    /// A token that reports failure without reverting must not leave a bonded operator behind.
    function test_a_failed_usdc_transfer_reverts_the_registration() public {
        usdc.setFailNextTransfer(true);
        vm.prank(operator);
        vm.expectRevert(TicklineVault.TransferFailed.selector);
        vault.registerOperator(signingKey, MIN_BOND);
        (address key,) = vault.operators(operator);
        assertEq(key, address(0), "no operator was stored");
    }

    function test_a_failed_usdc_transfer_reverts_a_withdrawal() public {
        vm.startPrank(operator);
        vault.registerOperator(signingKey, MIN_BOND);
        usdc.setFailNextTransfer(true);
        vm.expectRevert(TicklineVault.TransferFailed.selector);
        vault.withdrawBond(MIN_BOND);
        vm.stopPrank();
    }

    function test_the_bond_is_denominated_in_the_usdc_the_vault_was_built_with() public view {
        assertEq(vault.usdc(), address(usdc), "usdc");
        assertEq(vault.minBond(), MIN_BOND, "minBond");
    }
}
