---
name: evidence-first-research
description: Applies the no-hallucination sourcing protocol when adding external facts, protocol versions, standard clauses, solver capabilities, or release dates. Use whenever a claim about the outside world enters a document, review, or answer.
metadata:
  author: neryva
  version: "1.0"
---

# Evidence-first research

FlexCore documents are used to make engineering and commercial decisions. A
plausible sentence with no source is a defect, not a draft.

## 1. Classify the sentence before writing it

| Type | Test | Handling |
|---|---|---|
| Established fact | Someone outside Neryva can verify it | Cite a primary source |
| Recommendation | It is a design choice | Say "recommended" or "we will" |
| Assumption | Needed locally, not provable now | Label it, scope it, name what would confirm it |

If a sentence cannot be classified, it is probably doing too much. Split it.

## 2. Prefer primary sources

In descending order of trust:

1. The standard body's own publication (IEEE, IEC, NIST, FERC, OASIS).
2. The protocol's official specification or conformance suite.
3. The project's own repository, release notes, or documentation.
4. The vendor's own documentation.
5. Peer-reviewed literature.
6. Secondary summaries, acceptable only as a pointer, never as proof.

Never cite a blog for what a standard says. Never cite a standard for a clause
you have not opened.

## 3. Verify the specific claim

- Version numbers, dates, and clause numbers are where hallucination hides.
  Open the page; do not pattern-match the URL.
- Distinguish draft from final. An initial public draft is not a published
  standard.
- Distinguish the library from the solver class. A package name does not imply
  the capability.
- Distinguish "supports" from "certified for".

## 4. Record it

- Add a stable reference ID in §27 and cite it in the body.
- Record the access date for live pages; update the research cut-off if the
  source postdates it.
- If the source is unreachable from the working environment, say so and keep the
  citation only if it was verified earlier. Do not fabricate a mirror.

## 5. When you cannot verify

Say one of these, explicitly:

- "Not verified. The source was unreachable."
- "Assumption. To be confirmed with the vendor."
- "Recommendation. This is a Neryva choice, not an external requirement."

Silence is not an option, and neither is confident improvisation.

## 6. Reviewing someone else's text

Flag, in order:

1. Claims with no source.
2. Sources that do not contain the claim.
3. Version or date drift.
4. Capability inflation (a solver, protocol, or tool credited with more than it
   does).
5. Hedge words that hide an unverified claim ("typically", "generally",
   "supports" without qualification).

See `references/source-policy.md` for the per-domain source map.
