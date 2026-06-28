## Description

<!-- Briefly describe what this PR does and why. -->

## Type of Change

Please check the option(s) that apply:

- [ ] Bug fix (non-breaking change that fixes an issue)
- [ ] New feature (non-breaking change that adds functionality)
- [ ] Breaking change (fix or feature that would cause existing functionality to not work as expected)
- [ ] Documentation update
- [ ] Refactor / performance improvement
- [ ] Test addition or correction
- [ ] Chore (dependencies, CI, tooling, etc.)

## Related Issues

<!-- Link issues that this PR addresses. Use keywords like "Fixes #123" or "Closes #456". -->

Fixes #
Closes #

## Testing

- [ ] `cargo test --features cli` passes locally
- [ ] New tests added for any new functionality
- [ ] Regression test added for any bug fix
- [ ] Manually verified the change end-to-end

## Code Checklist

- [ ] `cargo fmt --all` has been applied
- [ ] `cargo clippy --all-features -- -D warnings` produces no warnings
- [ ] Commit messages follow [Conventional Commits](https://conventionalcommits.org) (feat / fix / docs / refactor / test / chore)
- [ ] Public API changes are documented with rustdoc comments
- [ ] Existing documentation updated where relevant (README, CHANGELOG, etc.)
- [ ] No breaking changes introduced, OR a migration guide has been provided below

### Migration Guide (if applicable)

<!-- If this PR introduces breaking changes, describe how users should update. -->

## Additional Notes

<!-- Anything else reviewers should know. -->
