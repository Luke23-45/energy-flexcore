# P18.08–P18.13 (wave 1, Agent B) — Close-out record: identity/registry/attestation batch

| Field | Value |
|---|---|
| Subtasks | P18.08, P18.09, P18.10, P18.11, P18.12, P18.13 (phase-18 plan §2, Agent B scope) |
| Wave / agent | Phase 18 wave 1, Agent B |
| Status | **IMPLEMENTED, AWAITING HUMAN BUILD/TEST** — every deliverable below is WRITTEN-UNRUN; execution of code, migrations, and tests is a human gate. Permitted verification tools run and quoted verbatim in §5 |
| Decision records | `21_p18_08_identity_lifecycle.md`, `22_p18_09_zero_touch_onboarding.md`, `23_p18_10_decommissioning_runbook.md`, `24_p18_11_esim_lifecycle.md`, `25_p18_12_fleet_registry_hardening.md`, `26_p18_13_attestation_design.md` |
| E-decisions | **P18-E-2 registered OPEN with DEFAULT (a)** (hardware-optional; assumption set PENDING PILOT HARDWARE SPEC) in record 26 — implemented as the DEFAULT only, never closed |

## 1. Files created/modified

| File | Change |
|---|---|
| `crates/adapters/postgres/migrations/0023_edge_fleet_registry_p18_12.up.sql` | NEW — expand-only companion state journal `edge_fleet_device_state` (see record 25; landed FIRST per the blocking order — Agent A's P18.06 consumes cohort membership) |
| `crates/adapters/postgres/migrations/0023_edge_fleet_registry_p18_12.down.sql` | NEW — expand-only reversal (drop the new table whole) |
| `crates/edge-agent/src/fleet_registry.rs` | NEW — P18.12 schema types: closed `FirmwareState`/`CohortMembership`/`ObservedHealth` vocabularies, validated journal-row shape, BTreeMap `FleetStateView` projection + complete in-module tests |
| `crates/edge-agent/src/identity_lifecycle.rs` | NEW — P18.08: `DeviceIdentityClass`/`IdentityTermMap` (IDevID/LDevID → P9.33 ceremony), `CredentialRotationSchedule` (injected-clock assessment), `RevocationPropagationPlan` + `KERNEL_REVOCATION_INTERPLAY_STATEMENT` + complete in-module tests |
| `crates/edge-agent/src/zero_touch.rs` | NEW — P18.09: `OwnershipVoucherRegister` (append-only events, closed transition machine), `RendezvousSeam` port with fail-closed `NoRendezvousConfigured` shipped default, `ZeroTouchClaimFlow` closed state machine (handoff issues nothing; the P9.33 approval gate runs unchanged) + complete in-module tests |
| `crates/edge-agent/src/esim_lifecycle.rs` | NEW — P18.11: closed coarse `EsimeProfileState` vocabulary, carrier-AGNOSTIC `EsimeObservation`, `ExpectedCellularReachability` + complete in-module tests |
| `crates/edge-agent/src/attestation.rs` | NEW — P18.13: `AttestationEvidence`/`MeasurementRecord` shapes, `AttestationVerificationPolicy`, `check_before_update_authorization` (ordered, named, fail-closed refusals), `FleetAttestationState` journal vocabulary, `HARDWARE_POSTURE_STATEMENT` + complete in-module tests |
| `crates/edge-agent/src/lib.rs` | MODIFIED — additive module declarations for the five new modules only (re-read immediately before the edit; Agent A's concurrent files untouched) |

Untouched by design: `crates/edge-agent/src/enrollment_ceremony.rs`,
`enrollment.rs`, `update_verification.rs`, `command_path.rs`, `ports.rs`
(the closed `EdgeAuditKind` vocabulary was consumed, not extended),
`crates/flexcore-kernel/src/revocation.rs` (byte-identical), the ledger,
plan.md, context.md, docs/foundations/*, docs/plan/*, .agent/*, and every
Agent A file (update path, orchestrator, records 01–19). No checkbox was
ticked anywhere; no new crate was created (the checker's 29 workspace
members are unchanged); no new dependency was added; the never-granted
permission set is byte-identical.

## 2. Deliverable depths (per plan §1)

| Subtask | Depth | Deliverable |
|---|---|---|
| P18.08 | Design-and-spec + types | `identity_lifecycle.rs` + record 21 |
| P18.09 | Design-and-spec + types | `zero_touch.rs` + record 22 |
| P18.10 | Record-only (runbook) | record 23 (consumes P18.08/P18.09/P18.13 types; no new code needed — the honest deliverable) |
| P18.11 | Design-and-spec (data model) | `esim_lifecycle.rs` + record 24 |
| P18.12 | Migration spec + schema types | migration 0023 up/down + `fleet_registry.rs` + record 25 |
| P18.13 | Policy record + evidence types | `attestation.rs` + record 26 (P18-E-2 registered OPEN, DEFAULT (a)) |

## 3. Honest status statements (per phase-18 constraint 6 and plan R-P18-6)

- **No fleet exists.** No device is enrolled, claimed, onboarded,
  decommissioned, attested, or tracked; no identity term is active; no
  voucher exists; no cohort is defined. Every flow is design against
  in-memory test doubles, and the tests themselves are UNRUN.
- **No migration runs.** Migration 0023 has never been executed against
  any database; the journal has never held a row.
- **No certificate was issued or revoked**; no CA was stood up (P14.03
  §6.1 stands); no PKI operation was performed or claimed.
- **No TPM/secure element is claimed, measured, or assumed** — every
  hardware determination is PENDING PILOT HARDWARE SPEC (P6.34 posture;
  P18-E-2). Software-only evidence is honestly labeled as consistency
  checking that cannot detect a compromised reporter.
- **UNVERIFIED labeling** — every FDO/IDevID/eSIM/62443 secondary-sourced
  detail is marked UNVERIFIED in its record ([S-165][S-166][S-167]
  [S-163][S-164] are survey/overview/webstore sources; no standard text is
  held by the program).
- **Compliance mapping, never a certification claim** — each record maps
  its deliverable to IEC 62443-4-1 SAR 3.4 / 4-2 CR 3.4 + CR 2.1 as cited
  by the phase plan, with the clause-content interpretation marked
  UNVERIFIED.

## 4. Handoffs

**To Agent A (P18.06 orchestrator — consumed, never redefined):**
- Cohort membership: `FleetStateView::current_cohort(device_id)` over
  `crates/edge-agent/src/fleet_registry.rs`; SQL read pattern in the
  migration 0023 header. Firmware-state eligibility:
  `current_firmware_state` (`None`/`UNKNOWN` is never an eligible target).
- Attestation gate:
  `crate::attestation::check_before_update_authorization(...)` — one gate
  among A's own (cohort state, command-lifecycle blocking); it never
  substitutes P9.34 manifest verification.
- Revocation propagation target 4:
  `AttestationDecision::RefuseRevokedDevice` for revoked devices.

**To wave 2 (P18.14 tests, P18.15 interlock, P18.16 close):**
- The named refusal vocabularies (`ZERO_TOUCH_*`, `ATTESTATION_*`,
  `FLEET_STATE_*`, `IDENTITY_*`) are the negative-test surface.
- P18-E-2 (record 26) stays OPEN for P18.16's decision compilation; the
  assumption set is PENDING PILOT HARDWARE SPEC.
- The eSIM vocabulary mapping against SGP.22/SGP.32 is a named human-gate
  check (record 24 §2) for any real cellular fleet.
- The in-module test files are complete and UNRUN — execution is the human
  gate (loop ban 3).

## 5. Permitted verification tools — verbatim outputs

Run 2026-11-17, after the final file edits, from the repository root:

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
OK ù dependency rule holds for 29 workspace members.
EXIT=0
```

(The `ù` in the third line is the console codepage rendering of the tool's
em-dash separator; the output is quoted as observed, not corrected. All
three tools exited 0.)

## 6. Deviations

None from the subtask brief. Notes: (1) P18.10 needed no new code — its
runbook composes the P18.08/P18.09/P18.13 types, which is recorded as the
honest deliverable in record 23; (2) P18.12's "registry extension" is
delivered as a companion append-only state journal rather than columns on
the 0018 registry, because the registry's append-only trigger and
INSERT-only grants make high-frequency state impossible there — the
rationale is recorded in the migration header and record 25 §1; (3) the
close-out record carries no claim of any run, build, or test pass.
