#!/usr/bin/env bash
# One-shot developer setup for a fresh Linux/WSL box. Idempotent.
# Run it, then `just doctor` should be all green.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1

have() { command -v "$1" >/dev/null 2>&1; }
say()  { printf '\n\033[1m→ %s\033[0m\n' "$*"; }

have cargo || { say "rust"; curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y; . "$HOME/.cargo/env"; }

have just   || { say "just";   cargo install just; }
have sqlx   || { say "sqlx-cli"; cargo install sqlx-cli --no-default-features --features rustls,postgres; }
# shellcheck source=scripts/tool-versions.sh
. scripts/tool-versions.sh
have cargo-mutants  || { say "cargo-mutants $CARGO_MUTANTS_VERSION";  cargo install cargo-mutants --version "$CARGO_MUTANTS_VERSION" --locked; }
have cargo-llvm-cov || { say "cargo-llvm-cov $CARGO_LLVM_COV_VERSION"; cargo install cargo-llvm-cov --version "$CARGO_LLVM_COV_VERSION" --locked; rustup component add llvm-tools-preview; }
have uv || { say "uv $UV_VERSION"; curl -LsSf "https://astral.sh/uv/$UV_VERSION/install.sh" | sh; }

# Same tag CI installs (see .github/actions/setup). Upgrades are their own PR.
have forge || { say "foundry v1.8.1"; curl -L https://foundry.paradigm.xyz | bash; "$HOME/.foundry/bin/foundryup" --install v1.8.1; }

if ! have node; then
  say "node $(cat .nvmrc) via nvm"
  [[ -d "$HOME/.nvm" ]] || curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.40.1/install.sh | bash
  # shellcheck disable=SC1091
  . "$HOME/.nvm/nvm.sh"
  nvm install "$(cat .nvmrc)" && nvm use "$(cat .nvmrc)"
fi
have pnpm || { say "pnpm"; corepack enable && corepack prepare pnpm@9.12.3 --activate; }

if ! have shellcheck; then
  say "shellcheck"
  SC=v0.10.0
  curl -sSL "https://github.com/koalaman/shellcheck/releases/download/$SC/shellcheck-$SC.linux.x86_64.tar.xz" \
    | tar -xJ -C /tmp && mkdir -p "$HOME/.local/bin" && cp "/tmp/shellcheck-$SC/shellcheck" "$HOME/.local/bin/"
fi
have jq || { say "jq"; mkdir -p "$HOME/.local/bin"; curl -sSL https://github.com/jqlang/jq/releases/download/jq-1.7.1/jq-linux-amd64 -o "$HOME/.local/bin/jq"; chmod +x "$HOME/.local/bin/jq"; }

have gh || say "gh: install from https://cli.github.com/ , then 'gh auth login'"
have docker || say "docker: install from https://docs.docker.com/engine/install/"
have gitleaks || say "gitleaks: download from https://github.com/gitleaks/gitleaks/releases"

say "done"
bash scripts/doctor.sh || true
