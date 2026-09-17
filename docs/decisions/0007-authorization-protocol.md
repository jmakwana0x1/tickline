# ADR-0007: Jay's decisions are recorded as commands on GitHub

- **Status:** accepted (Jay, on #33)
- **Date:** 2026-09-17
- **Phase:** 1
- **Invariants touched:** none; this governs how changes reach `main`

## Context

`CLAUDE.md` §2 said when Claude must stop and ask, but not what counts as an answer. Answers
arrived in several forms: a chat message in a Claude Code session, a comment on an issue, a
comment on a PR. None of them was tied to the code that was actually reviewed, so a PR could
change after Jay read it and still be merged on the strength of his earlier words.

The CI trigger was also too broad. #25 added a Postgres service container to the non-required
nightly workflow, a change with no effect on what blocks a merge, and still had to wait for Jay.

There is a structural limit underneath both problems. Claude Code pushes commits and posts
comments with Jay's own GitHub token. GitHub cannot tell a comment Claude wrote from one Jay
wrote, so no GitHub setting can currently distinguish "Jay approved" from "Claude said Jay
approved".

## Options

| Option | Cost | What it does |
|---|---|---|
| Keep answers informal | None | Approvals are not tied to a commit, and the record is scattered |
| Commands in comments, pinned to a commit SHA, with a rule that Claude never writes them | An honor system: nothing on GitHub enforces the last rule | Every approval names what was reviewed and is findable on the PR it applies to |
| A separate machine account for Claude Code, plus `CODEOWNERS` over `.github/**`, `CLAUDE.md` and `docs/decisions/**` with code-owner review required | A second account, its token management, and review friction | GitHub itself enforces that Jay approved |

## Decision

The second option, as written in `CLAUDE.md` §7 under **Authorization**: `/approve <sha>`,
`/approve-after <sha>`, `/changes` and `/reject`, with approvals void if the head moves for any
reason other than the listed changes or a rebase that leaves the diff unchanged. Decisions given
in a session are posted first as `Recorded from session with Jay:` followed by his words.
Claude never posts a comment that starts with one of the four commands; that rule is the
control.

`CLAUDE.md` §2's CI trigger is narrowed to changes that alter what `required` enforces, the
ruleset, workflow permissions or secrets, third-party actions, and `phase-gate.yml`'s commands.
Service containers, caches, tool pins, and non-required workflow changes that add no permissions
or secrets merge without `needs-jay`, with a one-line note in the PR body.

**The machine-account option is not adopted yet.** It is the real enforcement and the likely next
step.

## Consequences

Every merge that needed Jay can be traced to a comment naming the exact commit he reviewed.
Rebases stay cheap: a rebase that leaves the diff unchanged does not void an approval, and the PR
records how that was checked (for example by comparing `git patch-id`).

The protocol depends on Claude following the never-post rule. Until the machine account exists,
an approval comment on GitHub proves only that someone holding Jay's token wrote it.

## How this is enforced

By `CLAUDE.md` §7 and by review of the PR history. First applied on #25 and #28, where each
decision was posted as `Recorded from session with Jay:` and each rebase was checked against the
approved diff before merging.
