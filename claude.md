# CLAUDE.md: Tickline

Tickline is an agent-native prediction market. Any agent pays a bounded subsidy over x402 to open a
market on a machine-resolvable question and gets a live probability in minutes. Forecaster agents
trade against an LMSR market maker, paying per fill with x402 batch-settlement vouchers. Positions
are committed onchain per epoch and claimed after oracle resolution. A dishonest operator gets
slashed.

This file is loaded every session. Phase-by-phase scope and gates live in `docs/PHASES.md`.
GitHub labels, templates, and `gh` commands live in `docs/GITHUB.md`. Task state lives in GitHub
issues. The gate log lives in `docs/STATUS.md`. Read `PHASES.md` (current phase) and `STATUS.md`
before writing anything.

---

## 1. Prime directive: tests are the product

This codebase moves money. A feature without a test that would fail if the feature broke does not
exist. You are judged on the test suite first and the implementation second.

Rules, no exceptions:

1. **Test first.** For every behavior: write the test, run it, show it failing for the right reason,
   then implement, then show it passing. Paste the red and green output summaries in the session.
2. **Never weaken a test to make it pass.** No loosening assertions, no `#[ignore]`, no `skip`, no
   widening tolerances, no deleting cases. If you believe a test is wrong, stop and explain why to
   Jay before touching it.
3. **Bugs start as failing tests.** Reproduce every bug as a test before fixing it. The test stays.
4. **Done means gated.** Never report a task complete without running the relevant `just` command
   and pasting pass/fail counts. "Should work" is not a status.
5. **No mocks for the thing under test.** Mock only external boundaries. Tests use real Postgres,
   real anvil, real contracts, real signatures.
6. **Deterministic or deleted.** No sleeps, no wall clock, no real network outside the explicit fork
   suite. Time comes from an injected `Clock`. Randomness comes from recorded seeds. A flaky test is
   a P0 bug, not a retry.
7. **Property failures are kept.** Commit proptest regression files and Foundry fuzz counterexamples
   as permanent unit tests.
8. **Every invariant in section 4 has at least one test named after it**, in every layer where it
   can be checked.

## 2. Architecture

```
agents (TS, official x402 SDK)
   |  HTTP + x402 batch-settlement vouchers
   v
engine (Rust)
   api (axum) -> session actors (per agent escrow) -> market actors (per market, LMSR state)
                         |                                   |
                         +---------> ledger (Postgres, double-entry, append-only)
                                             |
                  settlement worker (redeem vouchers, commit epochs)   indexer (logs, reorgs)
                                             |                               ^
                                             v                               |
contracts (Solidity, Foundry): x402 batch-settlement escrow (spec reference) + TicklineVault
                                             ^
                                   Pyth (MockPyth locally)
```

### Money flow
1. Creator opens an escrow session and pays the market subsidy via voucher. Market opens immediately.
2. Forecasters pay `cost + fee` per fill via cumulative vouchers. Engine returns a signed
   `PositionReceipt` and the new price.
3. Readers pay a small fee per `GET /price`.
4. Settlement worker redeems vouchers to the operator settlement wallet in batches.
5. `commitEpoch` pulls each market's collateral delta from the operator wallet into the vault and
   writes changed agent positions to storage. The vault checks solvency on every commit.
6. After the deadline, anyone resolves the market with a Pyth update. Agents claim winnings from
   stored positions. Residual collateral returns to the creator.
7. If an agent holds an operator-signed receipt showing more shares than the vault stored, it calls
   `claimWithReceipt`, gets paid from the operator bond, and the operator is slashed.

### Design decisions (locked unless Jay changes them)
- **Positions live in vault storage, not Merkle roots.** Commits write only agents whose position
  changed that epoch, and totals update incrementally. This makes solvency exact onchain and the
  fraud proof a single receipt comparison. Merkle roots are out of scope.
- **Buy-only, binary, hold to resolution.** No selling before resolution.
- **One payment path.** Creation, fills, and price reads all use batch-settlement vouchers.
- **LMSR math is offchain only.** The vault never prices trades; it enforces solvency, monotonic
  positions, claims, and slashing.
- **Rounding always favors the vault.** Costs round up, shares and payouts round down.
- **Resolution templates are machine-only.** MVP template: Pyth price threshold at deadline.
  No human resolution, ever.
- **Operator floats settlement.** Redemptions land in the operator wallet; commits move per-market
  deltas into the vault atomically.

## 3. Stack

Pin the latest stable versions at scaffold time and record them in `docs/STATUS.md`.

| Area | Choice |
|---|---|
| Contracts | Solidity, Foundry, forge-std, OpenZeppelin (EIP712, ECDSA, SafeERC20), Pyth SDK (MockPyth), Slither |
| Engine | Rust workspace, tokio, axum, alloy, sqlx (Postgres), serde, thiserror, tracing, Prometheus exporter |
| Rust testing | cargo-nextest, proptest, `#[sqlx::test]`, cargo-llvm-cov, cargo-mutants |
| Agents | TypeScript, pnpm, official x402 SDK (batch-settlement scheme), viem, vitest |
| Reference math | Python with mpmath via `uv`, only in `tools/reference` |
| Local infra | docker compose (Postgres), anvil |
| Tasks | `just` |
| CI | GitHub Actions running `just gate <current phase>`, gitleaks, semantic PR title check |
| Repo ops | GitHub CLI (`gh`), public repo, squash merges, rulesets on `main` |
| Deploy target | Base Sepolia (Phase 8) |

### Repo layout
```
contracts/            Foundry project
  src/  test/unit/  test/fuzz/  test/invariant/  test/fork/
engine/               Rust workspace
  crates/lmsr/        fixed-point LMSR, zero IO
  crates/protocol/    EIP-712 types, x402 envelopes, market ids
  crates/ledger/      Postgres double-entry ledger
  crates/market/      session + market actors
  crates/settlement/  voucher redemption, epoch commits
  crates/indexer/     log ingestion, reorg handling, reconciliation
  crates/api/         axum binary
agents/               TS creator, forecaster, reader, load agents
e2e/                  full-stack scenario harness
testdata/vectors/     golden cross-stack test vectors (JSON)
tools/reference/      high-precision reference implementations
docs/                 PHASES.md, STATUS.md, spec-notes.md, decisions/
```

### Rust lint policy
Money crates (`lmsr`, `protocol`, `ledger`, `market`, `settlement`) carry:
```rust
#![deny(clippy::float_arithmetic, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
```
No `f64` anywhere money or shares are computed. Floats are allowed only in `tools/reference` and test
oracles that compare against fixed point.

## 4. Invariants

Test names reference these IDs, for example `inv_i1_vault_solvent_after_commit`.

| ID | Invariant |
|---|---|
| I1 | **Solvency.** After every commit, vault collateral for a market >= max(totalYes, totalNo) in USDC base units. |
| I2 | **LMSR bound.** C(q) >= max(qYes, qNo) and C(q) - max(qYes, qNo) <= b*ln2, after rounding. |
| I3 | **Subsidy covers loss.** Market subsidy >= ceil(b*ln2). Creator net cost <= subsidy. |
| I4 | **Monotonic vouchers.** An accepted cumulative voucher amount per session never decreases. |
| I5 | **Monotonic receipts.** Per (agent, market): nonce strictly increases, shares never decrease, costPaid equals the sum of fill costs. |
| I6 | **Money before shares.** Sum of costPaid + feesPaid across an agent's receipts <= that session's accepted voucher cumulative. |
| I7 | **Escrow backing.** Accepted voucher cumulative <= escrow deposit observed at confirmation depth. |
| I8 | **Ledger balance.** Ledger entries sum to zero per asset. Every entry references a fill, redemption, commit, claim, or refund. |
| I9 | **Replay.** Replaying the ledger from empty reproduces every session and market actor state exactly. |
| I10 | **Monotonic commits.** The vault rejects any commit that lowers a stored position. |
| I11 | **No double claim.** Each (agent, market) is paid at most once across `claim` and `claimWithReceipt`. |
| I12 | **Fraud is punished.** A valid receipt exceeding stored position always pays the agent and slashes the bond. |
| I13 | **Reconciliation.** Onchain redeemed and committed amounts equal ledger amounts exactly, to the base unit. |
| I14 | **Idempotent settlement.** A crash at any point in settlement never double-redeems or double-commits. |
| I15 | **Rounding direction.** Every rounding step favors the vault. |

## 5. Testing doctrine

### Layers
| Layer | Tooling | Where it is mandatory |
|---|---|---|
| Unit | nextest, forge `test_`, vitest | Everything |
| Property | proptest, forge `testFuzz_` | lmsr, protocol, ledger, vault functions |
| Invariant | Foundry handler-based `invariant_` | Vault (I1, I10, I11, I12) |
| Differential | Rust vs mpmath reference; Rust vs Solidity math | lmsr, exp/ln |
| Cross-stack vectors | TS, Rust, Solidity agree on `testdata/vectors` | EIP-712 hashes, signatures, market ids |
| Integration | `#[sqlx::test]`, anvil | ledger, actors, settlement, indexer |
| Adversarial | crafted inputs, reorg injection, crash injection | engine API, settlement, indexer |
| End-to-end | `e2e/` harness | full lifecycle and fraud scenarios |
| Mutation | cargo-mutants | lmsr, protocol, ledger |

### Test profiles
- `default`: fast local loop. proptest 256 cases, Foundry fuzz 256 runs, invariant depth 20.
- `ci`: proptest 2,000 cases, fuzz 2,000 runs, invariant runs 500 depth 50.
- `deep` (Phase 7, pre-demo): proptest 20,000 cases, fuzz 20,000 runs, invariant runs 5,000 depth 100.

### Naming
- Rust: `fn buy_yes_raises_yes_price()`, `fn inv_i2_cost_bounded_by_max_plus_b_ln2()`.
- Solidity: `test_claim_pays_winning_shares`, `testFuzz_commit_rejects_lower_position`,
  `invariant_I1_solvency`.
- TS: `it("rejects fill when cost exceeds ceiling")`.
- Names describe behavior, never implementation.

### Coverage floors (enforced at gates)
- `lmsr`, `protocol`: 95% lines, zero surviving mutants on public functions except documented
  equivalent mutants in `docs/decisions/`.
- `TicklineVault`: 95% lines and branches.
- `ledger`, `market`, `settlement`, `indexer`: 85% lines.
- Coverage is a floor, not a goal. A covered line with no assertion is uncovered.

## 6. Commands

```
just setup            install toolchains and deps
just up / just down   start or stop Postgres and anvil
just test-sol         forge tests (default profile)
just test-rs          nextest across the workspace
just test-ts          vitest in agents/
just vectors          regenerate golden vectors, fail if the diff is non-empty without --accept
just ref              regenerate mpmath reference vectors
just e2e              full-stack scenarios
just cov              coverage reports for Rust and Solidity
just mutants          cargo-mutants on money crates
just deep             deep profile across all property and invariant suites
just gate <n>         everything required through phase n (includes all earlier phases)
just gh-bootstrap     idempotent: labels, milestones, repo settings, ruleset (see docs/GITHUB.md)
```

## 7. GitHub workflow

GitHub is the system of record. Every line of code reaches `main` through
issue -> branch -> PR -> green CI -> squash merge -> issue closed. No exceptions after Phase 0's
bootstrap commit. Full conventions and commands: `docs/GITHUB.md`.

### Phases and issues
- Each phase is a milestone: `Phase N: <name>`. Each milestone has one tracking issue labeled
  `tracking` with a checkbox task list linking every child issue.
- At phase start, break the phase into issues before writing code. One issue = one behavior slice
  that fits in one session and a PR under ~400 changed lines excluding generated files and vectors.
- Every issue body has: context, acceptance criteria written as test names (checkboxes), invariants
  touched, out of scope, dependencies (`Blocked by #N`).
- **Jay approves the issue breakdown** for a phase before the first issue starts.
- Work discovered mid-issue gets its own issue. Do not grow the current PR. Label out-of-phase work
  `later` and leave it.
- A bug gets an issue labeled `bug` whose body contains the failing test that reproduces it.

### Branches and commits
- Branch per issue: `p<phase>/<issue>-<slug>`, for example `p1/12-lmsr-cost-function`.
- Branch history shows test-first work: a `test(...)` commit with the failing test, then `feat(...)`
  or `fix(...)` commits that turn it green. Only the first test commit may be red.
- Conventional commits with scope: `test(lmsr): add inv_i2 bound property`, footer `Refs #12`.
- Push at least at every green step. Never leave unpushed work at session end.

### Pull requests
- Open the PR as a draft as soon as the red test is pushed. Title is a conventional commit, since
  it becomes the squash commit message on `main`.
- The PR body follows `.github/pull_request_template.md`: `Closes #N`, invariants touched, tests
  added by name, red evidence, green evidence with gate counts, coverage delta, risk notes.
- Before marking ready: run `just gate <phase>`, read your own diff with `gh pr diff`, and post a
  self-review comment listing what you checked and anything you fixed.

### Merging
You may merge your own PR with `gh pr merge --squash --auto --delete-branch` only when all are true:
1. All required checks are green.
2. The PR is not labeled `needs-jay`.
3. The PR hits no "stop and ask" trigger below.

Otherwise, label it `needs-jay`, request review from Jay, and move to the next unblocked issue.
Never use `--admin`, never force-push to `main`, never merge with red or pending checks.

### After merge
- Confirm the issue closed via `Closes #N`; tick its box in the tracking issue.
- Pull `main` before starting the next branch.
- When every child issue is closed and `just gate <n>` is green on `main`, comment the gate counts
  on the tracking issue and label it `needs-jay`. After Jay approves: close the milestone and run
  `gh release create phase-<n>` with the gate log as release notes.

## 8. Session protocol

At session start:
1. Read the current phase in `docs/PHASES.md` and `docs/STATUS.md`.
2. `git checkout main && git pull`. Check `gh pr list` for your open PRs with new review comments or
   finished checks, and resolve those before new work.
3. Pick the next unblocked issue in the current milestone, assign it to yourself, and state the
   session goal in one sentence plus the tests you will write first.

During the session:
4. Red, green, refactor, one behavior at a time. Run the narrowest test command after each change.
5. Touch only code in the current issue. Do not scaffold future phases.
6. Record any non-obvious decision as a short ADR in `docs/decisions/NNNN-title.md`, linked from
   the PR.

At session end:
7. Run `just gate <current phase>`. Push. Update the PR body with green evidence.
8. Merge if the merge rules allow it; otherwise label `needs-jay`.
9. Update `docs/STATUS.md` gate log with date, commit, and pass counts. Known flaky tests must be
   zero; a flake gets an issue labeled `flaky` and `p0` immediately.

### Stop and ask Jay before (these PRs always get `needs-jay`)
- Changing any invariant, locked design decision, or public interface (API, contract ABI, EIP-712 type).
- Interpreting the x402 spec where `docs/spec-notes.md` is silent or ambiguous.
- Modifying or deleting an existing test.
- Adding a dependency not listed in section 3.
- Changing CI workflows, rulesets, or required checks.
- Starting a phase's first issue (breakdown approval) and closing a phase.

## 9. Out of scope (do not build, do not scaffold)

Selling positions before resolution. Order books. Multi-outcome markets. Human or UMA-style
resolution. Merkle position roots. Running an x402 facilitator. A token. A human trading UI.
MCP servers. Cross-chain anything. KMS key management. Multi-operator support. Uniswap TWAP
templates before Phase 9.

## 10. Conventions

- Every commit on `main` leaves the gate for the current phase green. Branch commits follow section 7.
- Secrets never enter git. Tests use anvil default keys only. Testnet keys live in GitHub Actions
  secrets (Phase 8). gitleaks runs in CI and as a pre-commit hook.
- Typed errors everywhere (`thiserror` in Rust, custom errors in Solidity). No string reverts.
- USDC amounts are `u128`/`uint256` base units (6 decimals). LMSR internals are signed WAD (1e18).
  Conversions happen only at the `lmsr` crate boundary, with I15 rounding.
- Every public Rust and Solidity function has a doc comment stating which invariants it touches.
- No em-dashes in docs, comments, or commit messages.