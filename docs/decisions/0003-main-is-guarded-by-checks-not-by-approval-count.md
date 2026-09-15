# ADR-0003: `main` is guarded by the `required` check, not by an approval count

- **Status:** accepted
- **Date:** 2026-09-15
- **Phase:** 0
- **Invariants touched:** none directly; this governs how every change reaches `main`

## Context

`docs/GITHUB.md` §2 originally specified a ruleset requiring **1 approving review**, code-owner
review, and last-push approval, with **no bypass actors**.

Applied to this repository that combination deadlocks. Tickline is a solo repository: every PR
is authored by `jmakwana0x1`, GitHub does not permit approving your own pull request, and with
no bypass actors an administrator cannot override. The result is a ruleset under which *nothing
can ever merge* — including the changes that would fix the ruleset.

This was found the honest way: by applying it and reasoning through the first merge.

## Options

| Option | Cost | What it does to the invariants |
|---|---|---|
| Keep 1 approval, no bypass | Correct on paper, unusable | No change can reach `main` at all |
| Keep 1 approval, add Jay as a bypass actor | Merging works | The ruleset stops binding everyone — the exact property `CLAUDE.md` §7 calls out as "the design working" |
| 0 approvals, keep every other rule | A PR can merge without a human reading it | The `required` check still blocks red code; review becomes asynchronous rather than blocking |

## Decision

The ruleset requires **0 approving reviews** and keeps everything else, with **no bypass
actors**:

- no direct pushes to `main`;
- a pull request is required;
- linear history;
- conversation resolution required;
- force-push and deletion blocked;
- the `required` status check must pass.

The merge gate is therefore **mechanical, not social**. `required` aggregates every CI job, so
code that fails format, lint, crate boundaries, the no-skips check, or any of the four test
suites cannot reach `main` no matter who wants it there.

Jay's review remains the thing that catches design errors — it simply happens on the PR rather
than as a blocking button press, and `CLAUDE.md` §2's stop-and-ask triggers still halt work for
a human decision.

## Consequences

Claude can run the full loop end to end without Jay being at the keyboard, which is what makes
"commit and PR after every significant turn" workable. The cost is real and should be stated
plainly: **a PR can now merge without a human having read it.** The compensating controls are
the breadth of `required` and the fact that the stop-and-ask triggers are about design
decisions, which are exactly the things CI cannot check.

If a second reviewer account or a review bot is ever added, this ADR should be superseded and
the approval count restored to 1.

## How this is enforced

`.github/rulesets/main.json` is the source of truth, applied by `just gh-bootstrap` and
asserted rule-by-rule by `just gh-verify`, which fails if any bypass actor appears or if
`required` stops being the named check.

Verified on 2026-09-15: a direct push to `main` was rejected with
`GH013: Repository rule violations found for refs/heads/main` — recorded in `docs/STATUS.md`.
