# kloksrs registry release automation

`main` pushes invoke portable tests and `cargo publish --dry-run`.
`release-plz` maintains version/changelog release PRs. Once the release PR is
merged, GitHub Actions publishes unpublished versions with crates.io Trusted
Publishing (GitHub OIDC), without a local token or persistent registry secret.

A one-time crates.io bootstrap is unavoidable for *new* crate names:
publish the initial crate version using an account-owned crates.io token
placed **only in a GitHub Actions secret**, then configure the Trusted
Publisher in the crate settings. It must match repository, workflow
`release-rust.yml`, and GitHub environment `crates-io`.

Protect GitHub environment `crates-io` for main only, and set
`CRATES_IO_TRUSTED_PUBLISHING_READY=true` repository variable after registration.
Until that variable is enabled, routine publishing cannot run.

Allow Actions to create release PRs. Default `GITHUB_TOKEN`-created PRs
do **not** automatically trigger other CI jobs. For hands-off verified
auto-merge, configure a GitHub App token and required status checks
rather than bypassing branch protection. Never auto-merge without CI.
Use Conventional Commits to govern semantic version bumps.
