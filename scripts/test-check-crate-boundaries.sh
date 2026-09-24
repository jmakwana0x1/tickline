#!/usr/bin/env bash
# Self-test for scripts/check-crate-boundaries.sh (issue #39).
# Builds throwaway workspaces and runs the check against each; `cargo metadata --no-deps` needs no
# network or registry for this.
#
# Cleanup deletes only the directories this script created with mktemp, recorded at creation and
# checked to sit under the temp directory. Nothing is ever deleted based on an argument: an
# earlier draft that did so deleted the whole repository.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
check="$PWD/scripts/check-crate-boundaries.sh"
tmp_base="$(cd "${TMPDIR:-/tmp}" && pwd -P)"
created=()
cleanup() {
  local dir
  for dir in "${created[@]}"; do
    case "$dir" in
      "$tmp_base"/tmp.*) rm -rf -- "$dir" ;;
      *) echo "refusing to delete unexpected path: $dir" >&2 ;;
    esac
  done
}
trap cleanup EXIT
failures=0

# Creates a workspace and stores its engine dir in $engine (no subshell, so `created` persists).
make_workspace() { # <lmsr-deps-toml> <protocol-deps-toml> [lmsr-source]
  local root crate deps
  root="$(cd "$(mktemp -d)" && pwd -P)"
  created+=("$root")
  engine="$root/engine"
  mkdir -p "$engine/crates/lmsr/src" "$engine/crates/protocol/src"
  cat > "$engine/Cargo.toml" <<TOML
[workspace]
resolver = "2"
members = ["crates/lmsr", "crates/protocol"]

[workspace.dependencies]
alloy-primitives = { version = "=1.7.3", default-features = false }
thiserror = "2.0"
TOML
  for crate in lmsr protocol; do
    deps="$1"
    [[ "$crate" == protocol ]] && deps="$2"
    printf '[package]\nname = "%s"\nversion = "0.0.0"\nedition = "2021"\n\n[dependencies]\n%s\n\n[dev-dependencies]\nproptest = "1"\n' \
      "$crate" "$deps" > "$engine/crates/$crate/Cargo.toml"
    echo "pub fn f() {}" > "$engine/crates/$crate/src/lib.rs"
  done
  if [[ -n "${3:-}" ]]; then echo "$3" >> "$engine/crates/lmsr/src/lib.rs"; fi
}

expect() { # <label> <want: pass|fail> <engine-dir>
  local status=0
  CARGO_NET_OFFLINE=true bash "$check" "$3" >/dev/null 2>&1 || status=$?
  if { [[ "$2" == pass ]] && (( status == 0 )); } || { [[ "$2" == fail ]] && (( status != 0 )); }; then
    printf '  \033[32m✓\033[0m %s\n' "$1"
  else
    printf '  \033[31m✗\033[0m %s (exit %s)\n' "$1" "$status"
    failures=$((failures + 1))
  fi
}

ALLOWED=$'alloy-primitives.workspace = true\nthiserror.workspace = true'
engine=""

echo "test-check-crate-boundaries:"
make_workspace "$ALLOWED" 'thiserror.workspace = true'
expect "check-crate-boundaries accepts lmsr's allowlist and dev-dependencies" pass "$engine"
make_workspace "$ALLOWED"$'\nserde = "1"' 'thiserror.workspace = true'
expect "check-crate-boundaries rejects an unlisted lmsr dependency" fail "$engine"
make_workspace $'alloy-primitives = "=1.7.3"\nthiserror.workspace = true' 'thiserror.workspace = true'
expect "check-crate-boundaries rejects alloy-primitives with default features" fail "$engine"
make_workspace "$ALLOWED"$'\n\n[build-dependencies]\ncc = "1"' 'thiserror.workspace = true'
expect "check-crate-boundaries rejects an unlisted lmsr build-dependency" fail "$engine"
make_workspace "$ALLOWED" $'thiserror.workspace = true\ntokio = "1"'
expect "check-crate-boundaries rejects an IO dependency in protocol" fail "$engine"
make_workspace "$ALLOWED" 'thiserror.workspace = true' 'pub fn now() { let _ = std::time::SystemTime::now(); }'
expect "check-crate-boundaries rejects a wall-clock read in lmsr" fail "$engine"
# protocol's allowlist (ADR-0011, #62). Versions are written out rather than inherited from the
# fixture workspace, so these cases add to the file without touching the cases above.
P_ALLOWED=$'alloy-primitives = { version = "=1.7.3", default-features = false, features = ["tiny-keccak"] }\nk256 = { version = "0.13", default-features = false, features = ["ecdsa"] }\nserde = "1"\nserde_json = "1"\nbase64 = "0.22"\nthiserror = "2.0"'
make_workspace "$ALLOWED" "$P_ALLOWED"
expect "check-crate-boundaries accepts protocol's allowlist" pass "$engine"
make_workspace "$ALLOWED" "$P_ALLOWED"$'\nregex = "1"'
expect "check-crate-boundaries rejects an unlisted protocol dependency" fail "$engine"
make_workspace "$ALLOWED" "$P_ALLOWED"$'\n\n[build-dependencies]\ncc = "1"'
expect "check-crate-boundaries rejects an unlisted protocol build-dependency" fail "$engine"
make_workspace "$ALLOWED" $'alloy-primitives = "=1.7.3"\nthiserror = "2.0"'
expect "check-crate-boundaries rejects alloy-primitives with default features in protocol" fail "$engine"
make_workspace "$ALLOWED" $'k256 = "0.13"\nthiserror = "2.0"'
expect "check-crate-boundaries rejects k256 with default features" fail "$engine"

expect "check-crate-boundaries passes on the current tree" pass "$PWD/engine"

(( failures == 0 )) || exit 1
