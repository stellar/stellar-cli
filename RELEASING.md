# Releasing

To release Soroban CLI, follow this process **in order**:

## Follow Rust Workflow
We will be running our main [Rust release workflow](https://github.com/stellar/actions/blob/main/README-rust-release.md).
Follow all the steps in order.


## Create a GitHub Release From a Tag
1. Create an annotated tag with `git tag -a v<release_version_number> -m "Description for release"`
2. Push the tag to remote with `git push origin --tags`
3. Create a new [GitHub release](https://github.com/stellar/soroban-tools/releases/new) from the previously created tag.
    * The release title MUST NOT start with a v, otherwise artifact uploads fail (see [workflow file](https://github.com/stellar/soroban-tools/blob/main/.github/workflows/publish.yml) and this [Slack thread](https://stellarfoundation.slack.com/archives/C04ECVCV162/p1694729751569919) for context)
4. Monitor GitHub actions until they succeed

## Update homebrew-tap to Point to the Latest Released CLI
Update [Formula/soroban-cli.rb](https://github.com/stellar/homebrew-tap/blob/main/Formula/soroban-cli.rb) to point to the released CLI version.
