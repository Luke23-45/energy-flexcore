# Phase 14 — Wave 1, Agent B close-out (P14.05–P14.08)

| Field | Value |
|---|---|
| Batch | Phase 14 wave 1, Agent B: P14.05 (OpenADR 3.0 VTN), P14.06 (2.0b assessment), P14.07 (SunSpec Modbus client), P14.08 (DNP3 outstation) |
| Status | WRITTEN-UNRUN. No build was run, no test was executed, no protocol was connected, no device was reached, no message was exchanged. The ledger was NOT edited (no checkbox ticked — that is the batch owner's human-gated step, and this agent's instructions forbid it) |
| Decision records | `21_p14_05_openadr_vtn.md`, `22_p14_06_openadr_2_0b_assessment.md` (registers **P14-E-2 OPEN** with DEFAULT "3.0-first, 2.0b on program demand"), `23_p14_07_sunspec_modbus_client.md`, `24_p14_08_dnp3_der_profile.md`, this close-out |

## 1. Files created (all NEW unless noted)

| Path | Content |
|---|---|
| `crates/adapters/protocol-openadr/` | `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/scope.rs`, `src/endpoints.rs`, `src/vtn.rs`, `src/event_mapping.rs`, `tests/event_mapping.rs` — record 21 |
| `crates/adapters/protocol-modbus/` | `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/registry.rs`, `src/units_map.rs`, `src/polling.rs`, `src/transport.rs`, `src/scope.rs`, `tests/register_conversion.rs` — record 23 |
| `crates/adapters/protocol-dnp3/` | `Cargo.toml`, `src/lib.rs`, `src/model.rs`, `src/outstation.rs`, `src/master.rs`, `src/secure_transport.rs`, `src/transport.rs`, `src/scope.rs` — record 24 |
| `docs/phases-11-20/phase-14-protocol-device-connectivity/records/21…24…md` | The four records above (the `records/` directory already existed — Agent A created it with records 01–03 during this batch) |

## 2. Shared-file diffs (described exactly; both files were re-Read immediately before each edit, and Agent A's entries were landed first)

1. **Root `Cargo.toml`** — additive only:
   - Three workspace MEMBER entries appended immediately after Agent A's
     `crates/adapters/protocol-20305` entry, each with a comment block:
     `crates/adapters/protocol-openadr`, `crates/adapters/protocol-modbus`,
     `crates/adapters/protocol-dnp3`.
   - Three `[workspace.dependencies]` entries appended immediately after
     Agent A's `flexcore-protocol-20305` line, under a new comment header:
     `flexcore-openadr-adapter`, `flexcore-modbus-adapter`,
     `flexcore-dnp3-adapter` (path forms).
   - No existing line was modified; Agent A's P14.01/P14.02 comments and
     entries are untouched.
2. **`tools/check_dependency_rule.py`** — additive only:
   - `COMPOSITION_CRATES`: `flexcore-openadr-adapter`,
     `flexcore-modbus-adapter`, `flexcore-dnp3-adapter` appended after
     Agent A's `flexcore-protocol-20305` entry, each with a comment naming
     the decision record and the designed-seam/no-transport rule.
   - `MEMBER_DIRS`: the three `crates/adapters/protocol-*` paths appended
     after Agent A's two protocol entries, under a comment block.
   - No existing entry was modified; the CORE/COMPOSITION classifications
     of every prior crate are byte-identical.

## 3. The Agent-A dependence (recorded, reconciled)

At this agent's wait-check, `crates/flexcore-protocol-ports` did NOT exist
(no `protocol-*` crate, no `records/` directory). Agent B therefore drafted
the three adapters against crate-local scope/manifest modules that mirrored
`crates/flexcore-grid-api/src/scope.rs` directly, per the contingency in the
batch instructions. Agent A landed the ports crate + records 01–03 mid-wave;
B then reconciled in the same batch:

- All three crates now DEPEND on `flexcore-protocol-ports` (workspace pin)
  and consume the common `ProtocolScopeDeclaration` (scope gate via
  `assert_supports`, mirroring A's `protocol-20305/src/proposal.rs`
  pattern) and the common `ProtocolAdapterManifest` (the one manifest
  shape, structurally `SupervisoryRelayOnly`).
- The crate-local mirror vocabulary (OpenADR `OpenAdrRole`/
  `OpenAdrInteraction`/local `NotModelled` set, Modbus
  `ModbusScopeDeclaration`/`OperationClass`, DNP3 `Dnp3Role`/local scope)
  and the three local capability-manifest modules were REMOVED — one
  reviewed scope and manifest vocabulary, no parallel one. Protocol-specific
  elaboration the common vocabulary cannot express (OpenADR delivery modes,
  SunSpec model families, DNP3 point kinds, the DNP3 master-role refusal)
  is kept and recorded in the scope notes / dedicated modules.
- Role mappings used: OpenADR VTN = common SERVER; Modbus client = common
  CLIENT; DNP3 outstation = common SERVER (the master = common CLIENT role
  is NOT declared and is refused at the gate and by name in `master.rs`).
- Per-crate dependence statements: record 21 §2, record 23 §2, record 24 §2.

## 4. Permitted-tool outputs (verbatim)

Run from the repo root; exit codes shown after each block. The dependency
checker's em dash reaches this console as byte 0x97 (a console-codepage
artifact); the actual output text follows the script's source.

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0

$ python -I tools/check_dependency_rule.py
OK — dependency rule holds for 25 workspace members.
EXIT=0
```

(The checker's count of 25 is the number of workspace members it verified
from the post-batch `MEMBER_DIRS` — 21 crate paths and 4 app paths,
including Agent A's `flexcore-protocol-ports` and `protocol-20305` entries
and this agent's three adapter crates. Every one is declared in the
checker's CORE_CRATES or COMPOSITION_CRATES sets.)

## 5. Honesty statements

1. No protocol was connected, no device was reached, no message was
   exchanged — the transports are designed, UNWIRED seams with named
   `SeamNotWired` refusals; no HTTP framework, Modbus stack, or DNP3 stack
   dependency exists in any of the three crates.
2. Every external protocol detail cited from a secondary source carries an
   UNVERIFIED label naming the implementation-time check (records 21 §5,
   23 §4, 24 §4; the code module docs carry the same labels at each point).
3. External events enter the command lifecycle at PROPOSED — enforced
   structurally in both mappings (const `entry_state()`, the frozen
   machine's own rejection of gate-skipping shortcuts, and the common
   manifest's `SupervisoryRelayOnly` authority) and stated in records
   21 §3 and 24 §3.
4. The never-granted set in `crates/flexcore-kernel/src/manifest.rs` was
   not touched; the common manifest vocabulary (record 01) makes those
   permissions inexpressible, and no crate-local permission list remains
   to drift.
5. The deliverable class is design + model types + written-unrun tests;
   nothing claims a complete protocol stack (plan §5 risk 1).
6. The two required mapping test files were written COMPLETE and UNRUN:
   `crates/adapters/protocol-openadr/tests/event_mapping.rs` and
   `crates/adapters/protocol-modbus/tests/register_conversion.rs`
   (execution is a human gate).

## 6. Deviations

1. **Mid-wave reconciliation onto `flexcore-protocol-ports`** (§3): the
   initial wait-check found A absent; A landed mid-batch; B migrated all
   three crates onto the common layer and deleted the crate-local
   scope/manifest modules. Recorded here and in records 21/23/24 §2.
2. **No other deviation.** No ledger edit, no checkbox tick, no
   foundation/plan/.agent edit, no new external crate (dependencies are
   exactly the workspace pins: serde, serde_json where needed,
   flexcore-contracts, flexcore-domain, flexcore-protocol-ports), no
   TODO/FIXME/todo!/unimplemented! anywhere in the three crates.

## 7. Handoff notes

**To Agent A:** the three adapter crates consume `flexcore-protocol-ports`
as landed (records 01–03 read; no API change requested). If the common
scope/manifest API changes, the three construction sites to re-point are
`scope.rs` in each crate (`vtn_scope`/`vtn_manifest`, `client_scope`/
`client_manifest`, `outstation_scope`/`outstation_manifest`) plus the two
mapping modules' scope gates.

**To wave 2 (P14.13–P14.16):** the common `ProtocolAdapterManifest` is the
kernel-profile wiring point for P14.16; the never-granted set was not
touched and needs no review action from this batch. P14.14's settings
workflow should consume the PROPOSED-entry pattern exactly as records
21 §3 / 24 §3 state it (external events → PROPOSED, three gates ahead).

**To wave 3 (P14.17–P14.21):** the simulated counterparts land on named
seams: a simulated VEN exercises `endpoints::VtnEndpointDesign` +
`vtn::VtnRegistry`/`DeliveryPlanner`; a simulated Modbus device implements
`transport::ModbusTransport` against the register-map registry; a simulated
DNP3 master implements `transport::Dnp3Transport` against
`outstation::OutstationModel`. Polling-budget observability (P14.20) binds
to `polling::GatewayPollingBudget` (Modbus) and the common manifest cadence
(all three).

**To the human gates:** first workspace build (program boundary 1); the
mapping-test execution; the implementation-time UNVERIFIED checks against
the OpenADR 3.0 text [S-10], the SunSpec Modbus specifications [S-16], and
the IEEE 1815.2-2025 text [S-14]; the P14-E-2 closure decision.
