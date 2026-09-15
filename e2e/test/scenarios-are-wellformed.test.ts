import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';
import { parse } from 'yaml';

const DIR = new URL('../scenarios', import.meta.url).pathname;

/**
 * Phase 0 smoke test. It runs today against the scenario stubs and keeps running as Phase 6
 * fills them in: a scenario that forgets its expected balances fails here rather than passing
 * vacuously in the harness.
 */
describe('scenario files', () => {
  const files = readdirSync(DIR).filter((f) => f.endsWith('.yaml'));

  it('exist', () => {
    expect(files.length).toBeGreaterThan(0);
  });

  it.each(files)('%s declares a seed and what it proves', (file) => {
    const doc = parse(readFileSync(join(DIR, file), 'utf8')) as Record<string, unknown>;
    expect(typeof doc['name']).toBe('string');
    expect(typeof doc['seed']).toBe('number');
    expect(typeof doc['proves']).toBe('string');
  });
});
