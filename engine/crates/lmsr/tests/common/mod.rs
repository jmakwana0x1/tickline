//! Loader for `testdata/vectors/lmsr.json`, the mpmath reference the LMSR is diffed against.
//!
//! Shared by every `lmsr` integration test that consumes the vectors (issue #38).

use std::{error::Error, fs, path::PathBuf};

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// The significant-digit floor every reference value was computed at (`PHASES.md` phase 1).
pub const REQUIRED_PRECISION_DIGITS: u64 = 60;

/// Path of the committed vector file, relative to this crate.
pub fn vectors_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../testdata/vectors/lmsr.json")
}

/// Parses the vector file. Fails, rather than skipping, when it is missing.
pub fn load() -> TestResult<serde_json::Value> {
    let path = vectors_path();
    let text =
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

/// A case group, which must exist and be a non-empty array.
pub fn group<'a>(
    vectors: &'a serde_json::Value,
    name: &str,
) -> TestResult<&'a [serde_json::Value]> {
    let cases = vectors
        .get(name)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("vector group '{name}' is missing"))?;
    if cases.is_empty() {
        return Err(format!("vector group '{name}' is empty").into());
    }
    Ok(cases)
}
