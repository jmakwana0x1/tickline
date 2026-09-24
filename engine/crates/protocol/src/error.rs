//! Every way a signed object can be refused, as a typed variant (issue #62).
//!
//! One variant per rejection rule in ADR-0012: the reason a signature was refused is part of the
//! protocol, because from #67 the same reasons are cross-stack vectors and from Phase 3 the vault
//! refuses the same inputs for the same reasons.
//!
//! **The code is the contract, not the message.** [`ProtocolError::code`] returns a flat, stable
//! string that Solidity and TypeScript can both produce: Solidity custom errors carry no message,
//! and a TypeScript client would otherwise invent its own wording. The English text is for humans
//! reading logs.

use alloy_primitives::Address;

/// Which scalar of a signature a rejection is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scalar {
    /// The `r` component, bytes 0 to 31.
    R,
    /// The `s` component, bytes 32 to 63.
    S,
}

impl core::fmt::Display for Scalar {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::R => write!(f, "r"),
            Self::S => write!(f, "s"),
        }
    }
}

/// A signed object was refused. Every variant is a rule in ADR-0012.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    /// The signature was not 65 bytes. ADR-0012 pins one wire form.
    #[error("signature must be 65 bytes, got {got}")]
    SignatureLength {
        /// The length that arrived.
        got: usize,
    },

    /// The signature was 64 bytes: an EIP-2098 compact signature, which is a real encoding of a
    /// real signature and still not the one this protocol accepts.
    #[error("EIP-2098 compact signatures are not accepted; send 65 bytes of r || s || v")]
    CompactSignature,

    /// `v` was not 27 or 28. A recovery id of 0 or 1 is rejected, never normalized.
    #[error("signature v must be 27 or 28, got {got}")]
    SignatureRecoveryId {
        /// The `v` byte that arrived.
        got: u8,
    },

    /// `r` or `s` was zero.
    #[error("signature {scalar} is zero")]
    ScalarZero {
        /// Which component.
        scalar: Scalar,
    },

    /// `r` or `s` was at or above the secp256k1 group order.
    #[error("signature {scalar} is at or above the curve order")]
    ScalarAboveCurveOrder {
        /// Which component.
        scalar: Scalar,
    },

    /// `s` was above `n / 2`. The low-s twin is the same authorization.
    #[error("signature s is above n/2; the low-s form is the only accepted encoding")]
    HighS,

    /// No public key could be recovered from the signature and digest.
    #[error("no signer could be recovered from this signature")]
    Unrecoverable,

    /// A signature recovered, but to a different address than the caller required.
    #[error("signature recovers to {recovered}, expected {expected}")]
    WrongSigner {
        /// The signer the caller required.
        expected: Address,
        /// The signer the signature actually carries.
        recovered: Address,
    },
}

impl ProtocolError {
    /// The stable code for this rejection, as the cross-stack vectors carry it (#67).
    ///
    /// Codes are flat, one per rejection, so [`Scalar`] stays a Rust detail: `SIG_R_ZERO` and
    /// `SIG_S_ZERO` are separate codes rather than one code with a field, because Solidity would
    /// otherwise have to encode which component failed. `TicklineTypes.sol` declares one custom
    /// error per code, and the TypeScript client carries the code on its error.
    ///
    /// These strings are protocol surface: changing one is a breaking change for every stack, so
    /// it takes an ADR.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::SignatureLength { .. } => "SIG_LENGTH",
            Self::CompactSignature => "SIG_COMPACT",
            Self::SignatureRecoveryId { .. } => "SIG_RECOVERY_ID",
            Self::ScalarZero { scalar: Scalar::R } => "SIG_R_ZERO",
            Self::ScalarZero { scalar: Scalar::S } => "SIG_S_ZERO",
            Self::ScalarAboveCurveOrder { scalar: Scalar::R } => "SIG_R_ABOVE_ORDER",
            Self::ScalarAboveCurveOrder { scalar: Scalar::S } => "SIG_S_ABOVE_ORDER",
            Self::HighS => "SIG_HIGH_S",
            Self::Unrecoverable => "SIG_UNRECOVERABLE",
            Self::WrongSigner { .. } => "SIG_WRONG_SIGNER",
        }
    }
}

/// The reserved code prefix for every signed object family (ADR-0012).
///
/// One namespace per family, fixed before the codes cross a stack boundary: retrofitting a
/// namespace once Solidity and TypeScript both carry the codes would be a breaking change in
/// three places at once. `SIG_` is signatures (#62); `RCPT_` receipts (#64), `MID_` market ids
/// (#65) and `ENV_` the 402 envelopes (#66) are reserved and unused so far.
pub const CODE_PREFIXES: [&str; 4] = ["SIG_", "RCPT_", "MID_", "ENV_"];

/// Every published code list. S3 to S5 append theirs, and the registry test then covers them.
pub const ALL_ERROR_CODES: [&[&str]; 1] = [&SIGNATURE_ERROR_CODES];

/// Every code [`ProtocolError::code`] can return.
///
/// The list exists so a test can assert that the set is exactly this and that no two variants
/// share a code. #67 checks the same list against the vector file, and Phase 3 against the
/// custom errors in `TicklineTypes.sol`.
pub const SIGNATURE_ERROR_CODES: [&str; 10] = [
    "SIG_LENGTH",
    "SIG_COMPACT",
    "SIG_RECOVERY_ID",
    "SIG_R_ZERO",
    "SIG_S_ZERO",
    "SIG_R_ABOVE_ORDER",
    "SIG_S_ABOVE_ORDER",
    "SIG_HIGH_S",
    "SIG_UNRECOVERABLE",
    "SIG_WRONG_SIGNER",
];
