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
    const manifest = JSON.parse(readFileSync(resolve(HERE, '../package.json'), 'utf8')) as {
      dependencies: Record<string, string>;
    };
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

    const all = [...v.invalid.codes, v.invalid.wrong_signer.code, v.invalid.channel_config.code];
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

describe('the rule order', () => {
  it('is stated in the file, and is not the published code order', () => {
    // SIGNATURE_ERROR_CODES is a published set whose array order is the enum's declaration order.
    // The order the rules are *checked* in is different, and a reader could take one for the other,
    // so both live in the file and this asserts they differ (Jay, on #80).
    const v = JSON.parse(
      readFileSync(resolve(HERE, '../../testdata/vectors/eip712.json'), 'utf8'),
    ) as {
      invalid: {
        codes: string[];
        check_order: string[];
        precedence: Array<{ code: string; violations: string[]; signature: string }>;
        precedence_codes: string[];
      };
    };

    expect(v.invalid.check_order).toEqual([
      'SIG_COMPACT',
      'SIG_LENGTH',
      'SIG_RECOVERY_ID',
      'SIG_R_ZERO',
      'SIG_R_ABOVE_ORDER',
      'SIG_S_ZERO',
      'SIG_S_ABOVE_ORDER',
      'SIG_HIGH_S',
    ]);

    // Every ordered rule is a published code, and exactly one published code is NOT in the order:
    // SIG_UNRECOVERABLE is decided after every syntactic check has passed, by recovery itself, so it
    // has no position among them. Naming it here is the point; a plain set comparison would have
    // hidden why the two lists differ in length.
    for (const code of v.invalid.check_order) {
      expect(v.invalid.codes).toContain(code);
    }
    const outsideTheOrder = v.invalid.codes.filter((c) => !v.invalid.check_order.includes(c));
    expect(outsideTheOrder).toEqual(['SIG_UNRECOVERABLE']);
    expect(v.invalid.check_order).not.toEqual(v.invalid.codes);
  });

  it('is exercised by cases that break more than one rule', () => {
    const v = JSON.parse(
      readFileSync(resolve(HERE, '../../testdata/vectors/eip712.json'), 'utf8'),
    ) as {
      invalid: {
        check_order: string[];
        precedence: Array<{ code: string; violations: string[]; signature: string }>;
        precedence_codes: string[];
      };
    };

    expect(v.invalid.precedence_codes).toEqual(v.invalid.precedence.map((c) => c.code));
    for (const entry of v.invalid.precedence) {
      // A single-violation case says nothing about order, which is why every other invalid vector
      // could not catch a reordering.
      expect(
        entry.violations.length,
        `${entry.code} must break more than one rule`,
      ).toBeGreaterThan(1);
      expect(v.invalid.check_order).toContain(entry.code);
    }

    // Each case's code must be the earliest of the rules it breaks, which is the claim being made.
    const position = (code: string): number => v.invalid.check_order.indexOf(code);
    expect(position('SIG_COMPACT')).toBeLessThan(position('SIG_R_ZERO'));
    expect(position('SIG_R_ZERO')).toBeLessThan(position('SIG_S_ZERO'));
    expect(position('SIG_R_ABOVE_ORDER')).toBeLessThan(position('SIG_S_ZERO'));
    expect(position('SIG_RECOVERY_ID')).toBeLessThan(position('SIG_HIGH_S'));
  });
});
