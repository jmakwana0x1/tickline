// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {TicklineTypes} from "../src/TicklineTypes.sol";
import {TicklineVault} from "../src/TicklineVault.sol";
import {MockUSDC} from "./mocks/MockUSDC.sol";
import {Test} from "forge-std/Test.sol";

/// @title TicklineVault: createMarket (#96)
/// @notice A market is the key every position, commit and claim is filed under, so this slice is
///         about identity and about refusing a market that cannot work.
/// @dev The subsidy floor is **not** here. Enforcing `subsidy >= b * ln2` onchain needs a decision
///      about which side of ADR-0009's approximation margin the vault should sit on, and that is a
///      money question open on #96. Everything else in the slice does not depend on it.
contract TicklineVaultMarketTest is Test {
    TicklineVault internal vault;
    MockUSDC internal usdc;

    address internal operator = makeAddr("operator");
    address internal signingKey = makeAddr("signingKey");
    address internal creator = makeAddr("creator");
    address internal stranger = makeAddr("stranger");

    /// @dev 1,000 USDC in base units, written out (CLAUDE.md section 5).
    uint128 internal constant MIN_BOND = 1_000_000_000;
    /// @dev 1,000 shares of liquidity in WAD, inside `lmsr`'s documented [1e18, 1e25] range.
    uint128 internal constant B = 1_000_000_000_000_000_000_000;
    uint32 internal constant EPOCH_LENGTH = 3600;
    uint128 internal constant SUBSIDY = 693_148_000;

    event MarketCreated(
        bytes32 indexed marketId,
        address indexed operator,
        address indexed creator,
        uint64 deadline,
        uint128 b,
        uint32 epochLength,
        uint128 subsidy
    );

    function setUp() public {
        usdc = new MockUSDC();
        vault = new TicklineVault(address(usdc), MIN_BOND);
        usdc.mint(operator, 10 * uint256(MIN_BOND));
        vm.startPrank(operator);
        usdc.approve(address(vault), type(uint256).max);
        vault.registerOperator(signingKey, MIN_BOND);
        vm.stopPrank();
        vm.warp(1_760_000_000);
    }

    function _params() internal view returns (TicklineTypes.MarketParams memory p) {
        p = TicklineTypes.MarketParams({
            creator: creator,
            templateId: keccak256("pyth-threshold-v1"),
            templateParamsHash: keccak256("params"),
            deadline: uint64(block.timestamp + 7 days),
            b: B,
            epochLength: EPOCH_LENGTH,
            salt: bytes32(uint256(1))
        });
    }

    function _create() internal returns (bytes32) {
        vm.prank(operator);
        return vault.createMarket(_params(), SUBSIDY);
    }

    function test_create_market_stores_every_parameter() public {
        bytes32 id = _create();
        TicklineTypes.MarketParams memory p = _params();
        (
            address op,
            address cr,
            bytes32 templateId,
            bytes32 templateParamsHash,
            uint64 deadline,
            uint128 b,
            uint32 epochLength,
            uint128 subsidy
        ) = vault.markets(id);
        assertEq(op, operator, "operator");
        assertEq(cr, p.creator, "creator");
        assertEq(templateId, p.templateId, "templateId");
        assertEq(templateParamsHash, p.templateParamsHash, "templateParamsHash");
        assertEq(deadline, p.deadline, "deadline");
        assertEq(b, p.b, "b");
        assertEq(epochLength, p.epochLength, "epochLength");
        assertEq(subsidy, SUBSIDY, "subsidy");
    }

    /// Phase 2 already proved `TicklineTypes.marketId` against the committed vectors, and three
    /// implementations agree on it. What was unproved is narrower: that the vault passes its own
    /// address and the live chain id, rather than something that merely encodes cleanly
    /// (CLAUDE.md section 5).
    function test_the_stored_id_is_market_id_of_this_chain_and_this_vault() public {
        bytes32 id = _create();
        assertEq(
            id,
            TicklineTypes.marketId(block.chainid, address(vault), _params()),
            "the vault must hash its own address and this chain"
        );
    }

    function test_the_same_params_at_a_different_vault_address_give_a_different_id() public {
        bytes32 id = _create();
        TicklineVault other = new TicklineVault(address(usdc), MIN_BOND);
        assertTrue(
            id != TicklineTypes.marketId(block.chainid, address(other), _params()),
            "two vaults must not share a market id"
        );
    }

    function test_the_same_params_on_a_different_chain_give_a_different_id() public {
        bytes32 id = _create();
        assertTrue(
            id != TicklineTypes.marketId(block.chainid + 1, address(vault), _params()),
            "two chains must not share a market id"
        );
    }

    function test_creating_the_same_market_twice_is_rejected() public {
        _create();
        vm.prank(operator);
        vm.expectRevert(TicklineVault.MarketExists.selector);
        vault.createMarket(_params(), SUBSIDY);
    }

    function test_a_different_salt_creates_a_second_market() public {
        bytes32 first = _create();
        TicklineTypes.MarketParams memory p = _params();
        p.salt = bytes32(uint256(2));
        vm.prank(operator);
        bytes32 second = vault.createMarket(p, SUBSIDY);
        assertTrue(first != second, "the salt must distinguish two otherwise identical markets");
    }

    function test_an_unregistered_caller_cannot_create_a_market() public {
        vm.prank(stranger);
        vm.expectRevert(TicklineVault.NotRegistered.selector);
        vault.createMarket(_params(), SUBSIDY);
    }

    function test_create_market_below_the_minimum_bond_reverts() public {
        vm.prank(operator);
        vault.withdrawBond(1);
        vm.prank(operator);
        vm.expectRevert(TicklineVault.BondBelowMinimum.selector);
        vault.createMarket(_params(), SUBSIDY);
    }

    /// The accepted boundary, not only the first rejected one.
    function test_create_market_at_exactly_the_minimum_bond_is_accepted() public {
        (, uint128 bond) = vault.operators(operator);
        assertEq(bond, MIN_BOND, "the fixture must sit exactly at the minimum");
        bytes32 id = _create();
        (address op,,,,,,,) = vault.markets(id);
        assertEq(op, operator, "a bond exactly at the minimum must be enough");
    }

    function test_a_deadline_in_the_past_is_rejected() public {
        TicklineTypes.MarketParams memory p = _params();
        p.deadline = uint64(block.timestamp - 1);
        vm.prank(operator);
        vm.expectRevert(TicklineVault.DeadlineNotInTheFuture.selector);
        vault.createMarket(p, SUBSIDY);
    }

    function test_a_deadline_exactly_now_is_rejected() public {
        TicklineTypes.MarketParams memory p = _params();
        p.deadline = uint64(block.timestamp);
        vm.prank(operator);
        vm.expectRevert(TicklineVault.DeadlineNotInTheFuture.selector);
        vault.createMarket(p, SUBSIDY);
    }

    /// One second ahead is the smallest accepted deadline, which is what says the boundary sits
    /// where it is thought to rather than one second off.
    function test_a_deadline_one_second_from_now_is_accepted() public {
        TicklineTypes.MarketParams memory p = _params();
        p.deadline = uint64(block.timestamp + 1);
        vm.prank(operator);
        bytes32 id = vault.createMarket(p, SUBSIDY);
        (,,,, uint64 deadline,,,) = vault.markets(id);
        assertEq(deadline, p.deadline, "deadline");
    }

    function test_a_zero_epoch_length_is_rejected() public {
        TicklineTypes.MarketParams memory p = _params();
        p.epochLength = 0;
        vm.prank(operator);
        vm.expectRevert(TicklineVault.ZeroEpochLength.selector);
        vault.createMarket(p, SUBSIDY);
    }

    function test_a_zero_b_is_rejected() public {
        TicklineTypes.MarketParams memory p = _params();
        p.b = 0;
        vm.prank(operator);
        vm.expectRevert(TicklineVault.ZeroLiquidity.selector);
        vault.createMarket(p, SUBSIDY);
    }

    function test_a_zero_creator_is_rejected() public {
        TicklineTypes.MarketParams memory p = _params();
        p.creator = address(0);
        vm.prank(operator);
        vm.expectRevert(TicklineVault.ZeroCreator.selector);
        vault.createMarket(p, SUBSIDY);
    }

    /// A market whose template the vault does not recognise is a market nobody can resolve, which
    /// would strand every agent who paid into it. Cheaper to refuse at creation than to discover.
    function test_an_unknown_template_is_rejected() public {
        TicklineTypes.MarketParams memory p = _params();
        p.templateId = keccak256("twap-v1");
        vm.prank(operator);
        vm.expectRevert(TicklineVault.UnknownTemplate.selector);
        vault.createMarket(p, SUBSIDY);
    }

    /// The template id is the hash of its name, and the committed vectors say so independently.
    function test_the_known_template_id_is_the_hash_of_its_name() public view {
        string memory vectors = vm.readFile("../testdata/vectors/eip712.json");
        assertEq(
            vault.PYTH_THRESHOLD_V1(),
            vm.parseJsonBytes32(vectors, ".market_id.template.template_id"),
            "the vault's template id must be the committed one"
        );
        assertEq(
            vault.PYTH_THRESHOLD_V1(),
            keccak256("pyth-threshold-v1"),
            "and it must be the hash of the template's name"
        );
    }

    function test_create_market_emits_market_created() public {
        TicklineTypes.MarketParams memory p = _params();
        bytes32 expected = TicklineTypes.marketId(block.chainid, address(vault), p);
        vm.expectEmit(true, true, true, true, address(vault));
        emit MarketCreated(expected, operator, creator, p.deadline, p.b, p.epochLength, SUBSIDY);
        vm.prank(operator);
        vault.createMarket(p, SUBSIDY);
    }

    function test_creating_a_market_makes_it_live_for_the_operator() public {
        assertEq(vault.liveMarkets(operator), 0, "none before");
        _create();
        assertEq(vault.liveMarkets(operator), 1, "one after");
    }

    function test_withdrawing_below_the_minimum_is_rejected_while_markets_are_live() public {
        _create();
        vm.prank(operator);
        vm.expectRevert(TicklineVault.BondBelowMinimum.selector);
        vault.withdrawBond(1);
    }

    /// The accepted edge of the same rule: the floor is the minimum, not the whole bond.
    function test_withdrawing_down_to_the_minimum_is_allowed_while_markets_are_live() public {
        vm.prank(operator);
        vault.increaseBond(MIN_BOND);
        _create();
        vm.prank(operator);
        vault.withdrawBond(MIN_BOND);
        (, uint128 bond) = vault.operators(operator);
        assertEq(bond, MIN_BOND, "the operator may withdraw down to the floor and no further");
    }
}
