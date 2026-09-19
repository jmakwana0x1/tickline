//! `cost` and `prices` against the mpmath vectors, within the bounds derived in ADR-0009
//! (issue #41).
//!
//! Every case is checked against its derived bound: `|value - exact| <= E(b)` for the cost,
//! `<= PRICE_ERROR_BOUND` for prices.
//!
//! Drift is caught separately, by the accuracy snapshot in `tests/error_baseline.rs`: the worst
//! error per function is committed and asserted exactly, so a one-wei change shows up (ADR-0010).
//! A fraction-of-the-bound check cannot do that, because it measures how conservative the
//! derivation is rather than whether behaviour changed.

// Shared by several test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use common::{group, load, TestResult};
use lmsr::{cost, cost_error_bound, prices, LmsrError, B_MAX, B_MIN};

/// Exact value of a case: `[floor, floor + 1)` when inexact, else exactly `floor`.
fn exact_bounds(
    case: &serde_json::Value,
    floor_key: &str,
    inexact_key: &str,
) -> TestResult<(i128, i128)> {
    let floor: i128 = case
        .get(floor_key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("case is missing '{floor_key}': {case}"))?
        .parse()?;
    let inexact = case
        .get(inexact_key)
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| format!("case is missing '{inexact_key}': {case}"))?;
    let ceil = if inexact {
        floor.checked_add(1).ok_or("ceil overflow")?
    } else {
        floor
    };
    Ok((floor, ceil))
}

fn field(case: &serde_json::Value, name: &str) -> TestResult<i128> {
    Ok(case
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("case is missing '{name}': {case}"))?
        .parse()?)
}

/// Distance from a value to the exact interval `[lo, hi]`, in wei.
fn distance(got: i128, (lo, hi): (i128, i128)) -> TestResult<i128> {
    Ok(if got < lo {
        lo.checked_sub(got).ok_or("distance overflow")?
    } else if got > hi {
        got.checked_sub(hi).ok_or("distance overflow")?
    } else {
        0
    })
}

#[test]
fn cost_matches_reference_vectors_within_derived_bound() -> TestResult {
    let vectors = load()?;
    let (mut checked, mut worst_ratio_num, mut worst_ratio_den, mut worst) =
        (0_usize, 0_i128, 1_i128, None);
    for case in group(&vectors, "cost")? {
        if case.get("error").is_some() {
            continue;
        }
        let (q_yes, q_no, b) = (
            field(case, "q_yes")?,
            field(case, "q_no")?,
            field(case, "b")?,
        );
        let got = cost(q_yes, q_no, b)?;
        let error = distance(got, exact_bounds(case, "floor", "inexact")?)?;
        let bound = cost_error_bound(b)?;
        assert!(
            error <= bound,
            "cost({q_yes}, {q_no}, {b}) is {error} wei from exact, above the derived bound {bound}"
        );
        if error.checked_mul(worst_ratio_den).ok_or("ratio")?
            > worst_ratio_num.checked_mul(bound).ok_or("ratio")?
        {
            worst_ratio_num = error;
            worst_ratio_den = bound;
            worst = Some((q_yes, q_no, b, error, bound));
        }
        checked = checked.checked_add(1).ok_or("count")?;
    }
    assert!(checked >= 800, "only {checked} cost cases checked");
    println!("cost: {checked} cases, worst error/bound {worst:?}");
    Ok(())
}

#[test]
fn price_matches_reference_vectors_within_derived_bound() -> TestResult {
    let vectors = load()?;
    let bound = lmsr::PRICE_ERROR_BOUND;
    let (mut checked, mut worst) = (0_usize, 0_i128);
    for case in group(&vectors, "price")? {
        let (q_yes, q_no, b) = (
            field(case, "q_yes")?,
            field(case, "q_no")?,
            field(case, "b")?,
        );
        let got = prices(q_yes, q_no, b)?;
        for (label, value, floor_key, inexact_key) in [
            ("yes", got.yes, "yes_floor", "yes_inexact"),
            ("no", got.no, "no_floor", "no_inexact"),
        ] {
            let error = distance(value, exact_bounds(case, floor_key, inexact_key)?)?;
            assert!(
                error <= bound,
                "{label} price({q_yes}, {q_no}, {b}) is {error} wei from exact, above the bound {bound}"
            );
            worst = worst.max(error);
        }
        assert_eq!(
            got.yes.checked_add(got.no).ok_or("price sum overflow")?,
            lmsr::WAD,
            "prices must sum to one WAD"
        );
        checked = checked.checked_add(1).ok_or("count")?;
    }
    assert!(checked >= 600, "only {checked} price cases checked");
    println!("price: {checked} cases, worst error {worst} wei (bound {bound})");
    Ok(())
}

#[test]
fn cost_at_min_b_matches_reference() -> TestResult {
    assert!(cases_at(B_MIN)? > 0, "no vector case at B_MIN");
    Ok(())
}

#[test]
fn cost_at_max_b_matches_reference() -> TestResult {
    assert!(cases_at(B_MAX)? > 0, "no vector case at B_MAX");
    Ok(())
}

/// Checks every cost case at one liquidity, returning how many were checked.
fn cases_at(b_wanted: i128) -> TestResult<usize> {
    let vectors = load()?;
    let mut checked = 0_usize;
    for case in group(&vectors, "cost")? {
        if case.get("error").is_some() || field(case, "b")? != b_wanted {
            continue;
        }
        let (q_yes, q_no) = (field(case, "q_yes")?, field(case, "q_no")?);
        let got = cost(q_yes, q_no, b_wanted)?;
        let error = distance(got, exact_bounds(case, "floor", "inexact")?)?;
        assert!(
            error <= cost_error_bound(b_wanted)?,
            "cost({q_yes}, {q_no}, {b_wanted}) is {error} wei from exact"
        );
        checked = checked.checked_add(1).ok_or("count")?;
    }
    Ok(checked)
}

#[test]
fn cost_rejects_every_out_of_domain_case_in_the_vectors() -> TestResult {
    let vectors = load()?;
    let mut checked = 0_usize;
    for case in group(&vectors, "cost")? {
        let Some(error) = case.get("error").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let (q_yes, q_no, b) = (
            field(case, "q_yes")?,
            field(case, "q_no")?,
            field(case, "b")?,
        );
        let expected = match error {
            "liquidity_not_positive" => LmsrError::NonPositiveLiquidity(b),
            "liquidity_below_min" => LmsrError::LiquidityBelowMin(b),
            "liquidity_above_max" => LmsrError::LiquidityAboveMax(b),
            "quantity_above_max" => LmsrError::QuantityAboveMax(q_yes.max(q_no)),
            "quantity_negative" => LmsrError::QuantityNegative(q_yes.min(q_no)),
            other => return Err(format!("unknown vector error '{other}'").into()),
        };
        assert_eq!(cost(q_yes, q_no, b), Err(expected), "{case}");
        assert_eq!(prices(q_yes, q_no, b), Err(expected), "{case}");
        checked = checked.checked_add(1).ok_or("count")?;
    }
    assert!(checked >= 8, "only {checked} domain-error cases checked");
    Ok(())
}
