# CLAUDE.md: Tickline working agreement

This file is the contract between Jay and Claude. `PHASES.md` says *what* gets built and in what
order; this file says *how*, *where*, and *what must never break*. When the two disagree, the
narrower statement wins and the conflict goes to Jay as a `needs-jay` issue.

---

## 1. What Tickline is

Tickline is a **prediction market priced per HTTP request and paid with x402 batch settlement**.

An operator runs an offchain seller (the *engine*). AI agents open an x402 payment session by
depositing USDC into the escrow contract, then pay per request with signed vouchers. Each paid
request to `POST /v1/markets/:id/fills` is priced by a **binary LMSR** market maker, and the engine
returns an EIP-712 **`PositionReceipt`** signed by the operator key. The engine periodically
**commits** aggregated positions to `TicklineVault` onchain and redeems accrued vouchers in batches.
At the deadline the market resolves against a **Pyth** price, and agents claim from the vault.

Three properties make it interesting, and each is a thing that can break:

1. **Thousands of fills, a handful of transactions.** Pricing is offchain; only aggregates land
   onchain. The vault must therefore never trust the operator further than the operator's bond.
2. **The ceiling.** x402 batch settlement lets a client sign an "up to" amount; the seller captures
   actual usage. A fill whose price moves between quote and execution must never capture more than
   was signed, and must never leave the unused remainder reserved.
3. **A lying operator is always slashable.** Every agent holds a signed receipt. If the committed
   position is smaller than the receipt, the receipt wins and the bond pays.

**Money moves in this system.** Every design choice resolves toward: lose no funds, pay no one
twice, and prefer halting to drifting.

### Locked design decisions (change only via `needs-jay`)

- Positions are stored per `(market, payer)` in `TicklineVault` storage. Each commit writes only payers whose position changed that epoch, and totals update incrementally. No Merkle roots.
- Binary markets, buy-only, held to resolution.
- One payment path: creation subsidy, fills, and price reads are all paid with batch-settlement vouchers.
- LMSR pricing is offchain only. The vault enforces solvency, monotonic positions, resolution, claims, and slashing, and never prices a trade.
- Resolution is machine-only. The MVP template is a Pyth price threshold.
- The operator floats settlement: claimed funds settle to the operator wallet, and `commitEpoch` pulls each market's collateral delta into the vault.
- No x402 facilitator. Vouchers are verified locally; only EOA payer authorizers are accepted.

What is deliberately not being built is listed in section 10.

---

## 2. How Claude works here

**Test-first, always.** Every slice starts with a commit containing a failing test that names the
behaviour. No production code is written before a red test exists for it. "Red commit, then green
commit" is visible in the PR history, and a reviewer must be able to see the test fail.

**One issue at a time.** A branch maps to exactly one issue. If work reveals new work, open an
issue; do not widen the branch.

**Never weaken a test to make it pass.** Loosening a tolerance, deleting an assertion, adding
`#[ignore]`, `skip`, `only`, or `vm.skip` is a `needs-jay` conversation, not a commit. CI enforces
this mechanically (`scripts/check-no-skipped-tests.sh`).

**Determinism over convenience.** No wall-clock reads outside an injected `Clock`. No unseeded
randomness in tests or agents. No network access in any suite except `contracts/test/fork/`.

**Stop and ask** (open a `needs-jay` issue and stop) when any of these is true:
- a spec question cannot be answered from a cited source in `docs/spec-notes.md`;
- an invariant in section 4 would need to change;
- a fix requires touching a phase that is already gated;
- an ADR-worthy design choice appears (anything a future reader would ask "why?" about);
- a test is flaky. A flaky test is a bug in the system or the test, never noise to retry;
- adding a dependency not already in the workspace manifests;
- changing a public interface: HTTP API, contract ABI, or EIP-712 type;
- changing what `required` enforces: removing or renaming a job it needs, changing the commands a required job runs, or adding conditions that can skip it;
- changing the ruleset, workflow permissions, or secrets;
- adding a third-party action;
- changing `phase-gate.yml`'s commands;
- modifying or deleting an existing test.

Merge without `needs-jay`, with a one-line note in the PR body, for: adding a service container or cache to an existing job, pinning or bumping a tool version, and changes to non-required workflows (except `phase-gate.yml`) that add no permissions or secrets.

**Write the ADR before the code**, not after, whenever the choice is structural. `docs/decisions/`.

**Guessing is banned in three places**: the x402 wire format, EIP-712 field order and types, and
rounding direction. Each has a cited source or a cross-stack vector file. If neither exists yet,
that is the work.

---

## 3. Repository layout

```
tickline/
├── CLAUDE.md                     This file. The working agreement.
├── PHASES.md                     Build plan, phase by phase.
├── README.md                     Public face. Filled in Phase 8.
├── .phase                        Current phase number. Gate scripts read it.
├── justfile                      Every command anyone runs (section 6).
├── docker-compose.yml            Postgres for local dev and tests.
│
├── engine/                       Rust workspace: the seller.
│   ├── Cargo.toml                Workspace root, shared lints and deps.
│   ├── migrations/               sqlx migrations, applied by `just migrate`.
│   └── crates/
│       ├── lmsr/                 Phase 1. Fixed-point LMSR. Zero IO, zero deps.
│       ├── protocol/             Phase 2. Voucher + receipt types, EIP-712, x402 envelopes.
│       ├── ledger/               Phase 4. Append-only double-entry ledger over Postgres.
│       ├── market/               Phase 4. Market actor: LMSR state, fills, epochs.
│       ├── api/                  Phase 4. axum HTTP surface + the engine binary.
│       ├── settlement/           Phase 5. Redemption + epoch commit worker, outbox.
│       └── indexer/              Phase 5. Chain follower, reorg-aware.
│
├── contracts/                    Foundry project: the vault.
│   ├── src/                      TicklineVault and friends.
│   ├── test/                     Unit, fuzz, invariant suites.
│   │   ├── invariant/            Handler-based invariant suites (section 4 IDs).
│   │   └── fork/                 Phase 8. The ONLY suite allowed network access.
│   ├── script/                   Deploy + verify scripts.
│   └── lib/                      Vendored/pinned dependencies (git submodules).
│
├── agents/                       TypeScript: x402 clients + the EIP-712 vector generator.
├── e2e/                          TypeScript: scenario harness (anvil + engine + agents).
│   └── scenarios/                Declarative YAML, each with expected final balances.
├── dashboard/                    Phase 8. Next.js, read-only.
│
├── tools/reference/              Python oracles (mpmath). Generate, never import at runtime.
├── testdata/vectors/             Generated cross-stack vectors. Committed, diffed in CI.
├── deployments/                  Phase 8. Deployed addresses per network.
│
├── docs/
│   ├── GITHUB.md                 The GitHub workflow in full detail.
│   ├── STATUS.md                 Current phase, pinned versions, gate log, open questions.
│   ├── spec-notes.md             Cited findings on x402, Pyth, escrow. Phase 0 deliverable.
│   └── decisions/                ADRs, numbered, never edited after acceptance.
│
├── scripts/                      Shell entry points the justfile calls.
└── .github/                      CI, templates, branch ruleset JSON.
```

**Layout rules.**
- A crate may depend only on crates to its left in the list above. `lmsr` depends on nothing in the
  workspace; `protocol` may not depend on `ledger`. Cycles are a build error and a design error.
- `lmsr` and `protocol` are **zero-IO**: no `tokio`, no `sqlx`, no `reqwest`, no clock, no `std::time`.
  This is what makes them exhaustively testable, and it is enforced in CI by a dependency check.
- Anything generated lives under `testdata/` and is committed. Generators are deterministic; `just
  vectors` must produce an empty `git diff`.

---

## 4. Invariants

These are the things that must be true after **every** state change. Each has a stable ID, an owner
(the layer that enforces it), and at least one test that fails if it is removed. Tests reference
these IDs by name so a failure points straight back here.

An invariant is never "mostly" true. If one cannot hold, the system **halts** that market rather
than continuing in an unknown state.

### Math: enforced by `engine/crates/lmsr` and mirrored in Solidity

| ID | Statement |
|---|---|
| **I2** | **Bounded cost.** `max(qY, qN) <= C(qY, qN) <= max(qY, qN) + b·ln2`. The market maker's total subsidy exposure is bounded by `b·ln2` regardless of the fill sequence. |
| **I3** | **Bounded loss.** `subsidy >= C(0,0)`, and for any sequence of fills, `subsidy + collected >= max payout`. The creator's worst case is known before the first trade. |
| **I15** | **Rounding favours the vault.** Every WAD → USDC base-unit conversion rounds **costs up** and **shares down**. Never the reverse, never "round half to even", no exceptions for "it's only a wei". |

### Onchain: enforced by `TicklineVault`

| ID | Statement |
|---|---|
| **I1** | **Solvency.** For every market, collateral held >= the maximum payout over both outcomes. Checked on every `commitEpoch`, and as a Foundry invariant (`invariant_I1_solvency`). Its corollary, `invariant_total_paid_never_exceeds_collateral_plus_bond`, holds across all markets at once. |
| **I10** | **Positions are monotonic.** A committed position for `(market, agent, outcome)` never decreases between epochs. A commit that lowers any position reverts. |
| **I11** | **No double claim.** A `(market, agent)` pair is paid at most once. `claim` and `claimWithReceipt` share one `claimed` flag. |
| **I12** | **A dishonest commit is always slashable.** If the operator commits shares below those in a valid signed receipt whose epoch <= the final committed epoch, the receipt holder is made whole from the bond and the operator is slashed. There is no ordering, timing, or gas condition under which the receipt loses. |

### Engine: enforced by `ledger`, `market`, `api`

| ID | Statement |
|---|---|
| **I4** | **Market state matches its fills.** A market's `(qY, qN)` equals the sum of all fills recorded in the ledger, and collateral collected equals `C(q) − C(0,0)` plus fees, to the base unit. |
| **I5** | **Receipts are faithful.** Every field of an issued receipt equals the ledger's cumulative totals for that `(market, agent)` at that receipt's nonce. A receipt is never issued from in-memory state alone. |
| **I6** | **Receipts are monotonic and unique.** Per `(market, agent)`: nonce strictly increases, every amount field is non-decreasing, exactly one receipt exists per accepted fill, and every receipt verifies against the operator's registered signing key. |
| **I7** | **Escrow headroom is never exceeded.** A voucher is accepted only if its channel has no pending withdrawal and `totalClaimed <= chargedCumulativeAmount <= signedMaxClaimable <= balance`, where `totalClaimed` and `balance` are onchain state indexed at confirmation depth. `chargedCumulativeAmount` advances only after the resource handler succeeds, and request processing is serialized per channel. Once a withdrawal is pending, `totalClaimed` must equal `chargedCumulativeAmount` before `finalizeAfter`. |
| **I8** | **The ledger balances.** Every ledger transaction is double-entry balanced (entries sum to zero), append-only (no `UPDATE`, no `DELETE`), and no account constrained non-negative ever goes negative. |
| **I9** | **Replay is deterministic.** Dropping all actors and replaying the ledger from empty reproduces byte-identical actor state. Actor memory is a cache of the ledger, never a source of truth. |
| **I16** | **Money before shares.** A `PositionReceipt` is issued only in the same ledger transaction that advances `chargedCumulativeAmount` by that fill's `cost + fee`. For every payer, the sum of `costPaid + feesPaid` across their latest receipts is <= the sum of `chargedCumulativeAmount` across their channels. |

The last sentence of I7 is an operational deadline rather than a pure state check. It is asserted
by the Phase 5 deadline-pressure test and by an alert metric in production. I16 is owned by
`ledger` and `market`, asserted by the Phase 4 property tests after every step, and at quiescence
in every Phase 6 scenario. IDs are never renumbered; a new invariant takes the next free number.

### Settlement: enforced by `settlement`, `indexer`

| ID | Statement |
|---|---|
| **I13** | **Offchain and onchain agree.** Indexed onchain totals equal ledger totals for every market. Any drift raises an alert metric and **halts new fills on that market**. Drift is never "reconciled away" by adjusting the ledger. |
| **I14** | **Chain actions are idempotent.** Every chain-mutating action is driven by an outbox intent row written before submission. Resubmission after a crash at any point produces at most one effective onchain action. Idempotency is keyed by the intent, never by process memory. |

---

## 5. Stack and pinned versions

Everything is pinned. "Latest" is not a version. Pins live in `docs/STATUS.md` and in the manifests;
`just doctor` reports drift. Upgrades are their own PR with their own gate run, never a drive-by.

| Layer | Choice | Why this one |
|---|---|---|
| Engine | Rust, `tokio`, `axum` | One language for money math, actors, and HTTP; property and mutation testing are first-class. |
| Numerics | Fixed-point signed WAD (1e18) | Floats are not allowed near money. `exp`/`ln` ported from Solady so Rust and Solidity share one numerical method. |
| Store | Postgres, `sqlx` | Serializable transactions, `#[sqlx::test]` gives a real database per test. |
| Contracts | Solidity, Foundry | Fuzzing and handler-based invariant testing in the same tool as the unit tests. |
| Oracle | Pyth | Pull-based updates let resolution name an exact timestamp. |
| Clients | TypeScript, official x402 SDK | The SDK is the spec's own client; if our seller satisfies it, we read the spec right. |
| Reference | Python + `mpmath` (60 digits) | An independent implementation to diff against. Never imported at runtime. |

**Local toolchain.** `just doctor` checks every tool and prints the install command for what is
missing. `scripts/dev-setup.sh` installs the lot on a fresh Linux/WSL box.

---

## 6. Commands

The justfile is the only supported interface. If a command is worth running twice, it belongs here.
Nothing in CI runs a command that a developer cannot run locally by the same name.

### Daily
| Command | Does |
|---|---|
| `just` | List everything. |
| `just doctor` | Check the toolchain; print install commands for what is missing. |
| `just up` / `just down` | Start/stop Postgres (docker compose) and anvil. |
| `just migrate` | Apply sqlx migrations. |
| `just build` | Build every stack. |
| `just fmt` / `just fmt-check` | Format / verify formatting (Rust, Solidity, TS). |
| `just lint` | clippy `-D warnings`, forge fmt check, eslint, shellcheck. |

### Tests
| Command | Does |
|---|---|
| `just test` | Everything below, in dependency order. |
| `just test-rust` | Workspace unit + property tests. |
| `just test-db` | `#[sqlx::test]` suites (needs `just up`). |
| `just test-sol` | `forge test`, `ci` profile. |
| `just test-ts` | vitest across the pnpm workspace. |
| `just e2e` | Scenario harness: anvil + contracts + engine + agents. |
| `just cover` | Coverage for Rust and Solidity; prints per-crate line/branch numbers. |

### Deep and adversarial
| Command | Does |
|---|---|
| `just deep` | Long-running fuzz, proptest, and invariant campaigns (`deep` profile, `PROPTEST_CASES` raised). |
| `just mutants` | `cargo mutants` over the crates listed per phase. |
| `just vectors` | Regenerate `testdata/vectors/`. Must produce an empty `git diff`. |
| `just slither` | Static analysis on `contracts/`. |
| `just snapshot` | `forge snapshot`; gate fails on >5% gas regression without an ADR. |

### Gates and release
| Command | Does |
|---|---|
| `just gate <n>` | Run phase `n`'s gate **and every earlier phase's gate**. The only definition of "done". |
| `just gate-closed` | Gate every closed phase. What per-PR CI runs (ADR-0006). |
| `just gh-bootstrap` | Create labels, milestones, and the branch ruleset on GitHub. |
| `just gh-verify` | Assert GitHub is configured exactly as `docs/GITHUB.md` says. |

**`just gate <n>` always reruns phases 0..n.** A phase is never "finished" in a way that lets a later
phase quietly break it. Per-PR CI runs `just gate-closed`, so finished phases stay green on every
change; the in-progress phase's gate runs in the `phase-gate` workflow when the phase closes
(ADR-0006). The gate is the same locally and in CI; a gate that passes locally and fails
in CI is a `needs-jay` bug in the gate itself.

---

## 7. The GitHub loop

Full detail (exact commands, templates, ruleset contents) lives in `docs/GITHUB.md`. The shape:

1. **Phase opens.** Create the milestone and a tracking issue from the phase template. Break the
   phase's Build and Tests sections into slice issues whose acceptance criteria *are test names*.
   Label the tracking issue `needs-jay` and stop until Jay approves the breakdown.
2. **Per slice.** Branch `<phase>/<issue#>-<slug>` → commit the failing test → open a **draft** PR
   linking `Closes #<issue>` → make it green → `just gate-closed` → self-review the diff line by line →
   mark ready → merge via squash once `required` is green (merge authority below).
3. **Phase closes.** Run the `phase-gate` workflow (`just gate <n>` in CI), post its log to the tracking issue, get Jay's explicit go-ahead, bump
   `.phase`, update `docs/STATUS.md`, close the milestone, `gh release create phase-<n>`.

**Merge authority.** Claude merges its own PR with `gh pr merge --squash --delete-branch` once
`required` is green, unless the PR is labeled `needs-jay` or hits a stop-and-ask trigger (section 2).
In those cases Claude labels the PR `needs-jay` and stops. The ruleset requires no approving review
(ADR-0003), so this rule, together with `required`, is the merge gate.

**Authorization.** Jay's decisions are recorded on GitHub, on the PR or issue they apply to.

| Jay comments | Claude does |
|---|---|
| `/approve <sha>` | Merges once `required` is green, if the PR head is still `<sha>`. |
| `/approve-after <sha>` followed by required changes | Implements exactly those changes, gets `required` green, merges, then comments listing what changed. |
| `/changes` followed by requests | Implements them, keeps `needs-jay`, and waits for a new `/approve`. |
| `/reject` followed by a reason | Closes the PR or issue, linking the reason, and does not reopen it. |

Rules:
- An approval covers only the named commit. If the head moves for any reason other than the listed changes, or a rebase that leaves the diff unchanged, the approval is void.
- For a decision Jay gives in a Claude Code session, Claude first posts it on the PR or issue as `Recorded from session with Jay:` followed by his words, then acts on it.
- Claude removes `needs-jay` when it acts on an approval, never before.
- Claude never posts a comment that starts with `/approve`, `/approve-after`, `/changes`, or `/reject`. Claude Code pushes and comments with Jay's token, so GitHub cannot tell the two apart. This rule is the control.

ADR-0007 records this protocol and the enforcement it does not yet have.

**Hard rules.**
- `main` is protected: no direct pushes, linear history, the `required` aggregator check must pass.
  Phase 0's single bootstrap commit is the one recorded exception.
- Every PR closes an issue. Every issue belongs to a milestone.
- Secrets never enter the repo; gitleaks runs pre-commit and in CI.
- Task state lives in **GitHub issues**, not in markdown. `docs/STATUS.md` records gates, versions,
  and open questions. It is not a to-do list.

---

## 8. Definition of done

A slice is done when **all** of these are true. Not most.

- [ ] The PR history shows the test failing before it passes.
- [ ] Acceptance criteria from the issue exist as named, passing tests.
- [ ] Every new error path has a test. Every custom error has a test that triggers it.
- [ ] Every invariant the change touches is named by ID in a test.
- [ ] Nothing is ignored, skipped, or `only`. No commented-out test.
- [ ] `just gate-closed` is green in CI, and the slice's own tests pass locally and in CI.
- [ ] Rounding direction is stated in a comment wherever a conversion happens (I15).
- [ ] Public functions carry doc comments naming domain limits and error conditions.
- [ ] An ADR exists if a future reader would ask "why was it done this way?".
- [ ] The diff has been self-reviewed line by line, as a reviewer looking for a way to lose money.

---

## 9. Glossary

| Term | Meaning |
|---|---|
| **Voucher** | A client-signed x402 batch-settlement authorization: pay *up to* a ceiling, cumulatively, from a funded session. |
| **Ceiling** | The "up to" amount signed per request. Actual charge is <= ceiling; the remainder is never charged and never reserved. |
| **Session** | One x402 batch-settlement channel. An agent runs concurrent sessions by varying the channel `salt`. |
| **Fill** | One priced trade against the LMSR: shares in, cost out, receipt issued. |
| **Epoch** | A fixed time slice. Positions are aggregated per epoch and committed once. |
| **Commit** | The operator writing an epoch's aggregated positions and collateral to the vault. |
| **Receipt** | `PositionReceipt`, EIP-712 signed by the operator. The agent's proof against a lying commit. |
| **Bond** | Operator collateral backing receipts. Pays out and slashes under I12. |
| **Subsidy** | The creator's `b·ln2` worst-case funding of the market maker (I3). |
| **Drift** | Any disagreement between ledger and chain. Halts, never reconciles silently (I13). |
| **Gate** | `just gate <n>`: phase `n` and all earlier phases green. |

---

## 10. Out of scope (do not build, do not scaffold)

Selling positions before resolution. Order books. Multi-outcome markets. Human or UMA-style resolution. Merkle position roots. Running or depending on an x402 facilitator. EIP-1271 payer authorizers. A token. A human trading UI. MCP servers. Cross-chain anything. KMS key management. Multi-operator support. TWAP templates before Phase 9.

Anything on this list that starts to look necessary is a `needs-jay` issue, not a branch.
