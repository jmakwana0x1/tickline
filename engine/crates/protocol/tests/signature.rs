//! ADR-0012, one named test per rule (issue #62).
//!
//! **Provenance.** The valid signatures come from `testdata/vectors/eip712-primitives.json`,
//! produced with `cast wallet sign --no-hash` from anvil's first two accounts, whose keys are
//! public test material and are deliberately not committed. Everything invalid is derived here
//! from those two, so each rejection case is an accepted signature with exactly one thing
//! changed. #67 turns every one of them into a cross-stack vector, and Phase 3's vault runs the
//! same set.

// Shared by both test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use alloy_primitives::{Address, B256, U256};
use common::{address, group, hash, load, section, signature_bytes, text, uint, TestResult};
use protocol::{
    signature::{CURVE_ORDER, HALF_CURVE_ORDER},
    ProtocolError, Scalar, Signature, SIGNATURE_ERROR_CODES,
};

/// One valid fixture: the bytes, the digest they were signed over, and who signed.
struct Fixture {
    bytes: [u8; 65],
    digest: B256,
    signer: Address,
}

fn fixture(vectors: &serde_json::Value, name: &str) -> TestResult<Fixture> {
    let case = group(vectors, "signatures")?
        .iter()
        .find(|c| text(c, "name").is_ok_and(|n| n == name))
        .ok_or_else(|| format!("no '{name}' signature fixture"))?;
    Ok(Fixture {
        bytes: signature_bytes(case, "signature")?,
        digest: hash(case, "digest")?,
        signer: address(case, "signer")?,
    })
}

/// The same, loading the file itself, for tests that need nothing else from it.
fn load_fixture(name: &str) -> TestResult<Fixture> {
    let vectors = load()?;
    fixture(&vectors, name)
}

/// Assemble 65 bytes from parts, so an invalid case differs from a valid one by one field.
fn bytes_of(r: U256, s: U256, v: u8) -> [u8; 65] {
    let mut buffer = Vec::with_capacity(65);
    buffer.extend_from_slice(&r.to_be_bytes::<32>());
    buffer.extend_from_slice(&s.to_be_bytes::<32>());
    buffer.push(v);
    let mut out = [0u8; 65];
    out.copy_from_slice(&buffer);
    out
}

fn parts(sig: &[u8; 65]) -> TestResult<(U256, U256, u8)> {
    let r = U256::from_be_slice(sig.get(..32).ok_or("r")?);
    let s = U256::from_be_slice(sig.get(32..64).ok_or("s")?);
    let v = *sig.get(64).ok_or("v")?;
    Ok((r, s, v))
}

#[test]
fn recover_accepts_v_27_and_28() -> TestResult {
    let vectors = load()?;
    let v27 = fixture(&vectors, "v27")?;
    let v28 = fixture(&vectors, "v28")?;

    assert_eq!(
        Signature::from_bytes(&v27.bytes)?.recover(v27.digest)?,
        v27.signer
    );
    assert_eq!(
        Signature::from_bytes(&v28.bytes)?.recover(v28.digest)?,
        v28.signer
    );

    assert_eq!(
        (parts(&v27.bytes)?.2, parts(&v28.bytes)?.2),
        (27, 28),
        "the fixtures cover both parities"
    );
    Ok(())
}

#[test]
fn recover_rejects_v_0_and_1() -> TestResult {
    // ADR-0012 rejects rather than normalizes: these are the same signatures with v written as a
    // bare recovery id, which viem never emits.
    let (r, s, _) = parts(&load_fixture("v27")?.bytes)?;
    for v in [0u8, 1u8] {
        assert_eq!(
            Signature::from_bytes(&bytes_of(r, s, v)),
            Err(ProtocolError::SignatureRecoveryId { got: v })
        );
    }
    Ok(())
}

#[test]
fn recover_rejects_v_outside_the_accepted_pair() -> TestResult {
    let (r, s, _) = parts(&load_fixture("v27")?.bytes)?;
    for v in [2u8, 26u8, 29u8, 255u8] {
        assert_eq!(
            Signature::from_bytes(&bytes_of(r, s, v)),
            Err(ProtocolError::SignatureRecoveryId { got: v })
        );
    }
    Ok(())
}

#[test]
fn signature_must_be_exactly_65_bytes() -> TestResult {
    for len in [0usize, 1, 32, 63, 66, 130] {
        let bytes = vec![0x11u8; len];
        assert_eq!(
            Signature::from_bytes(&bytes),
            Err(ProtocolError::SignatureLength { got: len }),
            "length {len} must be refused before anything else is inspected"
        );
    }
    assert!(Signature::from_bytes(&load_fixture("v27")?.bytes).is_ok());
    Ok(())
}

#[test]
fn recover_rejects_compact_signature() -> TestResult {
    // EIP-2098: r || yParityAndS, 64 bytes. A real encoding of a real signature, and not ours.
    let valid = load_fixture("v27")?.bytes;
    let compact = valid.get(..64).ok_or("compact")?;
    assert_eq!(
        Signature::from_bytes(compact),
        Err(ProtocolError::CompactSignature)
    );
    Ok(())
}

#[test]
fn recover_rejects_high_s_signature() -> TestResult {
    let (r, s, v) = parts(&load_fixture("v27")?.bytes)?;
    assert!(s <= HALF_CURVE_ORDER, "the fixture must start out low-s");

    let high_s = CURVE_ORDER.checked_sub(s).ok_or("n - s")?;
    assert!(high_s > HALF_CURVE_ORDER);
    let flipped_v = if v == 27 { 28 } else { 27 };

    // The malleable twin recovers the same signer, which is exactly why it is refused: accepting
    // both would give one authorization two valid encodings.
    assert_eq!(
        Signature::from_bytes(&bytes_of(r, high_s, flipped_v)),
        Err(ProtocolError::HighS)
    );
    Ok(())
}

#[test]
fn s_exactly_at_half_the_curve_order_is_accepted() -> TestResult {
    // EIP-2 invalidates s *greater than* n/2, so floor(n/2) is the largest accepted value and the
    // boundary is inclusive. A signature is refused one wei of scalar above it, and not at it.
    let (r, _, v) = parts(&load_fixture("v27")?.bytes)?;

    let at = Signature::from_bytes(&bytes_of(r, HALF_CURVE_ORDER, v))?;
    assert_eq!(at.s(), B256::from(HALF_CURVE_ORDER.to_be_bytes::<32>()));

    let above = HALF_CURVE_ORDER
        .checked_add(U256::from(1u8))
        .ok_or("n/2 + 1")?;
    assert_eq!(
        Signature::from_bytes(&bytes_of(r, above, v)),
        Err(ProtocolError::HighS)
    );
    Ok(())
}

#[test]
fn recover_rejects_r_or_s_zero() -> TestResult {
    let (r, s, v) = parts(&load_fixture("v27")?.bytes)?;
    assert_eq!(
        Signature::from_bytes(&bytes_of(U256::ZERO, s, v)),
        Err(ProtocolError::ScalarZero { scalar: Scalar::R })
    );
    assert_eq!(
        Signature::from_bytes(&bytes_of(r, U256::ZERO, v)),
        Err(ProtocolError::ScalarZero { scalar: Scalar::S })
    );
    Ok(())
}

#[test]
fn recover_rejects_r_or_s_at_or_above_curve_order() -> TestResult {
    let (r, s, v) = parts(&load_fixture("v27")?.bytes)?;
    let above = CURVE_ORDER.checked_add(U256::from(1u8)).ok_or("n + 1")?;
    for bad in [CURVE_ORDER, above, U256::MAX] {
        assert_eq!(
            Signature::from_bytes(&bytes_of(bad, s, v)),
            Err(ProtocolError::ScalarAboveCurveOrder { scalar: Scalar::R })
        );
        assert_eq!(
            Signature::from_bytes(&bytes_of(r, bad, v)),
            Err(ProtocolError::ScalarAboveCurveOrder { scalar: Scalar::S })
        );
    }
    Ok(())
}

#[test]
fn recover_fails_when_no_point_matches_r() -> TestResult {
    // r is a valid scalar but not the x coordinate of any point with this parity, so there is
    // nothing to recover. The signature passes every syntactic rule and still has no signer.
    let vectors = load()?;
    let digest = fixture(&vectors, "v27")?.digest;
    let r = CURVE_ORDER.checked_sub(U256::from(1u8)).ok_or("n - 1")?;
    let signature = Signature::from_bytes(&bytes_of(r, U256::from(1u8), 27))?;
    assert_eq!(signature.recover(digest), Err(ProtocolError::Unrecoverable));
    Ok(())
}

#[test]
fn verify_requires_the_expected_signer() -> TestResult {
    let vectors = load()?;
    let valid = fixture(&vectors, "v27")?;
    let other = address(&vectors, "other_signer")?;

    let signature = Signature::from_bytes(&valid.bytes)?;
    signature.verify(valid.digest, valid.signer)?;

    // Not "the recovered address is not zero": the caller names who must have signed.
    assert_eq!(
        signature.verify(valid.digest, other),
        Err(ProtocolError::WrongSigner {
            expected: other,
            recovered: valid.signer
        })
    );
    Ok(())
}

#[test]
fn a_signature_is_bound_to_its_digest() -> TestResult {
    let vectors = load()?;
    let valid = fixture(&vectors, "v27")?;
    let elsewhere = fixture(&vectors, "v28")?.digest;

    let signature = Signature::from_bytes(&valid.bytes)?;
    assert_ne!(
        signature.recover(elsewhere)?,
        valid.signer,
        "the same signature over another digest is another signer"
    );
    assert!(signature.verify(elsewhere, valid.signer).is_err());
    Ok(())
}

#[test]
fn accessors_and_round_trip_preserve_the_bytes() -> TestResult {
    let valid = load_fixture("v27")?.bytes;
    let signature = Signature::from_bytes(&valid)?;
    assert_eq!(signature.to_bytes(), valid);

    let (r, s, v) = parts(&valid)?;
    assert_eq!(signature.r(), B256::from(r.to_be_bytes::<32>()));
    assert_eq!(signature.s(), B256::from(s.to_be_bytes::<32>()));
    assert_eq!(signature.v(), v);
    Ok(())
}

#[test]
fn every_rejection_reason_renders_its_own_message() -> TestResult {
    // From #67 these strings are the expected rejection reasons in the vector file, which the
    // Solidity and TypeScript sides match against, so the wording is part of the protocol.
    let rendered = [
        (
            ProtocolError::SignatureLength { got: 64 },
            "signature must be 65 bytes, got 64",
        ),
        (
            ProtocolError::CompactSignature,
            "EIP-2098 compact signatures are not accepted; send 65 bytes of r || s || v",
        ),
        (
            ProtocolError::SignatureRecoveryId { got: 1 },
            "signature v must be 27 or 28, got 1",
        ),
        (
            ProtocolError::ScalarZero { scalar: Scalar::R },
            "signature r is zero",
        ),
        (
            ProtocolError::ScalarZero { scalar: Scalar::S },
            "signature s is zero",
        ),
        (
            ProtocolError::ScalarAboveCurveOrder { scalar: Scalar::R },
            "signature r is at or above the curve order",
        ),
        (
            ProtocolError::ScalarAboveCurveOrder { scalar: Scalar::S },
            "signature s is at or above the curve order",
        ),
        (
            ProtocolError::HighS,
            "signature s is above n/2; the low-s form is the only accepted encoding",
        ),
        (
            ProtocolError::Unrecoverable,
            "no signer could be recovered from this signature",
        ),
    ];
    for (error, message) in rendered {
        assert_eq!(error.to_string(), message);
    }

    // The two addresses have to be distinguishable in the message, not just present.
    let expected = Address::repeat_byte(0x11);
    let recovered = Address::repeat_byte(0x22);
    let text = ProtocolError::WrongSigner {
        expected,
        recovered,
    }
    .to_string();
    assert!(text.contains(&expected.to_string()), "{text}");
    assert!(text.contains(&recovered.to_string()), "{text}");
    Ok(())
}

#[test]
fn every_rejection_carries_its_stable_code() -> TestResult {
    // The code, not the wording, is what #67 puts in the vector file and what TicklineTypes.sol
    // declares as a custom error: Solidity errors carry no message, so a English string could
    // never be matched across the three stacks.
    let coded = [
        (ProtocolError::SignatureLength { got: 64 }, "SIG_LENGTH"),
        (ProtocolError::CompactSignature, "SIG_COMPACT"),
        (
            ProtocolError::SignatureRecoveryId { got: 1 },
            "SIG_RECOVERY_ID",
        ),
        (
            ProtocolError::ScalarZero { scalar: Scalar::R },
            "SIG_R_ZERO",
        ),
        (
            ProtocolError::ScalarZero { scalar: Scalar::S },
            "SIG_S_ZERO",
        ),
        (
            ProtocolError::ScalarAboveCurveOrder { scalar: Scalar::R },
            "SIG_R_ABOVE_ORDER",
        ),
        (
            ProtocolError::ScalarAboveCurveOrder { scalar: Scalar::S },
            "SIG_S_ABOVE_ORDER",
        ),
        (ProtocolError::HighS, "SIG_HIGH_S"),
        (ProtocolError::Unrecoverable, "SIG_UNRECOVERABLE"),
        (
            ProtocolError::WrongSigner {
                expected: Address::repeat_byte(0x11),
                recovered: Address::repeat_byte(0x22),
            },
            "SIG_WRONG_SIGNER",
        ),
    ];
    for (error, code) in &coded {
        assert_eq!(error.code(), *code, "{error}");
    }

    // The published list is exactly the codes the variants produce, in the same order, and every
    // one of them is distinct: two rejections sharing a code would be indistinguishable to a
    // client and to the vault.
    let produced: Vec<&str> = coded.iter().map(|(e, _)| e.code()).collect();
    assert_eq!(produced, SIGNATURE_ERROR_CODES.to_vec());
    let mut sorted = produced.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), produced.len(), "codes must be distinct");

    // Scalar is a Rust detail: the two components differ by code, not by a field Solidity would
    // have to encode.
    assert_ne!(
        ProtocolError::ScalarZero { scalar: Scalar::R }.code(),
        ProtocolError::ScalarZero { scalar: Scalar::S }.code()
    );
    Ok(())
}

#[test]
fn the_curve_constants_are_the_secp256k1_ones() -> TestResult {
    // n from SEC 2 section 2.4.1, carried in the fixture file so a typo in the source limbs
    // cannot agree with itself.
    let vectors = load()?;
    let fixture = section(&vectors, "secp256k1")?;
    assert_eq!(CURVE_ORDER, uint(fixture, "n")?);
    assert_eq!(HALF_CURVE_ORDER, uint(fixture, "half_n")?);

    // n is odd, so floor(n/2) is the largest scalar in the lower half.
    let doubled = HALF_CURVE_ORDER
        .checked_mul(U256::from(2u8))
        .ok_or("2 * (n/2)")?;
    assert_eq!(
        doubled.checked_add(U256::from(1u8)).ok_or("n")?,
        CURVE_ORDER
    );
    Ok(())
}
