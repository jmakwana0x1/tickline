# Tickline

A prediction market priced **per HTTP request** and paid with **x402 batch settlement**.

Agents open a payment session, then pay per request. Each paid request is priced by an offchain
binary LMSR market maker and answered with an EIP-712 receipt signed by the operator. Positions
are aggregated and committed onchain once per epoch, so **thousands of fills cost a handful of
transactions**. At the deadline the market resolves against a Pyth price and agents claim from
the vault.

The operator holds the pricing, so the operator is the thing that must not be trusted. Every
agent leaves each request holding a signed receipt. If the committed position is smaller than
the receipt, the receipt wins and the operator's bond pays — with no timing, ordering, or gas
condition under which it does not.

> **Status: Phase 0 of 9 — spec capture and scaffold.** Nothing here prices real money yet.
> `PHASES.md` is the build plan; `docs/STATUS.md` is where the build actually is.

---

## Start here

| If you want to | Read |
|---|---|
| Understand how work happens here | [`CLAUDE.md`](CLAUDE.md) — the working agreement |
| See what gets built, and when | [`PHASES.md`](PHASES.md) |
| Know what is actually done | [`docs/STATUS.md`](docs/STATUS.md) |
| Know what must never break | [`CLAUDE.md` §4](CLAUDE.md#4-invariants) — the fifteen invariants |
| Know why something is the way it is | [`docs/decisions/`](docs/decisions/) |
| Work on the repo itself | [`docs/GITHUB.md`](docs/GITHUB.md) |

## Getting set up

```bash
scripts/dev-setup.sh     # rust, just, foundry, node, pnpm, sqlx-cli — idempotent
just doctor              # what is installed, and the install line for what is not
just up                  # postgres + anvil + migrations
just test                # every stack
just gate "$(cat .phase)"   # the only definition of "done"
```

`just` on its own lists every command. Nothing in CI runs a command you cannot run locally by
the same name.

## Layout

```
engine/      Rust: the seller. lmsr · protocol · ledger · market · api · settlement · indexer
contracts/   Foundry: TicklineVault. Unit, fuzz, and handler-based invariant suites
agents/      TypeScript: x402 clients and the cross-stack vector generator
e2e/         Declarative scenarios, each asserting final balances to the base unit
tools/       A 60-digit mpmath oracle the Rust math is diffed against
docs/        The spec notes, the status, and every ADR
```

Full annotated tree in [`CLAUDE.md` §3](CLAUDE.md#3-repository-layout).

## How it is tested

Money software earns trust by the kinds of failure it has already survived, so the suite is
built around failure kinds rather than coverage percentages:

- **Differential** — Rust LMSR against 60-digit `mpmath`, to within 1 wei, rounding toward the
  vault. Solidity against the same vectors.
- **Cross-stack** — Rust, Solidity, and TypeScript must produce byte-identical EIP-712 digests
  from one committed vector file.
- **Property** — random fill sequences, random agents, random markets, with invariants asserted
  after every step.
- **Invariant (Foundry)** — handler-based, including a deliberately dishonest operator.
- **Concurrency** — 50 agents × 5 markets firing at once: no voucher accepted twice, no
  reservation leaked.
- **Crash and reorg injection** — fail-points at every outbox transition; reorgs deeper than the
  confirmation depth must halt and alert rather than quietly diverge.
- **Mutation** — surviving mutants on public functions are a gate failure, not a metric.

A flaky test is a bug, never noise to retry. A counterexample becomes a permanent unit test
before its phase can close.

## Licence

MIT.
