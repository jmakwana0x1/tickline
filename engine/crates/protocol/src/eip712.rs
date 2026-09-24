//! EIP-712 hashing: type hashes, the Tickline domain separator, and the signing digest (#62).
//!
//! Stub for the red commit; the implementation follows.

use alloy_primitives::{Address, B256};

/// The EIP-712 domain type, as it is hashed. Field order is part of the hash, so this string is
/// the definition and not a description of one.
pub const DOMAIN_TYPE: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

/// `keccak256` of an EIP-712 type string.
#[must_use]
pub fn type_hash(_type_string: &str) -> B256 {
    B256::ZERO
}

/// The EIP-712 domain every Tickline-owned signed struct is bound to.
///
/// `name` and `version` are fixed by [`crate::DOMAIN_NAME`] and [`crate::DOMAIN_VERSION`], so a
/// caller can vary only the two fields that separate one deployment from another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    chain_id: u64,
    verifying_contract: Address,
}

impl Domain {
    /// A domain for `verifying_contract` on `chain_id`.
    #[must_use]
    pub const fn new(chain_id: u64, verifying_contract: Address) -> Self {
        Self { chain_id, verifying_contract }
    }

    /// The chain this domain is bound to.
    #[must_use]
    pub const fn chain_id(&self) -> u64 {
        self.chain_id
    }

    /// The contract this domain is bound to.
    #[must_use]
    pub const fn verifying_contract(&self) -> Address {
        self.verifying_contract
    }

    /// `keccak256(abi.encode(typeHash, keccak(name), keccak(version), chainId, verifyingContract))`.
    #[must_use]
    pub fn separator(&self) -> B256 {
        B256::ZERO
    }

    /// The digest a signer signs: `keccak256(0x19 0x01 || separator || structHash)`.
    #[must_use]
    pub fn digest(&self, struct_hash: B256) -> B256 {
        let _ = struct_hash;
        B256::ZERO
    }
}

/// The digest for a domain separator that was computed elsewhere.
#[must_use]
pub fn digest(_domain_separator: B256, _struct_hash: B256) -> B256 {
    B256::ZERO
}
