# Phase 17 — Wave 2, Agent A close-out (P17.12–P17.15): PnC gate, V2G readiness, conformance suite, simulated charge points

| Field | Value |
|---|---|
| Wave / agent | Phase 17 wave 2, Agent A |
| Subtasks | P17.12 (record 41, RECORD-ONLY / PHASE-GATED), P17.13 (record 42), P17.14 (record 43), P17.15 (record 44) |
| Status | **WRITTEN-UNRUN** (P17.12 additionally RECORD-ONLY): all deliverables are additive design-baseline code + written-unrun test suites + records. Nothing compiled, nothing executed, no charge point connected, no OCPP message exchanged with a real device, no PnC/PKI implementation, no CA, no conformance/certification/interoperability/metrology/safety claim anywhere |
| Bans honoured | No installs; no builds; no tests run (execution = human gate); zero `todo!()`/`unimplemented!()`/TODO/FIXME placeholders in any file written (grep-verified — the only match in the scan is the pre-existing Phase 2 `unreachable!` exhaustiveness guard in `crates/flexcore-simulator/src/profiles.rs` line 322, untouched code, outside the banned set); permitted verification tools only, outputs quoted verbatim below; no edits to `ledger_phase_17.md`, `plan.md`, `context.md`, foundations, `docs/plan/*`, `.agent/*` — every ledger checkbox remains unticked by this agent |

## 1. Files created / modified

| File | Change |
|---|---|
| `crates/flexcore-domain/src/ev_v2g_readiness.rs` | NEW (P17.13): per-EVSE/vehicle bidirectional declarations + derived readiness verdicts CONSUMING the frozen five-condition `V2xConfig` gate; the OCPP 2.1 hooks (`NotifyAllowedEnergyTransfer`, `AFRRSignal`, `SetDERControl`) as DESIGN records with UNVERIFIED labels; the NA-AC path statement ([S-4][S-131][S-144]; SAE J3072 labelled UNVERIFIED/ASSUMPTION); ADR-010 `READINESS_NEVER_AUTHORIZES_DISPATCH`; deterministic `BTreeMap` readiness ledger; in-module tests |
| `crates/flexcore-domain/tests/ev_v2g_readiness.rs` | NEW (written-unrun): the 2×2 readiness matrix, gate-consumed-not-redefined, fail-closed constructions, ledger determinism, hook/NA-AC statement pins |
| `crates/flexcore-domain/src/lib.rs` | Additive `pub mod ev_v2g_readiness;` entry only (shared file; re-read immediately before the edit; no existing line changed) |
| `crates/adapters/protocol-ocpp/src/conformance.rs` | NEW (P17.14): the declared 23-row OCA use-case matrix SUBSET (provisioning, transactions, smart charging, security events, firmware status, negative) + the shared negative vectors (the `tests/framing_codec.rs` negatives RESTATED with citation; subprotocol-mismatch rows through the real `negotiate`; certificate-failure rows) + the honest-status statements; in-module tests |
| `crates/adapters/protocol-ocpp/src/lib.rs` | Additive `pub mod conformance;` entry only (shared file; re-read immediately before the edit) |
| `crates/adapters/protocol-ocpp/tests/conformance_suite.rs` | NEW (written-unrun): every matrix row pinned against the real wave-1 surfaces; the declared-matrix↔covered-set identity pinned; zero conformance claims |
| `crates/flexcore-simulator/src/adapters/sim_ocpp.rs` | NEW (P17.15): `SimulatedChargePoint`, the deterministic in-process OCPP-J loop (`SimulatedOcppLoop`, default depot script), `SimulatedCsmsTransport` implementing the adapter's `CsmsTransport` seam, the relay fixture through the REAL composer + ISSUED-only gate (labelled HARNESS FIXTURE), the SYNTHETIC `SimulatedV2gProfile` gated by `V2gBehaviourLink::attach` (disabled five-condition gate refused), in-module tests |
| `crates/flexcore-simulator/src/adapters/mod.rs` | Additive `pub mod sim_ocpp;` + the table row (this agent's file) |
| `crates/flexcore-simulator/src/profiles.rs` | Additive linkage COMMENT only — the EVSE profile's charging-only code and its "no V2G" statement are untouched |
| `crates/flexcore-simulator/Cargo.toml` | Additive `flexcore-ocpp-adapter = { workspace = true }` with the composition-legality note (shared file; re-read immediately before the edit; the workspace entry existed since wave 1 — no new EXTERNAL dependency, no checker change needed: composition→composition edge, the P14.17 precedent) |
| `crates/flexcore-simulator/tests/p17_15_simulated_ocpp_charge_points.rs` | NEW (written-unrun): full depot script through the real surfaces, the seam contract BOTH ways (unwired refuses / simulated completes), relay step with stored-not-applied verbatim, 1.6 meets the real P17-E-1 negotiation, V2G gating, harness-fixture contrast (PROPOSED refused by the real relay gate), fail-fast constructions |
| `records/41_p17_12_pnc_certificate_management_design.md` | P17.12 record (record-only; the §3 five-condition ungate checklist; §4 UNVERIFIED register) |
| `records/42_p17_13_v2g_readiness_modeling.md` | P17.13 record |
| `records/43_p17_14_ocpp_conformance_suite.md` | P17.14 record |
| `records/44_p17_15_simulated_charge_points.md` | P17.15 record |
| this file (59) | The wave-2 Agent A close-out |

**Untouched, verified:** `crates/flexcore-domain/src/command_state.rs`, `crates/flexcore-kernel/src/manifest.rs` (the never-granted set), `crates/flexcore-contracts/src/quality.rs` (the frozen six-state vocabulary), `crates/flexcore-domain/src/ev_fleet_model.rs` (the five-condition gate — byte-identical), `docs/foundations/*`, `docs/plan/*`, `.agent/*`, `ledger_phase_17.md`, `plan.md`, `context.md` (git diff over all of these is EMPTY; the working tree's other modifications belong to other waves). No checkbox was ticked anywhere.

## 2. Verbatim permitted-tool outputs (final state, all exit 0)

```text
$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 29 workspace members.
EXIT=0

$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0
```

(The `ù` in the checker line is the console rendering of the source's
em-dash, the same artifact the wave-1 records note; 29 workspace members
— unchanged from wave 1: this batch added no crate.)

## 3. Honesty compliance (brief-by-brief)

1. **PnC implementation is GATED** — record 41 is record-only; no code
   was written for P17.12; the record states exactly what would ungate it
   (§3: named PKI operator, anchor distribution, OCSP service, OCA-text
   pass, D5 closed) and none is started. The adapter's action vocabulary
   was NOT widened (Get15118EVCertificate / GetCertificateChainStatus are
   design names in the record, never `framing::Action` variants).
2. **The simulated V2G profile is SYNTHETIC and labelled** —
   `SIMULATED_V2G_IS_SYNTHETIC_STATEMENT` travels in the module and is
   pinned verbatim by tests; the profile attaches ONLY through the frozen
   five-condition gate (disabled gate = named refusal, tested).
3. **No conformance claim** — the suite (record 43) is written-unrun; the
   module statements (`WRITTEN_UNRUN_STATEMENT`,
   `MATRIX_SCOPE_STATEMENT`) say so and are pinned by tests; the use-case
   ids are this program's subset keys, never OCA case ids.
4. **Every external claim cites [S-n] or is marked UNVERIFIED/ASSUMPTION**
   — including the honest SAE J3072 label (named in the phase plan, no §11
   source entry) and the Edition 2 date-conflict pointer for the OCA
   certification-profile mapping.
5. **The command-relay-only rule holds** — the relay is exercised only
   through the real ISSUED-only gate; the harness-fixture Issued state is
   labelled and its contrast (PROPOSED refused) is tested; the frozen
   14-state/27-edge set and the never-granted set are byte-identical.

## 4. Deviations and honest notes

1. **No deviations from the brief's bans or scope.** The record range is
   used as 41–44 (one record per subtask) + this close-out at 59; records
   45–58 remain free.
2. **WRITTEN-UNRUN defects are possible** (the workspace has never
   compiled). Static self-review caught and fixed during the batch: a
   broken doc-comment continuation and three `Eq` derives on f64-bearing
   types in `sim_ocpp.rs` (f64 is not `Eq`); a nonexistent
   `payload_json` accessor replaced with a real frame match; format
   placeholders and an unused-import hack removed; a wrong
   `expected_negotiation_refusal` mapping (the "ocpp2.0" row now expects
   the aggregate refusal message the real `negotiate` actually produces)
   and a `suite_server` signature fixed in `conformance.rs`. The first
   human build remains the gate and may surface more.
3. **The framing-codec reuse is restatement with citation**, not import —
   integration-test files cannot import each other; the divergence rule
   is recorded (record 43 §3).
4. **The firmware-status family is covered AS the refusal** (UC-FW-01/02):
   the closed vocabulary is unchanged and no action was added for the
   suite's sake; the wire-level NotImplemented translation is recorded as
   an UNVERIFIED transport-seam check, never claimed.
5. **`unreachable!` in `profiles.rs` line 322** is pre-existing Phase 2
   code (the profile-dispatch exhaustiveness guard), found by the honesty
   grep and deliberately NOT touched (minimal-edit rule; outside the
   banned macro set).

## 5. Handoffs

**To wave-2 Agent B (P17.16–P17.18):**

- Record 41 §3 is the PnC gate inventory — the E-decision/close records
  must not imply the PnC gate is closable before D5 and the PKI conditions.
- The OCPP 2.1 hooks (record 42 §4) are scope citations, not deliverables
  — the AFIR/OCPI records should describe them as design records.
- The conformance matrix (record 43) is evidence preparation, never
  conformance progress. Records 45–58 are free; 60+ is your range
  (61–79 per the brief).

**To the orchestrator / human gates:**

- First workspace build (this batch's static-review list is §4.2), then
  test execution (domain V2G suite, protocol-ocpp conformance suite,
  simulator loopback suite), then the OCA-text verification pass over the
  wave-1 + wave-2 UNVERIFIED registers.
- P17-E-1 (via D5, human-closed) and P17-E-2 stay OPEN; this agent closed
  no E-decision and ticked no checkbox.
- The simulated charge points fulfill the P14.17 OCPP promise as an
  IN-PROCESS loopback only: the REAL WebSocket/TLS transport remains the
  implementation-time human gate of record 01 §7.
