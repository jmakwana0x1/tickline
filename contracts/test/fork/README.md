# contracts/test/fork

Phase 8. **The only suite in the repository allowed network access.** Run with
`FOUNDRY_PROFILE=fork`; every other profile excludes this path via `no_match_path`, so a fork
test can never silently slow down or flake the ordinary gate.
