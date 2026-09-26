//! `PositionReceipt` against values computed outside this crate (issue #64).
//!
//! **Provenance.** Every expected value comes from `testdata/vectors/eip712.json`,
//! produced with foundry `cast` 1.8.1 on 2026-09-24. The Tickline domain separator it uses is the
//! one S1 already pinned, recomputed and found identical, so the receipt vectors and the
//! primitive vectors cannot drift apart.
//!
//! **What these tests do not say.** The widths here are *type* boundaries. The domain boundary is
//! `Q_MAX`, 1e12 shares or 1e18 base units, about twenty orders of magnitude below `u128::MAX`.
//! A receipt at every maximum is well formed and economically impossible, and the engine must
//! never issue one: the share ceiling is Phase 4's and the solvency check is Phase 3's.

// Shared by the test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use alloy_primitives::{Address, B256};
use common::{
    address, group, hash, integer, load, number, section, signature_bytes, text, TestResult,
};
use protocol::{
    eip712::{type_hash, Domain},
    receipt::{PositionReceipt, POSITION_RECEIPT_TYPE},
    ProtocolError, Signature,
};

fn tickline_domain(vectors: &serde_json::Value) -> TestResult<Domain> {
    let domain = section(vectors, "domain")?;
    Ok(Domain::new(
        number(domain, "chain_id")?,
        address(domain, "verifying_contract")?,
    ))
}

fn amount(case: &serde_json::Value, key: &str) -> TestResult<u128> {
    Ok(text(case, key)?.parse::<u128>()?)
}

fn receipt_from(case: &serde_json::Value) -> TestResult<PositionReceipt> {
    Ok(PositionReceipt {
        market_id: hash(case, "market_id")?,
        agent: address(case, "agent")?,
        yes_shares: amount(case, "yes_shares")?,
        no_shares: amount(case, "no_shares")?,
        cost_paid: amount(case, "cost_paid")?,
        fees_paid: amount(case, "fees_paid")?,
        // A decimal string in the fixture: u64::MAX is not representable as a JSON number.
        nonce: integer(case, "nonce")?,
        epoch: u32::try_from(number(case, "epoch")?)?,
    })
}

fn receipts(vectors: &serde_json::Value) -> TestResult<&serde_json::Value> {
    section(vectors, "receipt")
}

#[test]
fn position_receipt_type_hash_matches_committed_string() -> TestResult {
    let vectors = load()?;
    let fixture = receipts(&vectors)?;
    assert_eq!(POSITION_RECEIPT_TYPE, text(fixture, "type_string")?);
    assert_eq!(
        type_hash(POSITION_RECEIPT_TYPE),
        hash(fixture, "type_hash")?
    );

    // The widths are in the type string, so Q5's decision is asserted rather than assumed.
    assert!(POSITION_RECEIPT_TYPE.contains("uint128 yesShares"));
    assert!(POSITION_RECEIPT_TYPE.contains("uint64 nonce"));
    assert!(POSITION_RECEIPT_TYPE.contains("uint32 epoch"));
    assert!(
        !POSITION_RECEIPT_TYPE.contains("uint256"),
        "Q5: the amounts are uint128"
    );
    Ok(())
}

#[test]
fn position_receipt_digest_matches_vector() -> TestResult {
    let vectors = load()?;
    let domain = tickline_domain(&vectors)?;
    let case = section(receipts(&vectors)?, "typical")?;
    let receipt = receipt_from(case)?;

    assert_eq!(receipt.struct_hash(), hash(case, "struct_hash")?);
    assert_eq!(receipt.digest(&domain), hash(case, "digest")?);
    Ok(())
}

#[test]
fn the_typical_receipt_carries_the_fee_model() -> TestResult {
    // 100 bps of cost (Qa on #1), so the fee model is visible in the data and not only in prose.
    let vectors = load()?;
    let case = section(receipts(&vectors)?, "typical")?;
    let receipt = receipt_from(case)?;
    assert_eq!(receipt.fees_paid * 100, receipt.cost_paid);
    Ok(())
}

#[test]
fn receipt_amount_fields_are_u128() -> TestResult {
    // An ENCODING test. u128::MAX shares is well formed and economically impossible: Q_MAX caps
    // shares at 1e18 base units (Phase 4) and the vault's solvency check caps payouts (Phase 3).
    // This crate says what a receipt may encode, not what the engine may issue.
    let vectors = load()?;
    let fixture = receipts(&vectors)?;
    let widths = section(fixture, "widths")?;
    let domain = tickline_domain(&vectors)?;

    // Written out in the fixture, never recomputed here (CLAUDE.md §5).
    assert_eq!(amount(widths, "u128_max")?, u128::MAX);
    assert_eq!(
        text(widths, "u128_max")?,
        "340282366920938463463374607431768211455"
    );

    let case = section(fixture, "at_every_type_maximum")?;
    let at_max = receipt_from(case)?;
    for field in [
        at_max.yes_shares,
        at_max.no_shares,
        at_max.cost_paid,
        at_max.fees_paid,
    ] {
        assert_eq!(field, u128::MAX);
    }

    // The largest valid value is accepted, which is what says the boundary sits here rather than
    // one below it.
    assert_eq!(at_max.struct_hash(), hash(case, "struct_hash")?);
    assert_eq!(at_max.digest(&domain), hash(case, "digest")?);
    Ok(())
}

#[test]
fn receipt_nonce_is_u64() -> TestResult {
    let vectors = load()?;
    let widths = section(receipts(&vectors)?, "widths")?;
    assert_eq!(text(widths, "u64_max")?, "18446744073709551615");
    assert_eq!(text(widths, "u64_max")?.parse::<u64>()?, u64::MAX);

    let at_max = receipt_from(section(receipts(&vectors)?, "at_every_type_maximum")?)?;
    assert_eq!(at_max.nonce, u64::MAX);
    Ok(())
}

#[test]
fn receipt_epoch_is_u32() -> TestResult {
    let vectors = load()?;
    let widths = section(receipts(&vectors)?, "widths")?;
    assert_eq!(text(widths, "u32_max")?, "4294967295");
    assert_eq!(text(widths, "u32_max")?.parse::<u32>()?, u32::MAX);

    let at_max = receipt_from(section(receipts(&vectors)?, "at_every_type_maximum")?)?;
    assert_eq!(at_max.epoch, u32::MAX);
    Ok(())
}

#[test]
fn receipt_digest_changes_when_any_field_changes() -> TestResult {
    let vectors = load()?;
    let domain = tickline_domain(&vectors)?;
    let base = receipt_from(section(receipts(&vectors)?, "typical")?)?;
    let original = base.digest(&domain);

    let variants = [
        (
            "marketId",
            PositionReceipt {
                market_id: B256::ZERO,
                ..base
            },
        ),
        (
            "agent",
            PositionReceipt {
                agent: Address::ZERO,
                ..base
            },
        ),
        (
            "yesShares",
            PositionReceipt {
                yes_shares: base.yes_shares + 1,
                ..base
            },
        ),
        (
            "noShares",
            PositionReceipt {
                no_shares: base.no_shares + 1,
                ..base
            },
        ),
        (
            "costPaid",
            PositionReceipt {
                cost_paid: base.cost_paid + 1,
                ..base
            },
        ),
        (
            "feesPaid",
            PositionReceipt {
                fees_paid: base.fees_paid + 1,
                ..base
            },
        ),
        (
            "nonce",
            PositionReceipt {
                nonce: base.nonce + 1,
                ..base
            },
        ),
        (
            "epoch",
            PositionReceipt {
                epoch: base.epoch + 1,
                ..base
            },
        ),
    ];
    assert_eq!(variants.len(), 8, "one case per field");

    let mut seen = vec![original];
    for (field, variant) in variants {
        let digest = variant.digest(&domain);
        assert_ne!(digest, original, "changing {field} must change the digest");
        assert!(
            !seen.contains(&digest),
            "{field} collides with an earlier variant"
        );
        seen.push(digest);
    }
    Ok(())
}

#[test]
fn receipt_signed_by_a_different_key_recovers_a_different_signer() -> TestResult {
    let vectors = load()?;
    let domain = tickline_domain(&vectors)?;
    let fixture = receipts(&vectors)?;
    let case = section(fixture, "typical")?;

    let receipt = receipt_from(case)?;
    let signature = Signature::from_bytes(&signature_bytes(case, "signature")?)?;
    let operator = address(fixture, "operator")?;

    assert_eq!(receipt.recover(&domain, &signature)?, operator);
    receipt.verify(&domain, &signature, operator)?;

    // Anvil account 1 signed nothing here, so it must not be accepted as the operator.
    let other = address(&vectors, "other_signer")?;
    assert_ne!(other, operator);
    assert_eq!(
        receipt.verify(&domain, &signature, other),
        Err(ProtocolError::WrongSigner {
            expected: other,
            recovered: operator
        })
    );
    Ok(())
}

#[test]
fn a_receipt_signature_recovers_the_operator_only_over_its_own_digest() -> TestResult {
    // The guarantee is about *who* it recovers to. A signature checked against the wrong digest
    // still recovers, to a stranger, which is why "the recovered address is not zero" is never
    // the check (ADR-0012). Each stranger is committed, so a wrong digest has to reproduce a
    // named address by luck rather than merely differ.
    //
    // A stranger is a function of the digest and the signature together: regenerate either and
    // all of them move, so one changing in a diff means an unintended encoding change.
    let vectors = load()?;
    let domain = tickline_domain(&vectors)?;
    let fixture = receipts(&vectors)?;
    let typical = section(fixture, "typical")?;

    let receipt = receipt_from(typical)?;
    let signature = Signature::from_bytes(&signature_bytes(typical, "signature")?)?;
    let operator = address(fixture, "operator")?;
    assert_eq!(receipt.recover(&domain, &signature)?, operator);

    let base = receipt;
    for case in group(section(fixture, "tampered")?, "cases")? {
        let changed = text(case, "changed")?;
        let tampered = match changed {
            "epoch" => PositionReceipt {
                epoch: u32::try_from(number(case, "to")?)?,
                ..base
            },
            "nonce" => PositionReceipt {
                nonce: number(case, "to")?,
                ..base
            },
            "every field" => receipt_from(section(fixture, "at_every_type_maximum")?)?,
            other => return Err(format!("unknown tampered case '{other}'").into()),
        };

        assert_eq!(
            tampered.digest(&domain),
            hash(case, "digest")?,
            "{changed} digest"
        );

        let stranger = address(case, "recovers_to")?;
        assert_ne!(
            stranger, operator,
            "{changed} must not recover the operator"
        );
        assert_eq!(
            tampered.recover(&domain, &signature)?,
            stranger,
            "{changed} stranger"
        );
        assert_eq!(
            tampered.verify(&domain, &signature, operator),
            Err(ProtocolError::WrongSigner {
                expected: operator,
                recovered: stranger
            }),
            "{changed} must fail as a wrong signer, not as unrecoverable"
        );
    }
    Ok(())
}
