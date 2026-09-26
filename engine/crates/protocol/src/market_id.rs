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
//! The preimage is `abi.encode`, never `abi.encodePacked`: packed encoding does not pad, so two
//! different field tuples can produce identical bytes, and this preimage keys money.
//! `scripts/check-no-encode-packed.sh` enforces the same rule on the Solidity side.

use alloy_primitives::{b256, keccak256, Address, B256, U256};

/// The domain tag, as a `bytes32` literal: `"Tickline MarketId v1"` right-padded with zeros.
///
/// A literal rather than its `keccak256`, so a preimage dump reads as text when a hash disagrees
/// (ADR-0013). The `v1` is deliberate: if the preimage changes shape, the tag changes with it and
/// old ids cannot collide with new ones.
pub const MARKET_ID_TAG: B256 =
    b256!("5469636b6c696e65204d61726b65744964207631000000000000000000000000");

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

impl Direction {
    /// The `uint8` the template parameters hash encodes: 0 above, 1 below.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Above => 0,
            Self::Below => 1,
        }
    }
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
    ///
    /// Ten words of `abi.encode`, tag first. The chain and the vault are arguments rather than
    /// fields because they are properties of the deployment, not of the market: the same
    /// parameters are a different market on a different chain or a different vault, which is the
    /// whole point of ADR-0013.
    #[must_use]
    pub fn market_id(&self, chain_id: u64, vault: Address) -> B256 {
        let mut encoded = Vec::with_capacity(32 * 10);
        encoded.extend_from_slice(MARKET_ID_TAG.as_slice());
        encoded.extend_from_slice(&U256::from(chain_id).to_be_bytes::<32>());
        encoded.extend_from_slice(B256::left_padding_from(vault.as_slice()).as_slice());
        encoded.extend_from_slice(B256::left_padding_from(self.creator.as_slice()).as_slice());
        encoded.extend_from_slice(self.template_id.as_slice());
        encoded.extend_from_slice(self.template_params_hash.as_slice());
        encoded.extend_from_slice(&U256::from(self.deadline).to_be_bytes::<32>());
        encoded.extend_from_slice(&U256::from(self.b).to_be_bytes::<32>());
        encoded.extend_from_slice(&U256::from(self.epoch_length).to_be_bytes::<32>());
        encoded.extend_from_slice(self.salt.as_slice());
        keccak256(&encoded)
    }
}

/// `keccak256` of a template name, which is what `template_id` holds.
#[must_use]
pub fn template_id(name: &str) -> B256 {
    keccak256(name.as_bytes())
}

/// `keccak256(abi.encode(priceId, threshold, direction))` for the Pyth threshold template.
///
/// `threshold` is an `int64` in the feed's own exponent, and Pyth prices are signed, so it is
/// **sign-extended** to 32 bytes as `abi.encode` does: `-1` must not hash like `u64::MAX`.
/// Resolution is machine-only (ADR-0005), so every value the resolution rule reads is in here.
#[must_use]
pub fn pyth_threshold_params_hash(price_id: B256, threshold: i64, direction: Direction) -> B256 {
    let mut encoded = Vec::with_capacity(96);
    encoded.extend_from_slice(price_id.as_slice());

    // Sign extension, written out rather than routed through a conversion that could fail: the
    // high 24 bytes are 0xff for a negative threshold and 0x00 otherwise, which is what
    // abi.encode does for an int64.
    let sign_byte = if threshold < 0 { 0xffu8 } else { 0x00u8 };
    encoded.extend_from_slice(&[sign_byte; 24]);
    encoded.extend_from_slice(&threshold.to_be_bytes());

    encoded.extend_from_slice(&U256::from(direction.as_u8()).to_be_bytes::<32>());
    keccak256(&encoded)
}
