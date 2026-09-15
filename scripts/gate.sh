#!/usr/bin/env bash
# just gate <n>: run phase n's gate and every earlier phase's gate.
# A phase is never "done" in a way that lets a later phase quietly break it.
set -euo pipefail

cd "$(dirname "$0")/.."

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
  step "toolchain"            ; just doctor
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

gate_1() {
  step "lmsr suites"          ; cd engine && cargo test -p lmsr --all-features && cd ..
  step "lmsr differential"    ; just vectors
  step "lmsr mutants"         ; just mutants lmsr
  step "lmsr coverage >= 95%" ; bash scripts/check-coverage.sh lmsr 95
}

gate_2() {
  step "protocol suites"      ; cd engine && cargo test -p protocol --all-features && cd ..
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
  step "engine core"          ; cd engine && cargo test -p ledger -p market -p api --all-features && cd ..
  step "coverage >= 85%"      ; bash scripts/check-coverage.sh ledger 85 && bash scripts/check-coverage.sh market 85
}

gate_5() {
  step "settlement + indexer" ; cd engine && cargo test -p settlement -p indexer --all-features && cd ..
  step "coverage >= 85%"      ; bash scripts/check-coverage.sh settlement 85 && bash scripts/check-coverage.sh indexer 85
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
  step "fork suite"           ; cd contracts && FOUNDRY_PROFILE=ci forge test --match-path 'test/fork/*' && cd ..
  step "deploy smoke"         ; bash scripts/post-deploy-smoke.sh
  step "dashboard"            ; pnpm --filter @tickline/dashboard test
}

gate_9() { step "twap template"; cd engine && cargo test -p twap --all-features && cd ..; }

for (( PHASE=0; PHASE<=TARGET; PHASE++ )); do
  printf '\n\033[1;36m══ phase %s ═══════════════════════════════════════\033[0m\n' "$PHASE"
  "gate_${PHASE}"
done

printf '\n\033[1;32m✓ gate %s green\033[0m (phases 0..%s, %ss)\n' \
  "$TARGET" "$TARGET" "$(( $(date -u +%s) - START ))"
printf 'commit %s\n' "$(git rev-parse --short HEAD)"
echo "record this run in docs/STATUS.md and on the phase tracking issue."
