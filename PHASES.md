# PHASES.md: Tickline build plan

Each phase ends with a green `just gate <n>` and Jay's explicit go-ahead. `just gate <n>` always
reruns every earlier phase, so regressions block progress. Invariant IDs refer to `CLAUDE.md`
section 4.

Every phase runs the same GitHub loop (`CLAUDE.md` section 7, commands in `docs/GITHUB.md`):
1. Create the tracking issue in the phase milestone.
2. Break the phase's Build and Tests sections below into slice issues, each with acceptance criteria
   written as test names. Label the tracking issue `needs-jay` for breakdown approval.
3. Work issues one at a time: branch, red commit, draft PR, green, gate, self-review, merge.
4. Close the phase: gate log on the tracking issue, Jay approval, bump `.phase`, close milestone,
   `gh release create phase-<n>`.

| Phase | Name | Core risk it retires |
|---|---|---|
| 0 | Spec capture and scaffold | Building against a misread x402 spec |
| 1 | LMSR math core | Wrong prices, insolvent rounding |
| 2 | Protocol types and cross-stack vectors | TS, Rust, and Solidity disagreeing on signatures |
| 3 | Contracts | Loss of funds onchain |
| 4 | Engine core | Double-spent vouchers, ledger drift, race conditions |
| 5 | Settlement and indexer | Reorgs, crashes, reconciliation drift |
| 6 | Agents and end-to-end | Pieces that pass alone but fail together |
| 7 | Hardening | Unknown unknowns under load and adversaries |
| 8 | Testnet, dashboard, demo | Works locally, breaks live |
| 9 | Stretch: TWAP template | Resolution manipulation |

---

## Phase 0: Spec capture and scaffold

**Goal:** a repo where every test command runs green on CI, and a written, reviewed understanding of
the x402 batch-settlement scheme.

### Research (write findings to `docs/spec-notes.md`, cite doc URLs)
1. Batch-settlement scheme at docs.x402.org: voucher EIP-712 typed data, escrow contract interface,
   deposit, redemption, refund and withdrawal semantics, session lifecycle.
2. How the "up to" ceiling works inside batch settlement: what the client signs per request, how the
   seller captures actual usage, and how the cumulative amount advances when actual < ceiling.
3. x402 V2 envelope: 402 challenge format, payment header names, CAIP network identifiers.
4. Where the reference EVM escrow contracts live and whether they build under Foundry.
5. Official TS SDK package names for batch settlement, client side.
6. Whether the `x402-rs` crate has V2 envelope types worth reusing. It does not implement batch
   settlement; the seller side is ours.
7. Pyth: MockPyth availability, and whether `parsePriceFeedUpdatesUnique` (or its current
   equivalent) guarantees the first update at or after a given timestamp.

### Bootstrap (the single direct commit to `main`)
- `CLAUDE.md`, `docs/`, `.phase` = `0`, `justfile`, `.github/` (CI workflow, issue and PR templates,
  ruleset JSON), `scripts/gh-bootstrap.sh`, `scripts/gh-verify.sh`, gitleaks pre-commit config.
- Run `just gh-bootstrap`, then `just gh-verify`. Every later change in this phase goes through issues
  and PRs, starting with the Phase 0 tracking issue.

### Build
- Monorepo layout from `CLAUDE.md` section 3, with empty crates, Foundry project, pnpm workspace.
- `justfile` with every command from `CLAUDE.md` section 6, each running real (empty) suites.
- `docker-compose.yml` for Postgres. anvil started by `just up`.
- Foundry profiles `default`, `ci`, `deep`. proptest config reading `PROPTEST_CASES`.
- CI workflow per `docs/GITHUB.md` section 3, with the `required` aggregator job.
- `docs/STATUS.md` with sections: Current phase, Pinned versions, Gate log (date, commit, counts),
  Open questions (links to `needs-jay` issues). Task state lives in GitHub issues, not here.
- `docs/decisions/0001-record-architecture-decisions.md`.

### Tests
- One smoke test per stack proving the harness runs: a Rust test, a forge test, a vitest test, and a
  `#[sqlx::test]` that creates and drops a database.
- A CI job that fails if any test is marked ignored or skipped.
- A deliberately red PR (a failing assertion) proves the `required` check blocks merge; it is closed
  unmerged and linked from STATUS.md.
- A direct push to `main` is rejected; the rejection output is recorded in STATUS.md.
- gitleaks catches a planted fake key on a throwaway branch.

### Gate 0
`just gate 0`: all smoke tests pass locally and on CI; lint and format checks pass;
`just gh-verify` passes.

### Exit
Jay has reviewed `docs/spec-notes.md` and signed off on every open question in it. Every Phase 0
issue is closed through a merged PR. Release `phase-0` exists.

---

## Phase 1: LMSR math core (`engine/crates/lmsr`)

**Goal:** a zero-IO, fixed-point, binary LMSR library that is provably safe to price real money.

### Math (signed WAD, 1e18)
- Cost: `C(qY, qN) = m + b * ln(1 + exp(-|qY - qN| / b))`, with `m = max(qY, qN)`.
  Use this log-sum-exp form, never the naive form, to avoid overflow.
- Price: `pY = 1 / (1 + exp((qN - qY) / b))`, `pN = 1 - pY`.
- Cost to buy `d` YES shares: `C(qY + d, qN) - C(qY, qN)`. Symmetric for NO.
- Subsidy: `ceil(b * ln2)` converted to USDC base units.
- `exp_wad` and `ln_wad`: port the Solady `FixedPointMathLib` algorithms (MIT) so Rust and Solidity
  share one numerical method. Document domain limits and return typed errors outside them.
- Boundary conversions WAD to USDC base units: costs round up, shares round down (I15).

### Tests
- **Unit:** known values at `q = (0, 0)`, symmetric quantities, large imbalance, minimum and maximum
  `b`, domain-edge errors.
- **Property (proptest):**
  - `pY + pN == 1 WAD` within 1 wei.
  - Buying YES strictly raises `pY`.
  - Cost to buy is strictly positive for `d > 0` and superadditive under rounding: buying `d1` then
    `d2` costs >= buying `d1 + d2` in one fill.
  - `inv_i2`: `C(q) >= max(qY, qN)` and `C(q) - max(qY, qN) <= b*ln2`.
  - `inv_i3`: subsidy >= `C(0,0)`, and subsidy + collected >= max payout for any fill sequence.
  - `inv_i15`: converted cost >= exact cost; converted shares <= exact shares.
- **Differential:** `tools/reference/lmsr_ref.py` (mpmath, 60 digits) generates
  `testdata/vectors/lmsr.json` with at least 5,000 cases, including edge cases. Rust must match
  within its derived error bounds, with rounding direction always toward the vault:
  - `exp_wad`, `ln_wad` and `price` within constant bounds (ADR-0009);
  - `cost` within `E(b) = ceil(4 * b / WAD) + 1` wei, because Solady's `ln` error is multiplied
    by `b / WAD`, which reaches 1e7 at `B_MAX`. A flat 1 wei is unreachable without abandoning the
    shared Solady method (ADR-0008);
  - every bound is derived from per-step bounds before being asserted, and the measured maximum
    must be at most half the bound;
  - I15 is guaranteed at the base-unit boundary by adding the bound as a margin before rounding
    costs up. Shares still round down with no margin.
- **Mutation:** `just mutants` on `lmsr`, zero surviving mutants on public functions.

### Gate 1
`just gate 1` plus `just mutants` scoped to `lmsr`. Coverage >= 95% lines.

### Out of scope this phase
Budget-to-shares inverse. Multi-outcome LMSR. Anything with IO.

---

## Phase 2: Protocol types and cross-stack vectors (`engine/crates/protocol`)

**Goal:** TS, Rust, and Solidity produce byte-identical hashes and signatures for every signed object.

### Build
- Voucher types exactly as defined in `docs/spec-notes.md`. Do not invent fields.
- `PositionReceipt` (ours), EIP-712 domain `Tickline` version `1`, `chainId`, `verifyingContract` = vault:
  `marketId bytes32, agent address, yesShares uint256, noShares uint256, costPaid uint256,
  feesPaid uint256, nonce uint64, epoch uint32`.
- `MarketId = keccak256(abi.encode(creator, templateId, templateParamsHash, deadline, b, epochLength, salt))`.
- 402 challenge and payment header encode/decode for V2 envelopes.
- Vector generator in `agents/` (viem plus the official SDK) writes `testdata/vectors/eip712.json`
  with fixed private keys and inputs.

### Tests
- **Cross-stack:** Rust test and forge test both load `eip712.json` and assert identical struct hashes,
  digests, recovered signers, and market ids. A TS test regenerates and diffs.
- **Property:** encode/decode round-trips for every type; any single-field mutation changes the digest;
  signatures from a different chain id or verifying contract fail to verify.
- **Adversarial unit:** malleable signatures (high-s) rejected; zero address signer rejected; truncated
  and oversized payment headers rejected with typed errors.
- **Mutation:** zero surviving mutants on public functions.

### Gate 2
`just gate 2`. `just vectors` produces an empty diff.

---

## Phase 3: Contracts (`contracts/`)

**Goal:** a vault that cannot lose funds, cannot double-pay, and always punishes a lying operator.

### Build
- Integrate the spec's reference batch-settlement escrow (vendored if needed, pinned by commit).
- `TicklineVault`:
  - Operator registration with signing key and bond. Minimum bond to create markets.
  - `createMarket(params)`: stores template (Pyth price threshold), `b`, deadline, epoch length,
    creator, subsidy requirement.
  - `commitEpoch(marketId, epoch, positions[], collateralDelta)`: operator only; pulls
    `collateralDelta` USDC; rejects any lowered position (I10); updates totals incrementally; checks
    solvency (I1); enforces commit deadline `epochEnd + commitGrace`.
  - `resolve(marketId, pythUpdate)`: permissionless after deadline + final grace; uses the first
    Pyth update at or after the deadline.
  - `claim(marketId)`: pays winning shares from stored position; marks claimed (I11); returns
    residual collateral to creator once the claim window closes.
  - `claimWithReceipt(marketId, receipt, sig)`: if receipt epoch <= final committed epoch and receipt
    shares exceed stored shares, pays per receipt from the bond and slashes (I12).
  - Missed-commit path: market becomes `Failed`, trading-side receipts can reclaim `costPaid` from
    the bond. Write ADR before implementing; stop and ask Jay if the design needs to change.
- Custom errors, events for every state change.

### Tests
- **Unit:** every function's happy path and every revert, one test per custom error.
- **Fuzz:** commits with random position vectors; claims with random receipts; resolve with random
  Pyth prices around the threshold; deadline boundaries at `t - 1`, `t`, `t + 1`.
- **Invariant (handler-based):** handlers for createMarket, commitEpoch (honest and dishonest
  operator), resolve, claim, claimWithReceipt, warp time. Ghost variables track expected positions
  and payouts.
  - `invariant_I1_solvency`
  - `invariant_I10_positions_monotonic`
  - `invariant_I11_no_double_claim`
  - `invariant_I12_dishonest_commit_always_slashable`
  - `invariant_total_paid_never_exceeds_collateral_plus_bond`
- **Differential:** Solidity `expWad`/`lnWad` against `testdata/vectors/lmsr.json` (math parity with
  Phase 1, even though the vault does not price trades).
- **Static analysis:** Slither with zero unaddressed high or medium findings; each suppression
  justified in an ADR.
- **Gas snapshot:** `forge snapshot` committed; gate fails on regressions above 5% without an ADR.

### Gate 3
`just gate 3` with `ci` profile. Coverage >= 95% lines and branches on `TicklineVault`.

---

## Phase 4: Engine core (`ledger`, `market`, `api`)

**Goal:** a seller that accepts vouchers, prices fills, issues receipts, and never lets its ledger
drift, under concurrency.

### Build
- **Ledger:** append-only double-entry tables for accounts (agent session, market collateral,
  operator fees, creator subsidy, operator wallet), entries, fills, receipts, vouchers. One Postgres
  transaction per fill.
- **Session actor** (per agent escrow): serializes voucher acceptance, reserves the ceiling against
  escrow headroom (I7), captures actual cost + fee after the fill, releases the remainder.
- **Market actor** (per market): owns LMSR state, executes fills, advances epochs from the injected
  `Clock`, halts trading at deadline.
- Actor state is updated only after the ledger transaction commits. On startup, state is rebuilt by
  replaying the ledger (I9).
- **API (axum):**
  - `GET /v1/markets` free listing.
  - `POST /v1/markets` paid: subsidy.
  - `POST /v1/markets/:id/fills` paid: body `{ outcome, shares, maxCost }`; 402 advertises ceiling;
    returns receipt, signature, new price.
  - `GET /v1/markets/:id/price` paid: small read fee.
  - `GET /v1/agents/:addr/markets/:id/receipt` free: latest receipt.
  - `GET /health`, `GET /metrics`.
- Escrow balances come from a trait; this phase uses an in-memory implementation seeded by tests.
  Phase 5 replaces it with the indexer.

### Tests
- **Unit:** fee math, epoch derivation from `Clock`, slippage rejection, deadline halt.
- **Integration (`#[sqlx::test]`):** fill lifecycle writes balanced entries (I8); receipt fields
  match ledger (I5, I6).
- **Property:** random fill sequences across random agents and markets; after each step assert I4,
  I5, I6, I8; at the end, drop all actors, replay the ledger, assert identical state (I9).
- **Concurrency:** 50 agents x 5 markets firing concurrent fills, including the same agent across
  markets at once. Assert no voucher accepted twice, no reservation leak, ledger balanced, final
  cumulative per session equals sum of captured amounts.
- **Adversarial API:** replayed voucher, stale cumulative (lower than accepted), voucher above escrow
  headroom, wrong chain id, wrong payee, expired session, ceiling below actual cost, fill after
  deadline, malformed headers, unknown market. Each returns the correct status and a typed error,
  and writes zero ledger entries.
- **Contract tests:** 402 challenge bodies snapshot-tested against `docs/spec-notes.md` examples.
- **Crash:** kill the process between ledger commit and response; restart; the receipt is
  recoverable via the receipt endpoint and state matches replay.

### Gate 4
`just gate 4`. Coverage >= 85% on `ledger` and `market`.

---

## Phase 5: Settlement worker and indexer (`settlement`, `indexer`)

**Goal:** offchain state and chain state agree to the base unit through crashes and reorgs.

### Build
- **Indexer:** subscribes to escrow and vault events, stores blocks with hashes, applies events at
  confirmation depth `N` (config), detects reorgs by parent-hash mismatch, rolls back and reapplies.
  Replaces the Phase 4 in-memory escrow balance source.
- **Settlement worker:**
  - Redeems vouchers in batches when accrued value exceeds a gas-cost threshold or max age.
  - Commits each epoch before `epochEnd + commitGrace` with changed positions and collateral delta.
  - Uses an outbox table: intent row written before tx submission, tx hash recorded, confirmation
    recorded by the indexer. Resubmission is keyed by intent, never by memory (I14).
- **Reconciler:** periodic job comparing ledger totals with indexed onchain totals; mismatch raises
  an alert metric and halts new fills on the affected market (I13).

### Tests (anvil-backed)
- **Integration:** redeem and commit a batch; indexer observes; reconciler reports zero drift.
- **Reorg injection:** use anvil snapshot/revert and reorg RPCs to drop blocks containing a commit and
  a deposit; assert rollback, resubmission, and final zero drift. Include a reorg deeper than `N`
  that must halt and alert rather than silently diverge.
- **Crash injection:** fail-points at every outbox transition (before submit, after submit before
  hash saved, after hash saved before confirmation). Restart each time; assert no double redemption,
  no double commit, eventual convergence (I14).
- **Deadline pressure:** clock advanced near `epochEnd + commitGrace` with a slow RPC; the worker
  prioritizes the commit and the test asserts it lands in time.
- **Property:** random interleavings of fills, redemptions, commits, reorgs, and restarts; I13 holds
  at quiescence.

### Gate 5
`just gate 5`. Coverage >= 85% on `settlement` and `indexer`.

---

## Phase 6: Agents and end-to-end (`agents/`, `e2e/`)

**Goal:** the official x402 TS SDK drives the Rust seller through full market lifecycles.

### Build
- **Creator agent:** opens a session, creates a Pyth threshold market, polls price.
- **Forecaster agents:** three strategies (momentum on MockPyth feed, mean reversion, noisy random)
  with seeded randomness.
- **Reader agent:** pays for price reads only.
- **E2E harness:** boots anvil, deploys contracts, runs migrations, starts engine, runs a scenario
  file, tears down. Scenarios are declarative YAML with expected final balances.

### Scenarios (each asserts every agent's final USDC balance to the base unit)
1. Happy path: create, 400 fills across 3 forecasters, commits, resolve YES, claims, creator refund.
2. Resolve NO with identical trading: mirror balances.
3. Dishonest operator: engine test hook under-commits one agent; agent calls `claimWithReceipt`;
   bond slashed; agent made whole.
4. Missed commit: settlement worker disabled past grace; market fails; receipts reclaim cost.
5. Agent escrow exhausted mid-session: fills rejected with 402, no receipt issued, balances intact.
6. Reorg during trading plus engine restart mid-epoch: final balances unchanged versus scenario 1.

### Tests
- Each scenario runs 10 consecutive times in CI with fixed seeds, 10 of 10 must pass.
- A vitest suite for agent strategy logic with the engine stubbed at the HTTP boundary only.

### Gate 6
`just gate 6` including `just e2e`.

---

## Phase 7: Hardening

**Goal:** numbers for the README and confidence under hostile conditions.

### Work
- `just deep` across all proptest, fuzz, and invariant suites. Any counterexample becomes a
  permanent unit test before the phase closes.
- `just mutants` on `lmsr`, `protocol`, `ledger`, `market`, `settlement`. Surviving mutants are
  killed with new tests or documented as equivalent.
- **Load test:** load agent ramps to sustained fills; record p50, p95, p99 fill latency, fills per
  onchain transaction, gas per 1,000 fills, ledger write throughput. Assert zero drift at the end.
- **Adversarial suite:** fill flooding from one session, many sessions from one funder, oversized
  bodies, slowloris connections, voucher signature grinding. Each has an expected defense and a test.
- **Observability:** tracing spans across API, actors, ledger, settlement; Prometheus metrics for
  fills, rejections by reason, reconciliation drift, commit lateness; one Grafana dashboard JSON.

### Gate 7
`just gate 7` plus `just deep` and `just mutants` green. Benchmarks recorded in `docs/STATUS.md`.

---

## Phase 8: Testnet, dashboard, demo

**Goal:** a live, recordable demo on Base Sepolia.

### Build
- Deploy scripts with verification; addresses recorded in `deployments/base-sepolia.json`.
- **Fork test suite** (`contracts/test/fork/`): vault against real escrow and real Pyth on Base
  Sepolia fork. This is the only suite allowed network access.
- **Minimal dashboard (Next.js):** market list, price curve, fills vs onchain transactions counter,
  creator cost vs subsidy cap, links to explorer transactions. Read-only.
- README with architecture diagram, invariant table, test pyramid with counts, and Phase 7 numbers.
- Demo script: creator asks, forecasters trade, reader pays for price, resolve, claims, dishonest
  operator slashed.

### Tests
- Fork suite green.
- Post-deploy smoke script: create market, one fill, one commit, resolve with a real Pyth update on
  a short-deadline market, claim.
- Dashboard: one Playwright test that loads a live market and sees the counter increment.

### Gate 8
`just gate 8` plus the smoke script against the live deployment.

---

## Phase 9 (stretch): Uniswap TWAP resolution template

Only after Phase 8 ships. Requires an ADR with a manipulation cost analysis (cost to move the TWAP
over the window versus maximum subsidy per market) before any code. Tests must include a simulated
manipulation attempt that the subsidy cap makes unprofitable.