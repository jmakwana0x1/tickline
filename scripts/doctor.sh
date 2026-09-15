#!/usr/bin/env bash
# Report the toolchain. Never installs anything; prints what to run.
set -uo pipefail
cd "$(dirname "$0")/.."

MISSING=0
row() { # <name> <version-cmd> <install-hint> [optional]
  local name="$1" vcmd="$2" hint="$3" optional="${4:-}"
  if command -v "${name%% *}" >/dev/null 2>&1; then
    printf '  \033[32m✓\033[0m %-12s %s\n' "$name" "$(eval "$vcmd" 2>/dev/null | head -1)"
  elif [[ -n "$optional" ]]; then
    printf '  \033[33m·\033[0m %-12s optional — %s\n' "$name" "$hint"
  else
    printf '  \033[31m✗\033[0m %-12s %s\n' "$name" "$hint"
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
row gitleaks 'gitleaks version'      'https://github.com/gitleaks/gitleaks/releases'
echo "  optional:"
row cargo-mutants 'cargo mutants --version' 'cargo install cargo-mutants' opt
row cargo-llvm-cov 'cargo llvm-cov --version' 'cargo install cargo-llvm-cov' opt
row slither  'slither --version'     'pipx install slither-analyzer' opt
row shellcheck 'shellcheck --version | sed -n 2p' 'apt install shellcheck' opt

if (( MISSING )); then
  echo
  echo "Required tools are missing. scripts/dev-setup.sh installs everything above."
  exit 1
fi
echo "✓ toolchain complete"
