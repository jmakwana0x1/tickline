#!/usr/bin/env bash
# Report the toolchain. Never installs anything; prints what to run.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

MISSING=0
row() { # <name> <version-cmd> <install-hint> [optional]
  local name="$1" vcmd="$2" hint="$3" optional="${4:-}"
  if command -v "${name%% *}" >/dev/null 2>&1; then
    printf '  \033[32m✓\033[0m %-12s %s\n' "$name" "$(eval "$vcmd" 2>/dev/null | head -1)"
  elif [[ -n "$optional" ]]; then
    printf '  \033[33m·\033[0m %-12s optional: %s\n' "$name" "$hint"
  else
    printf '  \033[31m✗\033[0m %-12s %s\n' "$name" "$hint"
    MISSING=1
  fi
}

# shellcheck source=scripts/tool-versions.sh
. scripts/tool-versions.sh

# A tool whose version is pinned: missing or drifted both count as missing.
pinned() { # <name> <version-cmd> <pinned-version> <install-hint>
  local name="$1" vcmd="$2" want="$3" hint="$4" have
  if ! command -v "$name" >/dev/null 2>&1; then
    printf '  \033[31m✗\033[0m %-12s %s\n' "$name" "$hint"
    MISSING=1
    return
  fi
  have="$(eval "$vcmd" 2>/dev/null | head -1)"
  # Exact match on the version field ("uv 0.12.15 (...)", "cargo-mutants 27.1.0"), so 0.9.10
  # is not mistaken for 0.9.1.
  if [[ "$(awk '{print $2}' <<< "$have")" == "$want" ]]; then
    printf '  \033[32m✓\033[0m %-12s %s\n' "$name" "$have"
  else
    printf '  \033[31m✗\033[0m %-12s %s, but %s is pinned: %s\n' "$name" "$have" "$want" "$hint"
    MISSING=1
  fi
}

echo "Tickline toolchain (phase $(cat .phase)):"
row just     'just --version'        'cargo install just'
row cargo    'cargo --version'       'https://rustup.rs'
row rustc    'rustc --version'       'rustup toolchain install stable'
row forge    'forge --version'       'curl -L https://foundry.paradigm.xyz | bash && foundryup'
row anvil    'anvil --version'       'comes with foundryup'
row node     'node --version'        'nvm install "$(cat .nvmrc)"'
row pnpm     'pnpm --version'        'corepack enable && corepack prepare pnpm@latest --activate'
row docker   'docker --version'      'https://docs.docker.com/engine/install/'
row sqlx     'sqlx --version'        'cargo install sqlx-cli --no-default-features --features rustls,postgres'
row python3  'python3 --version'     'apt install python3'
row gh       'gh --version'          'https://cli.github.com/'
row jq       'jq --version'          'apt install jq, or the binary from https://jqlang.github.io/jq/'
row shellcheck 'shellcheck --version | sed -n 2p' 'apt install shellcheck (CI runners have it, so a local skip hides failures)'
row gitleaks 'gitleaks version'      'https://github.com/gitleaks/gitleaks/releases'
pinned uv             'uv --version'             "$UV_VERSION"             "curl -LsSf https://astral.sh/uv/$UV_VERSION/install.sh | sh"
pinned cargo-mutants  'cargo mutants --version'  "$CARGO_MUTANTS_VERSION"  "cargo install cargo-mutants --version $CARGO_MUTANTS_VERSION --locked"
pinned cargo-llvm-cov 'cargo llvm-cov --version' "$CARGO_LLVM_COV_VERSION" "cargo install cargo-llvm-cov --version $CARGO_LLVM_COV_VERSION --locked"
echo "  optional:"
row slither  'slither --version'     'pipx install slither-analyzer' opt

if (( MISSING )); then
  echo
  echo "Required tools are missing. scripts/dev-setup.sh installs everything above."
  exit 1
fi
echo "✓ toolchain complete"
