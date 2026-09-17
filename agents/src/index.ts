/**
 * x402 client agents.
 *
 * Phase 2 adds the EIP-712 vector generator (`src/vectors/generate.ts`), Phase 6 the creator,
 * forecaster, and reader agents. Phase 0 ships only what proves the harness runs.
 *
 * Amounts are always `bigint` USDC base units. Floats never touch money here; the eslint
 * config bans `Math` for exactly this reason.
 */

/** USDC has six decimals on every chain Tickline targets. */
export const USDC_DECIMALS = 6n;

/** One whole USDC, in base units. */
export const ONE_USDC = 10n ** USDC_DECIMALS;

/**
 * Convert whole USDC to base units.
 *
 * @throws if `whole` is negative. A negative payment is always a bug, never a refund.
 */
export function usdc(whole: bigint): bigint {
  if (whole < 0n) throw new RangeError(`amount must be non-negative, got ${whole.toString()}`);
  return whole * ONE_USDC;
}
