# P16.05 — M&V event reports: event performance vs the program's baseline, same-day feedback [S-43], evidence assembly with §10.4 quality states; every number is a financial record

| Field | Value |
|---|---|
| Subtask | P16.05 (M&V event reports — event performance against the program's baseline, the same-day feedback pattern [S-43], evidence assembly from interval data with the §10.4 quality states; every number is a financial record (constraint 6)) |
| Wave / agent | Phase 16 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the assembler is written, never compiled, never executed. **No M&V report exists, no performance number is claimed, and no settlement number, revenue figure, invoice, or payment exists anywhere** (plan §8 criterion 4). |
| Sources | [S-32] (FEMP M&V options — M&V framing), [S-43] (Uplight same-day M&V — the same-day feedback pattern), [S-59] (RMI VPP metering & telemetry — the financial-record framing); P16.04 baselines (consumed); `flexcore_contracts::quality` (the §10.4 vocabulary — single home, consumed) |
| Files touched | `crates/flexcore-application/src/mv_event_report.rs` (NEW); `crates/flexcore-application/src/lib.rs` (additive module line) |

## 1. What landed

| Item | Content |
|---|---|
| `MvReportClass` | The two structurally distinct classes: `SameDayPreliminary` (may assemble while the event window is open — the same-day feedback pattern [S-43]; exclusions RECORDED, not fatal) and `Finalized` (requires the window CLOSED per the injected clock AND zero exclusions). A preliminary report is never re-labeled finalized — the finalized report is re-assembled from the same deterministic inputs |
| `AcceptedQualitySet` | The program's accepted §10.4 states (never defaulted — P9.11 Q2 stays EMPTY until a program declares); fixed flags over the frozen six-state vocabulary (`flexcore_contracts::quality` — single home) |
| `assemble_mv_report` | Pure assembly against a P16.04 `BaselineProfile`: the P11 audit-chain LINK is REQUIRED FIRST (no link, no report); the measurements must align with the baseline grid (count, ascending order, inside the event window); the measured average power per interval is derived from the delivered energy and the DECLARED duration (MW = MWh / (duration/60) — units are declared data); a quality state outside the accepted set is EXCLUDED and the exclusion is RECORDED (never silently converted — §10.4); a FINALIZED report with exclusions is REFUSED |
| `MvEventReport` | The assembled record: report class, the baseline method name AS THE PROGRAM DECLARED IT (LR-073, carried end to end), baseline source-day keys, the recorded SDA applied-ratio, per-interval performance (baseline MW, measured MW, deviation MW, quality), the recorded exclusions, the quality composition (BTreeMap by canonical name — deterministic), the injected-clock assembly timestamp, and the P11 audit-chain LINK |

## 2. Recorded decisions

1. **Every number is a settlement-adjacent financial record** (`EVERY_NUMBER_IS_FINANCIAL_RECORD_STATEMENT`): the report cannot exist without its P11 audit-chain LINK; quality states travel WITH the numbers; exclusions are recorded, never silent; performance is measured in MW/MWh — money never enters this module.
2. **The same-day feedback pattern [S-43] is structural, not procedural** (`SAME_DAY_FEEDBACK_STATEMENT`): the preliminary class exists precisely so the operator sees the day's performance the same day; the finalized class is the settlement-grade gate.
3. **Persistence (recorded): the interval numbers live in the P16.01 `market_deliveries` rows** (each quality-tagged and chain-linked); the aggregated report is deterministically re-derivable from those persisted inputs plus the program's baseline configuration and the P16.04 engine. No separate M&V-report table exists in migration 0022 — the `MV_REPORT` retention class (P16.01) names the record class for the additive migration that lands a report row (wave-2 scope, recorded — never invented here).
4. **LR-073 end to end**: the report carries the program's declared baseline method name and refuses an empty one — the method stays the program's, from configuration through computation to report.
5. **ADR-010**: an M&V report is a record, never a command; no dispatch authority exists here.

## 3. Honest boundaries

1. No M&V report has been assembled for any real event; every input in the module's scope is potential fixture data; the assembler has never executed.
2. No settlement number, revenue figure, invoice, or payment exists anywhere in this module or its record — performance deltas in MW/MWh are measurements, not money.
3. The §10.4 quality vocabulary is consumed from `flexcore_contracts::quality` — nothing here restates or extends it.
4. The permitted-tool outputs are quoted in the close-out record (07).
