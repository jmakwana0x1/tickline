# ADR-0001: Record architecture decisions

- **Status:** accepted
- **Date:** 2026-09-15
- **Phase:** 0
- **Invariants touched:** none

## Context

Tickline is built in phases, across four languages, by an agent and a human who will both
forget why. The expensive questions here are not "what does this code do" (that is readable)
but "why is the rounding in that direction", "why is the bond sized like that", "why does the
indexer halt instead of retrying". Those answers live in the gap between the code and the spec,
and they evaporate.

`PHASES.md` already treats some decisions as ADR-gated (the missed-commit refund path, any
Slither suppression, a gas regression above 5%, the whole of Phase 9). That only works if there
is somewhere for an ADR to go and a shape for it to take.

## Options

| Option | Cost | What it does to the invariants |
|---|---|---|
| Comments in the code | Free, but scoped to one file and deleted with it | Invariant rationale scatters and dies |
| A design document | One place, but rewritten in place, so history is lost | "Why did this change?" becomes unanswerable |
| Numbered ADRs, never edited after acceptance | A file per decision, some ceremony | Each invariant gets a citable reason with a date |

## Decision

We keep numbered ADRs in `docs/decisions/`, in the style of Michael Nygard's original. An ADR
is written **before** the code it justifies, not after. Once accepted it is never edited: a
decision that changes gets a new ADR that supersedes the old one, and the old one gains a
`superseded by` line and nothing else.

`docs/decisions/0000-template.md` is the shape. Every ADR names the invariants it touches and
ends with the check that enforces it.

An ADR is required when any of these is true:

- an invariant in `CLAUDE.md` §4 is added, removed, or reworded;
- a rounding direction, a fee, a bond size, or a grace window is chosen;
- a dependency is vendored, pinned, or forked;
- a static-analysis finding is suppressed;
- gas regresses more than 5%;
- a future reader would ask "why was it done this way?": the catch-all, and the one that fires
  most often.

## Consequences

Decisions become slower to make and much cheaper to revisit. A reviewer can ask "which ADR
covers this?" and get a file name. The cost is real: some ADRs will be written for decisions
nobody ever questions, and that is the cheaper failure.

The risk is ADRs drifting out of date. The `superseded by` rule is what contains it: a stale
ADR is still an accurate record of what was believed on its date.

## How this is enforced

The slice issue template asks whether an ADR is needed before work starts, the PR template's
definition-of-done repeats the question, and `CLAUDE.md` §2 tells Claude to stop and write one
rather than proceed. `PHASES.md` names the specific decisions that cannot be implemented
without one.

## Update, 2026-09-24

Made at Jay's direction on #63, and appended rather than edited, which is the rule it states.

**Supersede when a decision reverses or narrows an earlier one. Update when it extends within the
same frame.**

An `## Update` section is added below the original, which is left as it was, so the record of what
was believed on the original date survives. A superseding ADR gets its own number, and the old one
is marked `superseded by ADR-NNNN`.

The distinction is for the reader, not the author. `superseded by` says the old decision is dead
and its text can no longer be trusted; a reader who sees it on an ADR whose substance still holds
has to diff two documents to discover that nothing changed. ADR-0012 is the example that prompted
this: its signature policy did not change when `X402_`, `POLICY_` and per-family reach were added
on 2026-09-24, so it took an update. ADR-0002 took one for the same reason when I4 to I7 were
confirmed and I16 was added.

Precedent so far:

| ADR | What happened | Which |
|---|---|---|
| 0002 | I4 to I6 confirmed, I7 reworded, I16 added | update |
| 0012 | `X402_` and `POLICY_` reserved, families given a reach | update |
