# Tickline. Every supported command lives here (CLAUDE.md section 6).
# CI never runs anything a developer cannot run locally by the same name.

set shell := ["bash", "-euo", "pipefail", "-c"]
set dotenv-load := true

export DATABASE_URL := env_var_or_default("DATABASE_URL", "postgres://tickline:tickline@localhost:5433/tickline")
export PROPTEST_CASES := env_var_or_default("PROPTEST_CASES", "256")
export FOUNDRY_PROFILE := env_var_or_default("FOUNDRY_PROFILE", "default")
export RUST_BACKTRACE := env_var_or_default("RUST_BACKTRACE", "1")

engine := justfile_directory() / "engine"
contracts := justfile_directory() / "contracts"

# List every command.
default:
    @just --list --unsorted

# ---------------------------------------------------------------- environment

# Check the toolchain and print install commands for anything missing.
doctor:
    @bash scripts/doctor.sh

# Start Postgres and anvil.
up:
    docker compose up -d --wait postgres
    @bash scripts/anvil.sh start
    @just migrate

# Stop Postgres and anvil.
down:
    @bash scripts/anvil.sh stop
    docker compose down -v

# Apply sqlx migrations.
migrate:
    cd {{engine}} && sqlx database create && sqlx migrate run

# ---------------------------------------------------------------- build & lint

build: build-rust build-sol build-ts

build-rust:
    cd {{engine}} && cargo build --workspace --all-targets

build-sol:
    cd {{contracts}} && forge build

build-ts:
    pnpm install --frozen-lockfile && pnpm -r build

fmt:
    cd {{engine}} && cargo fmt --all
    cd {{contracts}} && forge fmt
    pnpm exec prettier --write .

fmt-check:
    cd {{engine}} && cargo fmt --all -- --check
    cd {{contracts}} && forge fmt --check
    pnpm exec prettier --check .

lint:
    cd {{engine}} && cargo clippy --workspace --all-targets --all-features -- -D warnings
    cd {{contracts}} && forge build --deny warnings
    pnpm -r lint
    @bash scripts/test-no-em-dash.sh
    @bash scripts/check-no-em-dash.sh
    @bash scripts/shellcheck.sh
    @bash scripts/test-no-stray-files.sh

# Assert lmsr and protocol stay zero-IO (CLAUDE.md section 3).
deps-check:
    @bash scripts/check-crate-boundaries.sh

# ---------------------------------------------------------------- tests

# Everything, in dependency order.
test: test-rust test-db test-sol test-ts

test-rust:
    cd {{engine}} && cargo test --workspace --all-targets --no-fail-fast

# Needs `just up`.
test-db:
    cd {{engine}} && cargo test -p ledger --features db-tests --no-fail-fast

test-sol:
    cd {{contracts}} && FOUNDRY_PROFILE=ci forge test -vvv

test-ts:
    pnpm -r test

# Full scenario harness: anvil + contracts + engine + agents.
e2e:
    pnpm --filter @tickline/e2e run e2e

# Load and adversarial campaigns (gate 7).
e2e-load:
    pnpm --filter @tickline/e2e run load

# Coverage, per crate and per contract.
cover:
    cd {{engine}} && cargo llvm-cov --workspace --summary-only
    cd {{contracts}} && FOUNDRY_PROFILE=ci forge coverage --report summary

# ---------------------------------------------------------------- deep & adversarial

# Long campaigns. Nightly, and required by gate 7.
deep:
    cd {{engine}} && PROPTEST_CASES=20000 cargo test --workspace --release --all-features
    cd {{contracts}} && FOUNDRY_PROFILE=deep forge test

# Mutation testing. Pass crates: `just mutants lmsr protocol`.
mutants *crates:
    cd {{engine}} && cargo mutants --no-shuffle {{ if crates == "" { "" } else { "-p " + replace(crates, " ", " -p ") } }}

# Regenerate cross-stack vectors. Must leave an empty git diff.
vectors:
    uv run --no-project --with-requirements tools/reference/requirements.txt python3 tools/reference/lmsr_ref.py --out testdata/vectors/lmsr.json
    pnpm --filter @tickline/agents run vectors
    @git diff --exit-code testdata/vectors || { echo "vectors drifted; commit or fix the generator"; exit 1; }

slither:
    cd {{contracts}} && slither . --config-file slither.config.json

snapshot *args:
    cd {{contracts}} && FOUNDRY_PROFILE=ci forge snapshot {{args}}

# ---------------------------------------------------------------- gates

# Prove gate.sh stops at every failing step (runs inside gate 0).
test-gate:
    @bash scripts/test-gate.sh

# Run phase n's gate and every earlier phase's gate. The only definition of done.
gate n:
    @bash scripts/gate.sh {{n}}

# Gate every closed phase: what per-PR CI runs (ADR-0006). Phase 0 is gated while in progress,
# because its gate has no unbuilt steps.
gate-closed:
    @phase="$(cat .phase)"; bash scripts/gate.sh "$(( phase > 0 ? phase - 1 : 0 ))"

# ---------------------------------------------------------------- github

gh-bootstrap:
    @bash scripts/gh-bootstrap.sh

gh-verify:
    @bash scripts/gh-verify.sh
