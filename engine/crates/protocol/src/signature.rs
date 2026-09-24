//! The one signature encoding this protocol accepts, and the reasons it refuses the others (#62).
//!
//! ADR-0012 is the policy. Stub for the red commit; the implementation follows.

use alloy_primitives::{Address, B256, U256};

use crate::{ProtocolError, Scalar};

/// The secp256k1 group order `n`.
pub const CURVE_ORDER: U256 = U256::from_limbs([
    0xbfd2_5e8c_d036_4141,
    0xbaae_dce6_af48_a03b,
    0xffff_ffff_ffff_fffe,
    0xffff_ffff_ffff_ffff,
]);

/// `n / 2`, rounded down. A signature with `s` above this is rejected (ADR-0012).
pub const HALF_CURVE_ORDER: U256 = U256::from_limbs([
    0xdfe9_2f46_681b_20a0,
    0x5d57_6e73_57a4_501d,
    0xffff_ffff_ffff_ffff,
    0x7fff_ffff_ffff_ffff,
]);

/// A signature that has already passed every rule in ADR-0012.
///
/// The type exists so that "validated" is a state in the type system rather than a convention: a
/// value of this type cannot be high-s, cannot carry an out-of-range scalar, and cannot have a
/// `v` outside {27, 28}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Signature {
    r: B256,
    s: B256,
    v: u8,
}

impl Signature {
    /// Parse and validate 65 bytes of `r || s || v`.
    ///
    /// # Errors
    ///
    /// Stub.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        let _ = bytes;
        let _ = Scalar::R;
        Ok(Self { r: B256::ZERO, s: B256::ZERO, v: 27 })
    }

    /// The 65 bytes this signature was parsed from.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 65] {
        [0u8; 65]
    }

    /// The `r` component.
    #[must_use]
    pub const fn r(&self) -> B256 {
        self.r
    }

    /// The `s` component.
    #[must_use]
    pub const fn s(&self) -> B256 {
        self.s
    }

    /// The `v` byte, always 27 or 28.
    #[must_use]
    pub const fn v(&self) -> u8 {
        self.v
    }

    /// The address that signed `digest`.
    ///
    /// # Errors
    ///
    /// Stub.
    pub fn recover(&self, digest: B256) -> Result<Address, ProtocolError> {
        let _ = digest;
        Ok(Address::ZERO)
    }

    /// Check that `digest` was signed by `expected`.
    ///
    /// # Errors
    ///
    /// Stub.
    pub fn verify(&self, digest: B256, expected: Address) -> Result<(), ProtocolError> {
        let _ = (digest, expected);
        Ok(())
    }
}
