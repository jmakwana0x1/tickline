//! `exp_wad` and `ln_wad` against the mpmath reference vectors (issue #40).
//!
//! The port is Solady's algorithm, which is an approximation, so "matches" means within the
//! error Solady itself has against the exact values. Measured on 2026-09-17 by running Solady
//! `9fe23ffd` under forge over every input in `testdata/vectors/lmsr.json`:
//!
//! - `exp_wad`, `x <= 0` (the only range the log-sum-exp cost uses): always within the exact
//!   floor and ceiling.
//! - `exp_wad`, `x > 0`: relative error at most 1.58e-20 (606 cases); asserted as within one wei
//!   or 1e-19 of the exact value, whichever is larger. 1e-19 is the next decade above the
//!   measured maximum.
//! - `ln_wad`: always strictly within 2 wei of the exact value.
//!
//! The same run found this port bit-for-bit identical to Solady on all 2,200 inputs, reverts
//! included. That parity is checked in CI from Phase 3, where Solady is built.

// Shared by several test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use common::{group, load, TestResult};
use lmsr::{exp_wad, ln_wad, LmsrError, I256};

fn field(case: &serde_json::Value, name: &str) -> TestResult<I256> {
    let text = case
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("case is missing '{name}': {case}"))?;
    Ok(I256::from_dec_str(text)?)
}

/// The exact value lies in `[floor, floor + 1)` when inexact, or equals `floor`.
fn exact_bounds(case: &serde_json::Value) -> TestResult<(I256, I256)> {
    let floor = field(case, "floor")?;
    let inexact = case
        .get("inexact")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| format!("case is missing 'inexact': {case}"))?;
    let ceil = if inexact {
        floor.checked_add(I256::ONE).ok_or("ceil overflow")?
    } else {
        floor
    };
    Ok((floor, ceil))
}

fn widen(bounds: (I256, I256), by: I256) -> TestResult<(I256, I256)> {
    Ok((
        bounds.0.checked_sub(by).ok_or("bound underflow")?,
        bounds.1.checked_add(by).ok_or("bound overflow")?,
    ))
}

#[test]
fn exp_wad_matches_reference_vectors() -> TestResult {
    let vectors = load()?;
    let relative = I256::from_dec_str("10000000000000000000")?; // 1e19
    let mut checked = 0_usize;
    for case in group(&vectors, "exp_wad")? {
        let x = field(case, "x")?;
        if case.get("error").is_some() {
            assert_eq!(exp_wad(x), Err(LmsrError::ExpOverflow(x)), "{case}");
            continue;
        }
        let got = exp_wad(x)?;
        let exact = exact_bounds(case)?;
        let (lo, hi) = if x <= I256::ZERO {
            exact
        } else {
            let tolerance = (exact.0.checked_div(relative).ok_or("division")?).max(I256::ONE);
            widen(exact, tolerance)?
        };
        assert!(
            lo <= got && got <= hi,
            "exp_wad({x}) = {got}, expected within [{lo}, {hi}]"
        );
        checked = checked.checked_add(1).ok_or("count overflow")?;
    }
    assert!(checked >= 1_000, "only {checked} exp_wad cases checked");
    Ok(())
}

#[test]
fn ln_wad_matches_reference_vectors() -> TestResult {
    let vectors = load()?;
    let mut checked = 0_usize;
    for case in group(&vectors, "ln_wad")? {
        let x = field(case, "x")?;
        if case.get("error").is_some() {
            assert_eq!(ln_wad(x), Err(LmsrError::LnUndefined(x)), "{case}");
            continue;
        }
        let got = ln_wad(x)?;
        // Strictly within 2 wei of the exact value: [floor - 1, ceil + 1].
        let (lo, hi) = widen(exact_bounds(case)?, I256::ONE)?;
        assert!(
            lo <= got && got <= hi,
            "ln_wad({x}) = {got}, expected within [{lo}, {hi}]"
        );
        checked = checked.checked_add(1).ok_or("count overflow")?;
    }
    assert!(checked >= 1_000, "only {checked} ln_wad cases checked");
    Ok(())
}
