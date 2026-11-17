# P16.16 — Capacity-market participation records (RECORD-ONLY): PJM RPM / ISO-NE FCA19 / ISO-NE DECR qualification-evidence checklists; the LR-075 date conflict resolved by record

| Field | Value |
|---|---|
| Subtask | P16.16 (Capacity-market participation records, RECORD-ONLY depth — RPM (PJM Feb 2027 proposed) / FCA19 (Feb 2026) / DECR qualification-evidence checklists; the LR-075 PJM date conflict resolved by record with sourced dates, re-verified at use [S-26][S-103]. Checklists; no qualification claimed) |
| Wave / agent | Phase 16 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the checklist model, the sourced-date type, the LR-075 conflict record, and the three declared record packs are written, never compiled, never executed. **No qualification is claimed, no filing is prepared for submission, no auction is entered, no evidence exists anywhere** (every item starts UNSATISFIED; no QUALIFIED state exists in any vocabulary — structural). No market is connected (ADR-011; conventions §7.3). |
| Sources | [S-26] (FERC Order 2222 explainer — the timeline class), [S-103] (ISO-NE Order 2222 participant readiness — FCA19/DECR class), [S-104] (ISO-NE metering & telemetry checklist — inventoried, detail UNVERIFIED), [S-90] (PJM Order 2222 overview — the RERRA requirement class); `ledger/phase-09-pilot-qualification/11_market_program_requirements.md` §7 (the Phase 9 capture template and its PJM date-conflict row, checked 2026-11-07); the LR-075 disposition in the phase-16 context |
| Files touched | `crates/flexcore-application/src/capacity_market_records.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `SourcedDate` | The sourced-date type: date text as published + source label + verification date + the MANDATORY re-verify flag. `reverify_at_use == false` is a REFUSAL — a date without the rule is UNCONSTRUCTABLE (plan risk R-P16-1 made structural). Reused by P16.17 and P16.18 (one home for the rule) |
| `DateConflictRecord` | The conflict record: at least two sourced dates carried, NEITHER selected (no selection field exists), the re-verify authority named |
| `lr_075_pjm_capacity_date_conflict()` | The LR-075 conflict RESOLVED BY RECORD: side A "February 2027 (proposed)" ([S-26] gap-analysis timeline row) vs side B "1 February 2028 (energy expected; the capacity date explicitly conflicting between sources — an open verification task)" (the Phase 9 §7 capture, checked 2026-11-07). Both carried verbatim; the record names PJM's tariff [S-90] as the re-verification authority; the description states NEITHER is selected |
| `QualificationChecklistItem` | Requirement text recorded from a MANDATORY named source (the Phase 9 scope rule); starts UNSATISFIED with no evidence; `record_evidence` requires a non-empty evidence reference (a RECORD, never a validated claim — the P16.03 attestation rule inherited) |
| `CapacityQualificationChecklist` | Per-market checklists for `PJM_RPM` / `ISO_NE_FCA19` / `ISO_NE_DECR` (a closed three-market vocabulary; a fourth market is a NEW pack — ADR-011). The declared packs carry 3/2/2 recorded requirements respectively, each with its source label and UNVERIFIED markers where the detail is not verified in this repository |
| `CapacityChecklistStore` | BTreeMap-backed, register-only, deterministic |

## 2. Recorded decisions

1. **RECORD-ONLY depth, no qualification claimable** (`RECORD_ONLY_NO_QUALIFICATION_STATEMENT`, pinned by tests): there is NO QUALIFIED state in the item vocabulary, the market vocabulary, or anywhere else — the highest fact a checklist can carry is "all evidence recorded", and that is a fact about RECORDS only. The `all_evidence_recorded` test asserts this explicitly.
2. **Dates are sourced data; the re-verify rule is structural** (`DATES_ARE_SOURCED_DATA_STATEMENT`, pinned by tests): no date is baked into logic; conflicts resolve BY RECORD, never by silently picking a date (the LR-075 disposition carried out literally).
3. **The Phase 9 scope rule is inherited**: a requirement enters only from a named source; where the detail is not verified here (the PJM telemetry cadence, the DECR requirement detail, the ISO-NE checklist line items) the item is labeled UNVERIFIED and re-verified at use — never invented.
4. **Evidence starts EMPTY everywhere**: no evidence exists in this program; recording evidence is a human-gated record act through the store, never a constructor shortcut (the checklist constructor refuses items that arrive already carrying evidence).

## 3. Honest boundaries

1. No qualification, prequalification, filing, or auction entry is claimed or performed; no evidence exists; every fixture reference in the tests is synthetic.
2. The checklist requirement texts are RECORDED summaries of the named sources — they are not the sources themselves, and no conformance or compliance claim is made (conventions §7.3).
3. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the close-out record (47).
