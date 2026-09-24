//! EIP-712 hashing: type hashes, the Tickline domain separator, and the signing digest (#62).
//!
//! The EVM is the reference implementation, not this module: the tests diff every value here
//! against `cast`, and from #67 against a committed vector file that Solidity and TypeScript read
//! as well. Nothing here is clever, and that is deliberate.

use alloy_primitives::{keccak256, Address, B256, U256};

use crate::{DOMAIN_NAME, DOMAIN_VERSION};

/// The EIP-712 domain type, as it is hashed. Field order is part of the hash, so this string is
/// the definition and not a description of one.
pub const DOMAIN_TYPE: &str =
    "EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";

/// `keccak256` of an EIP-712 type string.
///
/// The argument is the encoded type: the primary type's signature followed by its referenced
/// types in alphabetical order, with no whitespace. Building that string is the caller's job,
/// because getting it wrong must be visible in a constant rather than hidden in a helper.
#[must_use]
pub fn type_hash(type_string: &str) -> B256 {
    keccak256(type_string.as_bytes())
}

/// The EIP-712 domain every Tickline-owned signed struct is bound to.
///
/// `name` and `version` come from the constructor, not from the caller: [`Domain::new`] is the
/// Tickline domain and [`Domain::x402`] the escrow's, and nothing can invent a third. A caller
/// varies only the two fields that separate one deployment from another, and both matter: without
/// them a receipt signed for one chain or one vault would verify against another (the same
/// reasoning as D3's `MarketId` tag, #59).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Domain {
    name: &'static str,
    version: &'static str,
    chain_id: u64,
    verifying_contract: Address,
}

impl Domain {
    /// The Tickline domain for `verifying_contract` (the vault) on `chain_id`.
    #[must_use]
    pub const fn new(chain_id: u64, verifying_contract: Address) -> Self {
        Self {
            name: DOMAIN_NAME,
            version: DOMAIN_VERSION,
            chain_id,
            verifying_contract,
        }
    }

    /// The x402 batch-settlement domain for the escrow at `verifying_contract` on `chain_id`.
    ///
    /// A separate domain, not a variant of ours: the escrow binds its own name and version, and
    /// `eip712Domain()` on the deployment is what those constants are checked against (#63).
    #[must_use]
    pub const fn x402(chain_id: u64, verifying_contract: Address) -> Self {
        Self {
            name: crate::x402::DOMAIN_NAME,
            version: crate::x402::DOMAIN_VERSION,
            chain_id,
            verifying_contract,
        }
    }

    /// The domain name this separator hashes.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        self.name
    }

    /// The domain version this separator hashes.
    #[must_use]
    pub const fn version(&self) -> &'static str {
        self.version
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
    ///
    /// `abi.encode` pads every value to 32 bytes, so the encoding is five words. Strings are
    /// hashed rather than embedded, which is what makes the result fixed width.
    #[must_use]
    pub fn separator(&self) -> B256 {
        let mut encoded = Vec::with_capacity(160);
        encoded.extend_from_slice(type_hash(DOMAIN_TYPE).as_slice());
        encoded.extend_from_slice(keccak256(self.name.as_bytes()).as_slice());
        encoded.extend_from_slice(keccak256(self.version.as_bytes()).as_slice());
        encoded.extend_from_slice(&U256::from(self.chain_id).to_be_bytes::<32>());
        encoded.extend_from_slice(
            B256::left_padding_from(self.verifying_contract.as_slice()).as_slice(),
        );
        keccak256(&encoded)
    }

    /// The digest a signer signs: `keccak256(0x19 0x01 || separator || structHash)`.
    #[must_use]
    pub fn digest(&self, struct_hash: B256) -> B256 {
        digest(self.separator(), struct_hash)
    }
}

/// The digest for a domain separator that was computed elsewhere.
///
/// `0x19` is EIP-191's "this is not a transaction" prefix and `0x01` is EIP-712's version byte.
/// Both are part of what stops a signed struct from being replayed as something else.
#[must_use]
pub fn digest(domain_separator: B256, struct_hash: B256) -> B256 {
    let mut preimage = Vec::with_capacity(66);
    preimage.extend_from_slice(&[0x19, 0x01]);
    preimage.extend_from_slice(domain_separator.as_slice());
    preimage.extend_from_slice(struct_hash.as_slice());
    keccak256(&preimage)
}
