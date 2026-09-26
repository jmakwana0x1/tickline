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
//! The receipt is plain data with one hashing method, one recovery, and one verification. It
//! holds no key: the operator signs elsewhere, and this crate only checks.

use alloy_primitives::{keccak256, Address, B256, U256};

use crate::{
    eip712::{type_hash, Domain},
    ProtocolError, Signature,
};

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
    /// The EIP-712 struct hash: nine words, every value padded to 32 bytes by `abi.encode`.
    ///
    /// The narrow widths are part of the type string and so part of the hash, but they do not
    /// change the encoding: `abi.encode` pads a `uint32` to 32 bytes like everything else.
    #[must_use]
    pub fn struct_hash(&self) -> B256 {
        let mut encoded = Vec::with_capacity(32 * 9);
        encoded.extend_from_slice(type_hash(POSITION_RECEIPT_TYPE).as_slice());
        encoded.extend_from_slice(self.market_id.as_slice());
        encoded.extend_from_slice(B256::left_padding_from(self.agent.as_slice()).as_slice());
        for amount in [
            self.yes_shares,
            self.no_shares,
            self.cost_paid,
            self.fees_paid,
        ] {
            encoded.extend_from_slice(&U256::from(amount).to_be_bytes::<32>());
        }
        encoded.extend_from_slice(&U256::from(self.nonce).to_be_bytes::<32>());
        encoded.extend_from_slice(&U256::from(self.epoch).to_be_bytes::<32>());
        keccak256(&encoded)
    }

    /// The digest the operator signs.
    #[must_use]
    pub fn digest(&self, domain: &Domain) -> B256 {
        domain.digest(self.struct_hash())
    }

    /// The address that signed this receipt.
    ///
    /// # Errors
    ///
    /// Whatever [`Signature::recover`] returns. Note that a signature made over a *different*
    /// receipt does not fail here: it recovers a stranger. Deciding whether that stranger is the
    /// operator is [`PositionReceipt::verify`]'s job, which is why callers should prefer it.
    pub fn recover(
        &self,
        domain: &Domain,
        signature: &Signature,
    ) -> Result<Address, ProtocolError> {
        signature.recover(self.digest(domain))
    }

    /// Check that this receipt was signed by `operator`.
    ///
    /// This is the check I12 rests on: a receipt is evidence only because the operator's
    /// signature over *this* receipt recovers the operator's key. Altering any field produces a
    /// signature that recovers to a stranger, so the answer is `WrongSigner` and never
    /// `Unrecoverable`.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::WrongSigner`] when the signature belongs to someone else, and whatever
    /// [`Signature::recover`] returns otherwise.
    pub fn verify(
        &self,
        domain: &Domain,
        signature: &Signature,
        operator: Address,
    ) -> Result<(), ProtocolError> {
        signature.verify(self.digest(domain), operator)
    }
}
