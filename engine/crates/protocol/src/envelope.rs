//! The x402 V2 wire format: what a 402 answer contains, and what a client sends back (#66).
//!
//! Quoted, not summarized, from `docs/spec-notes.md` §3. Tickline runs no facilitator (Q6), so the
//! engine produces and consumes these itself, but the format is the spec's own, which is what lets
//! the official TypeScript client work unchanged.
//!
//! **Nothing here is trusted.** A payment header is the engine's most hostile input: it arrives
//! before any payment has been verified. So the size is checked before the base64, the base64
//! before the JSON, and unknown fields are refused everywhere the bytes reach a hash or reach
//! state (ADR-0014).

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

use crate::{x402::ADVERTISED_WITHDRAW_DELAY, ProtocolError};

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

/// The scheme's error code for a corrective 402.
///
/// The scheme defines about fifty `invalid_batch_settlement_evm_*` codes and this is the one S5
/// needs. They are the wire vocabulary; ours (`ENV_`, `POLICY_`, ADR-0012) name why we refused
/// something internally. Mapping between them is Phase 4's, when the API starts emitting them.
pub const CUMULATIVE_AMOUNT_MISMATCH: &str =
    "invalid_batch_settlement_evm_cumulative_amount_mismatch";

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
    /// Only the `eip155` namespace is accepted, and the reference must be a plain decimal chain id.
    /// `base-sepolia` is the V1 spelling and is refused here rather than translated: accepting both
    /// would mean two names for one network, and the challenge we send would not match the one a
    /// client echoes back.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::NetworkNotCaip2`] for anything else, including chain id zero, which is a
    /// valid decimal and not a chain.
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

/// The body of a 402 answer: every payment this endpoint would accept.
///
/// The body is `accepts`, not one requirement: the official TypeScript client reads the array. A
/// corrective 402 is the same shape with `error` set and `channelState` and `voucherState` in the
/// entry's `extra`, which is what makes it a body rather than a special case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaymentRequired {
    /// Always [`X402_VERSION`].
    pub x402_version: u8,
    /// The scheme's error code, on a corrective 402.
    ///
    /// Declared before `accepts` because that is the order the scheme's own examples use, and S6
    /// diffs our bytes against a TypeScript generator: field order is part of what has to match.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub error: Option<String>,
    /// What this endpoint accepts. At least one entry.
    pub accepts: Vec<PaymentRequirements>,
}

/// One entry of `accepts`: a payment this endpoint would take.
///
/// `amount` is the **per-request ceiling** the client turns into `maxClaimableAmount`, not a price:
/// the engine charges the actual cost within it (`docs/spec-notes.md` §2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaymentRequirements {
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
    pub extra: RequirementsExtra,
}

/// The `extra` of a batch-settlement requirement.
///
/// Lenient about unknown fields, and the only object in this module that is: `extra` is where the
/// scheme itself puts extensions, it is never hashed, and we never read it into state (ADR-0014).
///
/// `name` and `version` are the **token's** EIP-3009 domain, not the x402 domain. Two version
/// strings live in this object, `"2"` for a token and `"1"` for x402, and a deposit signature
/// verifies against nothing if they are confused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequirementsExtra {
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
    /// A channel snapshot, on a corrective 402 only.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub channel_state: Option<ChannelState>,
    /// The last voucher we saw, on a corrective 402 only, so the client can verify its own
    /// signature before adopting our number.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub voucher_state: Option<VoucherState>,
}

/// A channel snapshot. Untrusted by the client, by the scheme's own rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChannelState {
    /// The channel.
    pub channel_id: String,
    /// Deposited, minus withdrawals and refunds.
    pub balance: String,
    /// Claimed onchain so far.
    pub total_claimed: String,
    /// Zero when no withdrawal is pending.
    pub withdraw_requested_at: u64,
    /// The channel's refund nonce.
    pub refund_nonce: String,
    /// What the server has charged cumulatively.
    pub charged_cumulative_amount: String,
}

/// The last voucher the server holds, on a corrective 402.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoucherState {
    /// The ceiling that voucher signed.
    pub signed_max_claimable: String,
    /// Its signature, so the client can verify its own prior work.
    pub signature: String,
}

/// Which kind of payload a client sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PayloadKind {
    /// Open or refill a channel. Not served at a fill endpoint.
    Deposit,
    /// Pay with a cumulative voucher. The only kind Tickline serves.
    Voucher,
    /// Request a cooperative refund. Deferred to #73.
    Refund,
}

impl PayloadKind {
    /// Parse the scheme's `type` discriminant.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::PayloadTypeUnknown`] for anything the scheme does not define. A `deposit`
    /// or `refund` parses here and is declined later, by [`PaymentPayload::validate`], because
    /// those are valid x402 payloads at the wrong endpoint rather than malformed ones.
    pub fn parse(text: &str) -> Result<Self, ProtocolError> {
        match text {
            "deposit" => Ok(Self::Deposit),
            "voucher" => Ok(Self::Voucher),
            "refund" => Ok(Self::Refund),
            other => Err(ProtocolError::PayloadTypeUnknown {
                got: other.to_owned(),
            }),
        }
    }

    /// The wire spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Deposit => "deposit",
            Self::Voucher => "voucher",
            Self::Refund => "refund",
        }
    }
}

/// What a client sends in `PAYMENT-SIGNATURE`.
///
/// It echoes the requirements it is answering in `accepted`, which is what lets a server check that
/// the client paid for what was advertised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PaymentPayload {
    /// Always [`X402_VERSION`].
    pub x402_version: u8,
    /// The requirements this payment answers.
    pub accepted: PaymentRequirements,
    /// The payment itself.
    pub payload: Payload,
}

/// The payment inside a [`PaymentPayload`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Payload {
    /// `deposit`, `voucher` or `refund`.
    #[serde(rename = "type")]
    pub payload_type: String,
    /// The channel this pays on.
    pub channel_config: ChannelConfigWire,
    /// The cumulative authorization.
    pub voucher: VoucherWire,
}

/// `channelConfig` as it arrives on the wire.
///
/// Strict about unknown fields, and this is the case that matters most: it is the EIP-712 struct
/// whose hash **is** the `channelId`. An unknown field means the client signed a different type
/// string than the one we hash, so ignoring it would give us a channel id over the seven fields we
/// know while they signed eight, and the failure would surface as `SIG_WRONG_SIGNER` (ADR-0014).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ChannelConfigWire {
    /// The client wallet the funds came from.
    pub payer: String,
    /// The EOA that signs vouchers.
    pub payer_authorizer: String,
    /// The server's payment destination.
    pub receiver: String,
    /// Who may claim and refund.
    pub receiver_authorizer: String,
    /// The ERC-20.
    pub token: String,
    /// Seconds before a timed withdrawal completes.
    pub withdraw_delay: u64,
    /// Distinguishes otherwise identical channels.
    pub salt: String,
}

/// `voucher` as it arrives on the wire. Signed, so strict.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoucherWire {
    /// The channel.
    pub channel_id: String,
    /// The cumulative ceiling, as a decimal string.
    pub max_claimable_amount: String,
    /// The payer's EIP-712 signature.
    pub signature: String,
}

impl PaymentRequirements {
    /// Every rule this crate can check about a requirement, in one call.
    ///
    /// Phase 4 calls this rather than reimplementing the checks, which is the point: a rule
    /// enforced only in a test is a rule the engine does not have.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::SchemeUnknown`] when the scheme is not batch-settlement,
    /// [`ProtocolError::NetworkNotCaip2`] when the network is not CAIP-2,
    /// [`ProtocolError::WithdrawDelayBelowFloor`] when `extra.withdrawDelay` is under Tickline's
    /// floor, [`ProtocolError::WithdrawDelayWidth`] when it is not a `uint40`, and
    /// [`ProtocolError::AmountNotU128`] when `amount` is not a `uint128`.
    pub fn validate(&self) -> Result<Network, ProtocolError> {
        if self.scheme != SCHEME {
            return Err(ProtocolError::SchemeUnknown {
                got: self.scheme.clone(),
            });
        }
        let network = Network::parse(&self.network)?;
        let _ = self.amount_base_units()?;

        // The same two rules the channel config is held to, in the same order, so an advertisement
        // and a channel cannot disagree about what we serve (crate::x402).
        if self.extra.withdraw_delay > crate::x402::UINT40_MAX {
            return Err(ProtocolError::WithdrawDelayWidth {
                got: self.extra.withdraw_delay,
            });
        }
        if self.extra.withdraw_delay < ADVERTISED_WITHDRAW_DELAY {
            return Err(ProtocolError::WithdrawDelayBelowFloor {
                got: self.extra.withdraw_delay,
                floor: ADVERTISED_WITHDRAW_DELAY,
            });
        }
        Ok(network)
    }

    /// `amount` as base units.
    ///
    /// It is a decimal string on the wire and becomes `maxClaimableAmount`, which the escrow gives
    /// `uint128`. Parsed here so the width is decided once, in the crate that owns the wire format,
    /// rather than guessed wherever it is next needed.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::AmountNotU128`] when it is not a decimal `uint128`. A leading `+`, a sign,
    /// whitespace, or anything above `340282366920938463463374607431768211455` is refused.
    pub fn amount_base_units(&self) -> Result<u128, ProtocolError> {
        if self.amount.is_empty() || !self.amount.bytes().all(|b| b.is_ascii_digit()) {
            return Err(ProtocolError::AmountNotU128 {
                got: self.amount.clone(),
            });
        }
        self.amount
            .parse::<u128>()
            .map_err(|_| ProtocolError::AmountNotU128 {
                got: self.amount.clone(),
            })
    }
}

impl PaymentPayload {
    /// The payload's kind.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::PayloadTypeUnknown`] when the discriminant is not one the scheme defines.
    pub fn kind(&self) -> Result<PayloadKind, ProtocolError> {
        PayloadKind::parse(&self.payload.payload_type)
    }

    /// Every rule this crate can check about a payment, in one call.
    ///
    /// # Errors
    ///
    /// [`ProtocolError::PayloadTypeUnknown`] for a discriminant the scheme does not define, and
    /// [`ProtocolError::PayloadTypeNotAccepted`] for `deposit` or `refund`: those are valid x402
    /// payloads at the wrong endpoint, so the code tells a client to use another route rather than
    /// to fix their payload. Plus whatever [`PaymentRequirements::validate`] returns for the
    /// echoed requirements.
    pub fn validate(&self) -> Result<Network, ProtocolError> {
        let kind = self.kind()?;
        if kind != PayloadKind::Voucher {
            return Err(ProtocolError::PayloadTypeNotAccepted {
                got: kind.as_str().to_owned(),
            });
        }
        self.accepted.validate()
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
