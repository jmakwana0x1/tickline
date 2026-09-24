//! The x402 batch-settlement types, against the deployed escrow (issue #63).
//!
//! **Provenance.** Every expected value in `testdata/vectors/eip712-primitives.json` was read
//! from `0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003` on Base Sepolia on 2026-09-24, through the
//! getter named beside it, or computed and then confirmed against one. Nothing here is checked
//! against our own reading of the spec. #67 moves these into the generated cross-stack vectors
//! and adds a fork suite that re-reads every getter.

// Shared by the test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use alloy_primitives::{keccak256, B256};
use common::{address, group, hash, load, number, section, text, TestResult};
use protocol::{
    eip712::{type_hash, Domain},
    x402::{
        ChannelConfig, ClaimBatch, ClaimEntry, Voucher, ADVERTISED_WITHDRAW_DELAY,
        CHANNEL_CONFIG_TYPE, CLAIM_BATCH_TYPE, CLAIM_ENTRY_TYPE, UINT40_MAX, VOUCHER_TYPE,
    },
    ProtocolError,
};

fn x402_domain(vectors: &serde_json::Value) -> TestResult<Domain> {
    let section = section(vectors, "x402")?;
    Ok(Domain::x402(
        number(section, "chain_id")?,
        address(section, "escrow")?,
    ))
}

fn config_from(case: &serde_json::Value) -> TestResult<ChannelConfig> {
    let config = section(case, "config")?;
    Ok(ChannelConfig::new(
        address(config, "payer")?,
        address(config, "payer_authorizer")?,
        address(config, "receiver")?,
        address(config, "receiver_authorizer")?,
        address(config, "token")?,
        number(config, "withdraw_delay")?,
        hash(config, "salt")?,
    )?)
}

fn amount(case: &serde_json::Value, key: &str) -> TestResult<u128> {
    Ok(text(case, key)?.parse::<u128>()?)
}

fn named<'a>(cases: &'a [serde_json::Value], name: &str) -> TestResult<&'a serde_json::Value> {
    cases
        .iter()
        .find(|c| text(c, "name").is_ok_and(|n| n == name))
        .ok_or_else(|| format!("no '{name}' case in the fixtures").into())
}

#[test]
fn type_hashes_match_the_deployed_contract() -> TestResult {
    // Each of these is a public constant on the escrow, so the fixture is the contract's own
    // opinion of its type hash rather than a second implementation of keccak agreeing with ours.
    let vectors = load()?;
    let ours = [
        ("ChannelConfig", CHANNEL_CONFIG_TYPE),
        ("Voucher", VOUCHER_TYPE),
        ("ClaimEntry", CLAIM_ENTRY_TYPE),
        ("ClaimBatch", CLAIM_BATCH_TYPE),
    ];

    let cases = group(section(&vectors, "x402")?, "type_hashes")?;
    assert_eq!(
        cases.len(),
        ours.len(),
        "one fixture per type this crate defines"
    );

    for (name, type_string) in ours {
        let case = named(cases, name)?;
        assert_eq!(
            type_string,
            text(case, "type_string")?,
            "{name} type string"
        );
        assert_eq!(
            type_hash(type_string),
            hash(case, "hash")?,
            "{name} type hash"
        );
        assert!(
            text(case, "getter")?.ends_with("_TYPEHASH()"),
            "{name} names its getter"
        );
    }
    Ok(())
}

#[test]
fn the_x402_domain_is_the_escrows_own() -> TestResult {
    // From eip712Domain() on the deployment: a different name and version from Tickline's, which
    // is why Domain carries them rather than hardcoding ours.
    let vectors = load()?;
    let fixture = section(section(&vectors, "x402")?, "domain")?;
    let domain = x402_domain(&vectors)?;

    assert_eq!(domain.name(), text(fixture, "name")?);
    assert_eq!(domain.version(), text(fixture, "version")?);
    assert_eq!(domain.separator(), hash(fixture, "separator")?);

    let tickline = Domain::new(domain.chain_id(), domain.verifying_contract());
    assert_ne!(
        tickline.separator(),
        domain.separator(),
        "our domain is not the escrow's"
    );
    Ok(())
}

#[test]
fn channel_id_matches_deployed_contract() -> TestResult {
    let vectors = load()?;
    let domain = x402_domain(&vectors)?;
    let cases = group(section(&vectors, "x402")?, "channels")?;

    for case in cases {
        let config = config_from(case)?;
        assert_eq!(
            config.channel_id(&domain),
            hash(case, "channel_id")?,
            "channel id for {}",
            text(case, "name")?
        );
        if case.get("struct_hash").is_some() {
            assert_eq!(config.struct_hash(), hash(case, "struct_hash")?);
        }
    }

    // Two configs differing only in salt must be two channels: the salt is what lets one agent
    // run concurrent sessions.
    let first = config_from(named(cases, "salt1")?)?;
    let second = config_from(named(cases, "salt2")?)?;
    assert_ne!(first.salt, second.salt);
    assert_ne!(first.channel_id(&domain), second.channel_id(&domain));
    Ok(())
}

#[test]
fn voucher_digest_matches_vector() -> TestResult {
    let vectors = load()?;
    let domain = x402_domain(&vectors)?;
    let case = named(group(section(&vectors, "x402")?, "vouchers")?, "one_usdc")?;

    let voucher = Voucher {
        channel_id: hash(case, "channel_id")?,
        max_claimable_amount: amount(case, "max_claimable_amount")?,
    };
    assert_eq!(voucher.digest(&domain), hash(case, "digest")?);
    Ok(())
}

#[test]
fn a_voucher_is_bound_to_its_channel_and_its_ceiling() -> TestResult {
    let vectors = load()?;
    let domain = x402_domain(&vectors)?;
    let case = named(group(section(&vectors, "x402")?, "vouchers")?, "one_usdc")?;
    let voucher = Voucher {
        channel_id: hash(case, "channel_id")?,
        max_claimable_amount: amount(case, "max_claimable_amount")?,
    };

    let other_channel = Voucher {
        channel_id: B256::ZERO,
        ..voucher
    };
    let other_ceiling = Voucher {
        max_claimable_amount: voucher.max_claimable_amount + 1,
        ..voucher
    };
    assert_ne!(other_channel.digest(&domain), voucher.digest(&domain));
    assert_ne!(other_ceiling.digest(&domain), voucher.digest(&domain));
    Ok(())
}

#[test]
fn claim_entry_digest_matches_vector() -> TestResult {
    let vectors = load()?;
    for case in group(section(&vectors, "x402")?, "claim_entries")? {
        let entry = ClaimEntry {
            channel_id: hash(case, "channel_id")?,
            max_claimable_amount: amount(case, "max_claimable_amount")?,
            total_claimed: amount(case, "total_claimed")?,
        };
        assert_eq!(
            entry.struct_hash(),
            hash(case, "struct_hash")?,
            "entry {}",
            text(case, "name")?
        );
    }
    Ok(())
}

#[test]
fn claim_batch_digest_matches_deployed_contract() -> TestResult {
    // Two entries, not one: a single-element array hides an ordering or concatenation mistake,
    // and the batch is what Phase 5 signs. The contract's getClaimBatchDigest is the oracle.
    let vectors = load()?;
    let domain = x402_domain(&vectors)?;
    let x402 = section(&vectors, "x402")?;
    let entries = group(x402, "claim_entries")?;
    let case = named(group(x402, "claim_batches")?, "two_entries")?;

    let batch = ClaimBatch {
        claims: batch_entries(entries, case)?,
    };
    assert_eq!(batch.entries_root(), hash(case, "entries_root")?);
    assert_eq!(batch.struct_hash(), hash(case, "struct_hash")?);
    assert_eq!(batch.digest(&domain), hash(case, "digest")?);
    Ok(())
}

#[test]
fn claim_batch_of_one_entry_matches_deployed_contract() -> TestResult {
    // Kept only so a failure tells wrong batch framing apart from wrong concatenation.
    let vectors = load()?;
    let domain = x402_domain(&vectors)?;
    let x402 = section(&vectors, "x402")?;
    let case = named(group(x402, "claim_batches")?, "one_entry")?;

    let batch = ClaimBatch {
        claims: batch_entries(group(x402, "claim_entries")?, case)?,
    };
    assert_eq!(batch.digest(&domain), hash(case, "digest")?);
    Ok(())
}

#[test]
fn claim_batch_hashes_the_entry_array_as_eip712_requires() -> TestResult {
    // The array member hash is keccak over the packed concatenation of the element struct hashes,
    // not an abi.encode of the array. The contract settles this; the test states it.
    let vectors = load()?;
    let x402 = section(&vectors, "x402")?;
    let entries = group(x402, "claim_entries")?;
    let case = named(group(x402, "claim_batches")?, "two_entries")?;
    let batch = ClaimBatch {
        claims: batch_entries(entries, case)?,
    };

    let mut packed = Vec::new();
    for entry in &batch.claims {
        packed.extend_from_slice(entry.struct_hash().as_slice());
    }
    assert_eq!(batch.entries_root(), keccak256(&packed));
    Ok(())
}

#[test]
fn claim_batch_order_is_part_of_the_digest() -> TestResult {
    let vectors = load()?;
    let domain = x402_domain(&vectors)?;
    let x402 = section(&vectors, "x402")?;
    let case = named(group(x402, "claim_batches")?, "two_entries")?;
    let batch = ClaimBatch {
        claims: batch_entries(group(x402, "claim_entries")?, case)?,
    };

    let mut reversed = batch.clone();
    reversed.claims.reverse();
    assert_ne!(reversed.digest(&domain), batch.digest(&domain));
    Ok(())
}

#[test]
fn total_claimed_is_cumulative_and_changes_the_entry() -> TestResult {
    // The field is totalClaimed, not a per-batch amount: the whole scheme rests on it only ever
    // rising, which is what makes old vouchers superseded rather than replayable.
    let vectors = load()?;
    let case = named(
        group(section(&vectors, "x402")?, "claim_entries")?,
        "partial",
    )?;
    let entry = ClaimEntry {
        channel_id: hash(case, "channel_id")?,
        max_claimable_amount: amount(case, "max_claimable_amount")?,
        total_claimed: amount(case, "total_claimed")?,
    };

    let advanced = ClaimEntry {
        total_claimed: entry.total_claimed + 1,
        ..entry
    };
    assert_ne!(advanced.struct_hash(), entry.struct_hash());
    assert!(
        entry.total_claimed <= entry.max_claimable_amount,
        "I7: claimed within the ceiling"
    );
    Ok(())
}

#[test]
fn withdraw_delay_at_or_above_uint40_is_rejected() -> TestResult {
    // Malformed as an x402 object: it cannot be the config the escrow hashed, so any stack
    // refuses it. X402_, not POLICY_ (ADR-0012, update of 2026-09-24).
    let vectors = load()?;
    let case = named(group(section(&vectors, "x402")?, "channels")?, "salt1")?;
    let config = section(case, "config")?;

    for delay in [UINT40_MAX + 1, u64::MAX] {
        let built = ChannelConfig::new(
            address(config, "payer")?,
            address(config, "payer_authorizer")?,
            address(config, "receiver")?,
            address(config, "receiver_authorizer")?,
            address(config, "token")?,
            delay,
            hash(config, "salt")?,
        );
        assert_eq!(built, Err(ProtocolError::WithdrawDelayWidth { got: delay }));
        assert_eq!(
            ProtocolError::WithdrawDelayWidth { got: delay }.code(),
            "X402_WITHDRAW_DELAY_WIDTH"
        );
    }
    Ok(())
}

#[test]
fn withdraw_delay_below_the_advertised_minimum_is_rejected() -> TestResult {
    // A valid x402 channel that Tickline declines to serve: the escrow accepts 15 minutes, and
    // the 3600 floor is ours (#8). POLICY_, so a client is not sent to the payment protocol to
    // fix something the payment protocol is happy with.
    let vectors = load()?;
    let case = named(group(section(&vectors, "x402")?, "channels")?, "salt1")?;
    let config = section(case, "config")?;
    assert_eq!(number(config, "withdraw_delay")?, ADVERTISED_WITHDRAW_DELAY);

    for delay in [0, 1, 900, ADVERTISED_WITHDRAW_DELAY - 1] {
        let built = ChannelConfig::new(
            address(config, "payer")?,
            address(config, "payer_authorizer")?,
            address(config, "receiver")?,
            address(config, "receiver_authorizer")?,
            address(config, "token")?,
            delay,
            hash(config, "salt")?,
        );
        assert_eq!(
            built,
            Err(ProtocolError::WithdrawDelayBelowFloor {
                got: delay,
                floor: ADVERTISED_WITHDRAW_DELAY
            })
        );
    }

    assert_eq!(
        ProtocolError::WithdrawDelayBelowFloor {
            got: 900,
            floor: ADVERTISED_WITHDRAW_DELAY
        }
        .code(),
        "POLICY_WITHDRAW_DELAY_BELOW_FLOOR"
    );

    // The floor is accepted, the width is not the same rule: 3600 exactly is a served channel.
    assert!(config_from(case).is_ok());
    Ok(())
}

/// The entries a batch case names, in the order it names them.
fn batch_entries(
    entries: &[serde_json::Value],
    case: &serde_json::Value,
) -> TestResult<Vec<ClaimEntry>> {
    let names = case
        .get("entries")
        .and_then(serde_json::Value::as_array)
        .ok_or("batch case has no entries")?;
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let wanted = name.as_str().ok_or("entry name is not a string")?;
        let entry = named(entries, wanted)?;
        out.push(ClaimEntry {
            channel_id: hash(entry, "channel_id")?,
            max_claimable_amount: amount(entry, "max_claimable_amount")?,
            total_claimed: amount(entry, "total_claimed")?,
        });
    }
    Ok(out)
}
