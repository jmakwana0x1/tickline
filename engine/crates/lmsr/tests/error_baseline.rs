//! Accuracy snapshot: the worst error each function shows against the vectors, committed and
//! asserted exactly (ADR-0010, decided by Jay on #50).
//!
//! The inputs are committed and the arithmetic is integer and deterministic, so each worst case is
//! a fixed number. Asserting a fraction of the derived bound would measure how conservative the
//! derivation is, not whether behaviour changed, and could not see a one-wei regression. This is
//! `forge snapshot --check` applied to accuracy.
//!
//! Regenerate with `just update-error-baseline`, and say in the commit why the number moved: a
//! vector regeneration, a Solady change, or a bug. A baseline that moves **up by more than 2x**,
//! or any case exceeding its bound, is a `needs-jay`.

// Shared by several test binaries; each uses a subset of its helpers.
#[allow(dead_code)]
mod common;

use std::fs;

use common::{baseline_path, group, load, TestResult};
use lmsr::{cost, cost_error_bound, exp_wad, ln_wad, prices, I256};
use serde_json::{json, Map, Value};

/// The worst error seen for one function, and the case that produced it.
struct Worst {
    error: I256,
    case: Value,
}

impl Worst {
    fn new() -> Self {
        Self {
            error: I256::ZERO,
            case: Value::Null,
        }
    }

    /// Keeps the first case that reaches a new maximum, so ties resolve deterministically.
    fn offer(&mut self, error: I256, case: Value) {
        // The first case seen is recorded even when every error is zero, so an entry always
        // names an input.
        if self.case.is_null() || error > self.error {
            self.case = case;
        }
        if error > self.error {
            self.error = error;
        }
    }

    fn to_json(&self) -> Value {
        json!({ "worst_error_wei": self.error.to_string(), "case": self.case })
    }
}

fn str_field(case: &Value, name: &str) -> TestResult<String> {
    Ok(case
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("case is missing '{name}': {case}"))?
        .to_owned())
}

fn wide(case: &Value, name: &str) -> TestResult<I256> {
    Ok(I256::from_dec_str(&str_field(case, name)?)?)
}

fn narrow(case: &Value, name: &str) -> TestResult<i128> {
    Ok(str_field(case, name)?.parse()?)
}

/// Distance from `got` to `[floor, floor + 1)` or to `floor` exactly, in wei.
fn distance(case: &Value, got: I256, floor_key: &str, inexact_key: &str) -> TestResult<I256> {
    let floor = wide(case, floor_key)?;
    let inexact = case
        .get(inexact_key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("case is missing '{inexact_key}': {case}"))?;
    let ceil = if inexact {
        floor.checked_add(I256::ONE).ok_or("ceil overflow")?
    } else {
        floor
    };
    Ok(if got < floor {
        floor.checked_sub(got).ok_or("distance overflow")?
    } else if got > ceil {
        got.checked_sub(ceil).ok_or("distance overflow")?
    } else {
        I256::ZERO
    })
}

/// Measures every function against the vectors. Deterministic, so the result is a snapshot.
fn measure() -> TestResult<Map<String, Value>> {
    let vectors = load()?;
    let (mut exp_non_positive, mut exp_positive, mut ln, mut cost_worst, mut price_worst) = (
        Worst::new(),
        Worst::new(),
        Worst::new(),
        Worst::new(),
        Worst::new(),
    );

    for case in group(&vectors, "exp_wad")? {
        if case.get("error").is_some() {
            continue;
        }
        let x = wide(case, "x")?;
        let error = distance(case, exp_wad(x)?, "floor", "inexact")?;
        let entry = json!({ "x": x.to_string() });
        if x <= I256::ZERO {
            exp_non_positive.offer(error, entry);
        } else {
            exp_positive.offer(error, entry);
        }
    }

    for case in group(&vectors, "ln_wad")? {
        if case.get("error").is_some() {
            continue;
        }
        let x = wide(case, "x")?;
        let error = distance(case, ln_wad(x)?, "floor", "inexact")?;
        ln.offer(error, json!({ "x": x.to_string() }));
    }

    // Two views of the cost, because they catch different regressions: the largest error in wei
    // (which lives at B_MAX, where the bound is largest), and the case that comes closest to its
    // own bound (which lives at small b). A change at either end moves the snapshot.
    let mut cost_closest: Option<(i128, i128, Value)> = None;
    for case in group(&vectors, "cost")? {
        if case.get("error").is_some() {
            continue;
        }
        let (q_yes, q_no, b) = (
            narrow(case, "q_yes")?,
            narrow(case, "q_no")?,
            narrow(case, "b")?,
        );
        let wide_error = distance(
            case,
            I256::try_from(cost(q_yes, q_no, b)?)?,
            "floor",
            "inexact",
        )?;
        let bound = cost_error_bound(b)?;
        let entry = json!({ "q_yes": q_yes.to_string(), "q_no": q_no.to_string(),
                            "b": b.to_string(), "bound_wei": bound.to_string() });
        cost_worst.offer(wide_error, entry.clone());

        let error = i128::try_from(wide_error)?;
        let closer = match &cost_closest {
            None => true,
            // error / bound > best_error / best_bound, in integers.
            Some((best_error, best_bound, _)) => {
                error.checked_mul(*best_bound).ok_or("ratio overflow")?
                    > best_error.checked_mul(bound).ok_or("ratio overflow")?
            }
        };
        if closer {
            cost_closest = Some((error, bound, entry));
        }
    }

    for case in group(&vectors, "price")? {
        let (q_yes, q_no, b) = (
            narrow(case, "q_yes")?,
            narrow(case, "q_no")?,
            narrow(case, "b")?,
        );
        let got = prices(q_yes, q_no, b)?;
        for (value, floor_key, inexact_key) in [
            (got.yes, "yes_floor", "yes_inexact"),
            (got.no, "no_floor", "no_inexact"),
        ] {
            let error = distance(case, I256::try_from(value)?, floor_key, inexact_key)?;
            price_worst.offer(
                error,
                json!({ "q_yes": q_yes.to_string(), "q_no": q_no.to_string(), "b": b.to_string() }),
            );
        }
    }

    let (closest_error, closest_bound, closest_case) =
        cost_closest.ok_or("no cost cases measured")?;

    let mut out = Map::new();
    out.insert(
        "cost_closest_to_bound".into(),
        json!({
            "worst_error_wei": closest_error.to_string(),
            "bound_wei": closest_bound.to_string(),
            // error / bound, scaled by 1e6 so the snapshot stays an exact integer.
            "ratio_per_million": closest_error
                .checked_mul(1_000_000)
                .and_then(|v| v.checked_div(closest_bound))
                .ok_or("ratio overflow")?
                .to_string(),
            "case": closest_case,
        }),
    );
    out.insert("exp_wad_non_positive".into(), exp_non_positive.to_json());
    out.insert("exp_wad_positive".into(), exp_positive.to_json());
    out.insert("ln_wad".into(), ln.to_json());
    out.insert("cost".into(), cost_worst.to_json());
    out.insert("price".into(), price_worst.to_json());
    Ok(out)
}

#[test]
fn accuracy_matches_the_committed_baseline() -> TestResult {
    let measured = measure()?;
    let path = baseline_path();

    if std::env::var_os("UPDATE_ERROR_BASELINE").is_some() {
        let mut doc = Map::new();
        doc.insert("schema".into(), json!(1));
        doc.insert(
            "note".into(),
            json!(
                "Worst observed error per function against testdata/vectors/lmsr.json. \
                   Regenerate with `just update-error-baseline`. A rise of more than 2x, or any \
                   case above its bound, is a needs-jay (ADR-0010)."
            ),
        );
        doc.extend(measured);
        fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&doc)?))?;
        println!("wrote {}", path.display());
        return Ok(());
    }

    let text = fs::read_to_string(&path).map_err(|e| {
        format!(
            "cannot read {}: {e}. Run `just update-error-baseline` to create it.",
            path.display()
        )
    })?;
    let committed: Value = serde_json::from_str(&text)?;
    for (name, value) in &measured {
        let recorded = committed
            .get(name)
            .ok_or_else(|| format!("the baseline has no entry for '{name}'"))?;
        assert_eq!(
            recorded, value,
            "accuracy for '{name}' moved.\n  committed: {recorded}\n  measured:  {value}\n\
             If this is intended, run `just update-error-baseline` and say in the commit why: a \
             vector regeneration, a Solady change, or a bug. A rise of more than 2x is a needs-jay."
        );
    }
    Ok(())
}

/// Coarse guard on the derived bound, not a drift detector: the snapshot above is what sees
/// drift. This fails only if the measured error climbs to within a quarter of `E(b)`, at which
/// point the bound has stopped being a meaningful ceiling and either the derivation or the
/// implementation has changed.
#[test]
fn cost_error_bound_is_not_vacuous() -> TestResult {
    let measured = measure()?;
    // The case that comes closest to its own bound is the one this guard cares about.
    let entry = measured
        .get("cost_closest_to_bound")
        .ok_or("no cost_closest_to_bound entry")?;
    let worst: i128 = entry
        .pointer("/worst_error_wei")
        .and_then(Value::as_str)
        .ok_or("no worst error")?
        .parse()?;
    let bound: i128 = entry
        .pointer("/bound_wei")
        .and_then(Value::as_str)
        .ok_or("no bound")?
        .parse()?;
    let limit = bound
        .checked_mul(3)
        .and_then(|v| v.checked_div(4))
        .ok_or("limit overflow")?;
    assert!(
        worst <= limit,
        "worst cost error {worst} wei is above three quarters of the bound {bound}"
    );
    Ok(())
}
