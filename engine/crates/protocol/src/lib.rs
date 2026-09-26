//! Wire types shared with the chain and with TypeScript clients.
//!
//! Phase 2 fills this in from `docs/spec-notes.md`. **No field here is invented**
//! (CLAUDE.md section 2): every voucher field comes from a cited source, and every
//! hash is proven byte-identical across Rust, Solidity, and TypeScript by
//! `testdata/vectors/eip712.json`.
//!
//! The crate is zero IO and verification only: it never signs, and it holds no key
//! (ADR-0011, ADR-0012).

#![forbid(unsafe_code)]

pub mod eip712;
pub mod envelope;
pub mod market_id;
pub mod receipt;
pub mod signature;
pub mod x402;

mod error;

pub use error::{
    CodeFamily, ProtocolError, Reach, Scalar, CODE_FAMILIES, CODE_PREFIXES, ENV_ERROR_CODES,
    POLICY_ERROR_CODES, SIGNATURE_ERROR_CODES, X402_ERROR_CODES,
};
pub use signature::Signature;

/// EIP-712 domain name for every Tickline-owned signed struct.
pub const DOMAIN_NAME: &str = "Tickline";

/// EIP-712 domain version. Bumping this invalidates every outstanding receipt,
/// so it changes only with an ADR.
pub const DOMAIN_VERSION: &str = "1";

#[cfg(test)]
mod tests {
    use super::*;

    /// Phase 0 smoke test. Phase 2 replaces this with the cross-stack vector suite.
    #[test]
    fn domain_is_pinned() {
        assert_eq!(DOMAIN_NAME, "Tickline");
        assert_eq!(DOMAIN_VERSION, "1");
    }
}
