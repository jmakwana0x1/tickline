//! EIP-712 primitives, against values computed outside this crate (issue #62).
//!
//! **Provenance.** Every expected value comes from `testdata/vectors/eip712-primitives.json`,
//! produced with foundry `cast` 1.8.1 on 2026-09-24, so these assertions are a diff against the
//! EVM's own tooling rather than against this crate's arithmetic. The file records the exact
//! commands. #67 replaces it with the generated cross-stack vectors, which viem writes and forge
//! checks as well.

// Shared by both test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use alloy_primitives::{keccak256, B256};
use common::{address, group, hash, load, number, section, text, TestResult};
use protocol::eip712::{digest, type_hash, Domain, DOMAIN_TYPE};

fn domain_from(fixture: &serde_json::Value) -> TestResult<Domain> {
    Ok(Domain::new(
        number(fixture, "chain_id")?,
        address(fixture, "verifying_contract")?,
    ))
}

#[test]
fn type_hash_matches_committed_string() -> TestResult {
    let vectors = load()?;
    let fixture = section(&vectors, "domain")?;

    // The type string is the definition, not a description of one: change it and every digest in
    // the system changes with it.
    assert_eq!(DOMAIN_TYPE, text(fixture, "type_string")?);
    assert_eq!(type_hash(DOMAIN_TYPE), hash(fixture, "type_hash")?);

    // The name and version are hashed into the separator, so they are pinned here too.
    assert_eq!(protocol::DOMAIN_NAME, text(fixture, "name")?);
    assert_eq!(protocol::DOMAIN_VERSION, text(fixture, "version")?);
    Ok(())
}

#[test]
fn domain_separator_matches_reference() -> TestResult {
    let vectors = load()?;
    let fixture = section(&vectors, "domain")?;
    assert_eq!(
        domain_from(fixture)?.separator(),
        hash(fixture, "separator")?
    );
    Ok(())
}

#[test]
fn domain_separator_changes_with_chain_id() -> TestResult {
    let vectors = load()?;
    let base = domain_from(section(&vectors, "domain")?)?;
    let case = group(&vectors, "domain_variants")?
        .iter()
        .find(|c| text(c, "what_changed").is_ok_and(|w| w == "chain_id"))
        .ok_or("no chain_id variant in the fixtures")?;

    let other = domain_from(case)?;
    assert_ne!(other.chain_id(), base.chain_id());
    assert_eq!(other.separator(), hash(case, "separator")?);
    assert_ne!(
        base.separator(),
        other.separator(),
        "a receipt signed for one chain must not verify on another"
    );
    Ok(())
}

#[test]
fn domain_separator_changes_with_verifying_contract() -> TestResult {
    let vectors = load()?;
    let base = domain_from(section(&vectors, "domain")?)?;
    let case = group(&vectors, "domain_variants")?
        .iter()
        .find(|c| text(c, "what_changed").is_ok_and(|w| w == "verifying_contract"))
        .ok_or("no verifying_contract variant in the fixtures")?;

    let other = domain_from(case)?;
    assert_ne!(other.verifying_contract(), base.verifying_contract());
    assert_eq!(other.separator(), hash(case, "separator")?);
    assert_ne!(
        base.separator(),
        other.separator(),
        "a receipt signed for one vault must not verify against another"
    );
    Ok(())
}

#[test]
fn digest_is_eip191_prefixed_domain_and_struct_hash() -> TestResult {
    let vectors = load()?;
    let domain_fixture = section(&vectors, "domain")?;
    let digest_fixture = section(&vectors, "digest")?;

    let domain = domain_from(domain_fixture)?;
    let separator = hash(domain_fixture, "separator")?;
    let struct_hash = hash(digest_fixture, "struct_hash")?;
    let expected = hash(digest_fixture, "digest")?;

    assert_eq!(domain.digest(struct_hash), expected);
    assert_eq!(
        digest(separator, struct_hash),
        expected,
        "the free function agrees with the method"
    );

    // Spelled out once, so the shape of the preimage is asserted and not only its hash.
    let mut preimage = Vec::with_capacity(66);
    preimage.extend_from_slice(&[0x19, 0x01]);
    preimage.extend_from_slice(separator.as_slice());
    preimage.extend_from_slice(struct_hash.as_slice());
    assert_eq!(keccak256(&preimage), expected);
    Ok(())
}

#[test]
fn digest_changes_with_the_struct_hash() -> TestResult {
    let vectors = load()?;
    let domain = domain_from(section(&vectors, "domain")?)?;
    let struct_hash = hash(section(&vectors, "digest")?, "struct_hash")?;
    assert_ne!(domain.digest(struct_hash), domain.digest(B256::ZERO));
    Ok(())
}

#[test]
fn domain_accessors_return_what_was_built() -> TestResult {
    let vectors = load()?;
    let fixture = section(&vectors, "domain")?;
    let domain = domain_from(fixture)?;
    assert_eq!(domain.chain_id(), number(fixture, "chain_id")?);
    assert_eq!(
        domain.verifying_contract(),
        address(fixture, "verifying_contract")?
    );
    Ok(())
}
