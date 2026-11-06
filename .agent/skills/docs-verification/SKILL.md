---
name: docs-verification
description: Verifies the FlexCore report after edits by checking headings, section references, tables, code fences, R1-R29 citations, and the table of contents. Use after any change under docs/plan, or when a documentation task must be proven complete.
metadata:
  author: neryva
  version: "1.0"
---

# Documentation verification

Documentation is the product in this repository, so "it looks fine" is not a
result. Run the checks and quote the output.

## Run

From the repository root:

```sh
python tools/verify_docs.py
python tools/validate_agent.py
```

`verify_docs.py` accepts an explicit path if you are checking an older report
copy, but the canonical target is the newest file in `docs/plan/`.

## What is checked

| Check | Failure means |
|---|---|
| Top-level headings sequential 1..N | A section was deleted, inserted, or renumbered |
| No duplicate heading numbers | Two sections share a number |
| `§N` / `§N.M` resolve | A cross-reference points at nothing |
| Code fences balanced | A diagram or code block swallows the rest of the file |
| Table pipe counts uniform | A row has a missing or extra cell |
| `[Rn]` defined and cited | An orphan reference or an undefined citation |
| Reference numbers contiguous | A gap in the numbering |
| TOC matches top-level headings | The contents block drifted |

## Fixing common failures

**Undefined citation.** Either add the definition in §27 with the next free
number, or remove the citation. Never invent a URL to make it pass.

**Defined but uncited.** Find the claim that should cite it. If the claim is
gone, remove the definition and renumber only if nothing else references the
number — stable IDs are preferred over tidy numbering.

**TOC mismatch.** Edit the "Table of contents" list to match the H1 titles
exactly, including capitalization and punctuation.

**Pipe-count mismatch.** Count the cells, not the characters. A cell containing
an escaped pipe still counts as one separator in this checker, so avoid `\|` in
tables and rephrase instead.

**Unbalanced fence.** Find the opening fence and close it with the same
backtick count.

## Verify the change, not just the file

- Re-read the edited paragraph in context; a passing checker does not prove the
  sentence is true.
- If you changed a fact, confirm the source still supports it.
- If you bumped the version, confirm header, revision note, and change list all
  moved together.
- Report the exact command output. If something failed and you did not fix it,
  say which check and why.

## Checklist

See `references/checklist.md` for the pre-commit review list.
