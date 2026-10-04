// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @title IERC20
/// @notice The three calls the vault makes on USDC, and nothing more.
/// @dev Declared here rather than imported from forge-std, which is a test library, and rather than
///      pulling in a token package for three signatures. Return values are `bool` and are checked at
///      every call site: USDC reports failure by returning `false`, not by reverting.
interface IERC20 {
    function transfer(address to, uint256 amount) external returns (bool);
    function transferFrom(address from, address to, uint256 amount) external returns (bool);
    function balanceOf(address account) external view returns (uint256);
}
