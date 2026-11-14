<!-- P9.18 — Site telemetry quality and clock-sync verification (decision record). VERIFICATION TEMPLATE ONLY: every procedure is written, every result/measurement cell is EMPTY. No site exists (P9.01 freeze UNSIGNED); no shadow operation exists (P9.17 NOT EXECUTED); no sync-source procurement exists (UNSIGNED). The pilot-profile clock constants cited from crates/edge-agent/src/clock_quality.rs are recorded DEFAULTS (decision-record decisions), NOT measurements — no offset, drift, or freshness figure exists anywhere in this record. -->
# P9.18 — Site Telemetry Quality and Clock-Sync Verification (Template — Result Cells EMPTY)

Date: 2026-11-14. Subtask: P9.18 (`ledger_phase_9.md` §P9.18).
Owner role: Platform engineering (with integration engineering).
Report trace: §23 Stage 9; §10.3 (time model) and §10.4 (telemetry
quality); §17.4 (the threat model rows "Malicious or incorrect
telemetry" and "Clock drift"; §17.5 invariant 9).
Inputs: P9.14 lab results (**NOT EXECUTED**), P9.17 shadow operation
(**NOT EXECUTED — no shadow data exists**), site telemetry sources and
their synchronization method (**DO NOT EXIST — no site, no customer, no
source inventory**).

## 1. Honest status — the plain statement

**No telemetry source has been verified at any site, no clock has been
measured, no quality mapping has been observed at a site, and no
stale/missing injection has been performed.** There is no site, no
telemetry source inventory, no shadow operation to sample, and no
synchronization method to verify. This record delivers what an agent
can deliver under ENTRY_NOTE.md boundary 6:

- the **§10.3 time-model verification procedure** (§2) with EMPTY
  result cells;
- the **§10.4 quality-mapping verification procedure** (§3) with the
  per-source mapping table EMPTY (the six canonical states are §10.4's;
  §10.3 is the time model the pairs come from);
- the **clock-sync verification procedure** (§4), which cites
  `crates/edge-agent/src/clock_quality.rs` — the closed clock-quality
  vocabulary, the closed-clock-expiry (hold-on-drift) discipline, and
  the pilot-profile DEFAULTS, which are **DEFAULTS, not measurements**;
- the **P9.39 latency/clock interaction** (§5): the load-side clock
  measurements are P9.39 M-5's deliverable; the pilot's acceptance
  criterion (b) is owned HERE;
- the **sync-source requirements** (§6) — a named, allowlisted source
  per the signed D6 record; the procurement is **UNSIGNED**;
- the **freshness-per-use-case procedure** (§7) and the
  **stale/missing end-to-end injection procedure** (§8);
- the **boundaries** (§9), reviewer procedure (§10), filling commands
  (§11), and open risks for P9.27 (§12).

Every measurement cell is `(EMPTY — UNRUN)`; no figure is fabricated,
estimated, or extrapolated. The code citations below are DESIGN
statements verified by READING the named files on 2026-11-14 — not
evidence that anything behaves a certain way at a site.

## 2. The §10.3 time model — verified per telemetry source (result cells EMPTY)

§10.3 (verbatim requirements): every time-dependent record must
distinguish `event_time` (when the measurement occurred) from
`ingest_time` (when FlexCore received it); carry `valid_from`/
`valid_until` where applicable, the source sequence or version, the
timezone/UTC offset where relevant, and clock-quality status if
available. UTC internally; the original site timezone retained for
operator display and market calendar calculations; no assumption that
a local calendar day is always 24 hours (daylight-saving).

**Procedure (per source, per site):**

| Step | What is checked | Result cell |
|---|---|---|
| TM-1 | Every record carries BOTH `event_time` and `ingest_time`, distinctly (the P0.09 honesty pair; produced at `crates/edge-agent/src/ingest.rs` — `event_time` from the source, `ingest_time` stamped at the edge from the injected `Clock`) | (EMPTY — UNRUN) |
| TM-2 | Storage/computation is UTC; the site timezone is retained for operator display and market calendar use; DST-day-length handling verified against the site's calendar (a DST transition day, scheduled deliberately, is the check) | (EMPTY — UNRUN) |
| TM-3 | Source sequence/version is recorded on every record (§10.3; §14.3's monotonic sequence discipline on the command side) | (EMPTY — UNRUN) |
| TM-4 | `valid_from`/`valid_until` present where the record type declares them | (EMPTY — UNRUN) |
| TM-5 | Clock-quality/synchronization status carried where available (the §4 record supplies it) | (EMPTY — UNRUN) |

## 3. The §10.4 six canonical quality states — verified at the pilot (result cells EMPTY)

§10.4 (verbatim rule): every telemetry value carries **exactly one**
canonical quality state — `GOOD`, `SUSPECT`, `BAD`, `MISSING`,
`ESTIMATED`, `STALE` (the same closed six in AGENTS.md §4). Richer
source states (e.g. `SUBSTITUTED`, `OUT_OF_RANGE`, `UNSYNCHRONIZED`,
`COMMUNICATION_LOSS`, `UNRELIABLE`) are mapped to exactly one canonical
state **at the adapter boundary**, with the original source code
retained as diagnostic metadata. **A quality flag must not be silently
converted into a normal reading.** Every use case defines which quality
states are acceptable.

**Procedure (per source, per site):**

| Step | What is checked | Result cell |
|---|---|---|
| QM-1 | For each source, its native states are enumerated and each maps to EXACTLY ONE canonical state in the frozen ACL table (the `adapters/openems.rs` mapping-profile discipline; `OPENEMS_STALE_CODE` → canonical `STALE` is the recorded example) | (EMPTY — UNRUN) |
| QM-2 | The original source code travels as diagnostic metadata on every mapped value | (EMPTY — UNRUN) |
| QM-3 | No upgrade path exists: a degraded state never arrives as `GOOD` anywhere downstream (the lab counterpart is the P9.14 L2 suite case `contract_quality_passthrough_no_upgrade`; the site check re-observes it on site data) | (EMPTY — UNRUN) |
| QM-4 | **No `SUSPECT` → `GOOD` collapse** (the ledger's named failure mode): every value whose source flagged suspicion arrives `SUSPECT` or worse | (EMPTY — UNRUN) |
| QM-5 | `ESTIMATED` values are never treated as measured in a schedule input (the ledger's named failure mode): the schedule path's acceptance set excludes `ESTIMATED` unless the use case explicitly declares it | (EMPTY — UNRUN) |
| QM-6 | Per-use-case acceptance sets recorded (§7): which of the six states each use case accepts, before the use case consumes pilot data | (EMPTY — UNRUN) |

Verification tooling note: the control-plane quality metrics are
per-STATE counts over all six canonical states, zeros included
(`apps/control-plane/src/metrics.rs` — the P2.21 families), so a state
that never occurs is VISIBLE as a zero rather than absent; the
verification reads these series, not an aggregate.

**The per-source mapping table (fill at execution FROM the frozen ACL
table and the observed site values — never from memory):**

| Source (native id) | Native states | → Canonical state (exactly one each) | Source code retained? | Observed at site (date) | Result |
|---|---|---|---|---|---|
| (none — no source inventory exists) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY — UNRUN) | (EMPTY) |

## 4. Clock synchronization — verified at the site and the edge (measurement cells EMPTY)

### 4.1 The code discipline cited (design facts, verified by reading `crates/edge-agent/src/clock_quality.rs` on 2026-11-14)

| Element | Recorded in the module |
|---|---|
| Closed clock-quality vocabulary | Exactly four states, no fifth: `Unknown` (no observation yet — the DEFAULT, never healthy-by-default), `Synchronized` (estimated offset inside tolerance), `Degraded` (an observation landed, offset BEYOND tolerance), `Unsynchronized` (sync source unreachable, or no fresh observation within the sync timeout) |
| **The closed-clock-expiry discipline (hold-on-drift)** | Expiry checks are trusted ONLY on `Synchronized`. On `Degraded`, `Unsynchronized`, or `Unknown` the edge does NOT trust expiry checks — commands are rejected/held fail-closed and the condition is AUDITED (`ClockQualityMonitor::expiry_check_allowed`; `audited_expiry_check` composes the audited hold; `EDGE_CLOCK_HOLD_ON_DRIFT` leads the audit detail) |
| Offset estimation | The MEDIAN of a bounded window (`DEFAULT_SAMPLE_WINDOW` = 8) of event_time-vs-ingest_time pairs (`TimeSamplePair`); robust to one corrupt/unlucky pair; a pair whose absolute offset exceeds `MAX_ABS_OFFSET_MILLIS` (one day, 86 400 000 ms) is REFUSED as corrupt, never folded into the estimate |
| Health seam | `Synchronized` → `HEALTHY`; `Degraded`/`Unsynchronized` → `DEGRADED_CLOCK_DRIFT`; `Unknown` → `DEGRADED_UNVERIFIED` (never healthy-by-default) — the ingest path keeps flowing flagged; the COMMAND path is what fails closed |
| **Pilot-profile DEFAULTS** | `DEFAULT_PILOT_TOLERANCE_MILLIS` = 100 (maximum acceptable offset); `DEFAULT_PILOT_SYNC_TIMEOUT_MILLIS` = 30 000. **These are recorded DEFAULTS (decision-record decisions), NOT measurements and NOT verified site behaviour.** The deployment may configure different values; the CONFIGURED values in force at the site are what §4.3 records |
| Sync source | A NAMED, ALLOWLISTED configuration reference — never an assumption that the host clock is correct; the D6 record must list it (P6.03). **NO NTP CLIENT IS WRITTEN** (a new dependency and a new D6 network path); if the site firewall blocks the allowlisted source, the module reports `Unsynchronized` and the edge holds commands — it never falls back to the host clock |

### 4.2 Procedure (per site)

| Step | What is checked | Result cell |
|---|---|---|
| CS-1 | Record the sync source in force: the named, allowlisted reference exactly as the SIGNED D6 record lists it (P6.03); a source not in the signed D6 record = fail | (EMPTY — UNRUN) |
| CS-2 | Record the CONFIGURED tolerance and sync timeout at the deployment (which may differ from the module's pilot DEFAULTS of 100 ms / 30 000 ms — the configured values govern) | (EMPTY — UNRUN) |
| CS-3 | MEASURE the offset/drift: the median event_time-vs-ingest_time offset over the observation window, from the monitor's estimate — never from a hand calculation on unrelated clocks | (EMPTY — UNRUN) |
| CS-4 | Verify the clock-quality STATE transitions observed at the site match the module's discipline (Unknown before the first pair; Synchronized inside tolerance; Degraded beyond; Unsynchronized past the timeout or on a noted outage) and that every TRANSITION is audited (`EDGE_CLOCK_QUALITY_CHANGED`) | (EMPTY — UNRUN) |
| CS-5 | **Verify command expiry still protects against the observed drift (§17.4)**: acceptance criterion (b) — the measured offset is within the bound the command expiry assumes; i.e. the measured median offset is inside the CONFIGURED tolerance, and a deliberately injected beyond-tolerance skew demonstrably HOLDS commands (the FAT-F3 counterpart at measurement precision) | (EMPTY — UNRUN) |
| CS-6 | Verify the health rendering at the site (`DEGRADED_CLOCK_DRIFT` / `DEGRADED_UNVERIFIED` observed where the state warrants; never healthy-by-default) | (EMPTY — UNRUN) |

### 4.3 Clock-sync measurement record — ALL CELLS EMPTY

| Field | Value (EMPTY — fill from actual measurement) |
|---|---|
| Site ID | |
| Sync source (as listed in the SIGNED D6 record) | |
| Configured tolerance / sync timeout / sample window (deployment values) | |
| Observation window (start/end, UTC) | |
| Median offset (ms) — measured | (EMPTY — not measured) |
| Offset range over the window (ms) — measured | (EMPTY — not measured) |
| Source outages observed (count, durations) | (EMPTY — not measured) |
| State transitions observed (count; audited?) | (EMPTY — not measured) |
| Beyond-tolerance injection → hold-on-drift demonstrated (yes/no + evidence) | (EMPTY — UNRUN) |
| Measurer / witness | **UNSIGNED** |

## 5. The P9.39 latency/clock interaction (owned here per the 39_ record)

`39_performance_load_test.md` M-5 measures clock-sync behaviour UNDER
LOAD (do time-sample pairs still arrive within the sync timeout at the
profile rate; does the median offset stay inside tolerance; does
hold-on-drift trigger under load; any spurious `DEGRADED_CLOCK_DRIFT`
transitions) and records, in its risk R-6, that **"the pilot's expiry
bound vs observed offset is P9.18's acceptance, and this test only
supplies the load-side measurements."** This record therefore OWNS the
acceptance:

| Interaction | Rule recorded |
|---|---|
| Load-side measurements | Supplied by P9.39 M-5 (its §5 threshold registration governs its own metric) — referenced, never re-measured here |
| Acceptance criterion (b) | Owned HERE (§4.2 CS-5): the measured clock offset (steady-state from §4.3, and under load from the P9.39 M-5 row) is within the bound the command expiry assumes (the CONFIGURED tolerance) |
| Hold-on-drift under load | If load delays sample pairs beyond the sync timeout, the edge HOLDS commands fail-closed — that is CORRECT behaviour with an operational cost; the acceptance question is whether the pilot's configured timeout/tolerance makes the hold rate operationally acceptable, decided per §7's registered criteria and carried to P9.27 if not |
| Ordering | The P9.39 load run precedes this record's CS-5 verdict where the load-side row is part of the evidence; where the pilot composition has no load run yet, CS-5 records the steady-state evidence and marks the load-side row PENDING-P9.39 — never a silent pass |

## 6. Sync-source requirements (procurement UNSIGNED)

| Requirement | Status |
|---|---|
| The site's time service must be a NAMED, ALLOWLISTED reference, listed in the SIGNED D6 record for the deployment (P6.03); the site network must permit the edge→source path (a new path is a D6 change, not an assumption) | **UNSIGNED — no D6 record is signed (P9.06 OPEN); no site network design is approved (P9.05 template)** |
| No NTP client is written into the edge agent (module header, verbatim discipline): acquiring pairs from the named source is the DEPLOYMENT's wiring responsibility | Recorded — the wiring does not exist |
| If the site has no existing time service meeting the tolerance, a source must be PROCURED for the pilot: the procurement is a human/organizational gate | **UNSIGNED — no procurement exists, none is scheduled** |
| Source reachability failure behaviour: the module reports `Unsynchronized` and the edge HOLDS commands — never a host-clock fallback | Design fact (§4.1); the site-level demonstration is CS-5's injection row |

## 7. Freshness per use case — measured against registered limits (cells EMPTY)

§10.4's closing rule verbatim: **"Every use case defines which quality
states are acceptable."** The ledger step 4: measure ingestion delay
and staleness against the freshness limit for EACH use case; record
which quality states each use case accepts.

| Use case (filled from the signed P9.01 scope) | Freshness limit (X-8-registered BEFORE the run — value + basis + definer + date) | Quality states accepted (declared before the run) | Measured ingestion delay (p50/p95/p99) | Measured staleness behaviour | Result |
|---|---|---|---|---|---|
| (none — no scope signed, no use case declared) | (EMPTY — not registered) | (EMPTY) | (EMPTY — not measured) | (EMPTY — not measured) | (EMPTY) |

The X-8 discipline (the P9.16 §6.2 rule, applied here): freshness
limits and acceptance sets are registered BEFORE the measurements they
judge, in a dated amendment; a limit fitted after a result was known is
INVALID and the affected measurement is discarded and re-run.

## 8. Stale/missing handling end-to-end — injection procedure (results EMPTY)

The ledger step 5, verbatim intent: inject a stale feed and a missing
feed and confirm the system holds/rejects per policy instead of
inferring values — **§17.5 invariant 9 verbatim: "A missing/uncertain/
stale model is not interpreted as a passing feasibility check."**

| Step | Injection | Expected (design-derived) | Result cell |
|---|---|---|---|
| SM-1 | Serve a STALE feed past the registered freshness limit at the site boundary | The value arrives `STALE` end-to-end; consumers see the degraded state; no upgrade; no inferred value | (EMPTY — UNRUN) |
| SM-2 | Serve a MISSING feed (expected interval with no value) | The value arrives `MISSING`; the use case's declared handling (hold/reject) applies; nothing is silently substituted or interpolated as measured | (EMPTY — UNRUN) |
| SM-3 | Confirm the FEASIBILITY path: with the stale/missing model input, the feasibility check does NOT pass on the degraded model (invariant 9) | Hold/reject recorded; the refusal audited | (EMPTY — UNRUN) |
| SM-4 | Confirm the schedule path never treats `ESTIMATED` as measured (QM-5 at injection precision) | Substituted/estimated values appear only as `ESTIMATED`, and only where the use case declared them acceptable | (EMPTY — UNRUN) |

Injection is COORDINATED WITH THE CUSTOMER (declared, bounded, in
shadow — the P9.16 §6.5 coordination rule). The reviewer's verification
method (the ledger's) includes RE-RUNNING one stale-injection case and
confirming the outcome — §10.

## 9. What this verification would and would not establish

**A completed verification at one site would establish ONLY:** that on
the recorded dates, on the frozen versions, the named sources' time
models carried the §10.3 distinctions, their quality states mapped to
exactly one canonical state with source metadata retained, the measured
clock offset was inside the CONFIGURED tolerance the command expiry
assumes (criterion (b)), freshness met the registered per-use-case
limits, and the stale/missing injections produced hold/reject with no
silent substitution — for THAT site, THAT source inventory, THAT
observation window.

**It would NOT establish:**

1. **Anything about other sites or future behaviour.** Clocks drift
   over time and sources change; the verification is a point-in-time
   record with recorded re-verification triggers (a source change, a
   D6 change, or a configured-tolerance change re-triggers §4). One
   site's offsets say nothing about another site's.
2. **That the DEFAULTS are correct for the site.** The module's pilot
   DEFAULTS (100 ms / 30 000 ms) are decision-record defaults; this
   verification MEASURES against the CONFIGURED values and records
   whether the configuration is operationally acceptable — it never
   retro-justifies the defaults.
3. **That telemetry is trustworthy under conditions not injected** (the
   X-4 discipline: a finite injection set is not a guarantee).
4. **Any safety, certification, or protection-grade claim** (AGENTS.md
   §3 rules 2–3): quality-flagging and clock hygiene are supervisory-
   layer data-integrity evidence, nothing more.
5. **That a degraded telemetry period was absent** — if the window
   contains `Degraded`/`Unsynchronized` periods with command holds,
   they are recorded AS THE OUTCOME (correct fail-closed behaviour with
   an operational cost), never averaged away.

## 10. Reviewer-verification procedure (the ledger's verification method)

1. Confirm §1's plain statement is unchanged: no measurement, no site,
   no source inventory, no figure exists; every result cell is
   `(EMPTY — UNRUN)`.
2. Confirm the §4.1 table cites the module faithfully: the closed
   four-state vocabulary, hold-on-drift on every non-`Synchronized`
   state INCLUDING `Unknown`, the median-of-window estimate, the
   corrupt-pair cap, and that the pilot DEFAULTS are labelled DEFAULTS,
   not measurements (the ledger's named risk: a source clock drifting
   beyond the expiry bound must surface as a FAILING CS-5, never as a
   tolerance re-fit).
3. For a filled future record: **re-inspect the §3 mapping table
   against the adapter code/config** (the ledger's verification
   method): each mapping row must trace to a frozen ACL-table row and
   to an observed site value; a mapping that collapses `SUSPECT` into
   `GOOD` is a named finding, not a table edit.
4. **Re-run ONE stale-injection case** (the ledger's verification
   method) and confirm the outcome matches §8's expected behaviour;
   record the re-run as a new row, never an overwrite.
5. Confirm the P9.39 interaction (§5) is honest: no load-side figure is
   quoted from anywhere but the P9.39 record; the load-side row is
   PENDING-P9.39 where that run has not happened.
6. Confirm §6's sync-source status is UNSIGNED where no D6 record is
   signed, and CS-1 fails on a source absent from the signed D6 record.
7. Confirm §7's X-8 rule: registered limits predate the measurements
   they judge (timestamps in the amendment).
8. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the template is written; nothing has been verified.

## 11. Filling commands (for the human executor)

```sh
# (a) Documented-tools gate (before and after any edit):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Fill order: signed P9.01 scope (source inventory + use cases) →
#     signed D6 record naming the sync source → P9.17 shadow operation
#     running (the ledger's dependency: this verification consumes
#     shadow data) → register §7's freshness limits as a DATED
#     AMENDMENT → execute §2/§3/§4/§8 procedures, filling result cells
#     from ACTUAL observations the same day → §4.3 measurement record
#     signed.

# (c) The P9.39 cross-reference: after the P9.39 load run exists, copy
#     its M-5 row reference into §5's interaction table (a reference,
#     never a re-measurement) and complete the CS-5 load-side verdict.

# (d) Reviewer re-run (the ledger's method): re-run one SM case; append
#     the re-run row with its own evidence reference.

# (e) Honesty grep — the DEFAULTS-not-measurements labels must survive:
grep -n "DEFAULTS, NOT measurements\|EMPTY — UNRUN\|not measured" ledger/phase-09-pilot-qualification/18_telemetry_clock_verification.md | head -30
```

No measurement was taken in this writing loop (banned; ENTRY_NOTE
boundary 6); every result cell stays EMPTY until people at a real site
fill it.

## 12. Open risks carried to P9.27 (this subtask's contribution)

| # | Risk | Why it is open |
|---|---|---|
| R-1 | **No site, no source inventory, no shadow data** — the whole record is unfillable until P9.01 is signed, D6 is closed with the sync source named, and P9.17 shadow operation produces data | §1, §4.2 CS-1, §6 |
| R-2 | **A source clock drifting beyond the expiry bound** (the ledger's named risk): makes "fresh" commands stale and the expiry bound operationally wrong; CS-5 is the control, but the configured tolerance is a decision still unsigned | §4.2 CS-5; §6 |
| R-3 | **`ESTIMATED` values treated as measured in a schedule** (the ledger's named risk): QM-5/SM-4 are the checks; the schedule path's acceptance set is declared per use case before the run, which cannot happen until a scope exists | §3 QM-5, §7, §8 SM-4 |
| R-4 | **A quality mapping that collapses `SUSPECT` into `GOOD`** (the ledger's named risk): the §3 table + reviewer re-inspection is the control; the collapse, if observed, is a FINDING against the deployment, never a table edit | §3 QM-4, §10 check 3 |
| R-5 | **The DEFAULTS over-read as verified values**: 100 ms / 30 000 ms are decision-record defaults; the site's configured values and measured offsets are unknown until §4.3 is filled | §4.1, §9 item 2 |
| R-6 | **Sync-source procurement/reachability unsigned**: if the site firewall blocks the allowlisted source, the edge holds commands (correct) and the pilot has an operational gap that is a D6 change, not a workaround | §6 |
| R-7 | **The P9.39 load-side clock row may never exist** (the pilot composition may not support the load profile): CS-5 then rests on steady-state evidence only and records PENDING-P9.39 honestly — an incomplete criterion (b), carried open | §5 |
| R-8 | **Freshness limits fitted to observed delays** (X-8 violation pressure): the dated-amendment rule is structural; enforcement is human | §7 |

## 13. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (no dependency edge added) |
| Any §2–§8 procedure | **NOT RUN — banned in this loop; human-gate input; no site exists** |
| Any clock offset, drift, or freshness figure | **NONE EXISTS — the module constants cited are DEFAULTS, not measurements; no measurement was taken or fabricated** |
| Sync-source procurement / D6 source listing | **UNSIGNED — nothing procured, no D6 record signed** |
| The ledger tick wording | "implemented, awaiting human build/test" — the template is written; nothing verified |

Status: **WRITTEN-UNRUN** — telemetry-quality and clock-sync
verification is measurement at a real site against real sources; it
has not happened and cannot be executed by an agent. Every result cell
is EMPTY and stays EMPTY until filled from actual observations.
