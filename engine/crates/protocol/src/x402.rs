//! The x402 batch-settlement types, exactly as `docs/spec-notes.md` §1 records them (issue #63).
//!
//! **No field here is invented.** Every type string is the one cited in §1 and verified against
//! the deployed escrow's own type hash getters, and every fixture in
//! `testdata/vectors/eip712-primitives.json` was read from the deployment rather than derived
//! from our reading of the spec.
//!
//! The types are plain data with one hashing method each. Nothing here decides anything: the
//! engine's rules about headroom (I7) live in Phase 4, and this crate only says what the escrow
//! would have hashed.

use alloy_primitives::{keccak256, Address, B256, U256};

use crate::{
    eip712::{type_hash, Domain},
    ProtocolError,
};

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
    /// [`ProtocolError::WithdrawDelayWidth`] when `withdraw_delay` does not fit `uint40`: such a
    /// value cannot be the config the escrow hashed, so no stack would accept it.
    ///
    /// [`ProtocolError::WithdrawDelayBelowFloor`] when it is below
    /// [`ADVERTISED_WITHDRAW_DELAY`]. That is a valid x402 channel we decline to serve, which is
    /// why it carries a `POLICY_` code and not an `X402_` one (ADR-0012).
    ///
    /// Width is checked first: a value that is not a `uint40` is not a channel at all, so
    /// reporting it as below our floor would name the wrong problem.
    pub fn new(
        payer: Address,
        payer_authorizer: Address,
        receiver: Address,
        receiver_authorizer: Address,
        token: Address,
        withdraw_delay: u64,
        salt: B256,
    ) -> Result<Self, ProtocolError> {
        if withdraw_delay > UINT40_MAX {
            return Err(ProtocolError::WithdrawDelayWidth {
                got: withdraw_delay,
            });
        }
        if withdraw_delay < ADVERTISED_WITHDRAW_DELAY {
            return Err(ProtocolError::WithdrawDelayBelowFloor {
                got: withdraw_delay,
                floor: ADVERTISED_WITHDRAW_DELAY,
            });
        }
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

    /// The EIP-712 struct hash: seven words, every value padded to 32 bytes by `abi.encode`.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        let mut encoded = Vec::with_capacity(32 * 8);
        encoded.extend_from_slice(type_hash(CHANNEL_CONFIG_TYPE).as_slice());
        for address in [
            self.payer,
            self.payer_authorizer,
            self.receiver,
            self.receiver_authorizer,
            self.token,
        ] {
            encoded.extend_from_slice(B256::left_padding_from(address.as_slice()).as_slice());
        }
        encoded.extend_from_slice(&U256::from(self.withdraw_delay).to_be_bytes::<32>());
        encoded.extend_from_slice(self.salt.as_slice());
        keccak256(&encoded)
    }

    /// The channel id: this config's digest under the escrow's domain.
    ///
    /// The escrow computes the same value in `getChannelId`, which is what the fixtures are read
    /// from. Because the domain binds the chain and the contract, one config is a different
    /// channel on a different chain.
    #[must_use]
    pub fn channel_id(&self, domain: &Domain) -> B256 {
        domain.digest(self.struct_hash())
    }
}

impl Voucher {
    /// The EIP-712 struct hash. Two fields: no amount per request, no expiry, no nonce.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        let mut encoded = Vec::with_capacity(96);
        encoded.extend_from_slice(type_hash(VOUCHER_TYPE).as_slice());
        encoded.extend_from_slice(self.channel_id.as_slice());
        encoded.extend_from_slice(&U256::from(self.max_claimable_amount).to_be_bytes::<32>());
        keccak256(&encoded)
    }

    /// The digest the payer signs, as `getVoucherDigest` returns it.
    #[must_use]
    pub fn digest(&self, domain: &Domain) -> B256 {
        domain.digest(self.struct_hash())
    }
}

impl ClaimEntry {
    /// The EIP-712 struct hash.
    ///
    /// It hashes the **derived** `channelId`, not the `ChannelConfig`, so the entry is one level
    /// flatter than the wire struct the escrow's `claim` takes.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        let mut encoded = Vec::with_capacity(128);
        encoded.extend_from_slice(type_hash(CLAIM_ENTRY_TYPE).as_slice());
        encoded.extend_from_slice(self.channel_id.as_slice());
        encoded.extend_from_slice(&U256::from(self.max_claimable_amount).to_be_bytes::<32>());
        encoded.extend_from_slice(&U256::from(self.total_claimed).to_be_bytes::<32>());
        keccak256(&encoded)
    }
}

impl ClaimBatch {
    /// `keccak256` over the packed concatenation of the entry struct hashes.
    ///
    /// This is EIP-712's rule for an array member, and **not** an `abi.encode` of the array.
    /// Order is part of the hash, which is why the fixtures carry a two-entry batch: a
    /// one-element array would hide a concatenation or ordering mistake.
    #[must_use]
    pub fn entries_root(&self) -> B256 {
        let mut packed = Vec::with_capacity(self.claims.len().saturating_mul(32));
        for claim in &self.claims {
            packed.extend_from_slice(claim.struct_hash().as_slice());
        }
        keccak256(&packed)
    }

    /// The EIP-712 struct hash.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        let mut encoded = Vec::with_capacity(64);
        encoded.extend_from_slice(type_hash(CLAIM_BATCH_TYPE).as_slice());
        encoded.extend_from_slice(self.entries_root().as_slice());
        keccak256(&encoded)
    }

    /// The digest the receiver authorizer signs, as `getClaimBatchDigest` returns it.
    #[must_use]
    pub fn digest(&self, domain: &Domain) -> B256 {
        domain.digest(self.struct_hash())
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

/// A batch of claims, signed once by the receiver authorizer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimBatch {
    /// The entries, in the order they are hashed.
    pub claims: Vec<ClaimEntry>,
}
