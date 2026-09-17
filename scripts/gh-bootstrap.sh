#!/usr/bin/env bash
# Create labels, milestones, and the branch ruleset. Idempotent: safe to re-run
# after any settings change, and `just gh-verify` is what proves it took.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1

command -v gh >/dev/null || { echo "gh not installed; see 'just doctor'" >&2; exit 1; }
command -v jq >/dev/null || { echo "jq not installed (apt install jq)" >&2; exit 1; }
gh auth status >/dev/null || { echo "run 'gh auth login' first" >&2; exit 1; }

REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner)"
say() { printf '\033[1m→ %s\033[0m\n' "$*"; }

say "repository settings on $REPO"
gh api -X PATCH "repos/$REPO" \
  -F has_issues=true -F has_wiki=false -F has_projects=false \
  -F allow_squash_merge=true -F allow_merge_commit=false -F allow_rebase_merge=false \
  -F delete_branch_on_merge=true \
  -f squash_merge_commit_title=PR_TITLE -f squash_merge_commit_message=PR_BODY \
  >/dev/null

say "secret scanning and push protection"
gh api -X PATCH "repos/$REPO" --input - >/dev/null <<'JSON' || echo "  (skipped: needs a plan that supports it on private repos)"
{"security_and_analysis":{"secret_scanning":{"status":"enabled"},"secret_scanning_push_protection":{"status":"enabled"}}}
JSON

say "labels"
existing="$(gh label list --limit 200 --json name -q '.[].name')"
jq -c '.[]' .github/labels.json | while read -r label; do
  name="$(jq -r .name <<<"$label")"
  color="$(jq -r .color <<<"$label")"
  desc="$(jq -r .description <<<"$label")"
  if grep -qxF "$name" <<<"$existing"; then
    gh label edit "$name" --color "$color" --description "$desc" >/dev/null
  else
    gh label create "$name" --color "$color" --description "$desc" >/dev/null
  fi
  printf '  %s\n' "$name"
done

say "milestones (phases 0..9)"
titles=(
  "Phase 0: Spec capture and scaffold" "Phase 1: LMSR math core"
  "Phase 2: Protocol types and cross-stack vectors" "Phase 3: Contracts"
  "Phase 4: Engine core" "Phase 5: Settlement and indexer"
  "Phase 6: Agents and end-to-end" "Phase 7: Hardening"
  "Phase 8: Testnet, dashboard, demo" "Phase 9: Stretch: TWAP template"
)
have="$(gh api "repos/$REPO/milestones?state=all&per_page=100" -q '.[].title')"
for t in "${titles[@]}"; do
  if grep -qxF "$t" <<<"$have"; then
    printf '  %s (exists)\n' "$t"
  else
    gh api "repos/$REPO/milestones" -f title="$t" -f state=open >/dev/null
    printf '  %s\n' "$t"
  fi
done

say "branch ruleset on main"
id="$(gh api "repos/$REPO/rulesets" -q '.[] | select(.name=="main") | .id' || true)"
if [[ -n "$id" ]]; then
  gh api -X PUT "repos/$REPO/rulesets/$id" --input .github/rulesets/main.json >/dev/null
  echo "  updated ruleset $id"
else
  gh api -X POST "repos/$REPO/rulesets" --input .github/rulesets/main.json >/dev/null
  echo "  created"
fi

say "done. Now run: just gh-verify"
