//! Buying, the subsidy and the base-unit boundary against the vectors (issue #42).
//!
//! The money properties are I15 (costs round up, shares round down) and I3 (the subsidy covers
//! the worst case). Both rest on the margin from ADR-0009: a converted cost carries `E(b)`, a
//! converted buy carries `2 * E(b)` because it is a difference of two costs, and the subsidy
//! carries `E(b)` because the vault must hold it. Shares carry none: a margin only ever favours
//! the vault.

// Shared by several test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use common::{group, load, TestResult};
use lmsr::{
    cost_error_bound, cost_to_base_units, cost_to_buy, cost_to_buy_base_units, round_cost_up,
    round_shares_down, subsidy_base_units, LmsrError, Outcome, I256,
};
use serde_json::Value;

fn narrow(case: &Value, name: &str) -> TestResult<i128> {
    Ok(text(case, name)?.parse()?)
}

fn text(case: &Value, name: &str) -> TestResult<String> {
    Ok(case
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("case is missing '{name}': {case}"))?
        .to_owned())
}

fn wide(case: &Value, name: &str) -> TestResult<I256> {
    Ok(I256::from_dec_str(&text(case, name)?)?)
}

fn outcome(case: &Value) -> TestResult<Outcome> {
    match text(case, "outcome")?.as_str() {
        "yes" => Ok(Outcome::Yes),
        "no" => Ok(Outcome::No),
        other => Err(format!("unknown outcome '{other}'").into()),
    }
}

/// Exact value of a case: `[floor, floor + 1)` when inexact, else `floor`.
fn exact_bounds(case: &Value) -> TestResult<(i128, i128)> {
    let floor = narrow(case, "floor")?;
    let inexact = case
        .get("inexact")
        .and_then(Value::as_bool)
        .ok_or("case is missing 'inexact'")?;
    let ceil = if inexact {
        floor.checked_add(1).ok_or("ceil overflow")?
    } else {
        floor
    };
    Ok((floor, ceil))
}

#[test]
fn cost_to_buy_matches_reference_vectors() -> TestResult {
    let vectors = load()?;
    let mut checked = 0_usize;
    for case in group(&vectors, "cost_to_buy")? {
        if case.get("error").is_some() {
            continue;
        }
        let (q_yes, q_no, b, d) = (
            narrow(case, "q_yes")?,
            narrow(case, "q_no")?,
            narrow(case, "b")?,
            narrow(case, "d")?,
        );
        let got = cost_to_buy(q_yes, q_no, b, outcome(case)?, d)?;
        let (lo, hi) = exact_bounds(case)?;
        // A difference of two costs carries two bounds (ADR-0009).
        let bound = cost_error_bound(b)?
            .checked_mul(2)
            .ok_or("bound overflow")?;
        let error = if got < lo {
            lo.checked_sub(got).ok_or("underflow")?
        } else if got > hi {
            got.checked_sub(hi).ok_or("overflow")?
        } else {
            0
        };
        assert!(
            error <= bound,
            "cost_to_buy({q_yes}, {q_no}, {b}, {d}) is {error} wei from exact, above 2 * E(b) = {bound}"
        );
        checked = checked.checked_add(1).ok_or("count")?;
    }
    assert!(checked >= 1_000, "only {checked} cost_to_buy cases checked");
    Ok(())
}

#[test]
fn cost_to_buy_rejects_every_out_of_domain_case_in_the_vectors() -> TestResult {
    let vectors = load()?;
    let mut checked = 0_usize;
    for case in group(&vectors, "cost_to_buy")? {
        let Some(error) = case.get("error").and_then(Value::as_str) else {
            continue;
        };
        let (q_yes, q_no, b, d) = (
            narrow(case, "q_yes")?,
            narrow(case, "q_no")?,
            narrow(case, "b")?,
            narrow(case, "d")?,
        );
        let expected = match error {
            "quantity_not_positive" => LmsrError::QuantityNotPositive(d),
            // Every cap case in this group is a buy from a legal state, so it is the buy
            // variant, which carries the whole picture rather than one number.
            "quantity_above_max" => {
                let existing = match outcome(case)? {
                    Outcome::Yes => q_yes,
                    Outcome::No => q_no,
                };
                LmsrError::BuyAboveQuantityMax {
                    existing,
                    requested: d,
                    resulting: existing.checked_add(d).ok_or("overflow")?,
                    max: lmsr::Q_MAX,
                }
            }
            other => return Err(format!("unknown vector error '{other}'").into()),
        };
        assert_eq!(
            cost_to_buy(q_yes, q_no, b, outcome(case)?, d),
            Err(expected),
            "{case}"
        );
        checked = checked.checked_add(1).ok_or("count")?;
    }
    assert!(checked >= 3, "only {checked} domain-error cases checked");
    Ok(())
}

/// I15 for a buy: the converted charge is never below the exact cost, and the double margin
/// never overcharges by more than two base units.
#[test]
fn cost_to_buy_converted_never_undercharges() -> TestResult {
    let vectors = load()?;
    let mut checked = 0_usize;
    for case in group(&vectors, "cost_to_buy")? {
        if case.get("error").is_some() {
            continue;
        }
        let (q_yes, q_no, b, d) = (
            narrow(case, "q_yes")?,
            narrow(case, "q_no")?,
            narrow(case, "b")?,
            narrow(case, "d")?,
        );
        let exact: u128 = text(case, "base_units")?.parse()?;
        let converted = cost_to_buy_base_units(cost_to_buy(q_yes, q_no, b, outcome(case)?, d)?, b)?;
        assert!(
            converted >= exact,
            "I15: buy converted to {converted} base units, below exact {exact}"
        );
        let over = converted.checked_sub(exact).ok_or("under exact")?;
        assert!(
            over <= 2,
            "the double margin overcharged by {over} base units for ({q_yes}, {q_no}, {b}, {d})"
        );
        checked = checked.checked_add(1).ok_or("count")?;
    }
    assert!(checked >= 1_000, "only {checked} buy conversions checked");
    Ok(())
}

#[test]
fn subsidy_is_ceil_b_ln2_in_base_units() -> TestResult {
    let vectors = load()?;
    let mut checked = 0_usize;
    for case in group(&vectors, "subsidy")? {
        let b = narrow(case, "b")?;
        let exact: u128 = text(case, "base_units")?.parse()?;
        let got = subsidy_base_units(b)?;
        // The margin can push the subsidy up by at most one base unit, never below exact: I3
        // needs the creator to fund at least the worst case.
        assert!(
            got >= exact && got <= exact.checked_add(1).ok_or("overflow")?,
            "subsidy({b}) = {got}, expected {exact} or one base unit above"
        );
        checked = checked.checked_add(1).ok_or("count")?;
    }
    assert!(checked >= 100, "only {checked} subsidy cases checked");
    Ok(())
}

#[test]
fn conversion_boundary_values_match_reference() -> TestResult {
    let vectors = load()?;
    let mut checked = 0_usize;
    for case in group(&vectors, "conversion")? {
        let kind = text(case, "kind")?;
        let wad = wide(case, "wad")?;
        let got = match kind.as_str() {
            "cost" => round_cost_up(wad),
            "shares" => round_shares_down(wad),
            other => return Err(format!("unknown conversion kind '{other}'").into()),
        };
        match case.get("error").and_then(Value::as_str) {
            Some("negative") => assert_eq!(got, Err(LmsrError::AmountNegative(wad)), "{case}"),
            Some("above_u128") => assert_eq!(got, Err(LmsrError::AmountAboveU128(wad)), "{case}"),
            Some(other) => return Err(format!("unknown vector error '{other}'").into()),
            None => {
                let expected: u128 = text(case, "base_units")?.parse()?;
                assert_eq!(got?, expected, "{kind} conversion of {wad}");
            }
        }
        checked = checked.checked_add(1).ok_or("count")?;
    }
    // 0, one base unit either side, the Q_MAX edge and u128::MAX, for both kinds.
    assert!(checked >= 30, "only {checked} conversion cases checked");
    Ok(())
}

#[test]
fn conversion_rejects_values_above_u128() -> TestResult {
    let too_big = I256::from_dec_str("340282366920938463463374607431768211456")? // 2^128
        .checked_mul(I256::try_from(lmsr::BASE_UNIT_SCALE)?)
        .ok_or("overflow")?;
    assert_eq!(
        round_cost_up(too_big),
        Err(LmsrError::AmountAboveU128(too_big))
    );
    assert_eq!(
        round_shares_down(too_big),
        Err(LmsrError::AmountAboveU128(too_big))
    );
    Ok(())
}

#[test]
fn prop_i15_converted_cost_ge_exact_from_vectors() -> TestResult {
    let vectors = load()?;
    let mut checked = 0_usize;
    for case in group(&vectors, "cost")? {
        if case.get("error").is_some() {
            continue;
        }
        let (q_yes, q_no, b) = (
            narrow(case, "q_yes")?,
            narrow(case, "q_no")?,
            narrow(case, "b")?,
        );
        let exact_base_units = exact_ceil_base_units(exact_bounds(case)?)?;
        let converted = cost_to_base_units(lmsr::cost(q_yes, q_no, b)?, b)?;
        assert!(
            converted >= exact_base_units,
            "I15: converted cost {converted} is below exact {exact_base_units} for ({q_yes}, {q_no}, {b})"
        );
        checked = checked.checked_add(1).ok_or("count")?;
    }
    assert!(checked >= 800, "only {checked} cases checked");
    Ok(())
}

#[test]
fn prop_converted_cost_never_exceeds_exact_by_more_than_two_base_units() -> TestResult {
    let vectors = load()?;
    for case in group(&vectors, "cost")? {
        if case.get("error").is_some() {
            continue;
        }
        let (q_yes, q_no, b) = (
            narrow(case, "q_yes")?,
            narrow(case, "q_no")?,
            narrow(case, "b")?,
        );
        let exact_base_units = exact_ceil_base_units(exact_bounds(case)?)?;
        let converted = cost_to_base_units(lmsr::cost(q_yes, q_no, b)?, b)?;
        let over = converted
            .checked_sub(exact_base_units)
            .ok_or("under exact")?;
        assert!(
            over <= 2,
            "the margin overcharged by {over} base units for ({q_yes}, {q_no}, {b})"
        );
    }
    Ok(())
}

/// The exact cost in base units, rounded up: what I15 requires the conversion to reach.
fn exact_ceil_base_units((floor, ceil): (i128, i128)) -> TestResult<u128> {
    let scale = lmsr::BASE_UNIT_SCALE;
    // Use the ceiling of the exact value, since the exact value lies in [floor, ceil].
    let exact = if ceil > floor { ceil } else { floor };
    let rounded = exact
        .checked_add(scale.checked_sub(1).ok_or("scale")?)
        .and_then(|v| v.checked_div(scale))
        .ok_or("conversion overflow")?;
    Ok(u128::try_from(rounded)?)
}
