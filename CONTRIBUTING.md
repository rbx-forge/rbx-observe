# Contributing

Early days: the shape of this tool is still being decided, so **open an issue
before writing code**. A PR that arrives without one risks being declined for
scope reasons that have nothing to do with its quality.

**Read the boundary in the [README](./README.md#the-boundary-which-does-not-move)
first.** Anything that collects person-level data is out, however it is framed.
That is the one non-negotiable in this repository.

## Setup

```sh
git clone https://github.com/rbx-forge/rbx-observe
cd rbx-observe
lefthook install          # one-time: pre-commit runs cargo fmt + clippy
cargo test --workspace
```

MSRV is Rust 1.88. No credentials are needed to build, test, or run: the tool
reads public endpoints only, and HTTP paths are tested against `wiremock`
rather than the live API.

## Conventions

House style:

- `main.rs` / `lib.rs` holds the clap surface and dispatch, `api/` holds one
  method per endpoint, `commands/` holds the logic.
- HTTP tests assert the **request** that was emitted, not only that a canned
  response parsed. A client that sends the wrong thing against a permissive
  mock is the failure mode that matters.
- Comments state constraints the code cannot express: why this order, what
  breaks if it changes. They do not narrate the next line. Prefer silence to
  narration.
- [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`,
  `fix:`, `chore:`). Not enforced, kept by discipline.

## What makes a PR mergeable

- `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings`
  are clean. The pre-commit hook runs both; CI runs them again.
- `cargo test --workspace` passes, and a behavior change comes with a test that
  fails without it.
- **Tests never touch the network.** Every HTTP path goes through `wiremock`,
  and the CLI tests use targets that fail before a request is built. A suite
  that calls Roblox is a suite that breaks when Roblox has a bad day, from
  everybody's CI at once.
- **Output changes go through `insta`.** The human rendering is snapshotted
  whole — `render()` returns a `String` for exactly that reason — because a
  `contains()` assertion passes while everything around it degrades. When a
  layout changes on purpose:

  ```sh
  cargo install cargo-insta   # once
  cargo insta test --review   # or: cargo test, then cargo insta review
  ```

  The diff belongs in the PR: that is how a layout regression gets seen.
- A user-visible change updates the README and adds a `## [Unreleased]` line to
  [CHANGELOG.md](./CHANGELOG.md).
- No new dead code; `dead_code` is denied, and a deliberate exception carries a
  narrow `#[allow]` with a reason.
- Any new endpoint the PR calls is named in the description, with one line on
  why it is game-level and public.

## Developer Certificate of Origin

Every commit must be signed off:

```sh
git commit -s -m "feat: ..."
```

which appends `Signed-off-by: Your Name <your.email@example.com>` — your
statement that you have the right to contribute the patch under this project's
license. Full text at <https://developercertificate.org/>. A missing sign-off
is fixed with `git commit --amend -s` and a force-push.

No copyright assignment is asked for. You keep your copyright.

## Licensing

[MPL-2.0](./LICENSE), file-level copyleft. If you adapt code from another
project, say so in the PR and add its notice to a `THIRD-PARTY-NOTICES.md` — a
doc-comment credit is not a license notice. MIT and Apache-2.0 sources are fine
with the notice; GPL-family sources are not compatible and will be declined.

## Cutting a release

`--version` prints whatever is in the manifest, not the tag, so the bump is
step one rather than an afterthought.

1. Bump `[workspace.package].version` in the root `Cargo.toml`, then
   `cargo check` to refresh `Cargo.lock`.
2. Move the `## [Unreleased]` content to `## [X.Y.Z] - YYYY-MM-DD` in
   [CHANGELOG.md](./CHANGELOG.md).
3. Commit, then tag and push:
   `git tag -a vX.Y.Z -m "vX.Y.Z" && git push origin main --follow-tags`

   **The `-a` is not optional.** `--follow-tags` pushes annotated tags only, so
   a plain `git tag vX.Y.Z` is created locally, silently skipped by the push,
   and the release workflow never fires.
4. `release.yml` builds the three targets, zips the bare binary of each with a
   `SHA256SUMS`, and publishes the release. Not a draft: Rokit resolves against
   the releases API, which does not show drafts, so a draft is indistinguishable
   from no release for every install command in the README.

## Security

Vulnerabilities go through [SECURITY.md](./SECURITY.md), never the tracker.
