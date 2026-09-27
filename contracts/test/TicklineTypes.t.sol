// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {Test} from "forge-std/Test.sol";
import {TicklineTypes} from "../src/TicklineTypes.sol";

/// @title The Solidity side of the cross-stack vectors (issue #67)
/// @notice Reads `testdata/vectors/eip712.json`, the same file the Rust and TypeScript suites read,
///         and asserts every hash and every rejection against it.
/// @dev This is the test that makes Phase 2 mean something. Rust agreeing with TypeScript proves we
///      are consistent; Solidity agreeing with both, over values the deployed escrow produced,
///      proves the vault will settle a dispute the way the engine promised.
contract TicklineTypesTest is Test {
    string internal vectors;

    function setUp() public {
        vectors = vm.readFile("../testdata/vectors/eip712.json");
    }

    // ------------------------------------------------------------------ the Tickline domain

    function test_DomainSeparatorMatchesVectors() public view {
        uint256 chainId = vm.parseJsonUint(vectors, ".domain.chain_id");
        address vault = vm.parseJsonAddress(vectors, ".domain.verifying_contract");
        assertEq(
            TicklineTypes.domainSeparator(chainId, vault),
            vm.parseJsonBytes32(vectors, ".domain.separator"),
            "domain separator"
        );
    }

    function test_DomainSeparatorChangesWithChainAndVault() public view {
        uint256 chainId = vm.parseJsonUint(vectors, ".domain.chain_id");
        address vault = vm.parseJsonAddress(vectors, ".domain.verifying_contract");
        bytes32 mine = TicklineTypes.domainSeparator(chainId, vault);

        for (uint256 i = 0; i < 2; i++) {
            string memory at = string.concat(".domain_variants[", vm.toString(i), "]");
            uint256 otherChain = vm.parseJsonUint(vectors, string.concat(at, ".chain_id"));
            address otherVault =
                vm.parseJsonAddress(vectors, string.concat(at, ".verifying_contract"));
            bytes32 expected = vm.parseJsonBytes32(vectors, string.concat(at, ".separator"));

            assertEq(TicklineTypes.domainSeparator(otherChain, otherVault), expected, "variant");
            assertTrue(expected != mine, "a variant must not equal the base");
        }
    }

    function test_DigestIsEip191PrefixedDomainAndStructHash() public view {
        bytes32 separator = vm.parseJsonBytes32(vectors, ".domain.separator");
        bytes32 sh = vm.parseJsonBytes32(vectors, ".digest.struct_hash");
        assertEq(
            TicklineTypes.digest(separator, sh),
            vm.parseJsonBytes32(vectors, ".digest.digest"),
            "0x1901 framing"
        );
    }

    // ------------------------------------------------------------------ PositionReceipt

    function test_PositionReceiptDigestMatchesVectors() public view {
        TicklineTypes.PositionReceipt memory receipt = receiptAt(".receipt.typical");
        assertEq(
            TicklineTypes.structHash(receipt),
            vm.parseJsonBytes32(vectors, ".receipt.typical.struct_hash"),
            "struct hash"
        );
        assertEq(
            TicklineTypes.digest(ticklineSeparator(), TicklineTypes.structHash(receipt)),
            vm.parseJsonBytes32(vectors, ".receipt.typical.digest"),
            "digest"
        );
    }

    function test_PositionReceiptAtEveryTypeMaximumMatchesVectors() public view {
        // An encoding test and nothing more: Q_MAX caps shares about twenty orders of magnitude
        // lower, and the vault's solvency check caps payouts. The engine must never issue one.
        TicklineTypes.PositionReceipt memory receipt = receiptAt(".receipt.at_every_type_maximum");
        assertEq(receipt.yesShares, type(uint128).max, "uint128 boundary is accepted");
        assertEq(receipt.nonce, type(uint64).max, "uint64 boundary is accepted");
        assertEq(receipt.epoch, type(uint32).max, "uint32 boundary is accepted");
        assertEq(
            TicklineTypes.digest(ticklineSeparator(), TicklineTypes.structHash(receipt)),
            vm.parseJsonBytes32(vectors, ".receipt.at_every_type_maximum.digest"),
            "digest at every maximum"
        );
    }

    function test_ValidReceiptSignatureVerifies() public view {
        TicklineTypes.PositionReceipt memory receipt = receiptAt(".receipt.typical");
        bytes32 d = TicklineTypes.digest(ticklineSeparator(), TicklineTypes.structHash(receipt));
        bytes memory signature = vm.parseJsonBytes(vectors, ".receipt.typical.signature");
        address operator = vm.parseJsonAddress(vectors, ".receipt.operator");

        assertEq(TicklineTypes.recoverSigner(d, signature), operator, "recovers the operator");
        TicklineTypes.verifySigner(d, signature, operator);
    }

    function test_TamperedReceiptRecoversAStranger() public view {
        // A signature over the wrong digest recovers, to a stranger, which is why "the recovered
        // address is not zero" is never the check. The stranger is committed, so a wrong digest has
        // to reproduce a named address by luck rather than merely differ.
        bytes memory signature = vm.parseJsonBytes(vectors, ".receipt.typical.signature");
        address operator = vm.parseJsonAddress(vectors, ".receipt.operator");
        // The count comes from the file, not from a constant here: a constant would silently
        // stop covering a case the generator adds.
        uint256 cases = vm.parseJsonStringArray(vectors, ".receipt.tampered.changed_fields").length;
        assertEq(cases, 3, "one case per tampered field, plus the all-fields case");

        for (uint256 i = 0; i < cases; i++) {
            string memory at = string.concat(".receipt.tampered.cases[", vm.toString(i), "]");
            bytes32 d = vm.parseJsonBytes32(vectors, string.concat(at, ".digest"));
            address stranger = vm.parseJsonAddress(vectors, string.concat(at, ".recovers_to"));

            assertEq(TicklineTypes.recoverSigner(d, signature), stranger, "stranger");
            assertTrue(stranger != operator, "a tampered receipt must not recover the operator");
        }
    }

    // ------------------------------------------------------------------ MarketId

    function test_MarketIdMatchesVectors() public view {
        uint256 chainId = vm.parseJsonUint(vectors, ".market_id.base.chain_id");
        address vault = vm.parseJsonAddress(vectors, ".market_id.base.vault");
        assertEq(
            TicklineTypes.marketId(chainId, vault, marketParams()),
            vm.parseJsonBytes32(vectors, ".market_id.base.market_id"),
            "market id"
        );
    }

    function test_MarketIdChangesWithChainAndVault() public view {
        uint256 chainId = vm.parseJsonUint(vectors, ".market_id.base.chain_id");
        address vault = vm.parseJsonAddress(vectors, ".market_id.base.vault");
        bytes32 mine = TicklineTypes.marketId(chainId, vault, marketParams());

        for (uint256 i = 0; i < 2; i++) {
            string memory at = string.concat(".market_id.domain_variants[", vm.toString(i), "]");
            uint256 otherChain = vm.parseJsonUint(vectors, string.concat(at, ".chain_id"));
            address otherVault = vm.parseJsonAddress(vectors, string.concat(at, ".vault"));
            bytes32 expected = vm.parseJsonBytes32(vectors, string.concat(at, ".market_id"));

            assertEq(TicklineTypes.marketId(otherChain, otherVault, marketParams()), expected);
            assertTrue(expected != mine, "the same market elsewhere is another market");
        }
    }

    // ------------------------------------------------------------------ the x402 type hashes

    function test_X402TypeHashesMatchVectors() public view {
        // Hashed here from the committed type strings, and compared with values read off the
        // deployed escrow's own constants.
        for (uint256 i = 0; i < 4; i++) {
            string memory at = string.concat(".x402.type_hashes[", vm.toString(i), "]");
            string memory typeString = vm.parseJsonString(vectors, string.concat(at, ".type_string"));
            bytes32 expected = vm.parseJsonBytes32(vectors, string.concat(at, ".hash"));
            assertEq(keccak256(bytes(typeString)), expected, vm.parseJsonString(vectors, string.concat(at, ".name")));
        }
    }

    // ------------------------------------------------------------------ the invalid vectors

    function test_AllInvalidSignaturesAreRejected() public {
        // The cross-stack half of ADR-0012: the same bytes, refused for the same named reason in
        // all three stacks. The code maps to a custom error one to one, which is why the codes are
        // flat: Solidity cannot carry a field saying which scalar failed.
        bytes32 d = vm.parseJsonBytes32(vectors, ".invalid.digest");
        address expected = vm.parseJsonAddress(vectors, ".invalid.expected_signer");
        string[] memory codes = vm.parseJsonStringArray(vectors, ".invalid.codes");
        assertEq(codes.length, 9, "every ADR-0012 rejection except the wrong-signer case");

        for (uint256 i = 0; i < codes.length; i++) {
            string memory at = string.concat(".invalid.signatures[", vm.toString(i), "]");
            assertEq(
                vm.parseJsonString(vectors, string.concat(at, ".code")),
                codes[i],
                "the flat code list and the entries must be the same list"
            );
            bytes memory signature = vm.parseJsonBytes(vectors, string.concat(at, ".signature"));
            vm.expectRevert(selectorFor(codes[i]));
            this.verify(d, signature, expected);
        }
    }

    function test_WrongSignerIsRejectedByName() public {
        bytes32 d = vm.parseJsonBytes32(vectors, ".invalid.digest");
        bytes memory signature = vm.parseJsonBytes(vectors, ".invalid.wrong_signer.signature");
        address other = vm.parseJsonAddress(vectors, ".invalid.wrong_signer.expected_signer");
        address recovers = vm.parseJsonAddress(vectors, ".invalid.wrong_signer.recovers_to");

        vm.expectRevert(
            abi.encodeWithSelector(TicklineTypes.WrongSigner.selector, other, recovers)
        );
        this.verify(d, signature, other);
    }

    /// @dev An external wrapper, so `vm.expectRevert` sees a call boundary.
    function verify(bytes32 d, bytes memory signature, address expected) external pure {
        TicklineTypes.verifySigner(d, signature, expected);
    }

    // ------------------------------------------------------------------ helpers

    function ticklineSeparator() internal view returns (bytes32) {
        return TicklineTypes.domainSeparator(
            vm.parseJsonUint(vectors, ".domain.chain_id"),
            vm.parseJsonAddress(vectors, ".domain.verifying_contract")
        );
    }

    function receiptAt(string memory at)
        internal
        view
        returns (TicklineTypes.PositionReceipt memory)
    {
        return TicklineTypes.PositionReceipt({
            marketId: vm.parseJsonBytes32(vectors, string.concat(at, ".market_id")),
            agent: vm.parseJsonAddress(vectors, string.concat(at, ".agent")),
            yesShares: uint128(parseDecimal(string.concat(at, ".yes_shares"))),
            noShares: uint128(parseDecimal(string.concat(at, ".no_shares"))),
            costPaid: uint128(parseDecimal(string.concat(at, ".cost_paid"))),
            feesPaid: uint128(parseDecimal(string.concat(at, ".fees_paid"))),
            nonce: uint64(parseDecimal(string.concat(at, ".nonce"))),
            epoch: uint32(vm.parseJsonUint(vectors, string.concat(at, ".epoch")))
        });
    }

    function marketParams() internal view returns (TicklineTypes.MarketParams memory) {
        return TicklineTypes.MarketParams({
            creator: vm.parseJsonAddress(vectors, ".market_id.base.creator"),
            templateId: vm.parseJsonBytes32(vectors, ".market_id.base.template_id"),
            templateParamsHash: vm.parseJsonBytes32(vectors, ".market_id.base.template_params_hash"),
            deadline: uint64(vm.parseJsonUint(vectors, ".market_id.base.deadline")),
            b: uint128(parseDecimal(".market_id.base.b")),
            epochLength: uint32(vm.parseJsonUint(vectors, ".market_id.base.epoch_length")),
            salt: vm.parseJsonBytes32(vectors, ".market_id.base.salt")
        });
    }

    /// @dev A decimal string, because a JSON number is an IEEE double and `uint128` values do not
    ///      fit in one. The generator writes anything that wide as a string for the same reason.
    function parseDecimal(string memory at) internal view returns (uint256) {
        return vm.parseUint(vm.parseJsonString(vectors, at));
    }

    /// @dev The custom error a code maps to. One to one, by rule (ADR-0012).
    function selectorFor(string memory code) internal pure returns (bytes4) {
        bytes32 key = keccak256(bytes(code));
        if (key == keccak256("SIG_LENGTH")) return TicklineTypes.SignatureLength.selector;
        if (key == keccak256("SIG_COMPACT")) return TicklineTypes.CompactSignature.selector;
        if (key == keccak256("SIG_RECOVERY_ID")) return TicklineTypes.SignatureRecoveryId.selector;
        if (key == keccak256("SIG_R_ZERO")) return TicklineTypes.ScalarRZero.selector;
        if (key == keccak256("SIG_S_ZERO")) return TicklineTypes.ScalarSZero.selector;
        if (key == keccak256("SIG_R_ABOVE_ORDER")) return TicklineTypes.ScalarRAboveOrder.selector;
        if (key == keccak256("SIG_S_ABOVE_ORDER")) return TicklineTypes.ScalarSAboveOrder.selector;
        if (key == keccak256("SIG_HIGH_S")) return TicklineTypes.HighS.selector;
        if (key == keccak256("SIG_UNRECOVERABLE")) return TicklineTypes.Unrecoverable.selector;
        revert(string.concat("no custom error for code ", code));
    }
}
