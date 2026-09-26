//! The x402 V2 wire format: the 402 challenge, and the header a client answers it with (#66).
//!
//! From `docs/spec-notes.md` §3. Tickline runs no facilitator (Q6), so the engine produces and
//! consumes these itself, but the format is the spec's own, which is what lets the official
//! TypeScript client work unchanged.
//!
//! **Nothing here is trusted.** A payment header is the most hostile input the engine takes: it
//! arrives before any payment has been verified, so every rejection is cheap and typed, and the
//! size bound is checked before anything is parsed.
//!
//! Everything here is checked in the cheapest order: the size before the base64, the base64
//! before the JSON, the JSON before anything semantic.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

use crate::ProtocolError;

/// The envelope version this engine speaks. V1's `X-PAYMENT` headers are not accepted.
pub const X402_VERSION: u8 = 2;

/// The request header a client sends a payment in.
pub const PAYMENT_SIGNATURE_HEADER: &str = "PAYMENT-SIGNATURE";

/// The response header the engine reports a charge in.
pub const PAYMENT_RESPONSE_HEADER: &str = "PAYMENT-RESPONSE";

/// The only payment scheme this engine accepts.
///
/// `upto` is a different scheme, where value moves per request; batch settlement is cumulative
/// (`docs/spec-notes.md` §2).
pub const SCHEME: &str = "batch-settlement";

/// The largest payment header the engine will look at, in bytes.
///
/// A voucher payload is about a kilobyte: a channel config, a voucher, and a signature. Eight
/// kibibytes is generous for that and still bounded, and it matches what a default nginx or Caddy
/// would pass through, so the engine's limit is not the one a client discovers first.
pub const MAX_HEADER_BYTES: usize = 8 * 1024;

/// A CAIP-2 chain identifier, the V2 spelling of a network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Network {
    chain_id: u64,
}

impl Network {
    /// Parse `eip155:<chainId>`.
    ///
    /// Only the `eip155` namespace is accepted, and the reference must be a plain decimal chain id
    /// with no leading plus, sign, or whitespace. `base-sepolia` is the V1 spelling and is refused
    /// here rather than translated: accepting both would mean two names for one network, and the
    /// challenge we send would not match the one a client echoes.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::NetworkNotCaip2`] for anything else.
    pub fn parse(text: &str) -> Result<Self, ProtocolError> {
        let refused = || ProtocolError::NetworkNotCaip2 {
            got: text.to_owned(),
        };
        let reference = text.strip_prefix("eip155:").ok_or_else(refused)?;
        if reference.is_empty() || !reference.bytes().all(|b| b.is_ascii_digit()) {
            return Err(refused());
        }
        let chain_id = reference.parse::<u64>().map_err(|_| refused())?;
        if chain_id == 0 {
            return Err(refused());
        }
        Ok(Self { chain_id })
    }

    /// The chain id.
    #[must_use]
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }
}

impl core::fmt::Display for Network {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "eip155:{}", self.chain_id)
    }
}

/// Decode a base64 payment header into `T`.
///
/// Checked in the cheapest order, which matters because this is the engine's most hostile input:
/// it arrives before any payment has been verified.
///
/// 1. the size, so a megabyte is refused without being decoded;
/// 2. the base64, so nothing malformed reaches the parser;
/// 3. the JSON, whose complaint is carried as text because a caller cannot act on it.
///
/// # Errors
///
/// [`ProtocolError::HeaderTooLarge`], [`ProtocolError::HeaderNotBase64`], or
/// [`ProtocolError::HeaderMalformed`].
pub fn decode_header<T: serde::de::DeserializeOwned>(header: &str) -> Result<T, ProtocolError> {
    if header.len() > MAX_HEADER_BYTES {
        return Err(ProtocolError::HeaderTooLarge {
            got: header.len(),
            max: MAX_HEADER_BYTES,
        });
    }
    let bytes = STANDARD
        .decode(header)
        .map_err(|_| ProtocolError::HeaderNotBase64)?;
    serde_json::from_slice(&bytes).map_err(|e| ProtocolError::HeaderMalformed {
        detail: e.to_string(),
    })
}

/// Encode a value as a base64 payment header.
///
/// Infallible for the types in this crate: they are plain data with no map keys that could fail to
/// serialize, so a failure here would be a bug rather than an input problem, and an empty header is
/// refused by the decoder on the other side.
#[must_use]
pub fn encode_header<T: serde::Serialize>(value: &T) -> String {
    match serde_json::to_vec(value) {
        Ok(bytes) => STANDARD.encode(bytes),
        Err(_) => String::new(),
    }
}

/// The body of a 402 answer: what payment this endpoint accepts.
///
/// `amount` is the **per-request ceiling** the client turns into `maxClaimableAmount`, not a price:
/// the engine charges the actual cost within it (`docs/spec-notes.md` §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Challenge {
    /// Always [`SCHEME`].
    pub scheme: String,
    /// CAIP-2, as text on the wire.
    pub network: String,
    /// The per-request ceiling, in token base units, as a decimal string.
    pub amount: String,
    /// The ERC-20 the channel is denominated in.
    pub asset: String,
    /// The receiver the channel pays.
    pub pay_to: String,
    /// How long the client may take.
    pub max_timeout_seconds: u64,
    /// Scheme-specific fields.
    pub extra: ChallengeExtra,
}

/// The `extra` of a batch-settlement challenge.
///
/// `name` and `version` are the **token's** EIP-3009 domain, not the x402 domain. A deposit
/// signature verifies against the wrong domain if the two are confused, which is why they are
/// documented here and carried in the fixtures.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeExtra {
    /// Who may claim and refund on the channel.
    pub receiver_authorizer: String,
    /// Seconds. Tickline requires 3600 (#8).
    pub withdraw_delay: u64,
    /// The token's EIP-3009 domain name, for example `USDC`.
    pub name: String,
    /// The token's EIP-3009 domain version, for example `2`.
    pub version: String,
    /// The smallest deposit worth opening a channel with, if advertised.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub min_deposit: Option<String>,
}

impl Challenge {
    /// Check that the challenge is one this engine would have produced, and return its network.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::NetworkNotCaip2`] when the network is not CAIP-2.
    pub fn validate(&self) -> Result<Network, ProtocolError> {
        Network::parse(&self.network)
    }
}
