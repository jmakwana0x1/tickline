//! Loader for `testdata/vectors/eip712-primitives.json` (issue #62).
//!
//! The fixtures live in a file rather than in the test source for two reasons. `.gitleaks.toml`
//! cannot tell a 32-byte hash from a 32-byte private key, so every hex literal in a `.rs` file is
//! a finding, and `testdata/vectors/` is where that config says fixed test material belongs.
//! It is also where #67 puts the generated cross-stack vectors, so the tests already read from
//! the shape they will keep.

use std::{error::Error, fs, path::PathBuf};

use alloy_primitives::{Address, B256, U256};

pub type TestResult<T = ()> = Result<T, Box<dyn Error>>;

/// Path of the committed fixture file.
///
/// `TICKLINE_VECTORS_DIR` overrides the directory. `just mutants` sets it, because cargo-mutants
/// copies only `engine/` to a scratch directory, where the relative path would not resolve.
pub fn vectors_path() -> PathBuf {
    match std::env::var_os("TICKLINE_VECTORS_DIR") {
        Some(dir) => PathBuf::from(dir).join("eip712-primitives.json"),
        None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../testdata/vectors/eip712-primitives.json"),
    }
}

/// Parses the fixture file. Fails, rather than skipping, when it is missing.
pub fn load() -> TestResult<serde_json::Value> {
    let path = vectors_path();
    let text =
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok(serde_json::from_str(&text)?)
}

/// A named field, which must be a string.
pub fn text<'a>(value: &'a serde_json::Value, key: &str) -> TestResult<&'a str> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("fixture field '{key}' is missing or not a string").into())
}

/// A named field parsed as a 32-byte value.
pub fn hash(value: &serde_json::Value, key: &str) -> TestResult<B256> {
    Ok(text(value, key)?.parse::<B256>()?)
}

/// A named field parsed as an address.
pub fn address(value: &serde_json::Value, key: &str) -> TestResult<Address> {
    Ok(text(value, key)?.parse::<Address>()?)
}

/// A named field parsed as a 256-bit unsigned integer written in hex.
pub fn uint(value: &serde_json::Value, key: &str) -> TestResult<U256> {
    Ok(U256::from_be_bytes(hash(value, key)?.0))
}

/// A named field parsed as a 65-byte signature.
pub fn signature_bytes(value: &serde_json::Value, key: &str) -> TestResult<[u8; 65]> {
    let decoded = alloy_primitives::hex::decode(text(value, key)?)?;
    let mut out = [0u8; 65];
    if decoded.len() != out.len() {
        return Err(format!("fixture '{key}' is {} bytes, expected 65", decoded.len()).into());
    }
    out.copy_from_slice(&decoded);
    Ok(out)
}

/// A named field that must be an unsigned integer.
pub fn number(value: &serde_json::Value, key: &str) -> TestResult<u64> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| format!("fixture field '{key}' is missing or not a number").into())
}

/// A case group, which must exist and be a non-empty array.
pub fn group<'a>(
    vectors: &'a serde_json::Value,
    name: &str,
) -> TestResult<&'a [serde_json::Value]> {
    let cases = vectors
        .get(name)
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("fixture group '{name}' is missing"))?;
    if cases.is_empty() {
        return Err(format!("fixture group '{name}' is empty").into());
    }
    Ok(cases)
}

/// A named object inside the fixture file.
pub fn section<'a>(
    vectors: &'a serde_json::Value,
    name: &str,
) -> TestResult<&'a serde_json::Value> {
    vectors
        .get(name)
        .ok_or_else(|| format!("fixture section '{name}' is missing").into())
}
