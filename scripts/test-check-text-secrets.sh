#!/usr/bin/env bash
# Self-test for scripts/check-text-secrets.sh (#81, #85).
#
# Every row of the table Jay measured on #85 is a case here, because the first version matched only
# the two spellings nobody writes: `\bprivate\b` cannot match `private_key`, since `_` is a word
# character, and it cannot match `privateKey` either.
#
# Cleanup deletes only the directories this script created with mktemp, recorded at creation and
# checked to sit under the temp directory.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1
check="$PWD/scripts/check-text-secrets.sh"
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

# A key-shaped value that is not an allowlisted anvil key, built rather than written out: a 64-hex
# literal in a .sh file is an evm-private-key finding, and this file would otherwise fail the scan it
# exists to extend.
NOT_ANVIL="$(printf '1%.0s' $(seq 64))"
ANVIL="ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
# A hash-shaped value, built for the same reason NOT_ANVIL is: a 64-hex literal in a .sh file is an
# evm-private-key finding, and this file tripped its own subject when the value was written out.
A_HASH="$(printf 'ab%.0s' $(seq 32))"

make_docs() { # <markdown>
  dir="$(cd "$(mktemp -d)" && pwd -P)"
  created+=("$dir")
  printf '%s\n' "$1" > "$dir/doc.md"
}

expect() { # <label> <want: pass|fail> <dir>
  local status=0
  bash "$check" "$3" >/dev/null 2>&1 || status=$?
  if { [[ "$2" == pass ]] && (( status == 0 )); } || { [[ "$2" == fail ]] && (( status != 0 )); }; then
    printf '  \033[32m✓\033[0m %s\n' "$1"
  else
    printf '  \033[31m✗\033[0m %s (exit %s)\n' "$1" "$status"
    failures=$((failures + 1))
  fi
}

dir=""
echo "test-check-text-secrets:"

make_docs "The operator private key is 0x$NOT_ANVIL and must never be committed."
expect "rejects a key on the same line as its label" fail "$dir"

make_docs "Set the signing key:

0x$NOT_ANVIL"
expect "rejects a key two lines under its label" fail "$dir"

make_docs "Here is the secret:

\`\`\`
0x$NOT_ANVIL
\`\`\`"
expect "rejects a key in a fenced block under its label" fail "$dir"

make_docs "The mnemonic seed follows.



0x$NOT_ANVIL"
expect "accepts a key four lines away, which is outside the window" pass "$dir"

make_docs "anvil's first account private key is 0x$ANVIL, public test material."
expect "accepts an allowlisted anvil key beside the word key" pass "$dir"

make_docs "The VOUCHER_TYPEHASH is 0x$A_HASH, read from the deployment."
expect "accepts a type hash with no key-ish word near it" pass "$dir"

make_docs "The channelId is

0x$A_HASH

and the digest below it is the same shape."
expect "accepts a digest under a hash-ish label" pass "$dir"

# One case per row of the table on #85. The first six all missed before keyish.py.
for spelling in 'privateKey: "0x'"$NOT_ANVIL"'"' \
                "private_key = 0x$NOT_ANVIL" \
                "deployerPrivateKey is 0x$NOT_ANVIL" \
                "secretKey 0x$NOT_ANVIL" \
                "priv_key 0x$NOT_ANVIL" \
                "signerSk 0x$NOT_ANVIL" \
                "key: 0x$NOT_ANVIL" \
                "PRIVATE KEY 0x$NOT_ANVIL"; do
  make_docs "$spelling"
  expect "rejects ${spelling%% *} as a label" fail "$dir"
done

# Words that merely contain a short token must not fire, which is what segmentation buys.
for innocent in "risk 0x$A_HASH" "monkey 0x$A_HASH" "the whisky 0x$A_HASH"; do
  make_docs "$innocent"
  expect "accepts '${innocent%% *}', which only contains a token" pass "$dir"
done

# Every type the gitleaks rule omits, not just markdown.
for ext in md txt sql py js; do
  dir="$(cd "$(mktemp -d)" && pwd -P)"
  created+=("$dir")
  printf 'private_key = 0x%s\n' "$NOT_ANVIL" > "$dir/thing.$ext"
  expect "covers .$ext" fail "$dir"
done

dir="$(cd "$(mktemp -d)" && pwd -P)"
created+=("$dir")
printf 'PRIVATE_KEY=0x%s\n' "$NOT_ANVIL" > "$dir/justfile"
expect "covers an extensionless file" fail "$dir"

# A type gitleaks already scans is left to gitleaks: two checks flagging one line is noise.
dir="$(cd "$(mktemp -d)" && pwd -P)"
created+=("$dir")
printf 'let private_key = "0x%s";\n' "$NOT_ANVIL" > "$dir/thing.rs"
expect "leaves .rs to gitleaks" pass "$dir"

expect "passes on the committed tree" pass "$PWD"

(( failures == 0 )) || exit 1
