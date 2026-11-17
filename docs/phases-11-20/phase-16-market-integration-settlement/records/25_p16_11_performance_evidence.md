# P16.11 — Performance evidence capture: evidence record types, no score claimed

| Field | Value |
|---|---|
| Subtask | P16.11 (Signal-following telemetry, response timestamps vs instruction, mileage/RMRTS tracking, SOC traces, precision-score inputs — post-Oct-2025 consolidated score AS REPORTED; RegUp/RegDn post-Phase-2 date UNVERIFIED, marked [S-94][S-95]; German FCR 30 s / aFRR 5 min evidence classes [S-119]. Evidence RECORD types; no score is claimed) |
| Wave / agent | Phase 16 wave 1, Agent B |
| Status | WRITTEN-UNRUN: the evidence record types are written, never compiled, never executed. No evidence has ever been captured from a market (none exists); no score, metric, or qualification is computed or claimed anywhere. |
| Sources | [S-94] (PJM DataMiner2 reg_market_results — the publication class the score inputs reference), [S-95] (PJM regulation precision scoring coverage — the post-Oct-2025 consolidated score as reported; the RegUp/RegDn post-Phase-2 date is reported without a verifiable source date), [S-119] (German TSO prequalification, regelleistung.net v1.05 Jul 2024 — FCR 30 s / aFRR 5 min), [S-120] (ESIG MMS aggregated-DER bids — the aggregation-evidence context) |
| Files touched | `crates/flexcore-application/src/performance_evidence.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line only) |

## 1. What landed

| Item | Content |
|---|---|
| `SignalFollowingSample` | Instruction-linked setpoint/observation pairs carried verbatim; a MISSING observation is carried as `None` — never zero-filled (a missing sample is evidence of a gap, never evidence of agreement) |
| `ResponseTimestampObservation` | Instruction time, observed full-response time, and the measured latency (validated non-negative at construction — a response cannot precede its instruction; refused, never clamped) |
| `GermanEvidenceClass` + window constants | The DECLARED full-response windows per [S-119]: FCR 30 s (`GERMAN_FCR_WINDOW_MILLIS = 30_000`), aFRR 5 min (`GERMAN_AFRR_WINDOW_MILLIS = 300_000`); re-verify at use. Latency observations are compared against the window by the CALLER — this module records observations, never pass/fail verdicts |
| `MileageObservation` | Mileage carried AS PUBLISHED (exact decimal, non-negative — negative magnitudes refused, never sign-flipped), with the reporting-source reference carried verbatim (the RMRTS reporting class per [S-94][S-95]; UNVERIFIED per-source) |
| `SocTracePoint` | SOC as a fraction in [0, 1] (the canonical-unit rule holds in evidence records; the units type enforces the range) |
| `PrecisionScoreInputRecord` + `ScoreDateStatus` | The score-INPUT record: program label verbatim (UNVERIFIED per-source), a mandatory explicit date status, and a non-empty input-reference set. The consolidated-score class carries `ReportedAsOf` (sourced, re-verify-at-use); the RegUp/RegDn post-Phase-2 class carries `RegUpRegDnDateUnverified` — the UNVERIFIED mark is structural, never a silently-assumed date [S-95] |
| Tests (written-unrun) | Inline suite: statement pins (no-score-claim, date-status rule), window constants, non-zero-fill of missing observations, latency/window/mileage refusals, both date-status variants, empty-input-set refusal |

## 2. Recorded decisions

1. **Evidence records, never scores** (`NO_SCORE_CLAIMED_STATEMENT`, pinned by tests): nothing computes a precision score, performance metric, or qualification verdict — scoring belongs to the market operator, and any FlexCore-side aggregation would be a claim this program cannot make (conventions §7.3).
2. **Dates are sourced data with the re-verify-at-use rule** (`EVIDENCE_DATE_STATUS_RULE`): a record without an explicit date status is unconstructable. The RegUp/RegDn post-Phase-2 date is UNVERIFIED and marked on every record of that class — the honesty requirement from the plan row, made structural.
3. **Every evidence record links to its instruction** where the class demands it: signal-following, response, and mileage records all require a non-empty instruction/command reference; an evidence record with no linkage is unconstructable for those classes.
4. **The German windows are declared expectations** [S-119], re-verified at use; verdicts are the caller's or the operator's, never this module's.

## 3. Honest boundaries

1. No evidence has been captured from any market; no command has ever been issued against a market instruction; every fixture value is synthetic.
2. No score is computed, claimed, predicted, or implied — including the post-Oct-2025 consolidated score, which exists here only as an input-record CLASS with the "as reported" posture [S-94][S-95].
3. No qualification, prequalification, or certification is claimed for any market, including the German prequalification classes — the class labels name the evidence expectations, not a met requirement.
4. Every test is WRITTEN-UNRUN; execution is the human gate. The permitted-tool outputs are quoted in the batch close-out record (26).
