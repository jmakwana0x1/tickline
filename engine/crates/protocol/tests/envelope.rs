//! The x402 V2 envelopes (issue #66), against `docs/spec-notes.md` §3.
//!
//! **Provenance.** The challenge body and the rejected network spellings come from
//! `testdata/vectors/eip712-primitives.json`, which carries the spec's own example shape. There is
//! nothing to read off a contract here: this is the HTTP surface, so the source is the scheme spec
//! and the committed fixture is what keeps our reading of it honest.

// Shared by the test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use common::{group, load, number, section, text, TestResult};
use protocol::{
    envelope::{
        decode_header, encode_header, Challenge, Network, MAX_HEADER_BYTES,
        PAYMENT_RESPONSE_HEADER, PAYMENT_SIGNATURE_HEADER, SCHEME, X402_VERSION,
    },
    ProtocolError,
};

fn envelope(vectors: &serde_json::Value) -> TestResult<&serde_json::Value> {
    section(vectors, "envelope")
}

fn challenge_json(vectors: &serde_json::Value) -> TestResult<String> {
    Ok(text(section(envelope(vectors)?, "challenge")?, "json")?.to_owned())
}

#[test]
fn header_names_match_the_scheme_spec() -> TestResult {
    // V2 names. V1's X-PAYMENT and X-PAYMENT-RESPONSE are a different protocol version and are
    // not accepted anywhere.
    let vectors = load()?;
    let headers = section(envelope(&vectors)?, "headers")?;
    assert_eq!(PAYMENT_SIGNATURE_HEADER, text(headers, "request")?);
    assert_eq!(PAYMENT_RESPONSE_HEADER, text(headers, "response")?);
    assert!(!PAYMENT_SIGNATURE_HEADER.starts_with("X-"));
    assert_eq!(
        u64::from(X402_VERSION),
        number(envelope(&vectors)?, "x402_version")?
    );
    Ok(())
}

#[test]
fn scheme_is_batch_settlement_not_upto() -> TestResult {
    // `upto` moves value per request; batch settlement is cumulative (§2). Accepting the wrong one
    // would mean charging a client twice for the same authorization.
    let vectors = load()?;
    let body = section(section(envelope(&vectors)?, "challenge")?, "body")?;
    assert_eq!(SCHEME, "batch-settlement");
    assert_eq!(SCHEME, text(body, "scheme")?);
    Ok(())
}

#[test]
fn network_id_is_caip2() -> TestResult {
    let vectors = load()?;
    let networks = section(envelope(&vectors)?, "networks")?;

    let sepolia = text(networks, "base_sepolia")?;
    let parsed = Network::parse(sepolia)?;
    assert_eq!(parsed.chain_id(), 84532);
    assert_eq!(
        parsed.to_string(),
        sepolia,
        "Display round-trips the wire form"
    );

    let mainnet = Network::parse(text(networks, "base_mainnet")?)?;
    assert_eq!(mainnet.chain_id(), 8453);
    assert_ne!(mainnet, parsed);
    Ok(())
}

#[test]
fn a_network_that_is_not_caip2_is_rejected_with_a_typed_error() -> TestResult {
    // `base-sepolia` is the V1 spelling and the most likely mistake, so it is a fixture rather
    // than an afterthought.
    let vectors = load()?;
    let networks = section(envelope(&vectors)?, "networks")?;
    let rejected = group(networks, "rejected")?;
    assert!(rejected.len() >= 7, "the V1 slug and the malformed shapes");

    for case in rejected {
        let spelling = case.as_str().ok_or("rejected case is not a string")?;
        assert_eq!(
            Network::parse(spelling),
            Err(ProtocolError::NetworkNotCaip2 {
                got: spelling.to_owned()
            }),
            "'{spelling}' must be refused"
        );
    }
    Ok(())
}

#[test]
fn challenge_round_trips() -> TestResult {
    let vectors = load()?;
    let json = challenge_json(&vectors)?;

    let challenge: Challenge = serde_json::from_str(&json)?;
    assert_eq!(challenge.scheme, SCHEME);
    assert_eq!(challenge.max_timeout_seconds, 3600);
    assert_eq!(
        serde_json::to_string(&challenge)?,
        json,
        "field order and shape survive"
    );
    Ok(())
}

#[test]
fn challenge_advertises_required_extra_fields() -> TestResult {
    let vectors = load()?;
    let fixture = section(envelope(&vectors)?, "challenge")?;
    let challenge: Challenge = serde_json::from_str(&challenge_json(&vectors)?)?;

    // The four the spec calls required, by name, so dropping one from the struct fails here.
    let required = group(fixture, "required_extra")?;
    assert_eq!(required.len(), 4);
    let body = serde_json::to_value(&challenge.extra)?;
    for field in required {
        let name = field.as_str().ok_or("required field name")?;
        assert!(body.get(name).is_some(), "extra.{name} is required");
    }

    // Each required field must have a value to advertise, not merely a key.
    assert!(!challenge.extra.receiver_authorizer.is_empty());
    assert!(!challenge.extra.name.is_empty());
    assert!(!challenge.extra.version.is_empty());
    Ok(())
}

#[test]
fn challenge_advertises_withdraw_delay_3600() -> TestResult {
    // Tickline's floor, from #8. The escrow would accept 15 minutes; we would not serve it.
    let vectors = load()?;
    let challenge: Challenge = serde_json::from_str(&challenge_json(&vectors)?)?;
    assert_eq!(
        challenge.extra.withdraw_delay,
        protocol::x402::ADVERTISED_WITHDRAW_DELAY
    );
    assert_eq!(challenge.extra.withdraw_delay, 3600);
    Ok(())
}

#[test]
fn the_token_domain_is_not_the_x402_domain() -> TestResult {
    // Two version strings in one object. extra.name and extra.version are the token's EIP-3009
    // domain; the x402 domain is "x402 Batch Settlement" version 1. A deposit signature verifies
    // against the wrong one if they are confused, so the distinction is asserted, not commented.
    let vectors = load()?;
    let challenge: Challenge = serde_json::from_str(&challenge_json(&vectors)?)?;

    assert_eq!(challenge.extra.name, "USDC");
    assert_eq!(challenge.extra.version, "2");
    assert_ne!(challenge.extra.name, protocol::x402::DOMAIN_NAME);
    assert_ne!(challenge.extra.version, protocol::x402::DOMAIN_VERSION);
    Ok(())
}

#[test]
fn payment_header_round_trips() -> TestResult {
    let vectors = load()?;
    let challenge: Challenge = serde_json::from_str(&challenge_json(&vectors)?)?;

    let header = encode_header(&challenge);
    assert!(!header.is_empty());
    assert_eq!(decode_header::<Challenge>(&header)?, challenge);
    Ok(())
}

#[test]
fn non_base64_payload_is_rejected_with_typed_error() -> TestResult {
    for bad in [
        "not base64!",
        "{\"scheme\":\"batch-settlement\"}",
        "====",
        "a",
    ] {
        assert_eq!(
            decode_header::<Challenge>(bad),
            Err(ProtocolError::HeaderNotBase64),
            "'{bad}' is not base64"
        );
    }
    Ok(())
}

#[test]
fn truncated_header_is_rejected_with_typed_error() -> TestResult {
    let vectors = load()?;
    let challenge: Challenge = serde_json::from_str(&challenge_json(&vectors)?)?;
    let header = encode_header(&challenge);

    // Valid base64 of a truncated JSON document: it decodes, and then it is not a challenge.
    let json = challenge_json(&vectors)?;
    let cut = json.get(..json.len() / 2).ok_or("halve the json")?;
    let truncated = encode_header(&cut.to_owned());
    assert!(matches!(
        decode_header::<Challenge>(&truncated),
        Err(ProtocolError::HeaderMalformed { .. })
    ));

    // And the whole header cut in half is not even base64 of anything meaningful.
    let short = header.get(..header.len() / 2).ok_or("halve the header")?;
    assert!(decode_header::<Challenge>(short).is_err());
    Ok(())
}

#[test]
fn oversized_header_is_rejected_before_it_is_parsed() -> TestResult {
    // The size is checked first, so a hostile client cannot make the engine parse a megabyte to
    // find out it is too big. A payment header arrives before any payment is verified.
    let huge = "A".repeat(MAX_HEADER_BYTES + 1);
    assert_eq!(
        decode_header::<Challenge>(&huge),
        Err(ProtocolError::HeaderTooLarge {
            got: MAX_HEADER_BYTES + 1,
            max: MAX_HEADER_BYTES
        })
    );

    // Exactly at the limit is accepted as a size, and then refused on its own merits: the
    // boundary is inclusive, which is what says it sits where we think.
    let at_limit = "A".repeat(MAX_HEADER_BYTES);
    assert!(!matches!(
        decode_header::<Challenge>(&at_limit),
        Err(ProtocolError::HeaderTooLarge { .. })
    ));
    assert_eq!(MAX_HEADER_BYTES, 8192, "8 KiB, written out");
    Ok(())
}
