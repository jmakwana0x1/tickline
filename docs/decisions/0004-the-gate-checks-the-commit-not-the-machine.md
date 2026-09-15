# ADR-0004: The gate checks the commit, not the machine

- **Status:** accepted
- **Date:** 2026-09-15
- **Phase:** 0
- **Invariants touched:** none

## Context

`just gate <n>` began by running `just doctor`, and `gate 0` ends with `just gh-verify`. Both
turned out to be checking something other than the commit under test:

- **`doctor`** inventories the developer's workstation. A CI runner deliberately carries a
  different toolchain — it has Docker preinstalled and no `gitleaks` binary, and it installs
  `forge` and `sqlx-cli` per job rather than globally. Demanding a full dev box there fails the
  gate for a reason that has nothing to do with the code.
- **`gh-verify`** inspects GitHub repository settings — labels, milestones, the branch ruleset.
  Those are properties of the repository, identical for every commit, and the default
  `GITHUB_TOKEN` cannot read rulesets at all.

Left alone, both would have produced a red gate on a green commit. A gate that fails for
reasons unrelated to the change is worse than no gate: it trains everyone to ignore it.

## Options

| Option | Cost | What it does to the invariants |
|---|---|---|
| Leave both in; give CI a PAT and preinstall everything | Slower jobs, a long-lived admin token in secrets | Gate stays red until the token is provisioned |
| Make `doctor` lenient under CI | Free | The one command whose job is strictness stops being strict |
| Take `doctor` out of the gate; let `gh-verify` skip explicitly where it cannot run | Settings drift is caught daily instead of per-PR | The gate becomes exclusively about the commit |

## Decision

`just doctor` is **not** part of any gate. It is a developer-onboarding command: `README.md`
and `scripts/dev-setup.sh` point at it, and it still exits non-zero when a tool is missing.

`just gh-verify` stays in gate 0, as `PHASES.md` requires, but **skips with an explicit,
loud message** when no authenticated `gh` is available *and* `CI` is set. It never skips
silently, and never skips locally — running it without credentials on a workstation is still a
hard failure.

Repository settings are instead verified by `.github/workflows/gh-verify.yml`, on a daily
schedule and on demand, and by `just gh-verify` locally before any phase closes — which is the
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
