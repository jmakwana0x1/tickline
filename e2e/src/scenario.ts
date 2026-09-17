/**
 * Scenario file schema.
 *
 * A scenario is declarative on purpose: the harness is the only thing that knows how to boot
 * anvil, deploy, migrate, and drive agents, so a new scenario is data, not code. Every
 * scenario asserts each participant's **final USDC balance to the base unit**. A scenario
 * that only checks "it didn't throw" tells us nothing (PHASES.md phase 6).
 *
 * Phase 6 implements the runner. Phase 0 defines the shape so scenarios can be written and
 * reviewed alongside the phases that produce them.
 */

/** A participant's expected end state, in USDC base units. */
export interface ExpectedBalance {
  /** Account label, resolved to an address by the harness. */
  readonly account: string;
  /** Exact expected balance. No tolerance: to the base unit or it is a bug. */
  readonly usdc: bigint;
}

/** One declarative end-to-end scenario. */
export interface Scenario {
  readonly name: string;
  /** Seed for every agent's randomness. Fixed, so a failure is reproducible. */
  readonly seed: number;
  /** What this scenario is here to prove. Shown on failure. */
  readonly proves: string;
  readonly expect: {
    readonly balances: readonly ExpectedBalance[];
    /** Ledger-versus-chain drift permitted at the end. Always zero (I13). */
    readonly drift: 0n;
  };
}
