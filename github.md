 # GITHUB.md: repo operations for Tickline

`CLAUDE.md` section 7 defines the rules. This file defines the exact setup, templates, and commands.
Everything here is created in Phase 0 and verified by `just gh-verify`.

---

## 1. One-time setup (Phase 0)

### Order
1. `gh repo create tickline --public --source . --push`. The repo is public: it is a portfolio
   project, and rulesets on private repos need a paid plan.
2. **The bootstrap commit is the only direct commit to `main` ever.** It contains `CLAUDE.md`,
   `docs/`, `justfile`, `.github/` (workflows, templates, ruleset JSON), `scripts/gh-bootstrap.sh`,
   and `.phase` containing `0`.
3. `just gh-bootstrap` applies repo settings, labels, milestones, and the ruleset.
4. `just gh-verify` asserts all of it via `gh api`. From here on, everything goes through PRs.

### `scripts/gh-bootstrap.sh` (idempotent; safe to rerun)
Repo settings:
```bash
gh repo edit --enable-squash-merge --disable-merge-commit --disable-rebase-merge \
  --enable-auto-merge --delete-branch-on-merge
gh api -X PATCH repos/{owner}/{repo} \
  -f squash_merge_commit_title=PR_TITLE -f squash_merge_commit_message=PR_BODY
```

Labels: loop over section 2 with `gh label create "$name" --color "$color" --description "$desc" --force`.

Milestones: for each phase in `docs/PHASES.md`, create `Phase N: <name>` via
`gh api repos/{owner}/{repo}/milestones -f title=... -f description=...` if the title does not
already exist (check with `--jq '.[].title'`).

Ruleset: `.github/rulesets/main.json`, created with `POST repos/{owner}/{repo}/rulesets` or updated
with `PUT` if a ruleset named `main-protection` exists.

```json
{
  "name": "main-protection",
  "target": "branch",
  "enforcement": "active",
  "bypass_actors": [],
  "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    { "type": "required_linear_history" },
    {
      "type": "pull_request",
      "parameters": {
        "required_approving_review_count": 0,
        "dismiss_stale_reviews_on_push": true,
        "require_code_owner_review": false,
        "require_last_push_approval": false,
        "required_review_thread_resolution": true
      }
    },
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": true,
        "required_status_checks": [{ "context": "required" }]
      }
    }
  ]
}
```

Approvals are set to 0 because GitHub does not let an author approve their own PR, and Claude Code
pushes as Jay. The `needs-jay` label plus unresolved review threads are the human gate instead.

### `scripts/gh-verify.sh`
Fails loudly unless: repo is public; only squash merge enabled; auto-merge and delete-on-merge on;
every label in section 2 exists; every phase milestone exists; `main-protection` is active with the
rules above; required check context is exactly `required`.

---

## 2. Labels

| Label | Color | Meaning |
|---|---|---|
| `feat` | `1d76db` | New behavior |
| `bug` | `d73a4a` | Defect, body contains failing test |
| `test` | `0e8a16` | Tests only, no behavior change |
| `chore` | `c5def5` | Tooling, deps, CI |
| `docs` | `0075ca` | Docs only |
| `adr` | `5319e7` | Introduces or changes a decision record |
| `tracking` | `000000` | Phase tracking issue |
| `needs-jay` | `e99695` | Blocked on Jay's review or decision |
| `blocked` | `b60205` | Blocked by another issue |
| `later` | `cfd3d7` | Out of current phase, do not start |
| `flaky` | `ff7619` | Nondeterministic test, always with `p0` |
| `security` | `b60205` | Touches funds safety, signatures, or auth |
| `p0` | `b60205` | Drop everything |
| `p1` | `fbca04` | Current phase critical path |
| `p2` | `fef2c0` | Current phase, not critical path |
| `area:contracts` `area:lmsr` `area:protocol` `area:ledger` `area:market` `area:api` `area:settlement` `area:indexer` `area:agents` `area:e2e` `area:ci` | `bfdadc` | Component |

Every issue and PR carries exactly one type label, at least one `area:` label, and one priority.

---

## 3. CI (`.github/workflows/ci.yml`)

Triggers: `pull_request` and `push` to `main`. Actions pinned by commit SHA.

Jobs:
- `lint`: rustfmt, clippy with deny policy, forge fmt check, eslint/prettier, markdown em-dash check.
- `test-sol`, `test-rs`, `test-ts`: suites at `ci` profile.
- `vectors`: `just vectors` must produce an empty diff.
- `gate`: `just gate $(cat .phase)` including coverage floors and the no-ignored-tests check.
- `secrets`: gitleaks.
- `pr-title`: semantic PR title check (PR events only).
- `required`: `needs` every job above, `if: always()`, fails if any needed job failed or was
  cancelled. This is the single required check in the ruleset, so adding jobs never requires
  editing the ruleset.

`.phase` changes only in a phase-closing PR, which is always `needs-jay`.

---

## 4. Templates

### `.github/ISSUE_TEMPLATE/slice.yml` (feature or test slice)
```yaml
name: Slice
description: One behavior slice, one session, one PR
title: "[P<phase>] <area>: <behavior>"
labels: ["feat"]
body:
  - type: textarea
    id: context
    attributes: { label: Context, description: Why this slice exists and what it builds on }
    validations: { required: true }
  - type: textarea
    id: acceptance
    attributes:
      label: Acceptance criteria (test names)
      description: One checkbox per test that must exist and pass
      placeholder: "- [ ] inv_i2_cost_bounded_by_max_plus_b_ln2"
    validations: { required: true }
  - type: input
    id: invariants
    attributes: { label: Invariants touched, placeholder: "I2, I15" }
    validations: { required: true }
  - type: textarea
    id: out_of_scope
    attributes: { label: Out of scope }
  - type: input
    id: deps
    attributes: { label: Dependencies, placeholder: "Blocked by #11" }
```

### `.github/ISSUE_TEMPLATE/bug.yml`
Fields: summary, failing test (code block, required), expected vs actual, invariants violated,
how it was found (test layer, scenario, seed).

### `.github/ISSUE_TEMPLATE/tracking.yml`
Fields: phase goal (copied from `PHASES.md`), task list of child issues, gate command, exit criteria.

### `.github/pull_request_template.md`
```markdown
## Summary
<one paragraph: what behavior this adds or fixes>

Closes #

## Invariants touched
<IDs, or "none">

## Tests added
- `test_name`: what it proves

## Red evidence
<failing output summary from the test-first commit>

## Green evidence
<`just gate <n>` pass/fail counts>

## Coverage delta
<before -> after for touched crates or contracts>

## Risk
<what could break, what is not covered and why>

## Checklist
- [ ] No test weakened, ignored, skipped, or deleted
- [ ] Every acceptance-criteria test from the issue exists and passes
- [ ] ADR added or updated if a decision was made
- [ ] No stop-and-ask trigger hit, or labeled `needs-jay`
- [ ] `docs/STATUS.md` gate log updated
- [ ] Self-review comment posted
```

---

## 5. Lifecycle cookbook

### Phase start
```bash
gh issue create --title "[P1] Phase 1 tracking: LMSR math core" \
  --label tracking,p1 --milestone "Phase 1: LMSR math core" --body-file /tmp/tracking.md
# create each slice issue, then edit the tracking issue task list with their numbers
gh issue edit <tracking> --add-label needs-jay   # breakdown approval
```

### Working an issue
```bash
git checkout main && git pull
gh issue edit 12 --add-assignee @me
git checkout -b p1/12-lmsr-cost-function

# red
git commit -m "test(lmsr): cost matches reference vectors" -m "Refs #12"
git push -u origin HEAD
gh pr create --draft --title "feat(lmsr): cost function in log-sum-exp form" \
  --label feat,area:lmsr,p1 --milestone "Phase 1: LMSR math core" --body-file /tmp/pr.md

# green
git commit -m "feat(lmsr): implement cost with log-sum-exp" -m "Refs #12"
just gate 1
git push

# ready
gh pr diff
gh pr comment --body-file /tmp/self-review.md
gh pr ready
gh pr merge --squash --auto --delete-branch   # only if merge rules in CLAUDE.md allow
gh pr checks --watch
```

### After merge
```bash
gh issue view 12 --json state --jq .state      # expect CLOSED
gh issue edit <tracking> --body-file /tmp/tracking-updated.md   # tick the box
git checkout main && git pull
```

### Phase close
```bash
just gate 1
gh issue comment <tracking> --body-file /tmp/gate-log.md
gh issue edit <tracking> --add-label needs-jay
# after Jay approves, in a needs-jay PR: bump .phase, update STATUS.md
gh api -X PATCH repos/{owner}/{repo}/milestones/<n> -f state=closed
gh release create phase-1 --title "Phase 1: LMSR math core" --notes-file /tmp/gate-log.md
```

---

## 6. When things go wrong

| Situation | Action |
|---|---|
| CI red on your PR | Fix on the branch. Never merge, never retry-until-green. |
| CI red on something unrelated to your change | Open a `flaky` + `p0` issue with the run link and seed. Stop feature work until fixed. |
| `main` moved, PR out of date | `git fetch && git rebase origin/main`, rerun `just gate <n>`, `git push --force-with-lease`. Force-push is allowed on feature branches only. |
| Jay leaves review comments | Address each in its own commit. Reply to each thread with the commit SHA. Leave resolving Jay's threads to Jay. |
| Regression found in merged code | New `bug` issue linking the merged PR, with the failing test. Do not reopen the closed issue. |
| Issue turns out bigger than one session | Split it: close nothing, create child issues, update the tracking list, keep the current PR to the first slice. |
| Auto-merge stuck | `gh pr checks` to find the blocker. If it is a missing label or unresolved thread, it is waiting on Jay: move on. |
| Direct push to `main` rejected | Correct behavior. Open a PR. |