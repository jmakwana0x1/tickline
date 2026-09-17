#!/usr/bin/env bash
# just gate <n>: run phase n's gate and every earlier phase's gate.
# A phase is never "done" in a way that lets a later phase quietly break it.
set -euo pipefail

cd "$(dirname "$0")/.." || exit 1

TARGET="${1:?usage: just gate <n>}"
[[ "$TARGET" =~ ^[0-9]$ ]] || { echo "phase must be 0-9, got '$TARGET'"; exit 2; }

CURRENT="$(cat .phase)"
if (( TARGET > CURRENT )); then
  echo "refusing: .phase is $CURRENT, cannot gate phase $TARGET before entering it" >&2
  exit 2
fi

START="$(date -u +%s)"
step() { printf '\n\033[1m── gate %s › %s\033[0m\n' "$PHASE" "$*"; }

gate_0() {
  # `just doctor` is deliberately NOT here: it checks the developer's machine, and CI
  # runners carry a different toolchain on purpose. The gate checks the commit.
  step "gate self-test"       ; bash scripts/test-gate.sh
  step "format"               ; just fmt-check
  step "lint"                 ; just lint
  step "crate boundaries"     ; just deps-check
  step "no skipped tests"     ; bash scripts/check-no-skipped-tests.sh
  step "smoke: rust"          ; just test-rust
  step "smoke: postgres"      ; just test-db
  step "smoke: solidity"      ; just test-sol
  step "smoke: typescript"    ; just test-ts
  step "github configuration" ; just gh-verify
}

# Rules for every gate function below (issue #17, enforced by scripts/test-gate.sh):
#   - one command per step, never `a && b`. Under `set -e`, bash does not exit when a
#     non-final command in an `&&` list fails, so the gate would carry on past it;
#   - never `cd`. Point tools at their project instead (`--manifest-path`, `--root`), so a
#     failed step cannot leave later steps running from the wrong directory.
ENGINE=(--manifest-path engine/Cargo.toml)

gate_1() {
  step "lmsr suites"          ; cargo test "${ENGINE[@]}" -p lmsr --all-features
  step "lmsr differential"    ; just vectors-lmsr
  step "lmsr mutants"         ; just mutants lmsr
  step "lmsr coverage >= 95%" ; bash scripts/check-coverage.sh lmsr 95
}

gate_2() {
  step "protocol suites"      ; cargo test "${ENGINE[@]}" -p protocol --all-features
  step "cross-stack vectors"  ; just vectors
  step "protocol mutants"     ; just mutants protocol
}

gate_3() {
  step "contracts"            ; just test-sol
  step "slither"              ; just slither
  step "gas snapshot"         ; just snapshot --check
  step "vault coverage >= 95%"; bash scripts/check-coverage.sh TicklineVault 95
}

gate_4() {
  step "engine core"          ; cargo test "${ENGINE[@]}" -p ledger -p market -p api --all-features
  step "ledger coverage >= 85%"; bash scripts/check-coverage.sh ledger 85
  step "market coverage >= 85%"; bash scripts/check-coverage.sh market 85
}

gate_5() {
  step "settlement + indexer" ; cargo test "${ENGINE[@]}" -p settlement -p indexer --all-features
  step "settlement coverage >= 85%"; bash scripts/check-coverage.sh settlement 85
  step "indexer coverage >= 85%"   ; bash scripts/check-coverage.sh indexer 85
}

gate_6() {
  step "agent unit tests"     ; just test-ts
  step "end-to-end scenarios" ; just e2e
}

gate_7() {
  step "deep campaigns"       ; just deep
  step "mutants"              ; just mutants lmsr protocol ledger market settlement
  step "load + adversarial"   ; just e2e-load
}

gate_8() {
  # The `fork` profile, not `ci`: `ci` inherits `no_match_path = "test/fork/*"`, so
  # `--match-path 'test/fork/*'` under `ci` would select nothing and pass vacuously.
  step "fork suite"           ; FOUNDRY_PROFILE=fork forge test --root contracts
  step "deploy smoke"         ; bash scripts/post-deploy-smoke.sh
  step "dashboard"            ; pnpm --filter @tickline/dashboard test
}

gate_9() {
  step "twap template"        ; cargo test "${ENGINE[@]}" -p twap --all-features
}

# Resolve the commit before running anything. As a plain assignment its failure stops the gate;
# inside a printf argument (as it used to be) a failure was ignored and the log named no commit.
COMMIT="$(git rev-parse --short HEAD)"

for (( PHASE=0; PHASE<=TARGET; PHASE++ )); do
  printf '\n\033[1;36m══ phase %s ═══════════════════════════════════════\033[0m\n' "$PHASE"
  "gate_${PHASE}"
done

END="$(date -u +%s)"
printf '\n\033[1;32m✓ gate %s green\033[0m (phases 0..%s, %ss)\n' "$TARGET" "$TARGET" "$(( END - START ))"
printf 'commit %s\n' "$COMMIT"
echo "record this run in docs/STATUS.md and on the phase tracking issue."
