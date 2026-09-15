#!/usr/bin/env bash
# lmsr and protocol are zero-IO (CLAUDE.md section 3). This is what makes them
# exhaustively testable: no clock, no database, no network, no async runtime.
set -euo pipefail
cd "$(dirname "$0")/../engine" || exit 1

BANNED='^(tokio|sqlx|reqwest|axum|hyper|alloy-provider|chrono|std-time|rand)$'
STATUS=0

for crate in lmsr protocol; do
  deps="$(cargo metadata --format-version 1 --no-deps 2>/dev/null \
    | python3 -c "
import json,sys
m=json.load(sys.stdin)
for p in m['packages']:
    if p['name']=='$crate':
        print('\n'.join(d['name'] for d in p['dependencies'] if d['kind'] is None))
")"
  while read -r dep; do
    [[ -z "$dep" ]] && continue
    if [[ "$dep" =~ $BANNED ]]; then
      echo "✗ $crate must stay zero-IO but depends on '$dep'" >&2
      STATUS=1
    fi
  done <<< "$deps"

  # Belt and braces: no direct wall-clock or network in the source either.
  if grep -rnE 'std::time::(SystemTime|Instant)|std::net' "crates/$crate/src" 2>/dev/null; then
    echo "✗ $crate reads the clock or the network directly" >&2
    STATUS=1
  fi
done

(( STATUS == 0 )) && echo "✓ lmsr and protocol are zero-IO"
exit $STATUS
