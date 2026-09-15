//! Append-only double-entry ledger.
//!
//! Phase 4 fills this in. Two invariants define the crate:
//!
//! - **I8** every transaction balances to zero, and nothing is ever `UPDATE`d or `DELETE`d;
//! - **I9** replaying from empty reproduces actor state byte for byte. Actor memory is a
//!   cache of this ledger, never a source of truth.

#![forbid(unsafe_code)]

/// Accounts in the double-entry system. Every entry names exactly one.
///
/// The set is closed: a new account kind is a schema migration and an ADR, not a new variant
/// slipped into a feature branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AccountKind {
    /// An agent's x402 payment session.
    AgentSession,
    /// Collateral held against a market's payouts.
    MarketCollateral,
    /// Fees accrued to the operator.
    OperatorFees,
    /// The creator's subsidy funding the market maker.
    CreatorSubsidy,
    /// The operator's own wallet.
    OperatorWallet,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 0 smoke test. Phase 4 replaces this with the `#[sqlx::test]` lifecycle suites.
    #[test]
    fn account_kinds_are_distinct() {
        let all = [
            AccountKind::AgentSession,
            AccountKind::MarketCollateral,
            AccountKind::OperatorFees,
            AccountKind::CreatorSubsidy,
            AccountKind::OperatorWallet,
        ];
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                assert_ne!(a, b);
            }
        }
    }
}
