//! The leaf bounds ADR-0009's derivation rests on, checked against the vectors (issue #41).
//!
//! `exp_wad` and `ln_wad` are Solady's rational approximations: their accuracy cannot be derived,
//! only measured. Everything else in the derivation is arithmetic on top of these two numbers, so
//! if either drifts, `E(b)` is wrong and these tests are what says so.

// Shared by several test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use common::{group, load, TestResult};
use lmsr::{exp_wad, ln_wad, I256};

const EXP_LEAF_BOUND: i128 = 1;
const LN_LEAF_BOUND: i128 = 2;

fn parse(case: &serde_json::Value, name: &str) -> TestResult<I256> {
    Ok(I256::from_dec_str(
        case.get(name)
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("case is missing '{name}': {case}"))?,
    )?)
}

/// Distance from `got` to the exact interval the case describes, in wei.
fn distance(case: &serde_json::Value, got: I256) -> TestResult<I256> {
    let floor = parse(case, "floor")?;
    let inexact = case
        .get("inexact")
        .and_then(serde_json::Value::as_bool)
        .ok_or("case is missing 'inexact'")?;
    let ceil = if inexact { floor + I256::ONE } else { floor };
    Ok(if got < floor {
        floor - got
    } else if got > ceil {
        got - ceil
    } else {
        I256::ZERO
    })
}

#[test]
fn exp_wad_leaf_bound_holds_for_the_arguments_the_cost_uses() -> TestResult {
    let vectors = load()?;
    let bound = I256::try_from(EXP_LEAF_BOUND)?;
    let mut checked = 0_usize;
    for case in group(&vectors, "exp_wad")? {
        if case.get("error").is_some() {
            continue;
        }
        let x = parse(case, "x")?;
        // The log-sum-exp cost only ever calls exp with a non-positive argument.
        if x > I256::ZERO {
            continue;
        }
        let error = distance(case, exp_wad(x)?)?;
        assert!(error <= bound, "exp_wad({x}) is {error} wei from exact");
        checked += 1;
    }
    assert!(
        checked >= 400,
        "only {checked} non-positive exp cases checked"
    );
    Ok(())
}

#[test]
fn ln_wad_leaf_bound_holds() -> TestResult {
    let vectors = load()?;
    let bound = I256::try_from(LN_LEAF_BOUND)?;
    let mut checked = 0_usize;
    for case in group(&vectors, "ln_wad")? {
        if case.get("error").is_some() {
            continue;
        }
        let x = parse(case, "x")?;
        let error = distance(case, ln_wad(x)?)?;
        assert!(error <= bound, "ln_wad({x}) is {error} wei from exact");
        checked += 1;
    }
    assert!(checked >= 1_000, "only {checked} ln cases checked");
    Ok(())
}

#[test]
fn ln_wad_leaf_bound_holds_on_the_range_the_cost_uses() -> TestResult {
    // The cost only calls ln on 1 + exp(negative), which lies in [WAD, 2 * WAD].
    let vectors = load()?;
    let bound = I256::try_from(LN_LEAF_BOUND)?;
    let wad = I256::try_from(lmsr::WAD)?;
    let mut checked = 0_usize;
    for case in group(&vectors, "ln_wad")? {
        if case.get("error").is_some() {
            continue;
        }
        let x = parse(case, "x")?;
        if x < wad || x > wad * I256::try_from(2)? {
            continue;
        }
        let error = distance(case, ln_wad(x)?)?;
        assert!(error <= bound, "ln_wad({x}) is {error} wei from exact");
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} in-range ln cases checked");
    Ok(())
}
