import { describe, expect, it } from 'vitest';

import { ONE_USDC, usdc } from '../src/index.js';

/** Phase 0 smoke test: the vitest harness runs. Phase 6 adds the strategy suites. */
describe('usdc base units', () => {
  it('converts whole units without floating point', () => {
    expect(usdc(1n)).toBe(ONE_USDC);
    expect(usdc(0n)).toBe(0n);
    expect(usdc(1_000_000n)).toBe(1_000_000_000_000n);
  });

  it('refuses negative amounts', () => {
    expect(() => usdc(-1n)).toThrow(RangeError);
  });
});
