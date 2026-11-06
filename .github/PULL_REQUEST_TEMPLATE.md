## Summary

- 

## Definition of Done

- [ ] `./mk ci` passes locally
- [ ] New or changed behavior is covered by deterministic tests (`./mk mutants-diff`)
- [ ] No gate weakened (`#[expect(..., reason = "...")]` used instead of `#[allow]`; any unavoidable exception recorded in `docs/DEBT.md`)
- [ ] Commit messages follow Conventional Commits (`./mk commits`)
