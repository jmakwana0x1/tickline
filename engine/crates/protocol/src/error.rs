//! Every way a signed object can be refused, as a typed variant (issue #62).
//!
//! One variant per rejection rule in ADR-0012: the reason a signature was refused is part of the
//! protocol, because from #67 the same reasons are cross-stack vectors and from Phase 3 the vault
//! refuses the same inputs for the same reasons.

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
