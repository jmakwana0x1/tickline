//! `MarketId`: the key every position, commit, and claim is filed under (issue #65, ADR-0013).
//!
//! Not EIP-712. This is a plain `keccak256` preimage, tagged and bound to the chain and the vault,
//! because the EIP-712 domain protects a *receipt* and not the id inside it. Without the tag,
//! the chain, and the vault, identical market parameters would produce the same id on a testnet
//! deployment and on its replacement, and a receipt for a settled market could be replayed
//! against a live one.
//!
//! **This module says what a market id encodes, not whether the market is one we would create.**
//! Whether `b` is inside the LMSR's domain, whether the deadline is in the future, and whether the
//! template parameters make sense are Phase 4's questions, at creation. A zero-IO crate cannot
//! know what time it is.
//!
//! Stub for the red commit; the implementation follows.

use alloy_primitives::{Address, B256};

/// The domain tag, as a `bytes32` literal: `"Tickline MarketId v1"` right-padded with zeros.
///
/// A literal rather than its `keccak256`, so a preimage dump reads as text when a hash disagrees
/// (ADR-0013). The `v1` is deliberate: if the preimage changes shape, the tag changes with it and
/// old ids cannot collide with new ones.
pub const MARKET_ID_TAG: B256 = B256::ZERO;

/// The MVP resolution template: a Pyth price threshold (ADR-0005).
pub const PYTH_THRESHOLD_TEMPLATE: &str = "pyth-threshold-v1";

/// Which side of the threshold resolves YES.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// YES if the price is at or above the threshold.
    Above,
    /// YES if the price is below the threshold.
    Below,
}

/// The parameters of a market, everything the id is derived from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MarketParams {
    /// Who created it, and who posted the subsidy.
    pub creator: Address,
    /// Which resolution template, by `keccak256` of its name.
    pub template_id: B256,
    /// `keccak256` of the template's own parameters.
    pub template_params_hash: B256,
    /// Unix seconds. Resolution is permissionless from here (ADR-0005).
    pub deadline: u64,
    /// LMSR liquidity, in WAD.
    pub b: u128,
    /// Epoch length in seconds.
    pub epoch_length: u32,
    /// Distinguishes markets that are otherwise identical.
    pub salt: B256,
}

impl MarketParams {
    /// The market id, bound to `chain_id` and `vault`.
    #[must_use]
    pub fn market_id(&self, chain_id: u64, vault: Address) -> B256 {
        let _ = (chain_id, vault);
        B256::ZERO
    }
}

/// `keccak256` of a template name, which is what `template_id` holds.
#[must_use]
pub fn template_id(_name: &str) -> B256 {
    B256::ZERO
}

/// `keccak256(abi.encode(priceId, threshold, direction))` for the Pyth threshold template.
#[must_use]
pub fn pyth_threshold_params_hash(
    _price_id: B256,
    _threshold: i64,
    _direction: Direction,
) -> B256 {
    B256::ZERO
}
