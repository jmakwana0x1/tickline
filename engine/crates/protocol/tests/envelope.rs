//! The x402 V2 envelopes (issue #66), against the shapes quoted in `docs/spec-notes.md` §3.
//!
//! **Provenance.** `testdata/vectors/eip712-primitives.json` carries the spec's own shapes, taken
//! from `scheme_batch_settlement_evm.md` rather than from a summary of it. There is nothing to read
//! off a contract here: this is the HTTP surface, so the spec is the source and the committed
//! fixture is what keeps our reading of it honest.

// Shared by the test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use common::{group, load, number, section, text, TestResult};
use protocol::{
    envelope::{
        decode_header, encode_header, Network, PayloadKind, PaymentPayload, PaymentRequired,
        CUMULATIVE_AMOUNT_MISMATCH, MAX_HEADER_BYTES, PAYMENT_RESPONSE_HEADER,
        PAYMENT_SIGNATURE_HEADER, SCHEME, X402_VERSION,
    },
    ProtocolError,
};

fn envelope(vectors: &serde_json::Value) -> TestResult<&serde_json::Value> {
    section(vectors, "envelope")
}

fn json_of(vectors: &serde_json::Value, path: &[&str]) -> TestResult<String> {
    let mut node = envelope(vectors)?;
    for step in path {
        node = section(node, step)?;
    }
    Ok(text(node, "json")?.to_owned())
}

fn plain(vectors: &serde_json::Value) -> TestResult<PaymentRequired> {
    Ok(serde_json::from_str(&json_of(
        vectors,
        &["payment_required", "plain"],
    )?)?)
}

fn voucher_payload(vectors: &serde_json::Value) -> TestResult<PaymentPayload> {
    Ok(serde_json::from_str(&json_of(
        vectors,
        &["payment_payload", "voucher"],
    )?)?)
}

#[test]
fn header_names_match_the_scheme_spec() -> TestResult {
    let vectors = load()?;
    let headers = section(envelope(&vectors)?, "headers")?;
    assert_eq!(PAYMENT_SIGNATURE_HEADER, text(headers, "request")?);
    assert_eq!(PAYMENT_RESPONSE_HEADER, text(headers, "response")?);
    assert!(
        !PAYMENT_SIGNATURE_HEADER.starts_with("X-"),
        "V1 header names are a different version"
    );
    assert_eq!(
        u64::from(X402_VERSION),
        number(envelope(&vectors)?, "x402_version")?
    );
    Ok(())
}

#[test]
fn scheme_is_batch_settlement_not_upto() -> TestResult {
    // `upto` moves value per request; batch settlement is cumulative. Accepting the wrong one would
    // mean charging a client twice for one authorization.
    let vectors = load()?;
    let body = plain(&vectors)?;
    assert_eq!(SCHEME, "batch-settlement");
    assert_eq!(body.accepts.first().ok_or("one entry")?.scheme, SCHEME);
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
    let vectors = load()?;
    let rejected = group(section(envelope(&vectors)?, "networks")?, "rejected")?;
    assert!(rejected.len() >= 7);
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
fn payment_required_round_trips() -> TestResult {
    let vectors = load()?;
    let json = json_of(&vectors, &["payment_required", "plain"])?;
    let body = plain(&vectors)?;

    assert_eq!(body.x402_version, X402_VERSION);
    assert_eq!(
        body.accepts.len(),
        1,
        "a one-entry accepts is still an array"
    );
    assert!(body.error.is_none(), "a plain 402 carries no error");
    assert_eq!(
        serde_json::to_string(&body)?,
        json,
        "field order and shape survive"
    );
    Ok(())
}

#[test]
fn the_body_is_accepts_not_one_requirement() -> TestResult {
    // The official TypeScript client reads `accepts`. Modelling the entry as the body was the
    // mistake Jay caught on #66, so this test is the one that would have caught it.
    let vectors = load()?;
    let raw: serde_json::Value =
        serde_json::from_str(&json_of(&vectors, &["payment_required", "plain"])?)?;
    assert!(raw
        .get("accepts")
        .and_then(serde_json::Value::as_array)
        .is_some());
    assert!(
        raw.get("scheme").is_none(),
        "the body has no scheme of its own"
    );
    assert!(
        raw.get("extra").is_none(),
        "the body has no extra of its own"
    );
    Ok(())
}

#[test]
fn a_corrective_402_is_the_same_body_with_both_states() -> TestResult {
    let vectors = load()?;
    let json = json_of(&vectors, &["payment_required", "corrective"])?;
    let body: PaymentRequired = serde_json::from_str(&json)?;

    assert_eq!(body.error.as_deref(), Some(CUMULATIVE_AMOUNT_MISMATCH));
    assert_eq!(
        CUMULATIVE_AMOUNT_MISMATCH,
        "invalid_batch_settlement_evm_cumulative_amount_mismatch"
    );

    let entry = body.accepts.first().ok_or("one entry")?;
    let state = entry.extra.channel_state.as_ref().ok_or("channelState")?;
    let voucher = entry.extra.voucher_state.as_ref().ok_or("voucherState")?;
    assert_eq!(state.charged_cumulative_amount, "3200");
    assert_eq!(
        voucher.signed_max_claimable,
        state.charged_cumulative_amount
    );
    assert!(
        !voucher.signature.is_empty(),
        "the client verifies its own prior signature"
    );

    // A plain 402 carries neither, which is what makes the corrective path a body shape rather
    // than a special case Phase 4 has to remember.
    let plain_entry = plain(&vectors)?;
    let plain_extra = &plain_entry.accepts.first().ok_or("one entry")?.extra;
    assert!(plain_extra.channel_state.is_none());
    assert!(plain_extra.voucher_state.is_none());

    assert_eq!(serde_json::to_string(&body)?, json);
    Ok(())
}

#[test]
fn challenge_advertises_required_extra_fields() -> TestResult {
    let vectors = load()?;
    let fixture = section(envelope(&vectors)?, "payment_required")?;
    let entry = plain(&vectors)?
        .accepts
        .into_iter()
        .next()
        .ok_or("one entry")?;

    let required = group(fixture, "required_extra")?;
    assert_eq!(required.len(), 4);
    let serialized = serde_json::to_value(&entry.extra)?;
    for field in required {
        let name = field.as_str().ok_or("required field name")?;
        assert!(serialized.get(name).is_some(), "extra.{name} is required");
    }
    assert!(!entry.extra.receiver_authorizer.is_empty());
    Ok(())
}

#[test]
fn challenge_advertises_withdraw_delay_3600() -> TestResult {
    let vectors = load()?;
    let entry = plain(&vectors)?
        .accepts
        .into_iter()
        .next()
        .ok_or("one entry")?;
    assert_eq!(
        entry.extra.withdraw_delay,
        protocol::x402::ADVERTISED_WITHDRAW_DELAY
    );
    assert_eq!(entry.extra.withdraw_delay, 3600);
    Ok(())
}

#[test]
fn challenge_extra_version_is_the_token_domain_not_x402() -> TestResult {
    // Two version strings in one object: "2" for the token's EIP-3009 domain and "1" for x402.
    // Phase 5 builds the deposit signature, and confusing them produces a signature that verifies
    // against nothing. This test is what stops it reaching for the wrong one.
    let vectors = load()?;
    let entry = plain(&vectors)?
        .accepts
        .into_iter()
        .next()
        .ok_or("one entry")?;

    assert_eq!(entry.extra.name, "USDC");
    assert_eq!(entry.extra.version, "2");
    assert_eq!(protocol::x402::DOMAIN_NAME, "x402 Batch Settlement");
    assert_eq!(protocol::x402::DOMAIN_VERSION, "1");
    assert_ne!(entry.extra.name, protocol::x402::DOMAIN_NAME);
    assert_ne!(entry.extra.version, protocol::x402::DOMAIN_VERSION);
    Ok(())
}

#[test]
fn payment_header_round_trips() -> TestResult {
    let vectors = load()?;
    let payload = voucher_payload(&vectors)?;

    let header = encode_header(&payload);
    assert!(!header.is_empty());
    assert_eq!(decode_header::<PaymentPayload>(&header)?, payload);

    // And the payload echoes the requirements it answers, which is what lets the engine check the
    // client paid for what was advertised.
    assert_eq!(payload.accepted.scheme, SCHEME);
    assert_eq!(payload.payload.payload_type, "voucher");
    Ok(())
}

#[test]
fn only_a_voucher_payload_is_accepted_here() -> TestResult {
    let vectors = load()?;
    let types = section(section(envelope(&vectors)?, "payment_payload")?, "types")?;
    let payload = voucher_payload(&vectors)?;

    assert_eq!(payload.kind()?, PayloadKind::Voucher);
    payload.validate()?;

    // deposit and refund are valid x402 payloads at the wrong endpoint: POLICY_, so the code tells
    // a client to use another route rather than to fix their payload.
    for case in group(types, "declined")? {
        let name = case.as_str().ok_or("declined type")?;
        let mut declined = payload.clone();
        declined.payload.payload_type = name.to_owned();
        assert_eq!(declined.kind()?.as_str(), name);
        assert_eq!(
            declined.validate(),
            Err(ProtocolError::PayloadTypeNotAccepted {
                got: name.to_owned()
            })
        );
    }

    // An unknown discriminant is a malformed envelope, not a routing problem.
    for case in group(types, "unknown")? {
        let name = case.as_str().ok_or("unknown type")?;
        let mut unknown = payload.clone();
        unknown.payload.payload_type = name.to_owned();
        assert_eq!(
            unknown.validate(),
            Err(ProtocolError::PayloadTypeUnknown {
                got: name.to_owned()
            })
        );
    }
    Ok(())
}

#[test]
fn unknown_field_is_refused_wherever_bytes_are_hashed_or_stored() -> TestResult {
    // ADR-0014. The rule is about hashing, not nesting, so the cases walk down the payload: the
    // top level, `payload`, `channelConfig` and `voucher` are strict, and `extra` is not.
    let vectors = load()?;
    let json = json_of(&vectors, &["payment_payload", "voucher"])?;

    for path in [
        vec![],
        vec!["accepted"],
        vec!["payload"],
        vec!["payload", "channelConfig"],
        vec!["payload", "voucher"],
    ] {
        let mut body: serde_json::Value = serde_json::from_str(&json)?;
        let mut node = &mut body;
        for step in &path {
            node = node.get_mut(step).ok_or("path step")?;
        }
        let object = node.as_object_mut().ok_or("object")?;
        object.insert("somethingNew".to_owned(), serde_json::Value::from(1));

        let refused = serde_json::from_value::<PaymentPayload>(body);
        assert!(
            refused.is_err(),
            "an unknown field at {path:?} must be refused"
        );
    }

    // `extra` is the one lenient object: the scheme puts extensions there, it is never hashed, and
    // we never read it into state.
    let mut body: serde_json::Value = serde_json::from_str(&json)?;
    body.get_mut("accepted")
        .and_then(|a| a.get_mut("extra"))
        .and_then(serde_json::Value::as_object_mut)
        .ok_or("extra")?
        .insert(
            "assetTransferMethod".to_owned(),
            serde_json::Value::from("erc3009"),
        );
    assert!(
        serde_json::from_value::<PaymentPayload>(body).is_ok(),
        "extra is lenient"
    );
    Ok(())
}

#[test]
fn amount_is_a_uint128_in_base_units() -> TestResult {
    let vectors = load()?;
    let amounts = section(envelope(&vectors)?, "amounts")?;
    let mut entry = plain(&vectors)?
        .accepts
        .into_iter()
        .next()
        .ok_or("one entry")?;

    // The ceiling written out, not computed (CLAUDE.md §5).
    assert_eq!(
        text(amounts, "u128_max")?,
        "340282366920938463463374607431768211455"
    );
    assert_eq!(text(amounts, "u128_max")?.parse::<u128>()?, u128::MAX);

    for case in group(amounts, "accepted")? {
        let value = case.as_str().ok_or("accepted amount")?;
        entry.amount = value.to_owned();
        assert_eq!(
            entry.amount_base_units()?,
            value.parse::<u128>()?,
            "'{value}' is a uint128"
        );
    }

    for case in group(amounts, "rejected")? {
        let value = case.as_str().ok_or("rejected amount")?;
        entry.amount = value.to_owned();
        assert_eq!(
            entry.amount_base_units(),
            Err(ProtocolError::AmountNotU128 {
                got: value.to_owned()
            }),
            "'{value}' is not a uint128"
        );
    }
    Ok(())
}

#[test]
fn validate_enforces_every_rule_and_not_only_the_network() -> TestResult {
    // A criterion checked in a test and nowhere else is a rule the engine does not have, so the one
    // function Phase 4 calls enforces all of them.
    let vectors = load()?;
    let base = plain(&vectors)?
        .accepts
        .into_iter()
        .next()
        .ok_or("one entry")?;
    assert_eq!(base.validate()?.chain_id(), 84532);

    // `upto` is a real x402 scheme and not this one, so it gets its own code rather than being
    // reported as an unknown payload type: a client that sent `upto` has the wrong scheme, not a
    // malformed payload.
    let mut wrong_scheme = base.clone();
    wrong_scheme.scheme = "upto".to_owned();
    assert_eq!(
        wrong_scheme.validate(),
        Err(ProtocolError::SchemeUnknown {
            got: "upto".to_owned()
        })
    );

    let mut wrong_network = base.clone();
    wrong_network.network = "base-sepolia".to_owned();
    assert!(matches!(
        wrong_network.validate(),
        Err(ProtocolError::NetworkNotCaip2 { .. })
    ));

    let mut wrong_amount = base.clone();
    wrong_amount.amount = "1e6".to_owned();
    assert!(matches!(
        wrong_amount.validate(),
        Err(ProtocolError::AmountNotU128 { .. })
    ));

    let mut short_delay = base.clone();
    short_delay.extra.withdraw_delay = 900;
    assert_eq!(
        short_delay.validate(),
        Err(ProtocolError::WithdrawDelayBelowFloor {
            got: 900,
            floor: 3600
        })
    );

    // The largest uint40 is accepted: far above our floor, and a valid width. Without this the
    // width check reads `>=` just as well as `>`, which is what cargo-mutants found.
    let mut widest_delay = base.clone();
    widest_delay.extra.withdraw_delay = 1_099_511_627_775;
    assert_eq!(widest_delay.validate()?.chain_id(), 84532);

    let mut wide_delay = base.clone();
    wide_delay.extra.withdraw_delay = 1_099_511_627_776;
    assert_eq!(
        wide_delay.validate(),
        Err(ProtocolError::WithdrawDelayWidth {
            got: 1_099_511_627_776
        })
    );
    Ok(())
}

#[test]
fn unknown_scheme_is_rejected_with_typed_error() -> TestResult {
    let vectors = load()?;
    let mut entry = plain(&vectors)?
        .accepts
        .into_iter()
        .next()
        .ok_or("one entry")?;
    for scheme in [
        "upto",
        "exact",
        "batch-settlement-v2",
        "",
        "Batch-Settlement",
    ] {
        entry.scheme = scheme.to_owned();
        assert_eq!(
            entry.validate(),
            Err(ProtocolError::SchemeUnknown {
                got: scheme.to_owned()
            }),
            "'{scheme}' is not this scheme"
        );
    }
    Ok(())
}

#[test]
fn non_base64_payload_is_rejected_with_typed_error() -> TestResult {
    for bad in ["not base64!", "{\"x402Version\":2}", "====", "a"] {
        assert_eq!(
            decode_header::<PaymentRequired>(bad),
            Err(ProtocolError::HeaderNotBase64),
            "'{bad}' is not base64"
        );
    }
    Ok(())
}

#[test]
fn truncated_header_is_rejected_with_typed_error() -> TestResult {
    let vectors = load()?;
    let json = json_of(&vectors, &["payment_required", "plain"])?;
    let cut = json.get(..json.len() / 2).ok_or("halve the json")?;
    let truncated = encode_header(&cut.to_owned());
    assert!(matches!(
        decode_header::<PaymentRequired>(&truncated),
        Err(ProtocolError::HeaderMalformed { .. })
    ));

    let header = encode_header(&plain(&vectors)?);
    let short = header.get(..header.len() / 2).ok_or("halve the header")?;
    assert!(decode_header::<PaymentRequired>(short).is_err());
    Ok(())
}

#[test]
fn oversized_header_is_rejected_before_it_is_parsed() -> TestResult {
    // The size is checked first, so a hostile client cannot make the engine parse a megabyte to
    // find out it is too big. A payment header arrives before any payment is verified.
    let huge = "A".repeat(MAX_HEADER_BYTES + 1);
    assert_eq!(
        decode_header::<PaymentRequired>(&huge),
        Err(ProtocolError::HeaderTooLarge {
            got: MAX_HEADER_BYTES + 1,
            max: MAX_HEADER_BYTES
        })
    );

    let at_limit = "A".repeat(MAX_HEADER_BYTES);
    assert!(!matches!(
        decode_header::<PaymentRequired>(&at_limit),
        Err(ProtocolError::HeaderTooLarge { .. })
    ));
    assert_eq!(MAX_HEADER_BYTES, 8192, "8 KiB, written out");
    Ok(())
}
