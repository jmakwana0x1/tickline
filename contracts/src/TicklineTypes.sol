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

    /// @notice The EIP-712 domain type hash.
    bytes32 internal constant EIP712_DOMAIN_TYPEHASH =
        keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)");

    /// @notice `keccak256("Tickline")`, the domain name.
    bytes32 internal constant DOMAIN_NAME_HASH = keccak256("Tickline");

    /// @notice `keccak256("1")`, the domain version. Bumping it invalidates every receipt.
    bytes32 internal constant DOMAIN_VERSION_HASH = keccak256("1");

    /// @notice The `PositionReceipt` type hash. Amounts are `uint128`, matching x402's widths (Q5).
    bytes32 internal constant POSITION_RECEIPT_TYPEHASH = keccak256(
        "PositionReceipt(bytes32 marketId,address agent,uint128 yesShares,uint128 noShares,uint128 costPaid,uint128 feesPaid,uint64 nonce,uint32 epoch)"
    );

    /// @notice `"Tickline MarketId v1"` as a right-padded `bytes32` literal (ADR-0013).
    /// @dev A literal rather than its `keccak256`, so a preimage dump reads as text when a hash
    ///      disagrees. The `v1` is deliberate: if the preimage changes shape, old ids cannot collide.
    bytes32 internal constant MARKET_ID_TAG = "Tickline MarketId v1";

    /// @notice secp256k1's group order, from SEC 2 section 2.4.1.
    uint256 internal constant CURVE_ORDER =
        0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141;

    /// @notice `n / 2`. EIP-2 invalidates `s` strictly above this, so the boundary is inclusive.
    uint256 internal constant HALF_CURVE_ORDER =
        0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0;

    /// @notice The largest `uint40`, the width the escrow gives `withdrawDelay`.
    uint256 internal constant UINT40_MAX = 1_099_511_627_775;

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
    /// @dev Strings are hashed rather than embedded, which is what makes the result fixed width.
    ///      Both arguments are in the hash: without them a receipt signed for one chain or one vault
    ///      would verify against another.
    function domainSeparator(uint256 chainId, address vault) internal pure returns (bytes32) {
        return keccak256(
            abi.encode(
                EIP712_DOMAIN_TYPEHASH, DOMAIN_NAME_HASH, DOMAIN_VERSION_HASH, chainId, vault
            )
        );
    }

    /// @notice `keccak256(0x1901 || separator || structHash)`.
    /// @dev `bytes.concat`, not `abi.encodePacked`: same bytes for these fixed-width inputs, and it
    ///      keeps the packed encoder out of every preimage in this codebase.
    function digest(bytes32 separator, bytes32 hashOfStruct) internal pure returns (bytes32) {
        return keccak256(bytes.concat(hex"1901", separator, hashOfStruct));
    }

    /// @notice The EIP-712 struct hash of a receipt: nine words, every value padded to 32 bytes.
    function structHash(PositionReceipt memory receipt) internal pure returns (bytes32) {
        return keccak256(
            abi.encode(
                POSITION_RECEIPT_TYPEHASH,
                receipt.marketId,
                receipt.agent,
                receipt.yesShares,
                receipt.noShares,
                receipt.costPaid,
                receipt.feesPaid,
                receipt.nonce,
                receipt.epoch
            )
        );
    }

    /// @notice The market id, bound to a chain and a vault (ADR-0013).
    /// @dev Ten words, tag first. The chain and the vault are arguments rather than fields because
    ///      they are properties of the deployment: the same parameters are a different market on a
    ///      different chain or a redeployed vault, so a receipt cannot be replayed across either.
    function marketId(uint256 chainId, address vault, MarketParams memory params)
        internal
        pure
        returns (bytes32)
    {
        return keccak256(
            abi.encode(
                MARKET_ID_TAG,
                chainId,
                vault,
                params.creator,
                params.templateId,
                params.templateParamsHash,
                params.deadline,
                params.b,
                params.epochLength,
                params.salt
            )
        );
    }

    /// @notice The address that signed `signed`, under ADR-0012.
    /// @dev Checked in the documented order, so one input has one reason: compact before length,
    ///      length before `v`, `v` before the scalars, the scalars before high-s. An out-of-range
    ///      `s` is reported as out of range and never as high-s.
    function recoverSigner(bytes32 signed, bytes memory signature) internal pure returns (address) {
        if (signature.length == 64) revert CompactSignature();
        if (signature.length != 65) revert SignatureLength(signature.length);

        bytes32 r;
        bytes32 s;
        uint8 v;
        // The only assembly here, and only to split fixed offsets of a length-checked buffer.
        assembly ("memory-safe") {
            r := mload(add(signature, 0x20))
            s := mload(add(signature, 0x40))
            v := byte(0, mload(add(signature, 0x60)))
        }

        if (v != 27 && v != 28) revert SignatureRecoveryId(v);
        if (uint256(r) == 0) revert ScalarRZero();
        if (uint256(r) >= CURVE_ORDER) revert ScalarRAboveOrder();
        if (uint256(s) == 0) revert ScalarSZero();
        if (uint256(s) >= CURVE_ORDER) revert ScalarSAboveOrder();
        if (uint256(s) > HALF_CURVE_ORDER) revert HighS();

        address recovered = ecrecover(signed, v, r, s);
        if (recovered == address(0)) revert Unrecoverable();
        return recovered;
    }

    /// @notice Revert unless `expected` signed `signed`.
    /// @dev The check names who must have signed. `ecrecover` returning zero means nothing was
    ///      recovered, which is a different failure from recovering the wrong person: a signature
    ///      over another message recovers a stranger, so "not the zero address" is never the test
    ///      that a receipt is genuine (ADR-0012).
    function verifySigner(bytes32 signed, bytes memory signature, address expected) internal pure {
        address recovered = recoverSigner(signed, signature);
        if (recovered != expected) revert WrongSigner(expected, recovered);
    }

    /// @notice Revert unless `withdrawDelay` fits `uint40`, the width the escrow hashes.
    /// @dev Tickline's 3600 floor is deliberately **not** here: that is a policy the engine applies
    ///      and the vault does not, so it has no Solidity error (ADR-0012's per-family reach).
    function requireWithdrawDelayWidth(uint256 withdrawDelay) internal pure {
        if (withdrawDelay > UINT40_MAX) revert WithdrawDelayWidth(withdrawDelay);
    }
}
