//! Settlement worker: redeems voucher batches and commits epochs before the grace deadline.
//!
//! Phase 5 fills this in. Every chain-mutating action is driven by an outbox intent row
//! written *before* submission, so a crash at any point costs at most a resubmission and
//! never a double spend (I14).

#![forbid(unsafe_code)]

/// The lifecycle of one outbox intent. Transitions are forward-only and each one is a
/// committed database write, so a crash resumes from the last durable state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntentState {
    /// Written, not yet submitted. A crash here means "submit it".
    Pending,
    /// Submitted; transaction hash recorded. A crash here means "wait, then check the hash".
    Submitted,
    /// Observed by the indexer at confirmation depth. Terminal.
    Confirmed,
    /// Permanently abandoned, with a reason. Terminal, and alertable.
    Abandoned,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 0 smoke test. Phase 5 replaces this with the crash-injection suites.
    #[test]
    fn intent_states_are_distinguishable() {
        assert_ne!(IntentState::Pending, IntentState::Submitted);
        assert_ne!(IntentState::Confirmed, IntentState::Abandoned);
    }
}
