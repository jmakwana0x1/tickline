//! The reference vector file is present, complete, and was produced at the required precision
//! (issue #38). The value checks against these vectors live with the code they test (S3 to S5).

// Shared by several test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use common::{group, load, TestResult, REQUIRED_PRECISION_DIGITS};

/// Every group the later slices consume.
const GROUPS: [&str; 7] = [
    "exp_wad",
    "ln_wad",
    "cost",
    "price",
    "cost_to_buy",
    "subsidy",
    "conversion",
];

#[test]
fn vectors_record_sixty_digit_precision() -> TestResult {
    let vectors = load()?;
    let precision = vectors
        .pointer("/provenance/precision_digits")
        .and_then(serde_json::Value::as_u64)
        .ok_or("provenance.precision_digits is missing")?;
    assert_eq!(
        precision, REQUIRED_PRECISION_DIGITS,
        "reference values must be computed at {REQUIRED_PRECISION_DIGITS} significant digits"
    );
    Ok(())
}

#[test]
fn vectors_record_their_provenance() -> TestResult {
    let vectors = load()?;
    for field in ["generator", "generator_blob", "mpmath", "python", "seed"] {
        let value = vectors
            .pointer(&format!("/provenance/{field}"))
            .ok_or_else(|| format!("provenance.{field} is missing"))?;
        assert!(!value.is_null(), "provenance.{field} is null");
    }
    Ok(())
}

#[test]
fn vectors_have_at_least_five_thousand_cases() -> TestResult {
    let vectors = load()?;
    let mut total = 0_usize;
    for name in GROUPS {
        total = total
            .checked_add(group(&vectors, name)?.len())
            .ok_or("case count overflow")?;
    }
    assert!(total >= 5_000, "only {total} reference cases");
    Ok(())
}
