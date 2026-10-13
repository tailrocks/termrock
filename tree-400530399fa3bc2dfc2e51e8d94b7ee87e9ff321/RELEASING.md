# Releasing TermRock

TermRock releases from `main`. The crate is not published to crates.io; a
release creates an immutable Git tag and GitHub release after the repository
gates pass.

## Declare a breaking boundary

The first breaking change targeting a new minor version lands atomically with:

1. the implementation and consumer-facing documentation;
2. the next sequential `migrations/000N-vX.Y.0-slug.md` guide;
3. its ordered `MIGRATING.md` row;
4. the workspace version bump to `X.Y.0`; and
5. the regenerated public API inventory and lockfile.

Commit `bb8ff31` is the first complete example. Earlier v0.7/v0.8 migration
guides were reconstructed retrospectively in `3a47a4f`; do not repeat that
historical gap.

A release boundary may contain multiple breaking changes. After the first bump,
each later breaking change adds another sequential migration file for the same
unreleased `vX.Y.0`; it does not repeatedly bump the version. Thus one minor
version maps to one or more migration guides, while every guide maps to exactly
one target version.

## Prepare and verify

1. Confirm every migration for the target version is indexed in
   `MIGRATING.md` and every public surface is current in
   `docs/api/public-api.txt`.
2. Regenerate and review the changelog:

   ```sh
   mise x -- git-cliff --output CHANGELOG.md v0.6.0..HEAD
   ```

3. Run the complete bootstrap gate:

   ```sh
   mise run gate
   ```

4. Commit and push the green release preparation directly to `main` with DCO
   sign-off.

## Dispatch the release

Run the `Release` GitHub Actions workflow with `tag: vX.Y.0`. The tag must
exactly match the current `termrock` Cargo version. The workflow tests,
packages, builds the docs site, generates grouped git-cliff notes, prepends
every migration guide for that version, creates the annotated tag, pushes it,
and creates the GitHub release.

Never create the release tag by hand for a new release; the workflow owns that
transaction. The v0.7.0–v0.9.0 tags are historical backfills only.

## What the semver gate means

`semver-candidate` compares HEAD with the latest reachable release tag. A
breaking API change fails while the Cargo version still equals that release.
Bumping the minor version and documenting the boundary intentionally authorizes
the change under TermRock's forward-only policy. Semver checks do not replace
migration documentation or the full verification gate.
