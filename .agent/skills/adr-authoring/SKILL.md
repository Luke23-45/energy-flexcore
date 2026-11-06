---
name: adr-authoring
description: Writes and maintains Architecture Decision Records for FlexCore. Use when a decision is made, when an open gate in DECISIONS.md is resolved, when a technology choice changes, or when a trade-off needs a durable recorded rationale.
metadata:
  author: neryva
  version: "1.0"
---

# Writing an ADR

An ADR records **one decision**, the forces behind it, and what it costs. It is
not a design document and not a meeting minute.

## Where

- File: `docs/adr/NNNN-short-slug.md` (four-digit number, lowercase slug).
- Index: append a row to the decision record in `.agent/DECISIONS.md`.
- Numbering: continue from the highest existing file. The report §22.1 already
  names ADR-001 to ADR-012 as the initial set; use those titles when you create
  the corresponding records rather than inventing competing ones.

## Status values

| Status | Meaning |
|---|---|
| `proposed` | Written, not yet accepted |
| `accepted` | In force |
| `rejected` | Considered and declined; kept for history |
| `superseded by ADR-NNNN` | Replaced; never deleted |

## Procedure

1. Confirm the decision is actually being made. If the gate is still open, the
   correct output is a `proposed` ADR listing options, not a fake decision.
2. State the decision in one sentence, active voice: "Use X for Y."
3. Record the forces: constraints, requirements, prior decisions, and the cost
   of being wrong.
4. List at least one real alternative and why it lost. "Do nothing" is a valid
   alternative.
5. State consequences, including the bad ones. An ADR with only benefits is
   marketing.
6. Name the revisit trigger: the condition that would reopen the decision.
7. Update `.agent/DECISIONS.md` if it closes a gate.

## Quality bar

- A reader in two years can reconstruct *why* without asking anyone.
- No unsourced external facts. If the ADR says a library supports a problem
  class, cite the documentation.
- No hidden coupling to a vendor. If the decision locks in a provider, say so.
- Keep it under two pages.

## Template

Copy `assets/adr-template.md`.

## Do not

- Do not retroactively rewrite an accepted ADR. Supersede it.
- Do not use an ADR to document a bug fix or a style preference.
- Do not mark an ADR accepted on the user's behalf. Acceptance is a human act.
