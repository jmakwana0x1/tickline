#!/usr/bin/env bash
# One-shot developer setup for a fresh Linux/WSL box. Idempotent.
# Run it, then `just doctor` should be all green.
set -euo pipefail
cd "$(dirname "$0")/.."

have() { command -v "$1" >/dev/null 2>&1; }
say()  { printf '\n\033[1m→ %s\033[0m\n' "$*"; }

have cargo || { say "rust"; curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; . "$HOME/.cargo/env"; }

have just   || { say "just";   cargo install just; }
have sqlx   || { say "sqlx-cli"; cargo install sqlx-cli --no-default-features --features rustls,postgres; }
have cargo-mutants  || { say "cargo-mutants";  cargo install cargo-mutants; }
have cargo-llvm-cov || { say "cargo-llvm-cov"; cargo install cargo-llvm-cov; rustup component add llvm-tools-preview; }

have forge || { say "foundry"; curl -L https://foundry.paradigm.xyz | bash; "$HOME/.foundry/bin/foundryup"; }

if ! have node; then
  say "node $(cat .nvmrc) via nvm"
  [[ -d "$HOME/.nvm" ]] || curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
  # shellcheck disable=SC1091
  . "$HOME/.nvm/nvm.sh"
  nvm install "$(cat .nvmrc)" && nvm use "$(cat .nvmrc)"
fi
have pnpm || { say "pnpm"; corepack enable && corepack prepare pnpm@9.12.3 --activate; }

have gh || say "gh: install from https://cli.github.com/ , then 'gh auth login'"
have docker || say "docker: install from https://docs.docker.com/engine/install/"
have gitleaks || say "gitleaks: download from https://github.com/gitleaks/gitleaks/releases"

say "done"
bash scripts/doctor.sh || true
