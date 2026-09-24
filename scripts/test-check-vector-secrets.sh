#!/usr/bin/env bash
# Self-test for scripts/check-vector-secrets.sh (issue #72).
#
# Cleanup deletes only the directories this script created with mktemp, recorded at creation and
# checked to sit under the temp directory. Nothing is ever deleted based on an argument.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
check="$PWD/scripts/check-vector-secrets.sh"
config="$PWD/.gitleaks.toml"
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

# An anvil key from the .gitleaks.toml allowlist, and a key-shaped value that is not one.
#
# The second one is built rather than written out: a 64-hex literal in a .sh file is an
# evm-private-key finding to gitleaks, which would make this file fail the very scan it exists to
# extend. It is 64 ones, and it is not a key, but nothing in a regex can know that.
ANVIL="ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
NOT_ANVIL="$(printf '1%.0s' $(seq 64))"

# Writes <json> to a fresh vectors directory and stores it in $dir (no subshell, so `created`
# persists).
make_vectors() { # <json>
  dir="$(cd "$(mktemp -d)" && pwd -P)"
  created+=("$dir")
  printf '%s\n' "$1" > "$dir/vectors.json"
}

expect() { # <label> <want: pass|fail> <vectors-dir> [config]
  local status=0
  bash "$check" "$3" "${4:-$config}" >/dev/null 2>&1 || status=$?
  if { [[ "$2" == pass ]] && (( status == 0 )); } || { [[ "$2" == fail ]] && (( status != 0 )); }; then
    printf '  \033[32m✓\033[0m %s\n' "$1"
  else
    printf '  \033[31m✗\033[0m %s (exit %s)\n' "$1" "$status"
    failures=$((failures + 1))
  fi
}

dir=""

echo "test-check-vector-secrets:"

make_vectors "{\"digest\": \"0x$NOT_ANVIL\", \"signer\": \"0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266\"}"
expect "accepts hashes under names that are not key-ish" pass "$dir"

make_vectors "{\"private_key\": \"0x$NOT_ANVIL\"}"
expect "rejects a key under private_key" fail "$dir"

make_vectors "{\"privateKey\": \"0x$NOT_ANVIL\"}"
expect "rejects camelCase privateKey" fail "$dir"

make_vectors "{\"signerSk\": \"0x$NOT_ANVIL\"}"
expect "rejects a segment match on sk" fail "$dir"

make_vectors "{\"risk\": \"0x$NOT_ANVIL\"}"
expect "accepts 'risk', which only contains sk" pass "$dir"

make_vectors "{\"mnemonic\": \"test test test test test test test test test test test junk\"}"
expect "rejects a mnemonic phrase" fail "$dir"

make_vectors "{\"cases\": [{\"name\": \"a\"}, {\"secret\": \"0x$NOT_ANVIL\"}]}"
expect "rejects a key nested inside an array" fail "$dir"

make_vectors "{\"accounts\": {\"deep\": {\"privkey\": \"0x$NOT_ANVIL\"}}}"
expect "rejects a key nested inside an object" fail "$dir"

bytes32="$(python3 -c 'print(", ".join("17" for _ in range(32)))')"
bytes64="$(python3 -c 'print(", ".join("17" for _ in range(64)))')"
bytes20="$(python3 -c 'print(", ".join("17" for _ in range(20)))')"
make_vectors "{\"private_key\": [$bytes32]}"
expect "rejects a key as 32 byte values" fail "$dir"

make_vectors "{\"secret\": [$bytes64]}"
expect "rejects a key as 64 byte values" fail "$dir"

make_vectors "{\"keys\": [[$bytes32], [$bytes32]]}"
expect "rejects several keys nested as byte arrays under a plural field" fail "$dir"

make_vectors "{\"accounts\": {\"secret\": {\"material\": [$bytes32]}}}"
expect "rejects a byte array nested deeper inside a key-ish subtree" fail "$dir"

make_vectors "{\"keys\": [$bytes20]}"
expect "accepts an array under a key-ish field that is not key length" pass "$dir"

make_vectors "{\"private_key\": \"0x$ANVIL\"}"
expect "accepts an allowlisted anvil key" pass "$dir"

make_vectors "{\"private_key\": \"$ANVIL\"}"
expect "accepts an allowlisted anvil key without the 0x prefix" pass "$dir"

make_vectors "{\"provenance\": {\"seed\": 402}}"
expect "accepts a numeric seed, which cannot be key material" pass "$dir"

make_vectors "{\"provenance\": {\"seed\": \"0x$NOT_ANVIL\"}}"
expect "rejects a key-shaped string under seed" fail "$dir"

make_vectors "{\"not\": json}"
expect "rejects a vector file that is not valid JSON" fail "$dir"

# Fails closed: a config whose allowlist cannot be read must not be treated as an empty allowlist.
make_vectors "{\"digest\": \"0x$NOT_ANVIL\"}"
empty_config="$dir/gitleaks.toml"
printf 'title = "no allowlist here"\n' > "$empty_config"
expect "fails when the gitleaks allowlist cannot be read" fail "$dir" "$empty_config"

expect "passes on the committed vector files" pass "$PWD/testdata/vectors"

(( failures == 0 )) || exit 1
