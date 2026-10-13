// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import {IERC20} from "./IERC20.sol";
import {TicklineTypes} from "./TicklineTypes.sol";

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
    /// @dev The operator's bond is below the minimum a live market requires.
    error BondBelowMinimum();
    /// @dev A market with these exact parameters already exists. Vary `salt` for a second one.
    error MarketExists();
    /// @dev The deadline is now or in the past, so the market could never trade.
    error DeadlineNotInTheFuture();
    /// @dev An epoch length of zero would make every epoch boundary the same instant.
    error ZeroEpochLength();
    /// @dev `b` of zero is not a market maker.
    error ZeroLiquidity();
    /// @dev A market with no creator has nobody to return residual collateral to.
    error ZeroCreator();
    /// @dev A template the vault cannot resolve. A market under one would strand its agents.
    error UnknownTemplate();

    event OperatorRegistered(address indexed operator, address indexed signingKey, uint128 bond);
    event BondIncreased(address indexed operator, uint128 added, uint128 total);
    event BondWithdrawn(address indexed operator, uint128 removed, uint128 total);
    event MarketCreated(
        bytes32 indexed marketId,
        address indexed operator,
        address indexed creator,
        uint64 deadline,
        uint128 b,
        uint32 epochLength,
        uint128 subsidy
    );

    /// @notice One operator's signing key and bond.
    /// @dev The key is the one a `PositionReceipt` must verify against (ADR-0012). There is no
    ///      rotation in Phase 3: a receipt signed by a retired key would need a rule for how long it
    ///      stays honourable, and that rule changes how I12 is enforced.
    struct Operator {
        address signingKey;
        uint128 bond;
    }

    /// @notice One market, as the vault holds it.
    /// @dev `operator` is the caller that created it and the only address allowed to commit to it.
    ///      `creator` is who residual collateral returns to once the claim window closes, and need
    ///      not be the operator. `subsidy` is the requirement in USDC base units, recorded rather
    ///      than collected: the operator floats settlement, and collateral arrives as
    ///      `commitEpoch`'s delta.
    struct Market {
        address operator;
        address creator;
        bytes32 templateId;
        bytes32 templateParamsHash;
        uint64 deadline;
        uint128 b;
        uint32 epochLength;
        uint128 subsidy;
    }

    /// @notice The only resolution template the vault understands: a Pyth price threshold.
    /// @dev The hash of its name, which is also the id the cross-stack vectors committed. Written as
    ///      the hash rather than the literal so a preimage reads as text, and asserted against the
    ///      vector file in the tests so neither form alone is the source (ADR-0013).
    bytes32 public constant PYTH_THRESHOLD_V1 = keccak256("pyth-threshold-v1");

    /// @notice The collateral and bond token. USDC, 6 decimals, immutable for the vault's life.
    address public immutable usdc;

    /// @notice The bond an operator must hold to create a market, in USDC base units.
    /// @dev Checked at `createMarket` (#96), not at registration: an operator with no markets has
    ///      nothing to be dishonest about, so nothing to bond against.
    uint128 public immutable minBond;

    /// @notice Registered operators, by address.
    mapping(address => Operator) public operators;

    /// @notice Markets by id.
    mapping(bytes32 => Market) public markets;

    /// @notice How many of an operator's markets are still live.
    /// @dev While this is above zero the operator's bond cannot go below `minBond`: the bond is what
    ///      pays a receipt holder under I12, so it must outlast the markets that could need it. It
    ///      falls again when a market's claim window closes (#99).
    mapping(address => uint256) public liveMarkets;

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

    /// @notice Create a market. Operator only.
    /// @param params Everything the market id is derived from, besides the chain and this vault.
    /// @param subsidy The creator's subsidy requirement, in USDC base units.
    /// @return id The market id, `TicklineTypes.marketId` of this chain, this vault and `params`.
    /// @dev Reverts `NotRegistered`, `BondBelowMinimum`, `ZeroCreator`, `UnknownTemplate`,
    ///      `DeadlineNotInTheFuture`, `ZeroEpochLength`, `ZeroLiquidity` or `MarketExists`.
    ///
    ///      No token moves here. The subsidy is recorded, not collected: the operator floats
    ///      settlement and collateral enters through `commitEpoch`'s delta (`CLAUDE.md` section 1).
    ///
    ///      `subsidy` is **not** yet checked against `b * ln2` (I3). Which side of ADR-0009's
    ///      approximation margin the vault should sit on is open on #96, and guessing it would put
    ///      the vault on the wrong side of a rounding rule that exists to protect it.
    function createMarket(
        TicklineTypes.MarketParams calldata params,
        uint128 subsidy
    ) external returns (bytes32 id) {
        Operator storage op = _registered();
        if (op.bond < minBond) revert BondBelowMinimum();
        if (params.creator == address(0)) revert ZeroCreator();
        if (params.templateId != PYTH_THRESHOLD_V1) revert UnknownTemplate();
        // A validator can shift `block.timestamp` by a few seconds, and here that is harmless: the
        // only markets whose acceptance changes are ones whose deadline is seconds away, and such a
        // market resolves immediately with no fills rather than losing anyone money. `resolve`
        // compares against the same deadline and does not get this reasoning for free (#98).
        // forge-lint: disable-next-line(block-timestamp)
        if (params.deadline <= block.timestamp) revert DeadlineNotInTheFuture();
        if (params.epochLength == 0) revert ZeroEpochLength();
        if (params.b == 0) revert ZeroLiquidity();

        id = TicklineTypes.marketId(block.chainid, address(this), params);
        // The operator field doubles as the existence flag: it is never zero for a stored market,
        // because only a registered operator reaches this line and a registered operator is
        // non-zero by construction.
        if (markets[id].operator != address(0)) revert MarketExists();

        markets[id] = Market({
            operator: msg.sender,
            creator: params.creator,
            templateId: params.templateId,
            templateParamsHash: params.templateParamsHash,
            deadline: params.deadline,
            b: params.b,
            epochLength: params.epochLength,
            subsidy: subsidy
        });
        liveMarkets[msg.sender] += 1;

        emit MarketCreated(
            id, msg.sender, params.creator, params.deadline, params.b, params.epochLength, subsidy
        );
    }

    /// @notice Withdraw part or all of the caller's bond.
    /// @param amount USDC base units to withdraw.
    /// @dev Reverts with `NotRegistered`, `BondTooLow`, `BondBelowMinimum`, or `TransferFailed`.
    ///      While any of the caller's markets is live the remainder must stay at or above
    ///      `minBond`; with none live the whole bond may go.
    function withdrawBond(uint128 amount) external {
        Operator storage op = _registered();
        if (amount > op.bond) revert BondTooLow();
        uint128 total = op.bond - amount;
        if (liveMarkets[msg.sender] != 0 && total < minBond) revert BondBelowMinimum();
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
