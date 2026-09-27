import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { describe, expect, it } from 'vitest';

/**
 * The committed vector file, and the versions that produced it (issue #67, ADR-0015).
 *
 * `just vectors` proves the file matches the generator by requiring an empty `git diff`. That check
 * is blind to the case that matters: a dependency bump moves the generator's output and the
 * committed file together, so the diff stays empty while every digest changes. The versions are
 * therefore recorded inside the file, and this suite compares them against the installed tree.
 */

const HERE = dirname(fileURLToPath(import.meta.url));

interface Vectors {
  schema: number;
  provenance: { versions: Record<string, string>; generator: string };
  roles: { operator: string; agent: string };
  x402: { confirmed_by: string; channels: Array<{ config: Record<string, string> }> };
  receipt: { confirmed_by: string };
  market_id: { confirmed_by: string };
  envelope: { confirmed_by: string };
}

function vectors(): Vectors {
  return JSON.parse(
    readFileSync(resolve(HERE, '../../testdata/vectors/eip712.json'), 'utf8'),
  ) as Vectors;
}

function installed(pkg: string): string {
  const manifest = resolve(HERE, '..', 'node_modules', pkg, 'package.json');
  return (JSON.parse(readFileSync(manifest, 'utf8')) as { version: string }).version;
}

describe('the committed vectors', () => {
  it('records the versions that produced it, and they are the installed ones', () => {
    const { versions } = vectors().provenance;
    expect(Object.keys(versions).sort()).toEqual(['@x402/core', '@x402/evm', 'viem']);
    for (const [pkg, recorded] of Object.entries(versions)) {
      expect(recorded, `${pkg} in the vector file`).toBe(installed(pkg));
    }
  });

  it('pins the SDK exactly, with no range operator', () => {
    const manifest = JSON.parse(
      readFileSync(resolve(HERE, '../package.json'), 'utf8'),
    ) as { dependencies: Record<string, string> };
    for (const [pkg, spec] of Object.entries(manifest.dependencies)) {
      expect(spec, `${pkg} must be pinned exactly (Q7)`).toMatch(/^\d+\.\d+\.\d+$/);
    }
  });

  it('says what confirmed each section', () => {
    // ADR-0015: the SDK has an opinion on the x402 envelopes and none on our own types, so a
    // reader must not have to guess which sections it validated.
    const v = vectors();
    expect(v.x402.confirmed_by).toBe('deployed-contract');
    expect(v.envelope.confirmed_by).toBe('spec');
    expect(v.receipt.confirmed_by).toBe('ours');
    expect(v.market_id.confirmed_by).toBe('ours');
  });

  it('has the agent paying and the operator receiving', () => {
    // A semantic assertion, because no digest can make one: getChannelId confirms a reversed
    // config exactly as happily (CLAUDE.md section 5).
    const v = vectors();
    expect(v.roles.operator).not.toBe(v.roles.agent);
    for (const channel of v.x402.channels) {
      expect(channel.config.payer).toBe(v.roles.agent);
      expect(channel.config.payer_authorizer).toBe(v.roles.agent);
      expect(channel.config.receiver).toBe(v.roles.operator);
      expect(channel.config.receiver_authorizer).toBe(v.roles.operator);
    }
  });
});

describe('the invalid vectors', () => {
  it('names a code for every rejection, and they are ADR-0012 codes', () => {
    const v = JSON.parse(
      readFileSync(resolve(HERE, '../../testdata/vectors/eip712.json'), 'utf8'),
    ) as {
      invalid: {
        codes: string[];
        signatures: Array<{ code: string; signature: string; why: string }>;
        wrong_signer: { code: string };
        channel_config: { code: string };
      };
    };

    // The flat list and the entries are the same list: Solidity reads the flat one because it
    // cannot do JSONPath wildcards, and a drift between them would silently shrink its coverage.
    expect(v.invalid.codes).toEqual(v.invalid.signatures.map((s) => s.code));

    const all = [
      ...v.invalid.codes,
      v.invalid.wrong_signer.code,
      v.invalid.channel_config.code,
    ];
    expect(new Set(all).size).toBe(all.length);
    for (const code of all) {
      expect(code, 'every code belongs to a reserved family').toMatch(/^(SIG_|X402_|POLICY_|ENV_)/);
    }

    // Every ADR-0012 signature rule is covered, by name, so a rule cannot be dropped quietly.
    expect(v.invalid.codes).toEqual([
      'SIG_COMPACT',
      'SIG_LENGTH',
      'SIG_RECOVERY_ID',
      'SIG_R_ZERO',
      'SIG_S_ZERO',
      'SIG_R_ABOVE_ORDER',
      'SIG_S_ABOVE_ORDER',
      'SIG_HIGH_S',
      'SIG_UNRECOVERABLE',
    ]);
  });

  it('carries a signature that is not 65 bytes for the length cases', () => {
    const v = JSON.parse(
      readFileSync(resolve(HERE, '../../testdata/vectors/eip712.json'), 'utf8'),
    ) as { invalid: { signatures: Array<{ code: string; signature: string }> } };

    const bytesOf = (hex: string): number => (hex.length - 2) / 2;
    const byCode = new Map(v.invalid.signatures.map((s) => [s.code, s.signature]));

    expect(bytesOf(byCode.get('SIG_COMPACT') ?? '0x')).toBe(64);
    expect(bytesOf(byCode.get('SIG_LENGTH') ?? '0x')).toBe(66);
    for (const code of ['SIG_R_ZERO', 'SIG_S_ZERO', 'SIG_HIGH_S', 'SIG_UNRECOVERABLE']) {
      expect(bytesOf(byCode.get(code) ?? '0x'), `${code} is a well-formed length`).toBe(65);
    }
  });
});
