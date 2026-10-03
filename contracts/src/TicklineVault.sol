// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IERC20} from "./IERC20.sol";

/// @title TicklineVault
/// @notice Holds market collateral and operator bonds. Enforces solvency, monotonic positions,
///         resolution, claims, and slashing, and never prices a trade (CLAUDE.md section 1).
/// @dev Phase 3, slice by slice. This file currently holds operator registration and the bond
///      (#95). Market state, commits, resolution and claims arrive in #96 onward.
///
///      The bond is what makes I12 true: an agent holding a signed receipt that the operator
///      committed against is paid from here, and the operator is slashed. Every guarantee in this
///      contract is therefore only as good as the bond's arithmetic, which is why the accounting is
///      plain addition and subtraction on one `uint128` with no branches to get wrong.
///
///      Every event is emitted before the external token call, never after. A log emitted after a
///      call can be reordered or fabricated by a reentrant token, and Phase 5's indexer compares
///      these logs against the ledger: under I13 a disagreement halts the market rather than being
///      reconciled away, so a fabricated log is an outage. If the call reverts the whole call frame
///      goes with it, logs included, so emitting first loses nothing.
contract TicklineVault {
    /// @dev An operator that is already registered cannot register again.
    error AlreadyRegistered();
    /// @dev A zero signing key would make every receipt unverifiable.
    error ZeroSigningKey();
    /// @dev The caller is not a registered operator.
    error NotRegistered();
    /// @dev The withdrawal is larger than the bond it would come from.
    error BondTooLow();
    /// @dev USDC reported failure by returning `false`. Nothing is stored when this reverts.
    error TransferFailed();
    /// @dev The vault was built with the zero address for USDC, which no later call can repair.
    error ZeroToken();

    event OperatorRegistered(address indexed operator, address indexed signingKey, uint128 bond);
    event BondIncreased(address indexed operator, uint128 added, uint128 total);
    event BondWithdrawn(address indexed operator, uint128 removed, uint128 total);

    /// @notice One operator's signing key and bond.
    /// @dev The key is the one a `PositionReceipt` must verify against (ADR-0012). There is no
    ///      rotation in Phase 3: a receipt signed by a retired key would need a rule for how long it
    ///      stays honourable, and that rule changes how I12 is enforced.
    struct Operator {
        address signingKey;
        uint128 bond;
    }

    /// @notice The collateral and bond token. USDC, 6 decimals, immutable for the vault's life.
    address public immutable usdc;

    /// @notice The bond an operator must hold to create a market, in USDC base units.
    /// @dev Checked at `createMarket` (#96), not at registration: an operator with no markets has
    ///      nothing to be dishonest about, so nothing to bond against.
    uint128 public immutable minBond;

    /// @notice Registered operators, by address.
    mapping(address => Operator) public operators;

    /// @param usdc_ The collateral token. Rejected if zero: every bond and every payout moves
    ///        through it, so a vault built against the zero address is a vault that can never pay,
    ///        and the check that stops it costs one comparison, once.
    /// @param minBond_ The bond required to create a market, in USDC base units.
    constructor(address usdc_, uint128 minBond_) {
        if (usdc_ == address(0)) revert ZeroToken();
        usdc = usdc_;
        minBond = minBond_;
    }

    /// @notice Register as an operator with a signing key and an opening bond.
    /// @param signingKey The address that signs `PositionReceipt`s. Must not be zero.
    /// @param bond The opening bond in USDC base units. May be zero.
    /// @dev Reverts with `AlreadyRegistered`, `ZeroSigningKey`, or `TransferFailed`. State is
    ///      written before the transfer only for the key, and the transfer is checked, so a token
    ///      that returns `false` leaves nothing behind: the whole call reverts.
    function registerOperator(address signingKey, uint128 bond) external {
        if (signingKey == address(0)) revert ZeroSigningKey();
        if (operators[msg.sender].signingKey != address(0)) revert AlreadyRegistered();
        operators[msg.sender] = Operator({signingKey: signingKey, bond: bond});
        emit OperatorRegistered(msg.sender, signingKey, bond);
        if (bond != 0) _pull(msg.sender, bond);
    }

    /// @notice Add to the caller's bond.
    /// @param amount USDC base units to add. Only the increase is pulled.
    /// @dev Reverts with `NotRegistered` or `TransferFailed`.
    function increaseBond(uint128 amount) external {
        Operator storage op = _registered();
        uint128 total = op.bond + amount;
        op.bond = total;
        emit BondIncreased(msg.sender, amount, total);
        _pull(msg.sender, amount);
    }

    /// @notice Withdraw part or all of the caller's bond.
    /// @param amount USDC base units to withdraw.
    /// @dev Reverts with `NotRegistered`, `BondTooLow`, or `TransferFailed`. Once a market is live
    ///      the minimum bond becomes a floor here (#96); with no live markets the whole bond may go.
    function withdrawBond(uint128 amount) external {
        Operator storage op = _registered();
        if (amount > op.bond) revert BondTooLow();
        uint128 total = op.bond - amount;
        op.bond = total;
        emit BondWithdrawn(msg.sender, amount, total);
        if (!IERC20(usdc).transfer(msg.sender, amount)) revert TransferFailed();
    }

    /// @dev The caller's operator record, or `NotRegistered`.
    function _registered() private view returns (Operator storage op) {
        op = operators[msg.sender];
        if (op.signingKey == address(0)) revert NotRegistered();
    }

    /// @dev Pull USDC from `from` into the vault, checking the boolean USDC actually returns.
    function _pull(address from, uint256 amount) private {
        if (!IERC20(usdc).transferFrom(from, address(this), amount)) revert TransferFailed();
    }
}
