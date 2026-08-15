<!--
Thanks for the patch. The checklist is short on purpose: it is what CI checks
plus the two things CI cannot.
-->

## What this changes

<!-- One paragraph. If it fixes an issue, "Closes #N" — one `closes` per
     number, since `Closes #1, #2` only closes the first. -->

## Why

<!-- The reasoning, not the diff. What breaks without it, or what it makes
     possible. -->

---

- [ ] `cargo fmt --all` and `cargo clippy --workspace --all-targets -- -D warnings` are clean
- [ ] `cargo test --workspace` passes, and behaviour changes come with a test that fails without them
- [ ] HTTP tests assert the **request** that goes out, not only that a canned response parsed
- [ ] Docs moved with the code (`README.md`, `docs/`, and a `## [Unreleased]` line in `CHANGELOG.md`)
- [ ] Every commit is signed off (`git commit -s`) — the DCO check enforces it

**If this PR calls a new Roblox endpoint**, name it here with one line on why
it is game-level and public. Anything keyed to a person is out of scope; see
[the boundary](../README.md#the-boundary-which-does-not-move) and the
"Not used, deliberately" table in `docs/endpoints.md`.
