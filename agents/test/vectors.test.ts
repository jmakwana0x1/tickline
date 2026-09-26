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
