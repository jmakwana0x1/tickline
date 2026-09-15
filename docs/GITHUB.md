# docs/GITHUB.md: the GitHub workflow

`CLAUDE.md` section 7 gives the shape. This is the detail: exact settings, exact commands, and what
`just gh-verify` asserts. If GitHub and this document disagree, this document is right and
`gh-verify` fails until GitHub is fixed.

---

## 1. Repository settings

| Setting | Value | Why |
|---|---|---|
| Default branch | `main` | — |
| Merge strategy | **Squash only**. Merge commits and rebase merges disabled. | One commit per issue on `main`; linear history is required by the ruleset. |
| Auto-delete head branches | on | Branches are cheap; stale branches are not. |
| Squash commit title | PR title | The issue number rides along from the PR title. |
| Issues | on | Task state lives here, not in markdown. |
| Wiki, Projects, Discussions | off | One place for state. |
| Secret scanning + push protection | on | Belt and braces with gitleaks. |

## 2. Branch ruleset on `main`

Defined as JSON in `.github/rulesets/main.json`, applied by `just gh-bootstrap`, asserted by
`just gh-verify`. Rules:

- **Block direct pushes.** Changes reach `main` only through a PR. (Phase 0's single bootstrap
  commit is the one recorded exception, taken before the ruleset is applied.)
- **Require a pull request**, 1 approving review, stale approvals dismissed on new commits,
  conversation resolution required.
- **Require status checks**: exactly one — `required`. See section 3.
- **Require linear history.** No merge commits.
- **Block force pushes and deletions.**
- **Require signed commits** if Jay's local signing is configured; `gh-verify` reports rather than
  fails when it is not.

The ruleset has **no bypass actors**. If Claude cannot merge without a human approval, that is the
design working.

## 3. CI workflow (`.github/workflows/ci.yml`)

One workflow, parallel jobs, one aggregator. Every job runs a `just` command that a developer can
run locally by the same name — CI never has a secret extra step.

| Job | Runs | Needs |
|---|---|---|
| `lint` | `just fmt-check`, `just lint` | — |
| `deps` | dependency-boundary check: `lmsr` and `protocol` stay zero-IO (`CLAUDE.md` §3) | — |
| `no-skips` | `scripts/check-no-skipped-tests.sh` — fails on any ignored/skipped/`only` test | — |
| `rust` | `just test-rust` | — |
| `db` | `just test-db` against a Postgres service container | — |
| `sol` | `just test-sol` (`ci` profile), `just snapshot --check` | — |
| `ts` | `just test-ts` | — |
| `gitleaks` | full-history secret scan | — |
| `gate` | `just gate $(cat .phase)` | all of the above |
| **`required`** | nothing — asserts every needed job succeeded | all of the above |

**`required` is the only check named in the ruleset.** Jobs are added and renamed over the phases;
the ruleset never has to change, and a job that is skipped or cancelled fails `required` rather than
silently passing. It is spelled with an explicit result check, not `if: success()`:

```yaml
required:
  if: always()
  needs: [lint, deps, no-skips, rust, db, sol, ts, gitleaks, gate]
  steps:
    - run: |
        [ "${{ contains(needs.*.result, 'failure') || contains(needs.*.result, 'cancelled') || contains(needs.*.result, 'skipped') }}" = "false" ]
```

Concurrency: one run per branch, older runs cancelled. `nightly-deep.yml` runs `just deep` and
`just mutants` on a schedule, and may fail without blocking merges — it opens an issue instead.

## 4. Labels

Applied by `just gh-bootstrap` from `.github/labels.json`.

| Label | Meaning |
|---|---|
| `phase-0` … `phase-9` | Which phase the issue belongs to. Mirrors the milestone. |
| `tracking` | The phase tracking issue. One per milestone. |
| `slice` | A unit of work a single PR closes. |
| `needs-jay` | **Blocked on a human decision. Claude stops here.** |
| `invariant` | Touches an invariant in `CLAUDE.md` §4. Demands extra review. |
| `adr` | Needs an ADR before code. |
| `red` | Deliberately failing, proving a guard works. Never merged. |
| `security`, `bug`, `chore`, `docs` | Ordinary triage. |

## 5. Milestones

One per phase: `Phase 0: Spec capture and scaffold` … `Phase 9: TWAP template`. A phase closes only
when its milestone has zero open issues and Jay has approved the gate log.

## 6. Issue and PR templates

- `.github/ISSUE_TEMPLATE/phase-tracking.yml` — goal, risk retired, slice checklist, gate log, exit
  criteria.
- `.github/ISSUE_TEMPLATE/slice.yml` — **acceptance criteria must be written as test names.** The
  template refuses vague criteria in review: "handles bad input" is not a test name;
  `rejects_voucher_above_escrow_headroom` is.
- `.github/ISSUE_TEMPLATE/needs-jay.yml` — the question, the options considered, Claude's
  recommendation, and what is blocked until it is answered.
- `.github/PULL_REQUEST_TEMPLATE.md` — closes-issue link, the red-then-green evidence, the
  definition-of-done checklist from `CLAUDE.md` §8, and invariant IDs touched.

## 7. The per-slice command sequence

```bash
# 1. Pick up the issue
gh issue develop <issue#> --checkout --name "<phase>/<issue#>-<slug>"

# 2. Red: the failing test, committed alone
git commit -am "test(<area>): <behaviour> — red for #<issue#>"
git push -u origin HEAD
gh pr create --draft --fill --title "<area>: <behaviour>" \
  --body "Closes #<issue#>" --milestone "Phase <n>: <name>"

# 3. Green: the implementation
git commit -am "feat(<area>): <behaviour>"

# 4. Prove it
just gate $(cat .phase)

# 5. Self-review the whole diff, then hand it over
gh pr diff --patch | less
gh pr ready
gh pr merge --squash --delete-branch   # only once `required` is green and Jay has approved
```

**`gh pr create --draft` is not optional.** A PR is draft until its gate is green; a non-draft PR is
a request for Jay's attention, and asking for attention on red work wastes it.

## 8. Phase open and close

```bash
# Open
gh api repos/:owner/:repo/milestones -f title="Phase <n>: <name>" -f state=open
gh issue create --title "Phase <n>: <name>" --label tracking,phase-<n> \
  --milestone "Phase <n>: <name>" --body-file <(...)   # from the tracking template

# Close
gh issue comment <tracking#> --body-file gate-log.md    # full `just gate <n>` output
# ... Jay comments an explicit go-ahead on the tracking issue ...
echo <n+1> > .phase && git commit -am "chore: enter phase <n+1>"
gh issue close <tracking#>
gh api -X PATCH repos/:owner/:repo/milestones/<id> -f state=closed
gh release create phase-<n> --title "Phase <n>: <name>" --notes-file gate-log.md
```

`.phase` is bumped **only** after Jay's go-ahead, in its own commit, and it is what `just gate` and
CI read. The tag is what makes a phase citable later: "this regressed since `phase-3`".

## 9. What `just gh-verify` asserts

It exits non-zero on the first mismatch, printing the expected and actual value:

1. `gh auth status` is authenticated and has `repo` scope.
2. Default branch is `main`; squash-only merges; auto-delete on; issues on; wiki/projects off.
3. The `main` ruleset exists, is `active`, has **no** bypass actors, and matches
   `.github/rulesets/main.json` rule for rule.
4. `required` is the one and only required status check.
5. Every label in `.github/labels.json` exists with the right colour and description.
6. Milestones exist for phases 0 through `.phase`.
7. Secret scanning and push protection are enabled.
8. `.github/workflows/ci.yml` defines a `required` job needing every other job in the file — so a
   newly added job cannot be forgotten in the aggregator.

Run it after any GitHub settings change, and in the phase gate. Settings drift silently; this is the
only thing that catches it.
