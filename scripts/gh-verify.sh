#!/usr/bin/env bash
# Assert GitHub is configured exactly as docs/GITHUB.md section 9 says.
# Settings drift silently; this is the only thing that catches it.
set -uo pipefail
cd "$(dirname "$0")/.."

FAILED=0
ok()   { printf '  \033[32m✓\033[0m %s\n' "$*"; }
bad()  { printf '  \033[31m✗\033[0m %s\n' "$*"; FAILED=1; }
warn() { printf '  \033[33m·\033[0m %s\n' "$*"; }
eq()   { # <label> <expected> <actual>
  [[ "$2" == "$3" ]] && ok "$1 = $2" || bad "$1: expected '$2', got '$3'"
}

command -v gh >/dev/null || { echo "gh not installed — see 'just doctor'" >&2; exit 1; }
command -v jq >/dev/null || { echo "jq not installed (apt install jq)" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "not authenticated: gh auth login" >&2; exit 1; }

REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner)"
echo "verifying $REPO against docs/GITHUB.md"

echo "1. repository settings"
R="$(gh api "repos/$REPO")"
eq "default_branch"          "main"  "$(jq -r .default_branch      <<<"$R")"
eq "allow_squash_merge"      "true"  "$(jq -r .allow_squash_merge  <<<"$R")"
eq "allow_merge_commit"      "false" "$(jq -r .allow_merge_commit  <<<"$R")"
eq "allow_rebase_merge"      "false" "$(jq -r .allow_rebase_merge  <<<"$R")"
eq "delete_branch_on_merge"  "true"  "$(jq -r .delete_branch_on_merge <<<"$R")"
eq "has_issues"              "true"  "$(jq -r .has_issues          <<<"$R")"
eq "has_wiki"                "false" "$(jq -r .has_wiki            <<<"$R")"
eq "has_projects"            "false" "$(jq -r .has_projects        <<<"$R")"

echo "2. secret scanning"
for f in secret_scanning secret_scanning_push_protection; do
  s="$(jq -r ".security_and_analysis.$f.status // \"unavailable\"" <<<"$R")"
  [[ "$s" == "enabled" ]] && ok "$f enabled" || warn "$f is '$s' (may need a paid plan on a private repo)"
done

echo "3. branch ruleset on main"
RS="$(gh api "repos/$REPO/rulesets" -q '.[] | select(.name=="main")')"
if [[ -z "$RS" ]]; then
  bad "no ruleset named 'main' — run 'just gh-bootstrap'"
else
  id="$(jq -r .id <<<"$RS")"
  FULL="$(gh api "repos/$REPO/rulesets/$id")"
  eq "enforcement" "active" "$(jq -r .enforcement <<<"$FULL")"
  n="$(jq '.bypass_actors | length' <<<"$FULL")"
  [[ "$n" == "0" ]] && ok "no bypass actors" || bad "$n bypass actor(s): the ruleset must bind everyone"
  for rule in deletion non_fast_forward required_linear_history pull_request required_status_checks; do
    jq -e --arg r "$rule" '.rules[] | select(.type==$r)' <<<"$FULL" >/dev/null \
      && ok "rule: $rule" || bad "rule missing: $rule"
  done
  checks="$(jq -r '[.rules[] | select(.type=="required_status_checks")
                   | .parameters.required_status_checks[].context] | sort | join(",")' <<<"$FULL")"
  eq "required status checks" "required" "$checks"
fi

echo "4. labels"
have="$(gh label list --limit 200 --json name -q '.[].name' | sort)"
missing="$(comm -23 <(jq -r '.[].name' .github/labels.json | sort) <(echo "$have"))"
[[ -z "$missing" ]] && ok "all $(jq length .github/labels.json) labels present" \
                    || bad "missing labels: $(tr '\n' ' ' <<<"$missing")"

echo "5. milestones through phase $(cat .phase)"
ms="$(gh api "repos/$REPO/milestones?state=all&per_page=100" -q '.[].title')"
for (( p=0; p<=$(cat .phase); p++ )); do
  grep -q "^Phase $p:" <<<"$ms" && ok "milestone for phase $p" || bad "no milestone for phase $p"
done

echo "6. the 'required' aggregator covers every CI job"
python3 - <<'AGG' || FAILED=1
import re, sys, pathlib

ci = pathlib.Path(".github/workflows/ci.yml").read_text()
# Only the jobs: block. `on:` has two-space keys too, and counting `push:` as a job
# would fail this check for entirely the wrong reason.
parts = re.split(r"^jobs:\s*$", ci, maxsplit=1, flags=re.M)
if len(parts) != 2:
    print("  \033[31m\u2717\033[0m ci.yml has no jobs: block"); sys.exit(1)
body = parts[1]
jobs = re.findall(r"^  ([a-z0-9-]+):$", body, re.M)
needs = re.search(r"^  required:.*?needs: \[([^\]]+)\]", body, re.M | re.S)
if not needs:
    print("  \033[31m\u2717\033[0m ci.yml has no 'required' job with a needs list"); sys.exit(1)
declared = {n.strip() for n in needs.group(1).split(",")}
expected = set(jobs) - {"required"}
missing = expected - declared
if missing:
    print(f"  \033[31m\u2717\033[0m 'required' does not need: {chr(44).join(sorted(missing))}")
    print("      A job outside the aggregator can fail without blocking a merge.")
    sys.exit(1)
print(f"  \033[32m\u2713\033[0m 'required' needs all {len(expected)} jobs")
AGG

echo
if (( FAILED )); then
  echo "✗ GitHub does not match docs/GITHUB.md. Run 'just gh-bootstrap', then re-verify." >&2
  exit 1
fi
echo "✓ GitHub matches docs/GITHUB.md"
