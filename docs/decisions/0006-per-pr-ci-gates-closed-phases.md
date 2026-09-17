# ADR-0006: Per-PR CI gates closed phases; the in-progress gate runs at close

- **Status:** accepted (Jay, on #28)
- **Date:** 2026-09-17
- **Phase:** 1
- **Invariants touched:** none directly; this decides when each phase's checks block a merge

## Context

CI's `gate` job ran `just gate $(cat .phase)`. That worked for Phase 0 only because gate 0 has no
steps that depend on unbuilt work. Gate 1 does: `just vectors` fails until the mpmath generator
exists (it refuses to write an empty vector file), `just mutants lmsr` needs a finished library
and a `cargo-mutants` install the job does not have, and the 95% coverage threshold cannot hold
while the library is half written. Gates 2 to 8 have the same shape.

With `.phase = 1`, the PR bumping `.phase` would therefore be red and could not merge, and every
later Phase 1 slice would stay red until the phase was complete. The per-slice loop in
`CLAUDE.md` §7 would stop working.

## Options

| Option | Cost | What it does |
|---|---|---|
| Per-PR CI gates **closed** phases; the in-progress gate runs on demand at close | Two CI entry points | Finished phases block every PR; the phase's own close criteria are checked when `PHASES.md` says they matter |
| A second state file (`.gated`) holding the last closed phase | Two files to keep in step | Same behaviour, more state |
| Gate steps that skip until their work exists | None | A gate that can skip is exactly what `scripts/test-gate.sh` exists to forbid |
| Finish a whole phase in one PR | None | No per-slice loop |

## Decision

- `just gate-closed` runs `just gate <.phase - 1>`, or gate 0 while `.phase` is 0. Per-PR CI's
  `gate` job runs it. A developer can run the same command by the same name.
- `.github/workflows/phase-gate.yml` runs `just gate $(cat .phase)` on demand, with the extra
  tools later gates need: `cargo-mutants`, `cargo-llvm-cov` and `uv`, at the versions pinned in
  `scripts/tool-versions.sh`. The oracle runs through `uv` there exactly as it does locally, and
  the workflow only ever calls `just`.
  Its log is the gate log posted at phase close.
- The in-progress phase's own tests still run on every PR through the `rust`, `db`, `sol` and
  `ts` jobs. Only the close-time extras (vector diffs, mutation, coverage thresholds) wait.

## Consequences

A regression in any closed phase still blocks every merge, which is the property `just gate <n>`
exists for. A Phase 1 slice can merge while, say, coverage is at 60%, which is intended: the
threshold is a phase-exit criterion, not a per-slice one.

The cost is that the in-progress gate is not proven on every PR. The mitigation is that it runs,
in CI, before the phase can close, and `CLAUDE.md` §7 now says so.

## How this is enforced

`docs/GITHUB.md` §3 names `just gate-closed` as the `gate` job's command and §8 makes the
`phase-gate` run part of closing a phase. `just gh-verify` still requires the `required`
aggregator to cover every job in `ci.yml`.
