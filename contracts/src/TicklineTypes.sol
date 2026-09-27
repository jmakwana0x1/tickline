// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @title TicklineTypes
/// @notice Every hash and every signature rule the vault and the engine must agree on.
/// @dev A real library rather than a test harness (Jay's D4 on #59): `TicklineVault` imports this
///      in Phase 3, so the constants it settles disputes with are the ones the cross-stack vectors
///      check here. A harness would only have proved the harness.
///
///      Scope is deliberately narrow: type hashes, the domain separator, digest helpers, the
///      `MarketId` preimage, and the signature policy. No storage, no market logic, no pricing.
///
///      `abi.encodePacked` appears nowhere, by rule: it does not pad, so two different field tuples
///      can encode to identical bytes, and every preimage here keys money. The EIP-712 `0x1901`
///      framing uses `bytes.concat`, which is fixed-width and unambiguous
///      (`scripts/check-no-encode-packed.sh`, ADR-0013).
///
///      Stub for the red commit; the implementation follows.
library TicklineTypes {
    /// @dev The signature was not 65 bytes. Code `SIG_LENGTH`.
    error SignatureLength(uint256 got);
    /// @dev 64 bytes: an EIP-2098 compact signature. Code `SIG_COMPACT`.
    error CompactSignature();
    /// @dev `v` was not 27 or 28. Code `SIG_RECOVERY_ID`.
    error SignatureRecoveryId(uint8 got);
    /// @dev `r` was zero. Code `SIG_R_ZERO`.
    error ScalarRZero();
    /// @dev `s` was zero. Code `SIG_S_ZERO`.
    error ScalarSZero();
    /// @dev `r` was at or above the curve order. Code `SIG_R_ABOVE_ORDER`.
    error ScalarRAboveOrder();
    /// @dev `s` was at or above the curve order. Code `SIG_S_ABOVE_ORDER`.
    error ScalarSAboveOrder();
    /// @dev `s` was above `n / 2`. Code `SIG_HIGH_S`.
    error HighS();
    /// @dev No signer could be recovered. Code `SIG_UNRECOVERABLE`.
    error Unrecoverable();
    /// @dev A valid signature belonging to someone else. Code `SIG_WRONG_SIGNER`.
    error WrongSigner(address expected, address recovered);
    /// @dev A `withdrawDelay` that does not fit `uint40`. Code `X402_WITHDRAW_DELAY_WIDTH`.
    error WithdrawDelayWidth(uint256 got);

    /// @notice One agent's position in one market, as of one nonce.
    struct PositionReceipt {
        bytes32 marketId;
        address agent;
        uint128 yesShares;
        uint128 noShares;
        uint128 costPaid;
        uint128 feesPaid;
        uint64 nonce;
        uint32 epoch;
    }

    /// @notice Everything a market id is derived from, besides the chain and the vault.
    struct MarketParams {
        address creator;
        bytes32 templateId;
        bytes32 templateParamsHash;
        uint64 deadline;
        uint128 b;
        uint32 epochLength;
        bytes32 salt;
    }

    /// @notice The Tickline domain separator for a chain and a vault.
    function domainSeparator(uint256, address) internal pure returns (bytes32) {
        return bytes32(0);
    }

    /// @notice `keccak256(0x1901 || separator || structHash)`.
    function digest(bytes32, bytes32) internal pure returns (bytes32) {
        return bytes32(0);
    }

    /// @notice The EIP-712 struct hash of a receipt.
    function structHash(PositionReceipt memory) internal pure returns (bytes32) {
        return bytes32(0);
    }

    /// @notice The market id, bound to a chain and a vault.
    function marketId(uint256, address, MarketParams memory) internal pure returns (bytes32) {
        return bytes32(0);
    }

    /// @notice The address that signed `signed`, under the policy in ADR-0012.
    function recoverSigner(bytes32, bytes memory) internal pure returns (address) {
        return address(0);
    }

    /// @notice Revert unless `expected` signed `signed`.
    function verifySigner(bytes32, bytes memory, address) internal pure {}
}
