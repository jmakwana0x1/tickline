#!/usr/bin/env bash
# Self-test for scripts/check-no-encode-packed.sh (issue #65).
#
# Cleanup deletes only the directories this script created with mktemp, recorded at creation and
# checked to sit under the temp directory. Nothing is ever deleted based on an argument.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
check="$PWD/scripts/check-no-encode-packed.sh"
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

make_contracts() { # <src-body> [lib-body]
  dir="$(cd "$(mktemp -d)" && pwd -P)"
  created+=("$dir")
  mkdir -p "$dir/src" "$dir/lib/forge-std/src"
  printf '%s\n' "$1" > "$dir/src/Thing.sol"
  [[ -n "${2:-}" ]] && printf '%s\n' "$2" > "$dir/lib/forge-std/src/Vendored.sol"
  return 0
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
echo "test-no-encode-packed:"

make_contracts 'contract Thing { function f() public pure returns (bytes32) { return keccak256(abi.encode(uint8(1), uint16(2))); } }'
expect "accepts abi.encode" pass "$dir"

make_contracts 'contract Thing { function f() public pure returns (bytes32) { return keccak256(abi.encodePacked(uint8(1), uint16(2))); } }'
expect "rejects abi.encodePacked" fail "$dir"

make_contracts 'contract Thing { function f() public pure returns (bytes32) { return keccak256(bytes.concat(hex"1901", bytes32(0), bytes32(0))); } }'
expect "accepts bytes.concat for the EIP-712 framing" pass "$dir"

make_contracts 'contract Thing {
    function f() public pure returns (bytes32) {
        return keccak256(
            abi.encodePacked(uint8(1))
        );
    }
}'
expect "rejects abi.encodePacked split across lines" fail "$dir"

make_contracts 'contract Thing { uint256 x; }' 'contract Vendored { function f() public pure returns (bytes memory) { return abi.encodePacked("a", "b"); } }'
expect "ignores vendored lib/" pass "$dir"

expect "passes on the committed contracts" pass "$PWD/contracts"

(( failures == 0 )) || exit 1
