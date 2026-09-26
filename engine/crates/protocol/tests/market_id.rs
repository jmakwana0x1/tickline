//! `MarketId`, against values computed outside this crate (issue #65, ADR-0013).
//!
//! **Provenance.** `testdata/vectors/eip712-primitives.json`, computed with foundry `cast` 1.8.1 on
//! 2026-09-26. The two domain variants are the reason D3 exists, so they are committed values and
//! not merely inequalities: identical market parameters under a different chain id or a different
//! vault must produce the ids recorded there.
//!
//! **What these tests do not say.** Whether `b` is inside the LMSR's domain, whether the deadline
//! is in the future, and whether a market is one we would create are Phase 4's questions, asked at
//! creation. This crate says what a market id encodes.

// Shared by the test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use alloy_primitives::{Address, B256};
use common::{address, group, hash, load, number, section, text, TestResult};
use protocol::market_id::{
    pyth_threshold_params_hash, template_id, Direction, MarketParams, MARKET_ID_TAG,
    PYTH_THRESHOLD_TEMPLATE,
};

fn market_section(vectors: &serde_json::Value) -> TestResult<&serde_json::Value> {
    section(vectors, "market_id")
}

fn params_from(case: &serde_json::Value) -> TestResult<MarketParams> {
    Ok(MarketParams {
        creator: address(case, "creator")?,
        template_id: hash(case, "template_id")?,
        template_params_hash: hash(case, "template_params_hash")?,
        deadline: number(case, "deadline")?,
        b: text(case, "b")?.parse::<u128>()?,
        epoch_length: u32::try_from(number(case, "epoch_length")?)?,
        salt: hash(case, "salt")?,
    })
}

#[test]
fn market_id_tag_matches_committed_string() -> TestResult {
    let vectors = load()?;
    let fixture = section(market_section(&vectors)?, "tag")?;

    assert_eq!(MARKET_ID_TAG, hash(fixture, "value")?);

    // The tag is the string as a bytes32 literal, right-padded, so a preimage dump reads as text.
    let text_form = text(fixture, "as_string")?;
    assert_eq!(text_form, "Tickline MarketId v1");
    assert_eq!(
        text_form.len(),
        20,
        "20 bytes, so nothing is truncated into 32"
    );
    let bytes = MARKET_ID_TAG.as_slice();
    assert_eq!(bytes.get(..text_form.len()), Some(text_form.as_bytes()));
    assert!(
        bytes
            .get(text_form.len()..)
            .is_some_and(|rest| rest.iter().all(|b| *b == 0)),
        "right-padded with zeros"
    );
    Ok(())
}

#[test]
fn market_id_matches_vector() -> TestResult {
    let vectors = load()?;
    let case = section(market_section(&vectors)?, "base")?;
    let params = params_from(case)?;

    assert_eq!(
        params.market_id(number(case, "chain_id")?, address(case, "vault")?),
        hash(case, "market_id")?
    );
    Ok(())
}

#[test]
fn market_id_changes_with_chain_id() -> TestResult {
    let vectors = load()?;
    let base = section(market_section(&vectors)?, "base")?;
    let params = params_from(base)?;
    let vault = address(base, "vault")?;
    let mine = params.market_id(number(base, "chain_id")?, vault);

    let case = named_variant(&vectors, "chain_id")?;
    let elsewhere = params.market_id(number(case, "chain_id")?, address(case, "vault")?);

    assert_eq!(elsewhere, hash(case, "market_id")?);
    assert_ne!(
        elsewhere, mine,
        "the same market on another chain is another market"
    );
    Ok(())
}

#[test]
fn market_id_changes_with_vault() -> TestResult {
    let vectors = load()?;
    let base = section(market_section(&vectors)?, "base")?;
    let params = params_from(base)?;
    let mine = params.market_id(number(base, "chain_id")?, address(base, "vault")?);

    let case = named_variant(&vectors, "vault")?;
    let elsewhere = params.market_id(number(case, "chain_id")?, address(case, "vault")?);

    assert_eq!(elsewhere, hash(case, "market_id")?);
    assert_ne!(
        elsewhere, mine,
        "a redeployed vault must not inherit the old market ids"
    );
    Ok(())
}

#[test]
fn market_id_changes_when_any_field_changes() -> TestResult {
    let vectors = load()?;
    let base_case = section(market_section(&vectors)?, "base")?;
    let base = params_from(base_case)?;
    let chain_id = number(base_case, "chain_id")?;
    let vault = address(base_case, "vault")?;
    let original = base.market_id(chain_id, vault);

    // Ten fields: the eight in MarketParams plus the chain and the vault the id is bound to.
    let variants: [(&str, B256); 10] = [
        (
            "creator",
            MarketParams {
                creator: Address::ZERO,
                ..base
            }
            .market_id(chain_id, vault),
        ),
        (
            "templateId",
            MarketParams {
                template_id: B256::ZERO,
                ..base
            }
            .market_id(chain_id, vault),
        ),
        (
            "templateParamsHash",
            MarketParams {
                template_params_hash: B256::ZERO,
                ..base
            }
            .market_id(chain_id, vault),
        ),
        (
            "deadline",
            MarketParams {
                deadline: base.deadline + 1,
                ..base
            }
            .market_id(chain_id, vault),
        ),
        (
            "b",
            MarketParams {
                b: base.b + 1,
                ..base
            }
            .market_id(chain_id, vault),
        ),
        (
            "epochLength",
            MarketParams {
                epoch_length: base.epoch_length + 1,
                ..base
            }
            .market_id(chain_id, vault),
        ),
        (
            "salt",
            MarketParams {
                salt: B256::ZERO,
                ..base
            }
            .market_id(chain_id, vault),
        ),
        ("chainId", base.market_id(chain_id + 1, vault)),
        ("vault", base.market_id(chain_id, Address::ZERO)),
        ("nothing", original),
    ];

    let mut seen = Vec::new();
    for (field, id) in variants {
        if field == "nothing" {
            assert_eq!(id, original);
            continue;
        }
        assert_ne!(id, original, "changing {field} must change the id");
        assert!(
            !seen.contains(&id),
            "{field} collides with an earlier variant"
        );
        seen.push(id);
    }
    assert_eq!(seen.len(), 9, "nine distinct single-field changes");
    Ok(())
}

#[test]
fn template_params_hash_for_the_pyth_threshold_template() -> TestResult {
    let vectors = load()?;
    let fixture = section(market_section(&vectors)?, "template")?;

    assert_eq!(PYTH_THRESHOLD_TEMPLATE, text(fixture, "name")?);
    assert_eq!(
        template_id(PYTH_THRESHOLD_TEMPLATE),
        hash(fixture, "template_id")?
    );

    let price_id = hash(fixture, "price_id")?;
    let threshold = text(fixture, "threshold")?.parse::<i64>()?;
    assert_eq!(
        pyth_threshold_params_hash(price_id, threshold, Direction::Above),
        hash(fixture, "template_params_hash")?
    );

    // Resolution is machine-only (ADR-0005), so every parameter the rule reads is in the hash:
    // change the feed, the threshold, or the side, and it is a different market.
    let base = pyth_threshold_params_hash(price_id, threshold, Direction::Above);
    assert_ne!(
        pyth_threshold_params_hash(B256::ZERO, threshold, Direction::Above),
        base
    );
    assert_ne!(
        pyth_threshold_params_hash(price_id, threshold + 1, Direction::Above),
        base
    );
    assert_ne!(
        pyth_threshold_params_hash(price_id, threshold, Direction::Below),
        base
    );
    Ok(())
}

#[test]
fn a_negative_threshold_is_encoded_as_a_signed_int64() -> TestResult {
    // Pyth prices are signed, and a threshold below zero is legitimate for some feeds. int64 is
    // sign-extended to 32 bytes by abi.encode, so -1 must not hash like u64::MAX.
    let vectors = load()?;
    let price_id = hash(section(market_section(&vectors)?, "template")?, "price_id")?;

    let negative = pyth_threshold_params_hash(price_id, -1, Direction::Above);
    let positive = pyth_threshold_params_hash(price_id, 1, Direction::Above);
    assert_ne!(negative, positive);
    assert_ne!(
        negative,
        pyth_threshold_params_hash(price_id, 0, Direction::Above)
    );
    Ok(())
}

/// The domain variant a case names.
fn named_variant<'a>(
    vectors: &'a serde_json::Value,
    what: &str,
) -> TestResult<&'a serde_json::Value> {
    group(market_section(vectors)?, "domain_variants")?
        .iter()
        .find(|c| text(c, "what_changed").is_ok_and(|w| w == what))
        .ok_or_else(|| format!("no '{what}' domain variant").into())
}
