# STATUS

The state of the build. **Task state lives in GitHub issues, not here** — this file records
gates, pins, open questions, and benchmarks.

Last updated: 2026-09-15

---

## Current phase

**Phase 0 — Spec capture and scaffold.** `.phase` = `0`.

Risk being retired: building against a misread x402 spec.

| Deliverable | State |
|---|---|
| `CLAUDE.md`, `docs/GITHUB.md`, `PHASES.md` | written |
| Monorepo layout, empty crates, Foundry project, pnpm workspace | scaffolded |
| `justfile` with every command from `CLAUDE.md` §6 | written |
| `docker-compose.yml`, anvil via `just up` | written |
| Foundry profiles `default` / `ci` / `deep`, `PROPTEST_CASES` | written |
| CI workflow with the `required` aggregator | written |
| Smoke test per stack | Rust ✅ 8 · Solidity ✅ 2 · TypeScript ✅ 9 · sqlx ⚠️ unrun (no Docker locally) |
| `docs/spec-notes.md` | **not started — blocks the phase** |
| GitHub bootstrap (`just gh-bootstrap`, `just gh-verify`) | ✅ both green |
| Guard proofs | 3 of 4 — see below |
| Release `phase-0` | not cut |

---

## Pinned versions

"Latest" is not a version. Upgrades are their own PR with their own gate run.

| Thing | Pin | Where |
|---|---|---|
| Rust | 1.97.1 | `rust-toolchain.toml` |
| Rust edition | 2021 | `engine/Cargo.toml` |
| solc | 0.8.28 | `contracts/foundry.toml` |
| EVM version | cancun | `contracts/foundry.toml` |
| Node | 22 | `.nvmrc` |
| pnpm | 9.12.3 | `package.json` `packageManager` |
| Postgres | 16.4-alpine | `docker-compose.yml`, CI service |
| Foundry | 1.8.1 (`stable` channel in CI) | `.github/actions/setup` — **to pin to a release once Phase 3 opens** |
| forge-std | v1.16.2 (`bf647bd`) | `contracts/lib/forge-std`, git submodule |
| gitleaks | 8.21.2 | `.pre-commit-config.yaml` |
| x402 escrow reference | not yet vendored | Phase 3, pinned by commit |

---

## Gate log

One row per `just gate <n>` run that was used to close a phase. Full output goes on the phase
tracking issue.

| Date | Phase | Commit | Rust | Solidity | TS | Coverage | Notes |
|---|---|---|---|---|---|---|---|
| — | — | — | — | — | — | — | no gate has been run yet |

### Guard evidence (Phase 0 exit)

These prove the guards themselves work, and each needs a link before Phase 0 closes:

| Guard | Evidence | State |
|---|---|---|
| `main` rejects direct pushes | see output below | ✅ 2026-09-15 |
| gitleaks catches a planted key | `gitleaks protect --staged` on a planted 32-byte hex key → `leaks found: 1`, exit 1 | ✅ 2026-09-15 |
| no-skips check catches an `#[ignore]` | planted `#[ignore]` in `lmsr` → `✗ disabled or focused tests found`, exit 1 | ✅ 2026-09-15 |
| `required` blocks merge on red | deliberately failing PR, closed unmerged | _pending — needs one CI run first_ |

Direct push to `main`, attempted 2026-09-15 with the ruleset active:

```
remote: error: GH013: Repository rule violations found for refs/heads/main.
remote: - Changes must be made through a pull request.
remote: - Required status check "required" is expected.
 ! [remote rejected] main -> main (push declined due to repository rule violations)
```

### Local environment gaps

Not a code problem, but it shapes what can be proven on this machine:

| Missing | Why it cannot be installed here | Covered by |
|---|---|---|
| Docker | no passwordless sudo, no Docker Desktop on the Windows side | the `db` CI job's Postgres service container |
| `sqlx-cli` | installable, not yet installed | same |

---

## Open questions

Each is a `needs-jay` issue. Claude does not proceed past one by guessing.

| # | Question | Blocks | Issue |
|---|---|---|---|
| Q1 | Every item in `docs/spec-notes.md` §Open questions | Phases 2–5 | _to open_ |
| Q2 | Invariants **I4, I5, I6** are Claude's reconstruction from `PHASES.md` — they are the only IDs the plan references without stating. Confirm or correct the wording in `CLAUDE.md` §4. | Phase 4 | _to open_ |
| Q3 | Fee model: `PHASES.md` §4 says "fee math" and a "small read fee" but never fixes the rates or who sets them. Per-market or protocol-wide? | Phase 4 | _to open_ |
| Q4 | Missed-commit refund path (`PHASES.md` §3) is flagged "write ADR before implementing". | Phase 3 | _to open_ |
| Q5 | Foundry has no semver releases on the stable channel; pinning by date-tag or by commit? | Phase 3 | _to open_ |
| Q6 | The `main` ruleset now requires 0 approving reviews (ADR-0003), so a PR can merge without a human reading it. Revisit if a second reviewer account or review bot is ever added. | — | answered 2026-09-15 |

---

## Benchmarks

Filled in Phase 7. Until then these read "not measured", not "fast".

| Metric | Value |
|---|---|
| p50 / p95 / p99 fill latency | not measured |
| Fills per onchain transaction | not measured |
| Gas per 1,000 fills | not measured |
| Ledger write throughput | not measured |
| Drift at end of load test | not measured |
