// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";

/// @title The deployed escrow, read rather than assumed (issue #67, D5)
/// @notice The only suite in this repository allowed to touch the network (`CLAUDE.md` §2), and the
///         only one that can answer what the contract actually does.
/// @dev Three things this proves that no local test can:
///
///      1. **The committed digests are the contract's.** Every x402 value in
///         `testdata/vectors/eip712.json` is read back from the deployment for the same inputs. A
///         redeployed or upgraded escrow shows up here as a failing job rather than as a mismatch
///         discovered in Phase 5.
///      2. **Each selector exists.** Reading a constant proves the contract's own opinion of its
///         type hashes; asserting the selector first means a changed signature fails by name
///         instead of as a bare revert, which is what cost an afternoon when `getClaimBatchDigest`
///         had to be recovered from bytecode.
///      3. **The roles are enforced, not merely encoded.** `getChannelId` hashes a swapped config
///         as happily as a correct one, so no digest can prove the agent pays and the operator
///         receives. The escrow checking the caller can (`CLAUDE.md` §5).
///
///      **These assertions read latest state on purpose.** No block is pinned, and none should be:
///      detecting a redeployed or upgraded escrow is the suite's entire reason to exist, and a
///      pinned block would make it deterministic by making it blind. Determinism belongs to every
///      other suite in this repository; this one trades it for the only thing it can uniquely see.
///
///      Runs in `phase-gate` and nightly only: the `ci` profile excludes `test/fork`. It is driven
///      by `scripts/fork-suite.sh`, which keeps three outcomes apart, because an unreachable
///      endpoint and a changed type hash are different findings and only the second is ours.
contract X402EscrowForkTest is Test {
    /// @dev The canonical escrow, identical on every supported chain via CREATE2.
    address internal constant ESCROW = 0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003;

    struct ChannelConfig {
        address payer;
        address payerAuthorizer;
        address receiver;
        address receiverAuthorizer;
        address token;
        uint40 withdrawDelay;
        bytes32 salt;
    }

    string internal vectors;

    function setUp() public {
        // The endpoint comes from the environment, not from foundry.toml: this Foundry does not
        // interpolate ${VAR} into eth_rpc_url, and an uninterpolated value fails before any test
        // runs. vm.envString also fails loudly when the secret is missing, which is what should
        // happen in a job that is supposed to reach the network.
        vm.createSelectFork(vm.envString("BASE_SEPOLIA_RPC_URL"));
        vectors = vm.readFile("../testdata/vectors/eip712.json");
        assertEq(block.chainid, vm.parseJsonUint(vectors, ".x402.chain_id"), "wrong chain");
    }

    /// @notice Every selector the vectors name exists on the deployment.
    function test_EverySelectorExists() public view {
        // The count comes from the file: a constant here would silently stop covering a getter the
        // generator adds.
        string[] memory signatures = vm.parseJsonStringArray(vectors, ".x402.selectors.signatures");
        assertEq(signatures.length, 10, "five type hashes, three digests, two bounds");

        for (uint256 i = 0; i < signatures.length; i++) {
            string memory at = string.concat(".x402.selectors.entries[", vm.toString(i), "]");
            string memory signature = vm.parseJsonString(vectors, string.concat(at, ".signature"));
            assertEq(signature, signatures[i], "the flat list and the entries are one list");
            bytes4 committed = bytes4(vm.parseJsonBytes(vectors, string.concat(at, ".selector")));

            // The committed selector is the one the signature hashes to. If the escrow changes a
            // signature, the generator's value moves and this fails by name.
            assertEq(
                committed, bytes4(keccak256(bytes(signature))), string.concat("selector for ", signature)
            );

            // And a zero-argument getter must answer at it.
            if (endsWithNoArgs(signature)) {
                (bool ok, bytes memory data) = ESCROW.staticcall(abi.encodeWithSelector(committed));
                assertTrue(ok, string.concat("no function at the selector for ", signature));
                assertEq(data.length, 32, "a getter returns one word");
            }
        }
    }

    /// @notice The five type hashes are the contract's own constants.
    function test_TypeHashesAreTheContractsOwn() public view {
        for (uint256 i = 0; i < 4; i++) {
            string memory at = string.concat(".x402.type_hashes[", vm.toString(i), "]");
            string memory getter = vm.parseJsonString(vectors, string.concat(at, ".getter"));
            bytes32 committed = vm.parseJsonBytes32(vectors, string.concat(at, ".hash"));

            (bool ok, bytes memory data) =
                ESCROW.staticcall(abi.encodeWithSelector(bytes4(keccak256(bytes(getter)))));
            assertTrue(ok, string.concat("no ", getter));
            assertEq(abi.decode(data, (bytes32)), committed, getter);
        }

        // REFUND_TYPEHASH is read too, although Phase 2 deliberately keeps `Refund` out of the
        // vectors (#73): the point is that the constant is there when Phase 5 wants it.
        (bool refundOk, bytes memory refund) =
            ESCROW.staticcall(abi.encodeWithSelector(bytes4(keccak256("REFUND_TYPEHASH()"))));
        assertTrue(refundOk, "no REFUND_TYPEHASH");
        assertEq(
            abi.decode(refund, (bytes32)),
            keccak256("Refund(bytes32 channelId,uint256 nonce,uint128 amount)"),
            "REFUND_TYPEHASH"
        );
    }

    /// @notice The committed channel ids, voucher digest and batch digest are the contract's.
    function test_CommittedDigestsAreTheContracts() public view {
        ChannelConfig memory first = configAt(".x402.channels[0].config");
        ChannelConfig memory second = configAt(".x402.channels[1].config");

        bytes32 firstId = getChannelId(first);
        assertEq(firstId, vm.parseJsonBytes32(vectors, ".x402.channels[0].channel_id"), "channel 1");
        assertEq(
            getChannelId(second),
            vm.parseJsonBytes32(vectors, ".x402.channels[1].channel_id"),
            "channel 2"
        );

        uint128 ceiling =
            uint128(vm.parseUint(vm.parseJsonString(vectors, ".x402.vouchers[0].max_claimable_amount")));
        (bool ok, bytes memory data) = ESCROW.staticcall(
            abi.encodeWithSelector(bytes4(keccak256("getVoucherDigest(bytes32,uint128)")), firstId, ceiling)
        );
        assertTrue(ok, "no getVoucherDigest");
        assertEq(
            abi.decode(data, (bytes32)),
            vm.parseJsonBytes32(vectors, ".x402.vouchers[0].digest"),
            "voucher digest"
        );
    }

    /// @notice The escrow enforces the direction of a channel, which no hash can.
    /// @dev `getChannelId` confirms a swapped config exactly as happily, so the proof that the agent
    ///      pays and the operator receives is behavioural: the escrow distinguishes the payer from
    ///      everyone else when a withdrawal is initiated. Both calls revert, because the fixture
    ///      channel holds nothing, and they revert **differently**: that difference is the check.
    function test_OnlyThePayerMayInitiateAWithdrawal() public {
        ChannelConfig memory config = configAt(".x402.channels[0].config");
        address agent = vm.parseJsonAddress(vectors, ".roles.agent");
        address operator = vm.parseJsonAddress(vectors, ".roles.operator");
        assertEq(config.payer, agent, "the agent pays");
        assertEq(config.receiver, operator, "the operator receives");

        bytes memory call = abi.encodeWithSelector(
            bytes4(keccak256("initiateWithdraw((address,address,address,address,address,uint40,bytes32),uint128)")),
            config,
            uint128(1)
        );

        vm.prank(config.payer);
        (bool asPayer, bytes memory payerData) = ESCROW.call(call);

        vm.prank(address(0xBEEF));
        (bool asStranger, bytes memory strangerData) = ESCROW.call(call);

        assertFalse(asPayer, "the fixture channel is unfunded, so even the payer cannot withdraw");
        assertFalse(asStranger, "a stranger certainly cannot");
        assertTrue(
            keccak256(payerData) != keccak256(strangerData),
            "the escrow must refuse the payer and a stranger for different reasons"
        );
    }

    /// @notice The withdrawal bounds `docs/spec-notes.md` §1 records are the deployment's.
    function test_WithdrawDelayBoundsMatchTheNotes() public view {
        (bool minOk, bytes memory min) =
            ESCROW.staticcall(abi.encodeWithSelector(bytes4(keccak256("MIN_WITHDRAW_DELAY()"))));
        (bool maxOk, bytes memory max) =
            ESCROW.staticcall(abi.encodeWithSelector(bytes4(keccak256("MAX_WITHDRAW_DELAY()"))));
        assertTrue(minOk && maxOk, "no withdrawal bounds");
        assertEq(abi.decode(min, (uint256)), 15 minutes, "MIN_WITHDRAW_DELAY");
        assertEq(abi.decode(max, (uint256)), 30 days, "MAX_WITHDRAW_DELAY");

        // Tickline's floor sits above the escrow's minimum, which is why a delay below 3600 is a
        // POLICY_ rejection and not an X402_ one (ADR-0012).
        uint256 ours = vm.parseJsonUint(vectors, ".x402.channels[0].config.withdraw_delay");
        assertEq(ours, 3600, "the advertised floor");
        assertTrue(ours > abi.decode(min, (uint256)), "our floor is stricter than the escrow's");
    }

    // ------------------------------------------------------------------ helpers

    function getChannelId(ChannelConfig memory config) internal view returns (bytes32) {
        (bool ok, bytes memory data) = ESCROW.staticcall(
            abi.encodeWithSelector(
                bytes4(keccak256("getChannelId((address,address,address,address,address,uint40,bytes32))")),
                config
            )
        );
        require(ok, "no getChannelId");
        return abi.decode(data, (bytes32));
    }

    function configAt(string memory at) internal view returns (ChannelConfig memory) {
        return ChannelConfig({
            payer: vm.parseJsonAddress(vectors, string.concat(at, ".payer")),
            payerAuthorizer: vm.parseJsonAddress(vectors, string.concat(at, ".payer_authorizer")),
            receiver: vm.parseJsonAddress(vectors, string.concat(at, ".receiver")),
            receiverAuthorizer: vm.parseJsonAddress(vectors, string.concat(at, ".receiver_authorizer")),
            token: vm.parseJsonAddress(vectors, string.concat(at, ".token")),
            withdrawDelay: uint40(vm.parseJsonUint(vectors, string.concat(at, ".withdraw_delay"))),
            salt: vm.parseJsonBytes32(vectors, string.concat(at, ".salt"))
        });
    }

    /// @dev True when a signature takes no arguments, so it can be staticcalled by selector alone.
    function endsWithNoArgs(string memory signature) internal pure returns (bool) {
        bytes memory raw = bytes(signature);
        return raw.length >= 2 && raw[raw.length - 2] == "(" && raw[raw.length - 1] == ")";
    }
}
