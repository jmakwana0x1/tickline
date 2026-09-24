//! The x402 batch-settlement types, exactly as `docs/spec-notes.md` §1 records them (issue #63).
//!
//! **No field here is invented.** Every type string is the one cited in §1 and verified against
//! the deployed escrow's own type hash getters, and every fixture in
//! `testdata/vectors/eip712-primitives.json` was read from the deployment rather than derived
//! from our reading of the spec.
//!
//! Stub for the red commit; the implementation follows.

use alloy_primitives::{Address, B256};

use crate::{eip712::Domain, ProtocolError};

/// EIP-712 domain name of the escrow, from its own `eip712Domain()`.
pub const DOMAIN_NAME: &str = "x402 Batch Settlement";

/// EIP-712 domain version of the escrow.
pub const DOMAIN_VERSION: &str = "1";

/// `ChannelConfig`, the tuple a channel's identity is derived from.
pub const CHANNEL_CONFIG_TYPE: &str = "ChannelConfig(address payer,address payerAuthorizer,address receiver,address receiverAuthorizer,address token,uint40 withdrawDelay,bytes32 salt)";

/// `Voucher`, two fields: no amount per request, no expiry, no nonce, no deadline.
pub const VOUCHER_TYPE: &str = "Voucher(bytes32 channelId,uint128 maxClaimableAmount)";

/// `ClaimEntry`, one channel's line in a batch.
pub const CLAIM_ENTRY_TYPE: &str =
    "ClaimEntry(bytes32 channelId,uint128 maxClaimableAmount,uint128 totalClaimed)";

/// `ClaimBatch`. The encoded type is the primary type followed by its one referenced struct.
pub const CLAIM_BATCH_TYPE: &str = "ClaimBatch(ClaimEntry[] claims)ClaimEntry(bytes32 channelId,uint128 maxClaimableAmount,uint128 totalClaimed)";

/// The largest `uint40`, the width the escrow gives `withdrawDelay`.
pub const UINT40_MAX: u64 = (1 << 40) - 1;

/// The `withdrawDelay` Tickline advertises and requires (#8).
///
/// This is a Tickline decision, not an x402 rule: the escrow accepts anything from 15 minutes to
/// 30 days. A channel below this floor is refused with `POLICY_WITHDRAW_DELAY_BELOW_FLOOR`, never
/// with an `X402_` code, because the payment protocol is satisfied and we are not (ADR-0012).
pub const ADVERTISED_WITHDRAW_DELAY: u64 = 3600;

/// One x402 channel: the tuple whose EIP-712 hash is the channel id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelConfig {
    /// The client wallet the funds came from.
    pub payer: Address,
    /// The EOA that signs vouchers for the payer.
    pub payer_authorizer: Address,
    /// The server's payment destination.
    pub receiver: Address,
    /// The address that authorizes claims and refunds.
    pub receiver_authorizer: Address,
    /// The ERC-20 the channel is denominated in.
    pub token: Address,
    /// Seconds between `initiateWithdraw` and `finalizeWithdraw`.
    pub withdraw_delay: u64,
    /// Distinguishes channels that are otherwise identical.
    pub salt: B256,
}

impl ChannelConfig {
    /// Validate a config Tickline is willing to serve.
    ///
    /// # Errors
    ///
    /// Stub.
    pub fn new(
        payer: Address,
        payer_authorizer: Address,
        receiver: Address,
        receiver_authorizer: Address,
        token: Address,
        withdraw_delay: u64,
        salt: B256,
    ) -> Result<Self, ProtocolError> {
        Ok(Self {
            payer,
            payer_authorizer,
            receiver,
            receiver_authorizer,
            token,
            withdraw_delay,
            salt,
        })
    }

    /// The EIP-712 struct hash.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        B256::ZERO
    }

    /// The channel id: this config's digest under the escrow's domain.
    #[must_use]
    pub fn channel_id(&self, domain: &Domain) -> B256 {
        let _ = domain;
        B256::ZERO
    }
}

/// A cumulative authorization: pay up to this much, in total, on this channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Voucher {
    /// The channel this voucher spends.
    pub channel_id: B256,
    /// The cumulative ceiling, never a per-request amount.
    pub max_claimable_amount: u128,
}

impl Voucher {
    /// The EIP-712 struct hash.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        B256::ZERO
    }

    /// The digest the payer signs.
    #[must_use]
    pub fn digest(&self, domain: &Domain) -> B256 {
        let _ = domain;
        B256::ZERO
    }
}

/// One channel's line in a claim batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimEntry {
    /// The channel being claimed.
    pub channel_id: B256,
    /// The ceiling the payer signed.
    pub max_claimable_amount: u128,
    /// The **cumulative** total claimed, never a per-batch delta.
    pub total_claimed: u128,
}

impl ClaimEntry {
    /// The EIP-712 struct hash.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        B256::ZERO
    }
}

/// A batch of claims, signed once by the receiver authorizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimBatch {
    /// The entries, in the order they are hashed.
    pub claims: Vec<ClaimEntry>,
}

impl ClaimBatch {
    /// `keccak256` over the packed concatenation of the entry struct hashes.
    #[must_use]
    pub fn entries_root(&self) -> B256 {
        B256::ZERO
    }

    /// The EIP-712 struct hash.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        B256::ZERO
    }

    /// The digest the receiver authorizer signs.
    #[must_use]
    pub fn digest(&self, domain: &Domain) -> B256 {
        let _ = domain;
        B256::ZERO
    }
}
