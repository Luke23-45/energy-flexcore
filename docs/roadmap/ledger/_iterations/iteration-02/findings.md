# Iteration 02 — Re-review (author's fresh re-read, 2026-11-21)

**Reviewer:** ledger author (re-read with fresh checks; the iteration-01
independent reviewer was a different subagent). **Method:** a Python
re-review script (`/tmp/ledger_rereview.py`, ephemeral) plus targeted
grep checks of every patched line, run after the iteration-01 patch
pass. **Verdict: 0 new findings. The ledger is FINALIZED as a plan.**

## Scope

Re-verified every iteration-01 finding's fix at the patched line, and
re-ran the structural checks that the iteration-01 review used, against
the post-patch ledger (72 subtasks).

## Per-finding verification (all FIXED, verified at the patched line)

- **F-01:** W4.07 Dependencies now `W4.01` only; W4.04 Dependencies
  `W4.02, W4.07` (kept per the fix). The one remaining "W4.04
  prototype" mention in W4.07 step 1 is the sanctioned prose ("later
  verification consumer, not a prerequisite").
- **F-02:** W6.06 Dependencies now `W4.12, W5.06, W5.10`; W6.05
  Dependencies now `W6.06, W4.12, W5.06, W5.10` (kept per the fix).
- **F-03:** W1.17 step 1 defers the three Wave 2 §13.3 items to the
  Wave 2 test record (W2.08) with a checkbox; W2.08 step 5 + checkbox
  verify them against the Wave 2 definitions.
- **F-04:** W2.03 step 4 and W2.06 Risks cite the W2.08 test record
  only; zero "W1.17" mentions remain in W2.03/W2.06.
- **F-05:** MASTER_LEDGER §3 Wave 1 entry gate requires G1 only
  ("G2 is a Wave 1 deliverable, decided in W1.02"); Wave 4 entry gate
  requires Gate 0 + §2.1 record only ("G5 is a Wave 4 deliverable,
  decided in W4.01"); the note after the table states G2/G5 are
  decided inside their own waves.
- **F-06:** W0.06 step 1 cites the D1 record (W0.04) only;
  MASTER_LEDGER §5 carries the G1↔G2 sequencing note.
- **F-07:** W0.05 carries the four §0 boundary sentences in step 4,
  objective, deliverable and acceptance (grep: 2 lines carrying the
  three "not a" phrases plus the "assessment is not an asset" line).
- **F-08:** W0.09 file exists; W0.08 Inputs/Dependencies/step
  1/acceptance include W0.09; W1.11 and W5.01 each carry a W0.09
  cross-reference step + dependency; MASTER_LEDGER §1/§2/§3/§4 name
  W0.09.
- **F-09:** W5.11 file exists; W5.10 Inputs/Dependencies/step
  1/acceptance cover W5.01–W5.11; MASTER_LEDGER §1/§2/§4 name W5.11.
- **F-10:** W5.02 step 1 defines all ten §9 records (each name
  appears 3 times in the file — step, deliverable, acceptance).
- **F-11:** W5.05 carries 5 "13.2" mentions (trace, step 5,
  checkbox).
- **F-12:** §1.1 reuse rules present in W1.09, W1.10, W2.02, W4.09
  (all four files match the "1.1" grep).
- **F-13:** flexible-load inventory in W2.04, BESS
  no-flexibility-credit rule in W2.05 (both files match the "1.1"
  grep via their trace lines and carry the new checkboxes).
- **F-14:** "unless a separate" = 0 hits in W3.03; read-only wording
  in place.
- **F-15:** W4.10 objective/step 2/deliverable state "interim
  position only" with the REopt API v3 PoC adapter as the prototype
  choice; W4.04 step 5 states the PoC adapter engine; W5.03 step 5 +
  checkbox records the final G7 decision.
- **F-16:** W6.07 is `07_regional-pack-review-expiry-governance.md`,
  governance only ("Pack format and contents are defined in W6.01,
  not redefined here"); W6.01 step 5 + checkbox cross-refers W6.07.
- **F-17:** W4.06 step 4/deliverable/acceptance give a usability
  verdict only; W4.13 step 1 notes the accuracy verdict is W4.05's.
- **F-18:** "threat considerations"/"pre-launch" = 0 hits in W5.08.
- **F-19:** W4.03 step 1 shows source for manual entry and source +
  imagery age for the provider.
- **F-20:** "(it does not)" = 0 hits in W6.06; the explicit
  site-location rule is in place.

New § citations introduced by the patch pass resolve in the design
doc: §2.1 (line 80), §8.2 (line 510), §13.2 (line 753), §1.1
(line 38).

## Structural checks (script, all PASS)

- "Acceptance criteria" lines: **72** — 9/18/8/5/13/11/8.
- Subtask headings: 72, unique, contiguous 1..N per wave.
- File numbers match subtask numbers.
- Checkboxes: **305 unchecked, 0 checked**.
- Dependency graph: 0 undefined refs, **0 cycles**.
- All `W<w>.<nn>` references anywhere in the ledger resolve to a
  defined subtask.
- "certified" sweep (wave files): 3 hits, all in W1.15, all
  negations ("no BuildingSync-certified claim ... without granted
  status").

## One check corrected during re-review (not a ledger defect)

The script's first run flagged the forward-dependency check as FAIL
because it only allowed the two exit-gate→review-added cases. The
other two forward dependencies (W4.04→W4.07, W6.05→W6.06) were
explicitly prescribed by the iteration-01 reviewer in its own fixes
(F-01, F-02), and all four are acyclic and documented. The check was
corrected to allow exactly the four documented cases; it now passes.
All four are recorded in MASTER_LEDGER §4 and README step 3:
execution follows Dependencies, not ID order — the v1 ledger's own
precedent (P2.29 depending on the review-added P2.31).

## Findings

**Zero new findings.** No duplicate subtasks, no invented content
beyond the design doc, no implementation/certification/safety claims,
no stale cross-references found in this pass.

## Repo checks (run 2026-11-21, after the patch pass)

- `python3 tools/verify_docs.py` — exit 0.
- `python3 tools/validate_agent.py` — exit 0.
- `git status --short` — only `?? docs/roadmap/` (untracked; nothing
  else changed). No commit, no push.
