/**
 * Generates `testdata/vectors/eip712.json`, the cross-stack vector file (issue #67).
 *
 * Rust, Solidity, and TypeScript all read this file and must agree with it byte for byte. That
 * makes the generator the one place where a value is decided, so it is deliberately boring:
 *
 * - **Deterministic.** Fixed keys, fixed inputs, no clock, no randomness. `just vectors-eip712`
 *   must leave an empty `git diff`, and a timestamp would break that on every run.
 * - **Provenance per entry.** Every entry says what confirmed it (ADR-0015): `deployed-contract`,
 *   `sdk`, `spec`, or `ours`. The SDK has an opinion on the x402 envelopes and none at all on
 *   `PositionReceipt` or `MarketId`, so a reader must not have to guess which.
 * - **The oracle's version is inside the file.** `just vectors` requires an empty diff, and that
 *   check is blind to a dependency bump moving the generator and the file together. The versions
 *   are recorded here and asserted against the installed tree by `agents/test/vectors.test.ts`.
 *
 * The keys are anvil's, public test material, allowlisted in `.gitleaks.toml`. No key is written
 * into the output: only addresses and signatures.
 */

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { BATCH_SETTLEMENT_DOMAIN, claimBatchTypes, voucherTypes } from '@x402/evm';
import { computeChannelId } from '@x402/evm/batch-settlement/client';
import {
  encodeAbiParameters,
  hashDomain,
  hashStruct,
  hashTypedData,
  keccak256,
  parseAbiParameters,
  recoverAddress,
  stringToHex,
  toBytes,
  type Address,
  type Hex,
} from 'viem';
import { privateKeyToAccount } from 'viem/accounts';

/** Schema version of the file. Bumping it is a breaking change for all three readers. */
const SCHEMA = 1;

/** Base Sepolia, the network Phase 8 deploys to. */
const CHAIN_ID = 84532;

/** The canonical x402 batch-settlement escrow, identical on every supported chain. */
const ESCROW: Address = '0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003';

/** Anvil's first deterministic deployment address, standing in for the vault. */
const VAULT: Address = '0x5FbDB2315678afecb367f032d93F642f64180aa3';

/** Base Sepolia USDC. */
const TOKEN: Address = '0x036CbD53842c5426634e7929541eC2318f3dCF7e';

/**
 * anvil accounts 0 and 1. Public test material by design, and allowlisted in `.gitleaks.toml`;
 * the operator key signs receipts, the agent key signs vouchers.
 */
const OPERATOR_KEY: Hex = '0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80';
const AGENT_KEY: Hex = '0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d';

/** What confirmed a value, per ADR-0015. */
type ConfirmedBy = 'deployed-contract' | 'sdk' | 'spec' | 'ours';

/** This package's own directory, which every relative path below is resolved against. */
const HERE = dirname(fileURLToPath(import.meta.url));

/**
 * The version actually installed, read from the manifest in `node_modules`.
 *
 * Not through `require('<pkg>/package.json')`: neither viem nor the SDK exports its manifest, and a
 * version the build could not read is a version this file could claim falsely.
 */
function installedVersion(pkg: string): string {
  const manifest = resolve(HERE, '../..', 'node_modules', pkg, 'package.json');
  const parsed = JSON.parse(readFileSync(manifest, 'utf8')) as { version?: string };
  if (typeof parsed.version !== 'string') {
    throw new Error(`no version in ${manifest}`);
  }
  return parsed.version;
}

/** The Tickline EIP-712 domain: ours, so nothing external has an opinion on it. */
const TICKLINE_DOMAIN = {
  name: 'Tickline',
  version: '1',
  chainId: CHAIN_ID,
  verifyingContract: VAULT,
} as const;

const POSITION_RECEIPT_TYPES = {
  PositionReceipt: [
    { name: 'marketId', type: 'bytes32' },
    { name: 'agent', type: 'address' },
    { name: 'yesShares', type: 'uint128' },
    { name: 'noShares', type: 'uint128' },
    { name: 'costPaid', type: 'uint128' },
    { name: 'feesPaid', type: 'uint128' },
    { name: 'nonce', type: 'uint64' },
    { name: 'epoch', type: 'uint32' },
  ],
} as const;

/** `"Tickline MarketId v1"` as a right-padded `bytes32` literal (ADR-0013). */
const MARKET_ID_TAG = stringToHex('Tickline MarketId v1', { size: 32 });

function marketId(params: {
  creator: Address;
  templateId: Hex;
  templateParamsHash: Hex;
  deadline: bigint;
  b: bigint;
  epochLength: number;
  salt: Hex;
}): Hex {
  return keccak256(
    encodeAbiParameters(
      parseAbiParameters(
        'bytes32, uint256, address, address, bytes32, bytes32, uint64, uint128, uint32, bytes32',
      ),
      [
        MARKET_ID_TAG,
        BigInt(CHAIN_ID),
        VAULT,
        params.creator,
        params.templateId,
        params.templateParamsHash,
        params.deadline,
        params.b,
        params.epochLength,
        params.salt,
      ],
    ),
  );
}

function pythThresholdParamsHash(priceId: Hex, threshold: bigint, direction: 0 | 1): Hex {
  return keccak256(
    encodeAbiParameters(parseAbiParameters('bytes32, int64, uint8'), [priceId, threshold, direction]),
  );
}

/** A receipt as the hasher wants it: every integer a bigint, including the narrow ones. */
interface ReceiptFields {
  marketId: Hex;
  agent: Address;
  yesShares: bigint;
  noShares: bigint;
  costPaid: bigint;
  feesPaid: bigint;
  nonce: bigint;
  /** `uint32`: viem types every uint up to 48 bits as a number, not a bigint. */
  epoch: number;
}

/** The EIP-712 domain type, as it is hashed. Field order is part of the hash. */
const DOMAIN_TYPE =
  'EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)';

/** secp256k1's group order and its half, from SEC 2 section 2.4.1. Written out, never computed. */
const CURVE_ORDER = '0xfffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141';
const HALF_CURVE_ORDER = '0x7fffffffffffffffffffffffffffffff5d576e7357a4501ddfe92f46681b20a0';

/** `uint128`, `uint64` and `uint32` ceilings, written out rather than computed (CLAUDE.md §5). */
const U128_MAX = '340282366920938463463374607431768211455';
const U64_MAX = '18446744073709551615';
const U32_MAX = '4294967295';

const X402_TYPE_STRINGS = {
  ChannelConfig:
    'ChannelConfig(address payer,address payerAuthorizer,address receiver,address receiverAuthorizer,address token,uint40 withdrawDelay,bytes32 salt)',
  Voucher: 'Voucher(bytes32 channelId,uint128 maxClaimableAmount)',
  ClaimEntry: 'ClaimEntry(bytes32 channelId,uint128 maxClaimableAmount,uint128 totalClaimed)',
  ClaimBatch:
    'ClaimBatch(ClaimEntry[] claims)ClaimEntry(bytes32 channelId,uint128 maxClaimableAmount,uint128 totalClaimed)',
} as const;

const X402_GETTERS = {
  ChannelConfig: 'CHANNEL_CONFIG_TYPEHASH()',
  Voucher: 'VOUCHER_TYPEHASH()',
  ClaimEntry: 'CLAIM_ENTRY_TYPEHASH()',
  ClaimBatch: 'CLAIM_BATCH_TYPEHASH()',
} as const;

const CHANNEL_CONFIG_TYPES = {
  ChannelConfig: [
    { name: 'payer', type: 'address' },
    { name: 'payerAuthorizer', type: 'address' },
    { name: 'receiver', type: 'address' },
    { name: 'receiverAuthorizer', type: 'address' },
    { name: 'token', type: 'address' },
    { name: 'withdrawDelay', type: 'uint40' },
    { name: 'salt', type: 'bytes32' },
  ],
} as const;

const CLAIM_ENTRY_TYPES = {
  ClaimEntry: [
    { name: 'channelId', type: 'bytes32' },
    { name: 'maxClaimableAmount', type: 'uint128' },
    { name: 'totalClaimed', type: 'uint128' },
  ],
} as const;

/** A CAIP-2 network id. A helper rather than a template literal, so the conversion is deliberate. */
function caip2(chainId: number): string {
  return `eip155:${chainId.toString()}`;
}

/** The type hash of a type string: `keccak256` of the encoded type, nothing more. */
function typeHash(typeString: string): Hex {
  return keccak256(toBytes(typeString));
}

/** A Tickline domain separator for an arbitrary chain and vault, so the variants can be generated. */
function ticklineSeparator(chainId: number, verifyingContract: Address): Hex {
  return hashDomain({
    domain: { name: 'Tickline', version: '1', chainId: BigInt(chainId), verifyingContract },
    types: {
      EIP712Domain: [
        { name: 'name', type: 'string' },
        { name: 'version', type: 'string' },
        { name: 'chainId', type: 'uint256' },
        { name: 'verifyingContract', type: 'address' },
      ],
    },
  });
}

/** The escrow's own domain separator, from the SDK's domain plus this chain and that address. */
function escrowSeparator(): Hex {
  return hashDomain({
    domain: {
      name: BATCH_SETTLEMENT_DOMAIN.name,
      version: BATCH_SETTLEMENT_DOMAIN.version,
      chainId: BigInt(CHAIN_ID),
      verifyingContract: ESCROW,
    },
    types: {
      EIP712Domain: [
        { name: 'name', type: 'string' },
        { name: 'version', type: 'string' },
        { name: 'chainId', type: 'uint256' },
        { name: 'verifyingContract', type: 'address' },
      ],
    },
  });
}

/** `keccak256(0x1901 || separator || structHash)`, spelled out so the framing is visible. */
function digestOf(separator: Hex, structHash: Hex): Hex {
  return keccak256(`0x1901${separator.slice(2)}${structHash.slice(2)}` as Hex);
}

async function main(): Promise<void> {
  const out = parseOutPath();
  const operator = privateKeyToAccount(OPERATOR_KEY);
  const agent = privateKeyToAccount(AGENT_KEY);

  // ---------------------------------------------------------------- the Tickline domain (ours)

  const separator = ticklineSeparator(CHAIN_ID, VAULT);
  const structHashFixture = keccak256(toBytes('tickline structHash fixture'));

  // ---------------------------------------------------------------- raw signatures, both parities
  //
  // Two digests are needed whose signatures differ in `v`, so both recovery ids are exercised. They
  // are found by walking a counter rather than by trying values by hand, which keeps the generator
  // deterministic and records which index was used.
  const parities = await findBothParities(operator);

  // ---------------------------------------------------------------- x402 (the escrow's types)

  const x402Domain = {
    ...BATCH_SETTLEMENT_DOMAIN,
    chainId: CHAIN_ID,
    verifyingContract: ESCROW,
  } as const;

  // Direction matters and is asserted by name, not by hash: the AGENT pays and the OPERATOR
  // receives. A swapped config hashes just as happily, and every digest below would still agree
  // with itself (CLAUDE.md §5).
  const channelConfig = {
    payer: agent.address,
    payerAuthorizer: agent.address,
    receiver: operator.address,
    receiverAuthorizer: operator.address,
    token: TOKEN,
    withdrawDelay: 3600,
    salt: '0x0000000000000000000000000000000000000000000000000000000000000001' as Hex,
  };
  const secondConfig = {
    ...channelConfig,
    salt: '0x0000000000000000000000000000000000000000000000000000000000000002' as Hex,
  };

  const channelId = computeChannelId(channelConfig, CHAIN_ID);
  const secondChannelId = computeChannelId(secondConfig, CHAIN_ID);

  const voucher = { channelId, maxClaimableAmount: 1_000_000n };
  const voucherDigest = hashTypedData({
    domain: x402Domain,
    types: voucherTypes,
    primaryType: 'Voucher',
    message: voucher,
  });
  const voucherSignature = await agent.signTypedData({
    domain: x402Domain,
    types: voucherTypes,
    primaryType: 'Voucher',
    message: voucher,
  });

  const claimEntries = [
    { name: 'partial', channelId, maxClaimableAmount: 1_000_000n, totalClaimed: 250_000n },
    {
      name: 'full',
      channelId: secondChannelId,
      maxClaimableAmount: 5_000_000n,
      totalClaimed: 5_000_000n,
    },
  ];
  const entryStructHashes = claimEntries.map((entry) =>
    hashStruct({
      data: {
        channelId: entry.channelId,
        maxClaimableAmount: entry.maxClaimableAmount,
        totalClaimed: entry.totalClaimed,
      },
      primaryType: 'ClaimEntry',
      types: CLAIM_ENTRY_TYPES,
    }),
  );
  const entriesRoot = keccak256(`0x${entryStructHashes.map((h) => h.slice(2)).join('')}`);
  const claims = claimEntries.map(({ channelId: id, maxClaimableAmount, totalClaimed }) => ({
    channelId: id,
    maxClaimableAmount,
    totalClaimed,
  }));
  const batchStructHash = hashStruct({
    data: { claims },
    primaryType: 'ClaimBatch',
    types: claimBatchTypes,
  });
  const claimBatchDigest = hashTypedData({
    domain: x402Domain,
    types: claimBatchTypes,
    primaryType: 'ClaimBatch',
    message: { claims },
  });
  const firstClaim = claims.at(0);
  if (firstClaim === undefined) throw new Error('the batch needs at least one entry');
  const singleBatchDigest = hashTypedData({
    domain: x402Domain,
    types: claimBatchTypes,
    primaryType: 'ClaimBatch',
    message: { claims: [firstClaim] },
  });

  // ---------------------------------------------------------------- MarketId (ours)

  const templateId = keccak256(toBytes('pyth-threshold-v1'));
  const priceId: Hex = '0xff61491a931112ddf1bd8147cd1b641375f79f5825126d665480874634fd0ace';
  const templateParamsHash = pythThresholdParamsHash(priceId, 400_000_000_000n, 0);
  const market = {
    creator: operator.address,
    templateId,
    templateParamsHash,
    deadline: 1_790_035_200n,
    b: 1_000_000_000_000_000_000_000n,
    epochLength: 3600,
    salt: '0x0000000000000000000000000000000000000000000000000000000000000001' as Hex,
  };
  const baseMarketId = marketId(market);

  const thresholds: Array<{ threshold: bigint; why?: string }> = [
    { threshold: 0n, why: 'Non-negative, so 24 bytes of 0x00.' },
    { threshold: 1n },
    { threshold: -1n, why: 'Negative, so 24 bytes of 0xff. Must not hash like u64::MAX.' },
    { threshold: 9_223_372_036_854_775_807n, why: 'i64::MAX, the largest accepted value.' },
    { threshold: -9_223_372_036_854_775_808n, why: 'i64::MIN, the smallest.' },
  ];

  // ---------------------------------------------------------------- PositionReceipt (ours)

  const receipt: ReceiptFields = {
    marketId: baseMarketId,
    agent: agent.address,
    yesShares: 1_500_000n,
    noShares: 0n,
    costPaid: 742_500n,
    feesPaid: 7_425n,
    nonce: 7n,
    epoch: 3,
  };
  const receiptStructHash = hashStruct({
    data: receipt,
    primaryType: 'PositionReceipt',
    types: POSITION_RECEIPT_TYPES,
  });
  const receiptDigest = digestOf(separator, receiptStructHash);
  const receiptSignature = await operator.signTypedData({
    domain: TICKLINE_DOMAIN,
    types: POSITION_RECEIPT_TYPES,
    primaryType: 'PositionReceipt',
    message: receipt,
  });

  const atMax: ReceiptFields = {
    marketId: baseMarketId,
    agent: agent.address,
    yesShares: BigInt(U128_MAX),
    noShares: BigInt(U128_MAX),
    costPaid: BigInt(U128_MAX),
    feesPaid: BigInt(U128_MAX),
    nonce: BigInt(U64_MAX),
    epoch: Number(U32_MAX),
  };
  const atMaxStructHash = hashStruct({
    data: atMax,
    primaryType: 'PositionReceipt',
    types: POSITION_RECEIPT_TYPES,
  });
  const atMaxDigest = digestOf(separator, atMaxStructHash);

  // The operator's ONE signature, checked against digests it was not made for. Each recovers a
  // stranger rather than failing, which is why "the recovered address is not zero" is never the
  // check (ADR-0012). A stranger is a function of the digest AND the signature together.
  const tampered: Array<{
    changed: string;
    from?: number;
    to?: number;
    receipt: ReceiptFields;
  }> = [
    { changed: 'epoch', from: 3, to: 4, receipt: { ...receipt, epoch: 4 } },
    { changed: 'nonce', from: 7, to: 8, receipt: { ...receipt, nonce: 8n } },
    { changed: 'every field', receipt: atMax },
  ];
  const tamperedCases = await Promise.all(
    tampered.map(async (entry) => {
      const structHash = hashStruct({
        data: entry.receipt,
        primaryType: 'PositionReceipt',
        types: POSITION_RECEIPT_TYPES,
      });
      const digest = digestOf(separator, structHash);
      const recovered = await recoverAddress({ hash: digest, signature: receiptSignature });
      return {
        changed: entry.changed,
        ...(entry.from === undefined ? {} : { from: entry.from }),
        ...(entry.to === undefined ? {} : { to: entry.to }),
        digest,
        recovers_to: recovered,
      };
    }),
  );

  // ---------------------------------------------------------------- the 402 envelopes (spec)

  const requirementsExtra = {
    receiverAuthorizer: operator.address,
    withdrawDelay: 3600,
    name: 'USDC',
    version: '2',
    minDeposit: '1000000',
  };
  const requirements = {
    scheme: 'batch-settlement',
    network: caip2(CHAIN_ID),
    amount: '100000',
    asset: TOKEN,
    payTo: operator.address,
    maxTimeoutSeconds: 3600,
    extra: requirementsExtra,
  };
  const plain402 = { x402Version: 2, accepts: [requirements] };
  const correctiveExtra = {
    receiverAuthorizer: operator.address,
    withdrawDelay: 3600,
    name: 'USDC',
    version: '2',
    channelState: {
      channelId,
      balance: '100000',
      totalClaimed: '500',
      withdrawRequestedAt: 0,
      refundNonce: '1',
      chargedCumulativeAmount: '3200',
    },
    voucherState: { signedMaxClaimable: '3200', signature: voucherSignature },
  };
  const corrective402 = {
    x402Version: 2,
    error: 'invalid_batch_settlement_evm_cumulative_amount_mismatch',
    accepts: [{ ...requirements, extra: correctiveExtra }],
  };
  const paymentPayload = {
    x402Version: 2,
    accepted: requirements,
    payload: {
      type: 'voucher',
      channelConfig,
      voucher: {
        channelId,
        maxClaimableAmount: voucher.maxClaimableAmount.toString(),
        signature: voucherSignature,
      },
    },
  };

  const vectors = {
    schema: SCHEMA,
    note:
      'Cross-stack EIP-712 vectors. Rust, Solidity and TypeScript must all agree with this file ' +
      'byte for byte. Every section names what confirmed it (ADR-0015): the x402 values were read ' +
      'off the deployed escrow and reproduced by the official SDK, while PositionReceipt and ' +
      'MarketId are ours and nothing external has an opinion on them. A confirmed digest proves an ' +
      'encoding of the input it was given, never that the input was right, so the roles are ' +
      'asserted by name too (CLAUDE.md section 5).',
    provenance: {
      generator: 'agents/src/vectors/generate.ts',
      deterministic: 'Fixed anvil keys and fixed inputs. No clock, no randomness, no timestamp.',
      versions: {
        viem: installedVersion('viem'),
        '@x402/core': installedVersion('@x402/core'),
        '@x402/evm': installedVersion('@x402/evm'),
      },
      versions_note:
        'Recorded because `just vectors` only proves the file matches the generator. A dependency ' +
        'bump moves both together, so agents/test/vectors.test.ts asserts these equal the ' +
        'installed tree (ADR-0015).',
      signers:
        'anvil accounts 0 (operator) and 1 (agent). Public test material; no key is written here.',
    },
    roles: {
      confirmed_by: 'ours' as ConfirmedBy,
      why:
        'The direction of a channel cannot be checked by a hash: getChannelId confirms a swapped ' +
        'config exactly as happily, and every digest would still agree with itself. So the parties ' +
        'are named here and asserted by name (CLAUDE.md section 5).',
      operator: operator.address,
      operator_role: 'signs receipts, receives on every channel, and claims',
      agent: agent.address,
      agent_role: 'signs vouchers, pays on every channel, and withdraws',
    },
    other_signer: agent.address,
    other_signer_note:
      'Signs vouchers but never a receipt, so it is the address a receipt must NOT verify against.',
    domain: {
      confirmed_by: 'ours' as ConfirmedBy,
      name: 'Tickline',
      version: '1',
      type_string: DOMAIN_TYPE,
      type_hash: typeHash(DOMAIN_TYPE),
      chain_id: CHAIN_ID,
      verifying_contract: VAULT,
      separator,
    },
    domain_variants: [
      {
        what_changed: 'chain_id',
        chain_id: 1,
        verifying_contract: VAULT,
        separator: ticklineSeparator(1, VAULT),
      },
      {
        what_changed: 'verifying_contract',
        chain_id: CHAIN_ID,
        verifying_contract: '0x000000000000000000000000000000000000dEaD',
        separator: ticklineSeparator(CHAIN_ID, '0x000000000000000000000000000000000000dEaD'),
      },
    ],
    digest: {
      confirmed_by: 'ours' as ConfirmedBy,
      struct_hash: structHashFixture,
      struct_hash_preimage: 'keccak256("tickline structHash fixture")',
      digest: digestOf(separator, structHashFixture),
    },
    signatures: parities.map((entry) => ({
      name: entry.name,
      preimage: entry.preimage,
      digest: entry.digest,
      signature: entry.signature,
      signer: operator.address,
    })),
    secp256k1: { source: 'SEC 2 section 2.4.1', n: CURVE_ORDER, half_n: HALF_CURVE_ORDER },
    x402: {
      confirmed_by: 'deployed-contract' as ConfirmedBy,
      confirmed_note:
        'Every digest below was read from the escrow on Base Sepolia on 2026-09-26 for exactly ' +
        'these inputs, and reproduced by the SDK. The getter is named per entry so one value can ' +
        'be rechecked with one cast call.',
      escrow: ESCROW,
      chain_id: CHAIN_ID,
      domain: {
        source: 'eip712Domain()',
        name: x402Domain.name,
        version: x402Domain.version,
        separator: escrowSeparator(),
      },
      type_hashes: (['ChannelConfig', 'Voucher', 'ClaimEntry', 'ClaimBatch'] as const).map(
        (name) => ({
          name,
          getter: X402_GETTERS[name],
          type_string: X402_TYPE_STRINGS[name],
          hash: typeHash(X402_TYPE_STRINGS[name]),
        }),
      ),
      channels: [
        {
          name: 'salt1',
          getter: 'getChannelId(ChannelConfig)',
          config: snakeConfig(channelConfig),
          struct_hash: hashStruct({
            data: channelConfig,
            primaryType: 'ChannelConfig',
            types: CHANNEL_CONFIG_TYPES,
          }),
          channel_id: channelId,
        },
        {
          name: 'salt2',
          getter: 'getChannelId(ChannelConfig)',
          config: snakeConfig(secondConfig),
          channel_id: secondChannelId,
        },
      ],
      vouchers: [
        {
          name: 'one_usdc',
          getter: 'getVoucherDigest(bytes32,uint128)',
          channel_id: channelId,
          max_claimable_amount: voucher.maxClaimableAmount.toString(),
          digest: voucherDigest,
          signature: voucherSignature,
          signer: agent.address,
        },
      ],
      claim_entries: claimEntries.map((entry, index) => {
        const structHash = entryStructHashes.at(index);
        if (structHash === undefined) throw new Error(`no struct hash for entry ${index.toString()}`);
        return {
          name: entry.name,
          channel_id: entry.channelId,
          max_claimable_amount: entry.maxClaimableAmount.toString(),
          total_claimed: entry.totalClaimed.toString(),
          struct_hash: structHash,
        };
      }),
      claim_batches: [
        {
          name: 'two_entries',
          getter: 'getClaimBatchDigest(VoucherClaim[])',
          entries: ['partial', 'full'],
          entries_root: entriesRoot,
          struct_hash: batchStructHash,
          digest: claimBatchDigest,
        },
        {
          name: 'one_entry',
          getter: 'getClaimBatchDigest(VoucherClaim[])',
          entries: ['partial'],
          digest: singleBatchDigest,
          why:
            'Kept only so a failure distinguishes wrong batch framing from wrong concatenation; ' +
            'the two-entry case is what Phase 5 signs.',
        },
      ],
      wire_struct: {
        note:
          'What getClaimBatchDigest takes, which is not the signed type: it carries the ' +
          'ChannelConfig and derives the channelId itself, and its third field is totalClaimed, ' +
          'cumulative, never a per-batch delta.',
        solidity:
          'VoucherClaim { Voucher { ChannelConfig config; uint128 maxClaimableAmount } voucher; ' +
          'bytes signature; uint128 totalClaimed }',
        selector: '0x488ccc3b',
      },
    },
    receipt: {
      confirmed_by: 'ours' as ConfirmedBy,
      note: 'Tickline\u2019s own type. The SDK has no opinion on it, and neither does the escrow.',
      type_string: receiptTypeString(),
      type_hash: typeHash(receiptTypeString()),
      widths: {
        note:
          'Written out rather than computed, because a test that recomputes a limit the way the ' +
          'code computes it agrees with itself (CLAUDE.md section 5). These are type boundaries: ' +
          'the domain boundary is Q_MAX, 1e12 shares or 1e18 base units, twenty orders lower.',
        u128_max: U128_MAX,
        u64_max: U64_MAX,
        u32_max: U32_MAX,
      },
      operator: operator.address,
      typical: {
        why:
          'A real fill: 1.5 YES shares for 0.7425 USDC plus 100 bps of fee (Qa on #1), at nonce 7 ' +
          'in epoch 3. The fee model is visible in the data rather than only in prose.',
        market_id: receipt.marketId,
        agent: receipt.agent,
        yes_shares: receipt.yesShares.toString(),
        no_shares: receipt.noShares.toString(),
        cost_paid: receipt.costPaid.toString(),
        fees_paid: receipt.feesPaid.toString(),
        nonce: receipt.nonce.toString(),
        epoch: receipt.epoch,
        struct_hash: receiptStructHash,
        digest: receiptDigest,
        signature: receiptSignature,
      },
      at_every_type_maximum: {
        why:
          'An ENCODING test and nothing more. Every field at its type maximum is well formed and ' +
          'economically impossible: Q_MAX caps shares far below this (Phase 4) and the vault ' +
          "solvency check caps payouts (Phase 3). The engine must never sign one.",
        market_id: atMax.marketId,
        agent: atMax.agent,
        yes_shares: U128_MAX,
        no_shares: U128_MAX,
        cost_paid: U128_MAX,
        fees_paid: U128_MAX,
        nonce: atMax.nonce.toString(),
        epoch: atMax.epoch,
        struct_hash: atMaxStructHash,
        digest: atMaxDigest,
      },
      tampered: {
        note:
          'The operator ONE signature from `typical`, checked against a digest it was not made ' +
          'for. Each recovers a stranger rather than failing, which is why "the recovered address ' +
          'is not zero" is never the check (ADR-0012). A stranger is a function of the digest AND ' +
          'the signature together: regenerate either and all of these move.',
        cases: tamperedCases,
      },
    },
    market_id: {
      confirmed_by: 'ours' as ConfirmedBy,
      note:
        'Not EIP-712: a plain keccak256 preimage, tagged and bound to the chain and the vault ' +
        '(ADR-0013), because the market id is the key every position, commit and claim is filed ' +
        'under.',
      tag: {
        value: MARKET_ID_TAG,
        as_string: 'Tickline MarketId v1',
        why:
          'The string as a bytes32 literal, right-padded. A preimage is what a human reads when a ' +
          'hash disagrees, and a literal shows as text in a cast dump where a keccak shows 32 ' +
          'opaque bytes.',
      },
      template: {
        name: 'pyth-threshold-v1',
        template_id: templateId,
        price_id: priceId,
        price_id_note:
          'Pyth ETH/USD. A fixture only; the feed a market uses is the creator choice.',
        threshold: '400000000000',
        threshold_note: 'int64 in the feed own exponent: 4000 USD at expo -8.',
        direction: 0,
        direction_note: 'uint8, 0 above and 1 below.',
        template_params_hash: templateParamsHash,
        threshold_cases: {
          note:
            'The signed boundaries. Zero is here because `threshold < 0` survived mutation to ' +
            '`== 0` and `<= 0`: nothing pinned the padding at zero.',
          cases: thresholds.map(({ threshold, why }) => ({
            threshold: threshold.toString(),
            params_hash: pythThresholdParamsHash(priceId, threshold, 0),
            ...(why === undefined ? {} : { why }),
          })),
        },
      },
      base: {
        chain_id: CHAIN_ID,
        vault: VAULT,
        creator: market.creator,
        template_id: market.templateId,
        template_params_hash: market.templateParamsHash,
        deadline: Number(market.deadline),
        b: market.b.toString(),
        epoch_length: market.epochLength,
        salt: market.salt,
        market_id: baseMarketId,
      },
      domain_variants: [
        {
          what_changed: 'chain_id',
          chain_id: 1,
          vault: VAULT,
          market_id: marketIdOn(1, VAULT, market),
          why:
            'Identical market parameters. A receipt signed for this market on one chain must not ' +
            'be valid on another.',
        },
        {
          what_changed: 'vault',
          chain_id: CHAIN_ID,
          vault: '0x000000000000000000000000000000000000dEaD',
          market_id: marketIdOn(CHAIN_ID, '0x000000000000000000000000000000000000dEaD', market),
          why:
            'Identical market parameters. A redeployed vault must not inherit the old market ids.',
        },
      ],
    },
    envelope: {
      confirmed_by: 'spec' as ConfirmedBy,
      note:
        'The x402 V2 wire format, quoted from scheme_batch_settlement_evm.md. The body is ' +
        'accepts[], not one requirement.',
      x402_version: 2,
      headers: {
        request: 'PAYMENT-SIGNATURE',
        response: 'PAYMENT-RESPONSE',
        why: 'V2 names, not V1 X-PAYMENT and X-PAYMENT-RESPONSE.',
      },
      networks: {
        note: 'CAIP-2, not a slug. base-sepolia is a V1 spelling and must be refused.',
        base_sepolia: caip2(CHAIN_ID),
        base_mainnet: 'eip155:8453',
        rejected: [
          'base-sepolia',
          'base',
          'eip155',
          'eip155:',
          'eip155:abc',
          ':84532',
          'eip155:84532:1',
          'eip155:+5',
          'eip155: 5',
          'eip155:5 ',
          'eip155:0',
          'EIP155:84532',
          'eip155:1e3',
        ],
        why_plus_is_listed:
          'u64::from_str accepts a leading plus, so eip155:+5 parses to 5 unless the digits are ' +
          'checked first. cargo-mutants found this by turning the digit check || into &&.',
        why_zero_is_listed: 'Chain id zero is a valid decimal and not a chain.',
      },
      payment_required: {
        why:
          'What the engine answers an unpaid request with. amount is the per-request ceiling the ' +
          'client turns into maxClaimableAmount, not a price.',
        required_extra: ['receiverAuthorizer', 'withdrawDelay', 'name', 'version'],
        optional_extra: ['assetTransferMethod', 'minDeposit', 'channelState', 'voucherState'],
        token_domain_note:
          'extra.name and extra.version are the TOKEN EIP-3009 domain, USDC and 2 here. They are ' +
          'NOT the x402 domain, x402 Batch Settlement version 1.',
        plain: { body: plain402, json: JSON.stringify(plain402) },
        corrective: {
          why:
            'The same body shape with error set and both states in the entry extra, which is what ' +
            'makes a corrective 402 a body rather than a special case.',
          error: 'invalid_batch_settlement_evm_cumulative_amount_mismatch',
          body: corrective402,
          json: JSON.stringify(corrective402),
        },
      },
      payment_payload: {
        why:
          'What a client sends in PAYMENT-SIGNATURE. It echoes the requirements it answers in ' +
          'accepted, so a server can check the client paid for what was advertised.',
        voucher: { body: paymentPayload, json: JSON.stringify(paymentPayload) },
        types: {
          accepted: ['voucher'],
          declined: ['deposit', 'refund'],
          unknown: ['claim', 'settle', 'Voucher', '', 'voucher '],
        },
      },
      strictness: {
        rule:
          'Leniency is allowed exactly where the bytes never enter a hash preimage and never ' +
          'enter state. extra qualifies. Nothing else does. (ADR-0014)',
        strict_objects: [
          'PaymentRequired',
          'PaymentRequirements',
          'PaymentPayload',
          'Payload',
          'channelConfig',
          'voucher',
        ],
        lenient_objects: ['extra', 'channelState', 'voucherState'],
      },
      amounts: {
        note:
          'amount is a decimal string on the wire and becomes maxClaimableAmount, which the escrow ' +
          'gives uint128. The ceiling is written out rather than computed.',
        u128_max: U128_MAX,
        accepted: ['0', '1', '100000', U128_MAX],
        rejected: [
          '',
          '340282366920938463463374607431768211456',
          '-1',
          '+1',
          ' 1',
          '1 ',
          '1e6',
          '0x10',
          '1_000',
          'abc',
        ],
      },
    },
  };

  writeFileSync(out, `${JSON.stringify(vectors, null, 2)}\n`);
  process.stdout.write(`wrote ${out}\n`);
}

/** The `PositionReceipt` type string, built from the same field list the digest uses. */
function receiptTypeString(): string {
  const fields = POSITION_RECEIPT_TYPES.PositionReceipt.map((f) => `${f.type} ${f.name}`).join(',');
  return `PositionReceipt(${fields})`;
}

/** The channel config with the snake_case keys the Rust reader expects. */
function snakeConfig(config: {
  payer: Address;
  payerAuthorizer: Address;
  receiver: Address;
  receiverAuthorizer: Address;
  token: Address;
  withdrawDelay: number;
  salt: Hex;
}): Record<string, string | number> {
  return {
    payer: config.payer,
    payer_authorizer: config.payerAuthorizer,
    receiver: config.receiver,
    receiver_authorizer: config.receiverAuthorizer,
    token: config.token,
    withdraw_delay: config.withdrawDelay,
    salt: config.salt,
  };
}

/** A market id on an arbitrary chain and vault, for the domain-separation variants. */
function marketIdOn(
  chainId: number,
  vault: Address,
  params: Parameters<typeof marketId>[0],
): Hex {
  return keccak256(
    encodeAbiParameters(
      parseAbiParameters(
        'bytes32, uint256, address, address, bytes32, bytes32, uint64, uint128, uint32, bytes32',
      ),
      [
        MARKET_ID_TAG,
        BigInt(chainId),
        vault,
        params.creator,
        params.templateId,
        params.templateParamsHash,
        params.deadline,
        params.b,
        params.epochLength,
        params.salt,
      ],
    ),
  );
}

/**
 * Two raw-digest signatures, one with `v` 27 and one with 28.
 *
 * Found by walking a counter rather than by hand, so the generator stays deterministic and the file
 * records which preimage produced each parity.
 */
async function findBothParities(
  account: ReturnType<typeof privateKeyToAccount>,
): Promise<Array<{ name: string; preimage: string; digest: Hex; signature: Hex }>> {
  const found = new Map<number, { name: string; preimage: string; digest: Hex; signature: Hex }>();
  for (let i = 0; i < 64 && found.size < 2; i += 1) {
    const preimage = `tickline signature fixture ${i.toString()}`;
    const digest = keccak256(toBytes(preimage));
    const signature = await account.sign({ hash: digest });
    const v = Number.parseInt(signature.slice(-2), 16);
    if (!found.has(v)) {
      found.set(v, { name: v === 27 ? 'v27' : 'v28', preimage, digest, signature });
    }
  }
  const v27 = found.get(27);
  const v28 = found.get(28);
  if (v27 === undefined || v28 === undefined) {
    throw new Error('could not find both signature parities in 64 attempts');
  }
  return [v27, v28];
}

function parseOutPath(): string {
  const flag = process.argv.indexOf('--out');
  if (flag === -1 || flag + 1 >= process.argv.length) {
    throw new Error('usage: generate.ts --out <path>');
  }
  const given = process.argv[flag + 1];
  if (given === undefined) throw new Error('--out needs a path');
  return resolve(HERE, '../..', given);
}

await main();
