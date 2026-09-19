# STATUS

The state of the build. **Task state lives in GitHub issues, not here.** This file records
gates, pins, open questions, and benchmarks.

Last updated: 2026-09-17

---

## Current phase

**Phase 1: LMSR math core.** `.phase` = `1`. Tracking issue #31. **Gate 1 is green**; the phase
closes on Jay's go-ahead.

Risk being retired: wrong prices, and insolvent rounding.

Per-PR CI gates the closed phases (`just gate-closed`, ADR-0006); Phase 1's own gate runs in the
`phase-gate` workflow when the phase closes. Decisions D1 to D4 are answered on #31, and the slices
run strictly in order, S1 to S7.

**Phase 0 closed** on 2026-09-17: gate 0 green on `main` at `44dc50d`, release `phase-0`.
Phase 0 retired this risk: building against a misread x402 spec. Its deliverables:

| Deliverable | State |
|---|---|
| `CLAUDE.md`, `docs/GITHUB.md`, `PHASES.md` | written |
| Monorepo layout, empty crates, Foundry project, pnpm workspace | scaffolded |
| `justfile` with every command from `CLAUDE.md` §6 | written |
| `docker-compose.yml`, anvil via `just up` | written |
| Foundry profiles `default` / `ci` / `deep`, `PROPTEST_CASES` | written |
| CI workflow with the `required` aggregator | written |
| Smoke test per stack | ✅ Rust 9 · Solidity 2 · TypeScript 9 · Postgres 1 (CI only; no Docker on the dev box) |
| `docs/spec-notes.md` | ✅ researched, cited, and every question answered by Jay |
| GitHub bootstrap (`just gh-bootstrap`, `just gh-verify`) | ✅ both green |
| Guard proofs | ✅ 4 of 4, see below |
| Self-tested checks | ✅ `gate.sh` (37 planted failures), em-dash (7 cases), stray file names (5 cases) |
| Release `phase-0` | ✅ cut at `44dc50d` |

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
| Foundry | **v1.8.1**, sha256 `37b45855…89c10` | `.github/actions/setup`, pinned and checksum-verified |
| forge-std | v1.16.2 (`bf647bd`) | `contracts/lib/forge-std`, git submodule |
| gitleaks | 8.21.2 | `.pre-commit-config.yaml` |
| alloy-primitives | =1.7.3, no default features | `engine/Cargo.toml`; ADR-0008 |
| uv | 0.12.15 | `scripts/tool-versions.sh`; CI installs it with `astral-sh/setup-uv` at `bec219d` (v10.1.0) |
| cargo-mutants | 27.1.0 | `scripts/tool-versions.sh` |
| cargo-llvm-cov | 0.9.1 | `scripts/tool-versions.sh` |
| `x402BatchSettlement` | `0x4020074e9dF2ce1deE5A9C1b5c3f541D02a10003` (canonical, CREATE2) | `docs/spec-notes.md` §4, verified live on Base Sepolia |
| x402 EIP-712 domain | `x402 Batch Settlement`, version `1` | `docs/spec-notes.md` §1 |
| x402 escrow reference | not yet vendored | Phase 3, pinned by commit |

---

## Gate log

One row per `just gate <n>` run that was used to close a phase. Full output goes on the phase
tracking issue.

| Date | Phase | Commit | Rust | Solidity | TS | DB | Notes |
|---|---|---|---|---|---|---|---|
| 2026-09-15 | 0 | `6ed3358` | 8 | 2 | 9 | 1 | First green `just gate 0`, in CI, all 10 checks incl. `required`. Not yet a phase close: `docs/spec-notes.md` open questions are unanswered. |
| 2026-09-17 | 0 | `44dc50d` | 9 | 2 | 9 | 1 | **Phase 0 close.** CI run 35186404431, all 10 jobs green; `just gh-verify` also green locally with an admin token. Log on #1. |
| 2026-09-19 | 1 | `9fd9d2c` | 80 | 2 | 9 | 1 | **Phase 1 gate green.** `phase-gate` run 35447353612, `just gate 1` (phases 0 and 1), 438s. `lmsr`: 66 mutants, 65 caught, 1 unviable, 0 missed; coverage 99.69% of 643 lines; vectors reproduce byte for byte. Log on #31. |

### Guard evidence (Phase 0 exit)

These prove the guards themselves work, and each needs a link before Phase 0 closes:

| Guard | Evidence | State |
|---|---|---|
| `main` rejects direct pushes | see output below | ✅ 2026-09-15 |
| gitleaks catches a planted key | `gitleaks protect --staged` on a planted 32-byte hex key → `leaks found: 1`, exit 1 | ✅ 2026-09-15 |
| no-skips check catches an `#[ignore]` | planted `#[ignore]` in `lmsr` → `✗ disabled or focused tests found`, exit 1 | ✅ 2026-09-15 |
| `required` blocks merge on red | PR #5, planted `WAD == 10^17`: `rust` failed, merge refused with `the base branch policy prohibits the merge`. Closed unmerged. | ✅ 2026-09-15 |

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

None open. Every question raised in Phase 0 was answered by Jay on 2026-09-17. They stay listed
so the reasoning is findable.

| # | Question | Answer | Where |
|---|---|---|---|
| Q1 to Q9 | The x402 and Pyth research questions | Recorded in `docs/spec-notes.md` §8 | #6, #7, #8, #9 |
| Qa | Fee model | Fixed per market at creation and stored in market params. Fill fee is **100 bps of cost**. A price read costs **1,000 base units (0.001 USDC)**. Receipts carry `feesPaid`. | #1 |
| Qb | Missed commit | The market becomes `Failed`; receipts reclaim `costPaid + feesPaid` **from the operator bond**. This differs from a void (ADR-0005), which refunds from collateral because nobody is at fault. **An ADR is required before any Phase 3 code.** | #1 |
| Qc | Foundry pin | Release tag `v1.8.1`. CI installs it by direct download with a verified sha256 (#12), which replaced the `foundry-toolchain` action; `scripts/dev-setup.sh` installs the same tag. Upgrades are their own PR. | #1, #11 |
| (ruleset) | 0 required approvals means a PR can merge unread | Accepted in ADR-0003. Claude self-merges once `required` is green, except `needs-jay` PRs and stop-and-ask triggers. Revisit if a second reviewer is added. | ADR-0003 |

---

## Deliberate divergences from PHASES.md

Recorded here so a reader who trusts `PHASES.md` is not surprised. Each has an ADR.

| `PHASES.md` says | We do | Why | ADR |
|---|---|---|---|
| `resolve` is "permissionless after deadline + final grace" | permissionless from the deadline, no grace | the grace guarded a timing race that Pyth's uniqueness check proves cannot happen | ADR-0005 |
| Phase 3 integrates "the spec's reference batch-settlement escrow (vendored if needed, pinned by commit)" | integrate against the deployed canonical address; no vendoring | it is already deployed at a CREATE2 address on every supported chain and audited three times | `docs/spec-notes.md` §4 |
| `PositionReceipt` fields `yesShares`, `noShares`, `costPaid`, `feesPaid` are `uint256` | all four are `uint128` | they then match x402's `maxClaimableAmount` and `totalClaimed`, so no width conversion happens at the escrow boundary | `docs/spec-notes.md` §8 Q5 |
| Phase 0 gate includes `just doctor` | it does not | `doctor` inventories the developer's machine; CI runners differ by design | ADR-0004 |
| ruleset requires 1 approving review | 0 required approvals | a solo repo deadlocks: GitHub forbids self-approval and there are no bypass actors | ADR-0003 |

## Benchmarks

Filled in Phase 7. Until then these read "not measured", not "fast".

| Metric | Value |
|---|---|
| p50 / p95 / p99 fill latency | not measured |
| Fills per onchain transaction | not measured |
| Gas per 1,000 fills | not measured |
| Ledger write throughput | not measured |
| Drift at end of load test | not measured |
