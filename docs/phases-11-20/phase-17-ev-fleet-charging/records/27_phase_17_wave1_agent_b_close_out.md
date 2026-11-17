# Phase 17 wave 1 — Agent B close-out (P17.06–P17.11)

| Field | Value |
|---|---|
| Batch | Phase 17 wave 1, Agent B: P17.06 (vehicle-proposal ingestion), P17.07 (MeterValues ingestion + signed-meter-value hooks), P17.08 (dual-source SoC arbitration), P17.09 (departure-time constraints), P17.10 (user-override precedence), P17.11 (depot/site load management) |
| Status | **WRITTEN-UNRUN** (the entire program's hard status boundary): modules and complete test files are written, never compiled, never executed. No charge point is connected, no OCPP message was ever exchanged with a real device, no SoC value was ever observed, no telematics vendor is named as integrated, no vendor terms were accepted, no conformance / certification / metrology / safety claim is made anywhere in this batch. |
| Records | `records/21_p17_06_vehicle_proposal_ingestion.md`, `records/22_p17_07_meter_values_ingestion.md`, `records/23_p17_08_soc_arbitration.md`, `records/24_p17_09_departure_time_constraints.md`, `records/25_p17_10_user_override_precedence.md`, `records/26_p17_11_depot_load_management.md`, this close-out (record 27). |
| Ledger | NOT edited (the ledger is explicitly outside this agent's ownership; no checkbox was ticked by this batch). |

## 1. Files created / modified

| File | Change |
|---|---|
| `crates/flexcore-domain/src/ev_proposal_records.rs` | NEW (P17.06 domain vocabulary) |
| `crates/flexcore-application/src/ev_proposal_ingest.rs` | NEW (P17.06 application assembly) |
| `crates/flexcore-application/src/ev_meter_values_ingest.rs` | NEW (P17.07 mapping + hooks) |
| `apps/worker/src/ev_ingest.rs` | NEW (P17.06/P17.07 worker routing; no parallel ingestion path) |
| `crates/flexcore-domain/src/ev_soc_arbitration.rs` | NEW (P17.08) |
| `crates/flexcore-domain/src/ev_departure_constraints.rs` | NEW (P17.09) |
| `crates/flexcore-domain/src/ev_override_policy.rs` | NEW (P17.10, incl. the negative tests) |
| `crates/flexcore-domain/src/ev_depot_load.rs` | NEW (P17.11) |
| `crates/flexcore-domain/src/lib.rs` | Additive module registration only (5 modules) |
| `crates/flexcore-application/src/lib.rs` | Additive module registration only (2 modules) |
| `apps/worker/src/lib.rs` | Additive module registration only (1 module) |

No crate was added (modules in existing crates — the hard-ban rule); no
dependency was added to any `Cargo.toml`; `docs/foundations/*`,
`docs/plan/*`, `.agent/*`, the ledger, and Agent A's files (the
`crates/adapters/protocol-ocpp/` proposal, the composer, records 01–19)
are untouched. The never-granted set
(`crates/flexcore-kernel/src/manifest.rs`), the frozen 14-state/27-edge
command set (`command_state.rs`), and the frozen six-state quality
vocabulary (`quality.rs`) are byte-identical — this batch only CONSUMES
them.

## 2. The A-dependence reconciliation (recorded honestly)

Agent A's P17.01 layout record and the `crates/adapters/protocol-ocpp/`
crate had NOT landed when this batch was written (checked:
`crates/adapters/` contained no `protocol-ocpp` at batch start and no
`docs/phases-11-20/phase-17-ev-fleet-charging/records/` existed). The
reconciliation used:

1. **Consumption BY CITATION.** The application and worker modules define
   the adapter-neutral shapes (`InboundProposalMessage`,
   `EvInboundFrame`, the `TelemetryOffer` seam over the existing
   `IngestionQueue`) that A's inbound path constructs at its boundary;
   no concrete adapter type is named anywhere in this batch (which also
   keeps the dependency rule trivially green — the application layer must
   not depend on adapter crates in any case).
2. **Records name the dependence** (record 21 §A-dependence, record 22
   §A-dependence): a layout mismatch when A's P17.01 record lands is a
   recorded edit of these shapes, never a silent reinterpretation.
3. **Handoffs to A** are listed in §5 below.

## 3. Verbatim permitted-tool outputs (conventions §1.5, §6.1; AGENTS.md §3 rule 7)

```
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 28 workspace members.
```

(The `ù` in the dependency-rule line is the tool's own verbatim output —
an encoding artifact of the console, recorded as printed. All three
tools exited 0. The member count is unchanged at 28: this batch added no
crate.)

Additional recorded self-checks (not permitted-tool outputs, only
honesty scans): `grep` for `todo!|unimplemented!|TODO|FIXME|unreachable!`
over all eight new module files found no matches (exit 1 = clean).

## 4. Deviations and honest notes

1. **No formal `ADR-010-EXT-02` file.** The task said "ADR-010 extension —
   record it, referencing the ADR-010-EXT-01 pattern". The extension is
   recorded in the domain constant
   `PROPOSALS_NEVER_AUTO_COMMAND_STATEMENT` and in record 21 §2 (the
   EXT-01 record form); `docs/architecture/adr/` is outside this agent's
   wave-1 file ownership, so no ADR-family file was created there. The
   human owner can promote record 21 §2 into an ADR file verbatim if
   wanted.
2. **`P17-E-3` registered OPEN with DEFAULT (a)** in record 23 §2 (as
   instructed); agents never close E-decisions.
3. **The proposed record range 21–39 is used 21–27**: six subtask records
   plus this close-out. Records 28–39 remain free for the wave-2
   numbering (Agent B's wave-2 records: P17.16–P17.18).
4. **Static (non-build) review corrections made during the batch** (the
   workspace has never compiled; these are self-caught honesty fixes,
   recorded so the human build knows what changed):
   - replaced the SOC-target and energy-request error reuse with proper
     named variants (`SocTargetOutOfRange`, `InvalidEnergyRequest`);
   - removed a placeholder-helper draft in the worker module in favor of
     the `TelemetryOffer` trait seam (no `unreachable!` remains);
   - fixed `EventTime::unix_millis()` being `i128` in the depot profile
     grid arithmetic (checked conversion, refusal on out-of-range);
   - fixed the `Provenance` import path in the depot tests (the type is
     not re-exported from `doe_envelope`);
   - fixed the departure-constraints test fixture (veh-02's departure was
     off the 900 s interval grid and uncovered by its window);
   - fixed the SoC-arbitration test fixtures (the `age_ms` helpers placed
     the event in the FUTURE of `now`, making every staleness verdict
     fresh; the events are now `age_ms` before the base instant, and the
     clock-skew test constructs its future event explicitly);
   - named the declared UTC bound in the proposal-timestamp error.
   These are code/record edits — the plan §7 note 5 discipline — not
   silent reinterpretations. THE FIRST HUMAN BUILD REMAINS THE GATE:
   anything this batch wrote may surface defects at build time, exactly
   as the conventions record for the WRITTEN-UNRUN class.

## 5. Handoffs

**To Agent A (wave 1):**

- `apps/worker/src/ev_ingest.rs` defines the adapter-neutral inbound
  shapes (`EvInboundFrame`, `TelemetryOffer` over the existing
  `IngestionQueue`) that the `protocol-ocpp` inbound path should
  construct at its boundary — see `ev_ingest.rs` module header and
  record 21 §A-dependence for the exact contract; a mismatch is a
  recorded edit.
- The proposal records (`EvProposalRecord`) carry the payload fields the
  composer's `TxProfile`/needs-informed composition may consume; the
  composer consumes them BY CITATION only, downstream of the lifecycle
  gates.
- The depot `ExternalConstraintProfile`
  (`flexcore_domain::ev_depot_load`) is the constraint data the composer
  maps onto the `ChargingStationExternalConstraints` purpose (evseId 0).

**To wave 2 (Agent B: P17.16–P17.18):**

- The signed-meter-value seam (`ev_meter_values_ingest.rs`) is the input
  surface for the AFIR/Eichrecht assessment (P17.16) — the seam claims
  nothing; the assessment's scope verdict stays with P17.16.
- `P17-E-3` stays OPEN with DEFAULT (a) (record 23 §2); the E-decision
  records (P17.18) close nothing.
- Records 28–39 are free for the wave-2 numbering; record 27 (this file)
  carries the wave-1 close-out.

**To the human gates:**

- The first human build compiles this batch for the first time; the
  static-review corrections in §4.4 are the recorded risk list.
- The three permitted-tool outputs in §3 are recorded verbatim; no
  build, test run, or CI run was performed by this agent.
