//! Chain follower: stores blocks with hashes, applies events at confirmation depth `N`,
//! detects reorgs by parent-hash mismatch, rolls back and reapplies.
//!
//! Phase 5 fills this in. A reorg deeper than `N` is not something to paper over: the
//! indexer halts and alerts rather than silently diverging from the chain (I13).

#![forbid(unsafe_code)]

/// How the indexer reacted to a newly seen block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ChainEvent {
    /// The block extends the known tip.
    Extended,
    /// A parent-hash mismatch within `N`: roll back and reapply.
    Reorged {
        /// How many blocks were rolled back.
        depth: u64,
    },
    /// A reorg deeper than the confirmation depth. Halt and alert; never guess.
    ReorgDeeperThanConfirmations {
        /// How deep the reorg went.
        depth: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 0 smoke test. Phase 5 replaces this with the anvil-backed reorg suites.
    #[test]
    fn deep_reorgs_are_a_distinct_outcome() {
        assert_ne!(
            ChainEvent::Reorged { depth: 3 },
            ChainEvent::ReorgDeeperThanConfirmations { depth: 3 }
        );
    }
}
