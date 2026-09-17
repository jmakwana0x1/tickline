//! The market actor: one per market, owns LMSR state, executes fills, advances epochs.
//!
//! Phase 4 fills this in. The actor never reads a wall clock directly; time arrives through
//! an injected [`Clock`] so every epoch boundary is testable to the second (CLAUDE.md section 2).

#![forbid(unsafe_code)]

/// Time, injected. The only way anything in the engine learns what time it is.
///
/// Production uses the system clock; tests use a clock they can step, so a deadline
/// boundary at `t - 1`, `t`, and `t + 1` is an ordinary unit test rather than a sleep.
pub trait Clock: Send + Sync + 'static {
    /// Seconds since the Unix epoch.
    fn now_unix(&self) -> u64;
}

/// A clock a test drives by hand.
#[derive(Debug, Default)]
pub struct FixedClock {
    now: u64,
}

impl FixedClock {
    /// A clock stopped at `now`.
    #[must_use]
    pub const fn at(now: u64) -> Self {
        Self { now }
    }

    /// Move the clock forward by `secs`. Saturates rather than wrapping.
    pub fn advance(&mut self, secs: u64) {
        self.now = self.now.saturating_add(secs);
    }
}

impl Clock for FixedClock {
    fn now_unix(&self) -> u64 {
        self.now
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 0 smoke test. Phase 4 replaces this with the fill and epoch suites.
    #[test]
    fn fixed_clock_only_moves_when_told() {
        let mut clock = FixedClock::at(1_000);
        assert_eq!(clock.now_unix(), 1_000);
        clock.advance(60);
        assert_eq!(clock.now_unix(), 1_060);
    }
}
