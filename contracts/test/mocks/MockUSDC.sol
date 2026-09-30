// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @title MockUSDC
/// @notice Six-decimal ERC20 for the vault's tests. Test-only: no dependency is added for it.
/// @dev `failNextTransfer` makes the token return `false` instead of reverting, which is the case a
///      vault must handle: a token that reports failure without reverting would otherwise leave a
///      bonded operator behind with no funds moved (#95).
contract MockUSDC {
    string public constant name = "Mock USD Coin";
    string public constant symbol = "USDC";
    uint8 public constant decimals = 6;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    bool public failNextTransfer;

    function setFailNextTransfer(bool fail) external {
        failNextTransfer = fail;
    }

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        if (failNextTransfer) {
            failNextTransfer = false;
            return false;
        }
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        if (failNextTransfer) {
            failNextTransfer = false;
            return false;
        }
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}
