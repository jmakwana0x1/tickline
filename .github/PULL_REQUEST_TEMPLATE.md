Closes #

## What changed

<!-- One paragraph. What is true after this merges that was not true before. -->

## Red, then green

<!-- The commit where the test failed, and what the failure said. A reviewer must be able to
     see the test fail before it passed. -->

- Red commit:
- Failure output:

```
```

## Invariants touched

<!-- IDs from CLAUDE.md section 4, and the test that would fail if each were removed. -->

| ID | Test that guards it |
|---|---|
|  |  |

## Definition of done (CLAUDE.md section 8)

- [ ] The history shows the test failing before it passes
- [ ] Every acceptance criterion from the issue exists as a named, passing test
- [ ] Every new error path has a test; every custom error has a test that triggers it
- [ ] Nothing ignored, skipped, or `only`; no commented-out test
- [ ] `just gate $(cat .phase)` green locally
- [ ] Rounding direction stated in a comment at every conversion (I15)
- [ ] Public functions document their domain limits and error conditions
- [ ] An ADR exists if a reader would ask "why was it done this way?"
- [ ] I have read this diff line by line, looking for a way to lose money
