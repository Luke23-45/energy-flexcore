# Pre-commit checklist

Use before committing a documentation change.

## Content

- [ ] Every new external fact has a primary source or is labelled as a
      recommendation or assumption.
- [ ] No sentence implies certification, safety-case status, or live-control
      permission.
- [ ] Solver classes are named correctly (HiGHS is not SOCP, Clarabel is not
      MISOCP, power flow is not OPF).
- [ ] Units follow MW/MVAr/MVA/MWh; sign conventions are stated, not assumed.
- [ ] Command states and gate names match the lifecycle table exactly.
- [ ] Release claims match the ladder: 0.x is simulation-only.

## Structure

- [ ] Section numbering is unchanged unless the change required it.
- [ ] Cross-references (`§N`, `§N.M`) still resolve.
- [ ] Reference IDs are stable; new sources use the next free number.
- [ ] Table of contents matches the H1 headings.
- [ ] Tables have consistent cell counts.
- [ ] Code fences are closed.
- [ ] The document still ends with its closing statement, not a truncated line.

## Version

- [ ] Version bumped only if the change warrants it.
- [ ] Header, revision note, and change list updated together.
- [ ] Supersedes line points at the previous version.

## Commands

- [ ] `python tools/verify_docs.py` passes
- [ ] `python tools/validate_agent.py` passes (if agent files changed)

## Honesty

- [ ] The completion message quotes real command output.
- [ ] Anything unverified is named as unverified.
- [ ] Removed or changed claims are called out explicitly.
