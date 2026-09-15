# ADR-0002: Invariants are numbered, and every test names its ID

- **Status:** accepted
- **Date:** 2026-09-15
- **Phase:** 0
- **Invariants touched:** all of them — this is how they are addressed

## Context

`PHASES.md` refers to invariants by bare ID — `I1`, `I7`, `I15` — in acceptance criteria,
Foundry invariant test names, and gate requirements, and says they are defined in `CLAUDE.md`
§4. Fifteen IDs span four languages and nine phases. Two failure modes are obvious in advance:

1. A test fails in Phase 5 and nobody can say which property broke, because the test is named
   after its mechanism rather than the property it protects.
2. An invariant is quietly weakened — a tolerance widened, an assertion dropped — because no
   single place says what it was supposed to be.

`PHASES.md` states I1–I3 and I7–I15 clearly enough to reconstruct. **I4, I5, and I6 it only
references** ("assert I4, I5, I6, I8"; "receipt fields match ledger (I5, I6)").

## Options

| Option | Cost | What it does to the invariants |
|---|---|---|
| Prose in each phase | Zero setup | The same property gets three wordings and drifts |
| A table in `CLAUDE.md` §4, IDs referenced by name in tests | One table to maintain | A failure points at a definition, not a mechanism |
| A machine-readable registry checked by CI | Real tooling to build | Stronger, and worth revisiting once the IDs stop moving |

## Decision

`CLAUDE.md` §4 is the single definition of every invariant, grouped by the layer that enforces
it. Every test that guards one **names the ID in its own name or in a doc comment** —
`invariant_I1_solvency`, `prop_i15_conversion_favours_the_vault` — so a red test points straight
back at the sentence it violated.

I4, I5, and I6 have been reconstructed from how `PHASES.md` uses them:

- **I4** — market state matches its fills;
- **I5** — receipts are faithful to the ledger;
- **I6** — receipts are monotonic and unique.

These three are **provisional** and flagged as open question Q2 in `docs/STATUS.md`. Phase 4
does not start until Jay confirms or corrects the wording, because Phase 4's property tests
assert them by name.

## Consequences

Renaming an invariant becomes a deliberate, repo-wide change — which is the point. The cost is
that test names get long. Long test names are good test names.

Reconstructing I4–I6 rather than stopping was a judgement call: the scaffold needed a complete
§4 table to be useful, and the alternative was leaving three numbered holes in the document
everything else cites. They are marked provisional rather than presented as settled.

## How this is enforced

`scripts/check-no-skipped-tests.sh` stops an invariant test being disabled. Beyond Phase 4, a
CI check that every ID in `CLAUDE.md` §4 appears in at least one test name is cheap to add and
worth adding — recorded here as the obvious next step, not yet implemented.
