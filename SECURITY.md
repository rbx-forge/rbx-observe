# Security policy

## Reporting a vulnerability

**Use GitHub private security advisories:**
<https://github.com/rbx-forge/rbx-observe/security/advisories/new>

Not a public issue. Include what you ran, what happened, and why it is a
security problem rather than a bug.

**What to expect.** Solo-maintained: an acknowledgement within a week is the
realistic commitment, a fix as fast as the severity warrants. Credit in the
advisory unless you ask otherwise. No bounty program.

## Supported versions

Only the latest release.

## Threat model

This tool takes **no credentials**. It reads public Roblox endpoints, and there
is nothing of yours for it to leak. That makes the interesting reports the ones
about what it writes and what it fetches:

- A path traversal or arbitrary write from remote-controlled data — experience
  names, product names, badge names all end up in output paths.
- Output that executes: a crafted name landing unescaped in generated TOML,
  CSV, or a shell-consumable format.
- A dependency vulnerability with a plausible path to either.

Out of scope:

- **That the tool reads public data at all.** Everything it fetches is visible
  to any logged-out visitor. If you believe a specific endpoint should not be
  read, that is worth an issue — see the boundary in the README — but it is a
  scope discussion, not a vulnerability.
- Roblox-side rate limits and API behavior. Report those to Roblox.
- Anything requiring write access to your machine.

## The boundary is a security property

The README's rule — game-level public data only, never person-level — is
enforced by review, not by a sandbox. A change that starts collecting
person-level data is a defect in this project even if nothing technically
breaks, and reporting one as such is welcome.
