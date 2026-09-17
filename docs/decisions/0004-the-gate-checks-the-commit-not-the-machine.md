# ADR-0004: The gate checks the commit, not the machine

- **Status:** accepted
- **Date:** 2026-09-15
- **Phase:** 0
- **Invariants touched:** none

## Context

`just gate <n>` began by running `just doctor`, and `gate 0` ends with `just gh-verify`. Both
turned out to be checking something other than the commit under test:

- **`doctor`** inventories the developer's workstation. A CI runner deliberately carries a
  different toolchain: it has Docker preinstalled and no `gitleaks` binary, and it installs
  `forge` and `sqlx-cli` per job rather than globally. Demanding a full dev box there fails the
  gate for a reason that has nothing to do with the code.
- **`gh-verify`** inspects GitHub repository settings: labels, milestones, the branch ruleset.
  Those are properties of the repository, identical for every commit.

  A first CI run corrected an assumption here. The default `GITHUB_TOKEN` **can** read the
  ruleset, the labels, the milestones, and the CI aggregator, which is every assertion that matters. What
  it cannot see are the merge-strategy flags (`allow_squash_merge` and friends), which come back
  as `null`. The original check treated `null` as a mismatch and reported "expected 'true', got
  'null'", a false failure about a setting that was in fact correct.

Left alone, both would have produced a red gate on a green commit. A gate that fails for
reasons unrelated to the change is worse than no gate: it trains everyone to ignore it.

## Options

| Option | Cost | What it does to the invariants |
|---|---|---|
| Leave both in; give CI a PAT and preinstall everything | Slower jobs, a long-lived admin token in secrets | Gate stays red until the token is provisioned |
| Make `doctor` lenient under CI | Free | The one command whose job is strictness stops being strict |
| Treat an invisible field as a failure | Free | False failures on correct settings; this is what the first run did |
| Take `doctor` out of the gate; distinguish "wrong" from "not visible" in `gh-verify` | Merge flags are checked daily rather than per-PR | The gate becomes exclusively about the commit, and says only what it can actually prove |

## Decision

`just doctor` is **not** part of any gate. It is a developer-onboarding command: `README.md`
and `scripts/dev-setup.sh` point at it, and it still exits non-zero when a tool is missing.

`just gh-verify` stays in gate 0, as `PHASES.md` requires, and runs for real there. The ruleset,
its absence of bypass actors, the `required` check, every label, every milestone, and the CI
aggregator are all asserted on every PR.

It distinguishes three outcomes rather than two: **correct**, **wrong**, and **not visible to
this token**. Only "wrong" fails. A field the token cannot read is reported as a warning naming
where it *is* verified, because a check that cannot see something must say so rather than guess.

If no authenticated `gh` exists at all and `CI` is set, it skips with an explicit, loud message.
It never skips silently, and never skips locally: running it without credentials on a
workstation is still a hard failure.

Repository settings are instead verified by `.github/workflows/gh-verify.yml`, on a daily
schedule and on demand, and by `just gh-verify` locally before any phase closes, which is the
moment `PHASES.md` actually cares about.

## Consequences

The gate now means one thing: *this commit is sound*. It runs identically on a laptop and on a
runner.

The softening is real and worth naming: a PR's gate job no longer proves GitHub is configured
correctly. Settings drift is caught within 24 hours by the scheduled workflow rather than
immediately. That is an acceptable trade for a signal that is trustworthy the rest of the time,
and the phase-close check is unchanged.

## How this is enforced

`scripts/gate.sh` carries a comment at `gate_0` saying why `doctor` is absent, so it is not
"helpfully" re-added. `scripts/gh-verify.sh` prints its skip reason and the two places
settings *are* verified. `.github/workflows/gh-verify.yml` runs on a schedule.
