//! The one signature encoding this protocol accepts, and the reasons it refuses the others (#62).
//!
//! ADR-0012 is the policy: 65 bytes of `r || s || v`, `v` in {27, 28}, low-s only, scalars in
//! range, and verification against a named signer. Everything else is refused with a typed error,
//! because from #67 those errors are cross-stack vectors and from Phase 3 the vault refuses the
//! same inputs for the same reasons.
//!
//! The crate verifies and never signs, so no key and no randomness enters here (ADR-0011).

use alloy_primitives::{keccak256, Address, B256, U256};
use k256::ecdsa::{RecoveryId, Signature as EcdsaSignature, VerifyingKey};

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
    /// Parse and validate 65 bytes of `r || s || v`, in that order, `v` as 27 or 28.
    ///
    /// # Errors
    ///
    /// Every rule in ADR-0012, in the order they are checked: [`ProtocolError::CompactSignature`]
    /// for exactly 64 bytes, [`ProtocolError::SignatureLength`] for any other wrong length,
    /// [`ProtocolError::SignatureRecoveryId`] for a `v` outside {27, 28},
    /// [`ProtocolError::ScalarZero`] and [`ProtocolError::ScalarAboveCurveOrder`] for `r` and then
    /// `s`, and [`ProtocolError::HighS`] for `s` above `n / 2`.
    ///
    /// Order matters for the caller: an out-of-range `s` is reported as out of range, not as
    /// high-s, so one input has one reason.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, ProtocolError> {
        // EIP-2098 compact is exactly this length and a real encoding, so it gets its own reason
        // rather than being lumped in with truncation.
        if bytes.len() == 64 {
            return Err(ProtocolError::CompactSignature);
        }
        if bytes.len() != 65 {
            return Err(ProtocolError::SignatureLength { got: bytes.len() });
        }
        let r_bytes = bytes
            .get(..32)
            .ok_or(ProtocolError::SignatureLength { got: bytes.len() })?;
        let s_bytes = bytes
            .get(32..64)
            .ok_or(ProtocolError::SignatureLength { got: bytes.len() })?;
        let v = *bytes
            .get(64)
            .ok_or(ProtocolError::SignatureLength { got: bytes.len() })?;

        // Rejected, never normalized: a recovery id of 0 or 1 is a form we did not pin.
        if v != 27 && v != 28 {
            return Err(ProtocolError::SignatureRecoveryId { got: v });
        }

        let r = U256::from_be_slice(r_bytes);
        let s = U256::from_be_slice(s_bytes);
        check_scalar(r, Scalar::R)?;
        check_scalar(s, Scalar::S)?;

        // The high-s twin recovers the same signer, so accepting both would give one
        // authorization two encodings and make "have I seen this signature?" unanswerable.
        if s > HALF_CURVE_ORDER {
            return Err(ProtocolError::HighS);
        }

        Ok(Self {
            r: B256::from(r.to_be_bytes::<32>()),
            s: B256::from(s.to_be_bytes::<32>()),
            v,
        })
    }

    /// The 65 bytes this signature was parsed from.
    #[must_use]
    pub fn to_bytes(&self) -> [u8; 65] {
        let mut buffer = Vec::with_capacity(65);
        buffer.extend_from_slice(self.r.as_slice());
        buffer.extend_from_slice(self.s.as_slice());
        buffer.push(self.v);
        let mut out = [0u8; 65];
        out.copy_from_slice(&buffer);
        out
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
    /// The digest is used as a prehash: EIP-712 has already done the hashing, and hashing it
    /// again would verify a different message.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::Unrecoverable`] when no public key corresponds to this signature and
    /// digest. A validated signature can still be unrecoverable: `r` is a scalar, and not every
    /// scalar is the x coordinate of a curve point.
    pub fn recover(&self, digest: B256) -> Result<Address, ProtocolError> {
        let mut scalars = Vec::with_capacity(64);
        scalars.extend_from_slice(self.r.as_slice());
        scalars.extend_from_slice(self.s.as_slice());
        let ecdsa =
            EcdsaSignature::from_slice(&scalars).map_err(|_| ProtocolError::Unrecoverable)?;

        // v is 27 or 28 by construction, so this subtraction cannot wrap; it is written checked
        // because the workspace denies arithmetic that could.
        let parity = self
            .v
            .checked_sub(27)
            .ok_or(ProtocolError::SignatureRecoveryId { got: self.v })?;
        let recovery = RecoveryId::from_byte(parity)
            .ok_or(ProtocolError::SignatureRecoveryId { got: self.v })?;

        let key = VerifyingKey::recover_from_prehash(digest.as_slice(), &ecdsa, recovery)
            .map_err(|_| ProtocolError::Unrecoverable)?;

        // An Ethereum address is the last 20 bytes of the keccak of the uncompressed public key
        // with its 0x04 tag removed.
        let point = key.to_encoded_point(false);
        let body = point
            .as_bytes()
            .get(1..)
            .ok_or(ProtocolError::Unrecoverable)?;
        let hashed = keccak256(body);
        let tail = hashed
            .as_slice()
            .get(12..)
            .ok_or(ProtocolError::Unrecoverable)?;
        Ok(Address::from_slice(tail))
    }

    /// Check that `digest` was signed by `expected`.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::WrongSigner`] when the signature is valid but belongs to someone else,
    /// and whatever [`Signature::recover`] returns otherwise. ADR-0012: the caller names who must
    /// have signed, and "the recovered address is not zero" is never the check.
    pub fn verify(&self, digest: B256, expected: Address) -> Result<(), ProtocolError> {
        let recovered = self.recover(digest)?;
        if recovered != expected {
            return Err(ProtocolError::WrongSigner {
                expected,
                recovered,
            });
        }
        Ok(())
    }
}

/// `r` and `s` must be in `[1, n)`: zero is not a signature, and `n` or above is not a scalar.
fn check_scalar(value: U256, which: Scalar) -> Result<(), ProtocolError> {
    if value.is_zero() {
        return Err(ProtocolError::ScalarZero { scalar: which });
    }
    if value >= CURVE_ORDER {
        return Err(ProtocolError::ScalarAboveCurveOrder { scalar: which });
    }
    Ok(())
}
