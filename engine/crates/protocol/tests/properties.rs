//! The property suite (issue #68). Every guarantee this crate makes, over generated inputs.
//!
//! `PROPTEST_CASES` sets the number of cases, so `just deep` raises it without touching the code.
//!
//! **The crate verifies and never signs** (ADR-0011), so the suite brings its own signer: `k256` is
//! a dev-dependency here purely to produce signatures worth verifying. Keys come from a generated
//! 32-byte seed rather than from randomness, so a counterexample names the key that produced it.
//!
//! Two properties are shaped by what the type system already guarantees. A `Signature` value cannot
//! be high-s or out of range, because `from_bytes` is the only constructor, so the properties about
//! rejection are stated over **bytes** rather than over values. And `round_trips` is about the wire
//! forms, since a struct hash is not invertible.

// Shared by the test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use alloy_primitives::{keccak256, Address, B256, U256};
use k256::ecdsa::{RecoveryId, SigningKey};
use proptest::prelude::*;
use protocol::{
    eip712::Domain,
    envelope::{decode_header, encode_header, Network, PaymentPayload, PaymentRequired},
    market_id::{pyth_threshold_params_hash, Direction, MarketParams},
    receipt::PositionReceipt,
    signature::{CURVE_ORDER, HALF_CURVE_ORDER},
    ProtocolError, Signature,
};

/// A signing key from a seed, so a counterexample is reproducible from the seed alone.
fn signer(seed: [u8; 32]) -> Option<SigningKey> {
    SigningKey::from_bytes(&seed.into()).ok()
}

/// The address a key signs as.
fn address_of(key: &SigningKey) -> Address {
    let point = key.verifying_key().to_encoded_point(false);
    let body = point.as_bytes().get(1..).unwrap_or_default();
    Address::from_slice(keccak256(body).as_slice().get(12..).unwrap_or_default())
}

/// Sign a digest the way an operator would, and encode it as the 65 bytes this crate accepts.
fn sign(key: &SigningKey, digest: B256) -> Option<[u8; 65]> {
    let (signature, recovery): (k256::ecdsa::Signature, RecoveryId) =
        key.sign_prehash_recoverable(digest.as_slice()).ok()?;
    let mut out = Vec::with_capacity(65);
    out.extend_from_slice(&signature.r().to_bytes());
    out.extend_from_slice(&signature.s().to_bytes());
    out.push(27u8.checked_add(recovery.to_byte())?);
    let mut fixed = [0u8; 65];
    fixed.copy_from_slice(&out);
    Some(fixed)
}

fn any_seed() -> impl Strategy<Value = [u8; 32]> {
    // Never all zeros: that is not a valid scalar, so it is not a key.
    prop::array::uniform32(1u8..=255)
}

fn any_b256() -> impl Strategy<Value = B256> {
    prop::array::uniform32(any::<u8>()).prop_map(B256::new)
}

fn any_address() -> impl Strategy<Value = Address> {
    prop::array::uniform20(any::<u8>()).prop_map(Address::new)
}

prop_compose! {
    fn any_receipt()(
        market_id in any_b256(),
        agent in any_address(),
        yes_shares in any::<u128>(),
        no_shares in any::<u128>(),
        cost_paid in any::<u128>(),
        fees_paid in any::<u128>(),
        nonce in any::<u64>(),
        epoch in any::<u32>(),
    ) -> PositionReceipt {
        PositionReceipt {
            market_id, agent, yes_shares, no_shares, cost_paid, fees_paid, nonce, epoch,
        }
    }
}

prop_compose! {
    fn any_market()(
        creator in any_address(),
        template_id in any_b256(),
        template_params_hash in any_b256(),
        deadline in any::<u64>(),
        b in any::<u128>(),
        epoch_length in any::<u32>(),
        salt in any_b256(),
    ) -> MarketParams {
        MarketParams {
            creator, template_id, template_params_hash, deadline, b, epoch_length, salt,
        }
    }
}

proptest! {
    /// A signature survives the trip through bytes and back, and the wire envelopes survive JSON.
    #[test]
    fn prop_encode_decode_round_trips(seed in any_seed(), digest in any_b256()) {
        let Some(key) = signer(seed) else { return Ok(()) };
        let Some(bytes) = sign(&key, digest) else { return Ok(()) };

        let signature = Signature::from_bytes(&bytes)?;
        prop_assert_eq!(signature.to_bytes(), bytes);
        prop_assert_eq!(Signature::from_bytes(&signature.to_bytes())?, signature);
    }

    /// The signer of a digest is the key that signed it, and nobody else.
    #[test]
    fn prop_recovered_signer_matches_the_signing_key(
        seed in any_seed(),
        other in any_seed(),
        digest in any_b256(),
    ) {
        let Some(key) = signer(seed) else { return Ok(()) };
        let Some(bytes) = sign(&key, digest) else { return Ok(()) };
        let signature = Signature::from_bytes(&bytes)?;

        let expected = address_of(&key);
        prop_assert_eq!(signature.recover(digest)?, expected);
        signature.verify(digest, expected)?;

        // Any other key is a wrong signer, not an error of some other kind.
        if let Some(stranger) = signer(other) {
            let stranger = address_of(&stranger);
            if stranger != expected {
                prop_assert_eq!(
                    signature.verify(digest, stranger),
                    Err(ProtocolError::WrongSigner { expected: stranger, recovered: expected })
                );
            }
        }
    }

    /// Every high-s twin is refused, whatever the signature was.
    #[test]
    fn prop_high_s_is_always_rejected(seed in any_seed(), digest in any_b256()) {
        let Some(key) = signer(seed) else { return Ok(()) };
        let Some(bytes) = sign(&key, digest) else { return Ok(()) };
        let signature = Signature::from_bytes(&bytes)?;

        // k256 signs low-s, so the twin is n - s and the parity flips with it.
        let s = U256::from_be_bytes(signature.s().0);
        prop_assume!(s != U256::ZERO);
        let high = CURVE_ORDER.checked_sub(s).ok_or(TestCaseError::reject("n - s"))?;
        prop_assert!(high > HALF_CURVE_ORDER);

        let mut twin = Vec::with_capacity(65);
        twin.extend_from_slice(signature.r().as_slice());
        twin.extend_from_slice(&high.to_be_bytes::<32>());
        twin.push(if signature.v() == 27 { 28 } else { 27 });
        prop_assert_eq!(Signature::from_bytes(&twin), Err(ProtocolError::HighS));
    }

    /// A receipt's digest changes when any field changes.
    #[test]
    fn prop_any_single_field_change_changes_the_digest(
        receipt in any_receipt(),
        chain_id in 1u64..=u64::MAX,
        vault in any_address(),
        bump in 1u128..=u128::MAX,
    ) {
        let domain = Domain::new(chain_id, vault);
        let original = receipt.digest(&domain);

        let variants = [
            PositionReceipt { market_id: keccak256(receipt.market_id), ..receipt },
            PositionReceipt { agent: Address::new([1u8; 20]), ..receipt },
            PositionReceipt { yes_shares: receipt.yes_shares.wrapping_add(bump), ..receipt },
            PositionReceipt { no_shares: receipt.no_shares.wrapping_add(bump), ..receipt },
            PositionReceipt { cost_paid: receipt.cost_paid.wrapping_add(bump), ..receipt },
            PositionReceipt { fees_paid: receipt.fees_paid.wrapping_add(bump), ..receipt },
            PositionReceipt { nonce: receipt.nonce.wrapping_add(1), ..receipt },
            PositionReceipt { epoch: receipt.epoch.wrapping_add(1), ..receipt },
        ];
        for variant in variants {
            if variant != receipt {
                prop_assert_ne!(variant.digest(&domain), original);
            }
        }
    }

    /// A signature is bound to its domain: the same receipt elsewhere is a different digest, and the
    /// signature over it recovers somebody else.
    #[test]
    fn prop_signature_verifies_only_for_its_own_domain(
        seed in any_seed(),
        receipt in any_receipt(),
        chain_id in 1u64..=1_000_000u64,
        other_chain in 1u64..=1_000_000u64,
        vault in any_address(),
        other_vault in any_address(),
    ) {
        let Some(key) = signer(seed) else { return Ok(()) };
        let operator = address_of(&key);
        let domain = Domain::new(chain_id, vault);
        let Some(bytes) = sign(&key, receipt.digest(&domain)) else { return Ok(()) };
        let signature = Signature::from_bytes(&bytes)?;
        receipt.verify(&domain, &signature, operator)?;

        for elsewhere in [Domain::new(other_chain, vault), Domain::new(chain_id, other_vault)] {
            if elsewhere.digest(receipt.struct_hash()) != domain.digest(receipt.struct_hash()) {
                prop_assert_ne!(receipt.digest(&elsewhere), receipt.digest(&domain));
                // It recovers a stranger rather than failing, which is why the check names who must
                // have signed (ADR-0012).
                prop_assert!(receipt.verify(&elsewhere, &signature, operator).is_err());
            }
        }
    }

    /// The market id is injective over its preimage: two different preimages, two different ids.
    #[test]
    fn prop_market_id_is_injective_over_its_preimage(
        first in any_market(),
        second in any_market(),
        chain_id in 1u64..=1_000_000u64,
        other_chain in 1u64..=1_000_000u64,
        vault in any_address(),
        other_vault in any_address(),
    ) {
        let id = first.market_id(chain_id, vault);
        prop_assert_eq!(id, first.market_id(chain_id, vault), "and deterministic");

        if first != second {
            prop_assert_ne!(second.market_id(chain_id, vault), id);
        }
        if other_chain != chain_id {
            prop_assert_ne!(first.market_id(other_chain, vault), id);
        }
        if other_vault != vault {
            prop_assert_ne!(first.market_id(chain_id, other_vault), id);
        }
    }

    /// The template parameters hash is injective over its three inputs.
    #[test]
    fn prop_template_params_hash_is_injective(
        price_id in any_b256(),
        other_price in any_b256(),
        threshold in any::<i64>(),
        step in 1i64..=i64::MAX,
    ) {
        let base = pyth_threshold_params_hash(price_id, threshold, Direction::Above);
        prop_assert_ne!(pyth_threshold_params_hash(price_id, threshold, Direction::Below), base);
        if other_price != price_id {
            prop_assert_ne!(pyth_threshold_params_hash(other_price, threshold, Direction::Above), base);
        }
        let moved = threshold.wrapping_add(step);
        if moved != threshold {
            prop_assert_ne!(pyth_threshold_params_hash(price_id, moved, Direction::Above), base);
        }
    }

    /// The header codec never panics, whatever arrives. This is the engine's most hostile input: it
    /// is read before any payment has been verified, so a panic here is a denial of service.
    #[test]
    fn prop_decode_header_never_panics(raw in ".{0,4096}") {
        let _ = decode_header::<PaymentRequired>(&raw);
        let _ = decode_header::<PaymentPayload>(&raw);
        let _ = Network::parse(&raw);
    }

    /// Anything this crate encodes as a header decodes back to itself.
    #[test]
    fn prop_header_round_trips_for_any_network(chain_id in 1u64..=u64::MAX) {
        let network = Network::parse(&format!("eip155:{chain_id}"))?;
        prop_assert_eq!(network.chain_id(), chain_id);
        prop_assert_eq!(Network::parse(&network.to_string())?, network);

        let header = encode_header(&network.to_string());
        prop_assert_eq!(decode_header::<String>(&header)?, network.to_string());
    }

    /// Bytes that are not 65 long are refused by length, whatever they contain.
    #[test]
    fn prop_only_65_bytes_is_a_signature(bytes in prop::collection::vec(any::<u8>(), 0..200)) {
        match bytes.len() {
            64 => prop_assert_eq!(Signature::from_bytes(&bytes), Err(ProtocolError::CompactSignature)),
            65 => { let _ = Signature::from_bytes(&bytes); }
            other => prop_assert_eq!(
                Signature::from_bytes(&bytes),
                Err(ProtocolError::SignatureLength { got: other })
            ),
        }
    }
}
