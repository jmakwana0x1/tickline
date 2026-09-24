//! `PositionReceipt`: what an agent holds against a lying operator (issue #64).
//!
//! **This module says what a receipt may encode, not what the engine may issue.** A receipt with
//! every field at its type maximum is well formed here and economically impossible: `Q_MAX` caps
//! shares about twenty orders of magnitude lower (Phase 4), and the vault's solvency check caps
//! what can be paid (Phase 3). Nothing in a zero-IO crate can know whether a market exists, so
//! nothing here pretends to.
//!
//! The widths are Q5's (`docs/spec-notes.md` §8): the four amounts are `uint128`, which is what
//! x402's `maxClaimableAmount` and `totalClaimed` are, so no width conversion happens at the
//! boundary where money changes hands.
//!
//! Stub for the red commit; the implementation follows.

use alloy_primitives::{Address, B256};

use crate::{eip712::Domain, ProtocolError, Signature};

/// The EIP-712 type, as it is hashed. Field order is part of the hash.
pub const POSITION_RECEIPT_TYPE: &str = "PositionReceipt(bytes32 marketId,address agent,uint128 yesShares,uint128 noShares,uint128 costPaid,uint128 feesPaid,uint64 nonce,uint32 epoch)";

/// One agent's position in one market, as of one nonce, signed by the operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PositionReceipt {
    /// The market this position is in.
    pub market_id: B256,
    /// The agent who holds it.
    pub agent: Address,
    /// Cumulative YES shares, in base units.
    pub yes_shares: u128,
    /// Cumulative NO shares, in base units.
    pub no_shares: u128,
    /// Cumulative cost paid, in USDC base units.
    pub cost_paid: u128,
    /// Cumulative fees paid, in USDC base units.
    pub fees_paid: u128,
    /// Strictly increasing per `(market, agent)` (I6).
    pub nonce: u64,
    /// The epoch this receipt belongs to.
    pub epoch: u32,
}

impl PositionReceipt {
    /// The EIP-712 struct hash.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        B256::ZERO
    }

    /// The digest the operator signs.
    #[must_use]
    pub fn digest(&self, domain: &Domain) -> B256 {
        let _ = domain;
        B256::ZERO
    }

    /// The address that signed this receipt.
    ///
    /// # Errors
    ///
    /// Stub.
    pub fn recover(
        &self,
        domain: &Domain,
        signature: &Signature,
    ) -> Result<Address, ProtocolError> {
        let _ = (domain, signature);
        Ok(Address::ZERO)
    }

    /// Check that this receipt was signed by `operator`.
    ///
    /// # Errors
    ///
    /// Stub.
    pub fn verify(
        &self,
        domain: &Domain,
        signature: &Signature,
        operator: Address,
    ) -> Result<(), ProtocolError> {
        let _ = (domain, signature, operator);
        Ok(())
    }
}
