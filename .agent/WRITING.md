# Writing Rules

Applies to the report, the foundations document, and anything an agent writes in
this repository.

## Evidence

- A factual claim about the outside world needs a source or must be labelled as
  an assumption or recommendation.
- Prefer primary sources: the standard body, the vendor's own documentation, the
  project's repository, the regulator's publication. A blog summarising a
  standard is not a source for what the standard says.
- When a source cannot be reached, say so. Do not silently substitute a guess.
- Distinguish three sentence types:
  - **Established fact** — externally verifiable, cited.
  - **Recommendation** — Neryva's design choice, no citation needed, no
    implication that the world requires it.
  - **Assumption** — stated, local to the decision, and flagged.
- Do not cite a document for a claim it does not contain. Checking the clause
  is not optional.

## Citation format

- Reference IDs are stable: `[R1]` … `[R29]` today. New sources take the next
  free number and are added to §27.
- Every defined reference must be cited at least once in the body, and every
  citation must resolve to a definition. `tools/verify_docs.py` enforces both.
- Use the organisation's own name for the source. Do not invent a publisher.
- Record the access date when the source is a live page. The report uses a
  single research cut-off in §27; update it when you add a later source.

## Units and numbers

- Canonical units: MW, MVAr, MVA, MWh. Do not mix kW/kWh into contracts.
- `P > 0` = export at the point of connection. Say "export" or "import" when the
  sign could be misread.
- Reactive power has no universal sign. State which convention applies.
- SOC is a fraction in [0,1] at the API boundary.
- Do not round a number in a way that changes its meaning (a 2 s vs 2.0 s
  timeout, a 10% vs 10.0% limit).

## Terminology

| Use | Avoid |
|---|---|
| license | licence |
| acknowledgement (noun) | acknowledgment, unless quoting a source |
| operator-approved control | manual control, human-in-the-loop |
| read-only / shadow mode | passive mode |
| SUPERSEDED | replaced (as a state name) |
| dispatch command | setpoint (when referring to the lifecycle object) |

- `SUPERSEDED` and `CANCELLED` are different states. Do not merge them.
- "Validation", "feasibility", and "authorization" are different gates. Do not
  use them interchangeably.
- "Certified" and "conformant" require a test result. Implementing an adapter is
  neither.

## Structure

- One canonical report. Do not fork copies or create "v1.1-final-2".
- Section references use `§N` or `§N.M` and must resolve.
- Tables for enumerations; keep pipe counts consistent within a table.
- Mermaid diagrams only where the relationship is genuinely a graph or state
  machine. Keep node text short.
- Fenced code blocks must be balanced.
- Keep the table of contents in sync with the top-level headings.

## Versioning

- Report versions use Semantic Versioning at the document level: major for a
  changed product position, minor for added decisions or sections, patch for
  corrections.
- A version bump updates the header, the revision note, and the change list in
  the same edit.
- The foundations specification and the report version independently.

## Tone

- Declarative, specific, and free of marketing language.
- State limits plainly. "This does not certify X" is better than silence.
- No emojis in normative documents.
