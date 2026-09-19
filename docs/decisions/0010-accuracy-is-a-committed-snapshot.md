# ADR-0010: Accuracy is a committed snapshot, not a fraction of the bound

- **Status:** accepted (Jay, on #50)
- **Date:** 2026-09-19
- **Phase:** 1
- **Invariants touched:** none directly; this is how I2, I3 and I15's numerical inputs are guarded

## Context

ADR-0009 derives `E(b)` and asserts `|cost - exact| <= E(b)`. To stop that bound drifting into
meaninglessness, it also asserted **tightness**: the measured worst error had to be at most half
the bound.

That check turned out to measure the wrong thing. The derivation allows `ln` 4 wei where Solady
delivers about 2, so the slack is structurally about 2x and the measured ratio sits at 0.481,
just under the 0.5 line. Two consequences:

- the assertion says how conservative the derivation is, not whether the implementation changed;
- it cannot see a one-wei regression, because one wei does not move a ratio near 0.5.

## Options

| Option | Cost | What it does |
|---|---|---|
| Widen the ratio to 0.75 | free | the same blind spot, one line further away |
| Retune the derivation until the ratio looks good | free | fits the bound to the implementation, which is what ADR-0009 forbids |
| **Commit the measured worst case and assert it exactly** | a file to regenerate when it legitimately moves | any change in behaviour, down to one wei, fails with the case that produced it |

## Decision

**`testdata/vectors/error-baseline.json` is a snapshot of the worst observed error per function**,
asserted exactly. The inputs are committed and the arithmetic is integer and deterministic, so
each worst case is a fixed number. This is `forge snapshot --check` applied to accuracy.

- Entries carry the case that produced them, so a change points straight at the input.
- `just update-error-baseline` regenerates the file. Doing so is an ordinary commit, and the
  reviewer sees the number move and reads why.
- **A baseline that rises by more than 2x, or any case above its bound, is a `needs-jay`.**
- The ratio survives only as a vacuity guard, `cost_error_bound_is_not_vacuous`, at 0.75. It
  fails only if the error climbs to within a quarter of the bound, at which point the bound has
  stopped being a meaningful ceiling. The snapshot is what detects drift.

`exp_wad` and `ln_wad` are snapshotted too: their 1-wei and 2-wei bounds are measured properties
of Solady's approximations, which is exactly the case a snapshot is for.

### What the file records

| Entry | What it catches |
|---|---|
| `cost` | the largest error in wei, which lives at `B_MAX` where the bound is largest |
| `cost_closest_to_bound` | the case that comes closest to its own bound, which lives at small `b`; also what the vacuity guard reads |
| `price` | worst error in wei |
| `exp_wad_non_positive` | the leaf bound the cost derivation rests on |
| `exp_wad_positive` | the rest of `exp`'s domain, where the error is large in absolute terms and tiny relative to the value |
| `ln_wad` | the other leaf bound |

Two cost entries because they catch different regressions: a change at either end of the `b` range
moves one of them.

## Consequences

A one-wei change in any function fails the suite with both numbers and the input, which is
strictly more informative than a ratio. Verified by planting `16767340` in place of the measured
`16767341`: the test failed and printed both values and the case.

The cost is a file that must be regenerated deliberately, and the discipline that regenerating it
is never a reflex. That discipline is written into the test's failure message and into
`docs/GITHUB.md`'s definition of done via `CLAUDE.md` section 2: weakening a test is a `needs-jay`
conversation, and so is a baseline that jumps.

## How this is enforced

`engine/crates/lmsr/tests/error_baseline.rs`, which runs in `just test-rust` and therefore in the
`rust` job and in every gate.
