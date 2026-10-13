# ADR-0016: The Solidity port keeps Solady's operations, and the lint suppressions that follow

- Status: accepted
- Date: 2026-10-04
- Issue: #103
- Supersedes: nothing. Extends ADR-0008, which made the same choice for the Rust port.

## Context

`contracts/src/FixedPointMath.sol` is the third implementation of `expWad` and `lnWad` in this
repository. `engine/crates/lmsr` is the second, ported from Solady
`9fe23ffdcd395c4169396064226ed27206b222c3` under ADR-0008. `tools/reference/lmsr_ref.py` is the
first and the independent one, computed in 60-digit `mpmath`.

The vault does not price trades (`CLAUDE.md` section 1), so nothing in this library sits on a
trading path. It exists so the vault can never disagree with the engine that does, which is why
`PHASES.md` asks for it in Phase 3 even though the vault never calls it to price anything.

Writing it raised a choice that `forge lint` surfaced as eight warnings, and `forge build
--deny warnings` is what the `ci` and `deep` profiles run.

## Decision

**The port keeps Solady's operations, in Solady's order, and the lint findings at those lines are
suppressed inline with a reason.**

Two families:

1. **`unsafe-typecast`**, six times. Solady treats these values as raw 256-bit words and
   reinterprets between signed and unsigned without converting: `uint256(r)` before the final
   multiply, `uint256(x)` before the normalising shift, `int256(r)` where `r` is a bit index. Each
   is a reinterpretation of a value the algorithm has already bounded, not a narrowing. Adding a
   checked cast would add a branch no input inside the documented domain can reach, which is an
   untestable line and therefore a hole in branch coverage rather than a safety gain.
2. **`divide-before-multiply`**, once, at `p = LN_SCALE * p` immediately after `p = p / q`. The
   warning is right in general and wrong here: the division is part of the rational approximation
   and the scale factor is applied to its result by construction. Reordering them changes the
   output, which is the one thing this library must not do.

**What is not suppressed:** `>>` on a signed value stays an arithmetic shift, matching Solady's
`sar`, and is never rewritten as a division. Division truncates toward zero, an arithmetic shift
rounds toward negative infinity, and they differ for every negative intermediate.

## Consequences

Eight suppressions exist in one file, each on its own line with the reason beside it. A reader who
asks "why is a money-adjacent file silencing a truncation warning?" gets this answer at the line and
in full here.

The port departs from Solady in exactly two ways, both of which the Rust port already made and both
of which are documented in the library's own header: no assembly, and checked arithmetic instead of
the EVM's silent wrapping. Inside the documented domains these algorithms do not overflow, so a
revert from arithmetic marks a bug rather than a normal result.

**What holds this to account:** `contracts/test/FixedPointMath.diff.t.sol`. If a suppression ever
hides a real truncation, the committed vectors stop matching. That suite is the reason a suppression
here is a documented trade rather than a silenced alarm.

## Alternatives rejected

**Rewrite the algorithms in lint-clean Solidity.** Then the vault's math would be a fourth method
rather than a third port, and "three implementations agree" would stop being true. The whole value
of this file is that it is the same method.

**Vendor Solady as a dependency.** A new dependency is a `needs-jay` conversation
(`CLAUDE.md` section 2), and it would bring a library of which this repository needs two functions.
The Rust port made the same call for the same reason.

**Relax `deny = "warnings"` in `foundry.toml`.** That would silence the next real finding too. A
suppression that names its line and its reason is the narrower statement, and the narrower statement
wins.
