# Record 04 — P18.04: Device update flow specification (download → verify → install → status)

Date: 2026-11-17. Subtask: P18.04 (`ledger_phase_18.md` §P18.04). Status:
**FLOW SPEC + STATE-MACHINE TYPES, WRITTEN-UNRUN** — the update-progress
state machine and status-notification types are real additive code
(`crates/edge-agent/src/update_session.rs`); **no device update occurs,
no device exists, no fleet exists.**

| Field | Value |
|---|---|
| Sources | [S-161][S-162] (device-flow concepts); [S-121][S-122] (OCPP 2.0.1 L01 firmware-update flow as a CHECKLIST — see §4); `UPDATE_SLOT_DECISION` (`crates/edge-agent/src/update_verification.rs`); P6.34 posture |
| Code | `UpdateSessionStateMachine`, `UpdateProgressState`, `UpdateStatusNotification`, the closed failure-state vocabulary |

## 1. The flow (the recorded state machine)

| Transition | Guard | Refusal on guard failure |
|---|---|---|
| `Idle → Downloading` | **BLOCKING RULE**: zero commands ISSUED/TRACKING on the device (`begin_download(issued_or_tracking, …)`) — the phase-plan P18.15 rule consumed by citation; P18.15 formalizes the interlock in wave 2 | `UPDATE_BLOCKED_COMMANDS_IN_FLIGHT` (the drain-before-update discipline, report §5.6) |
| `Downloading → Verifying` | bundle bytes complete (nothing trusted yet) | — |
| `Downloading → UpdateFailed` | download failed | `UPDATE_FAILURE_DOWNLOAD_FAILED` |
| `Verifying → VerifiedPendingInstall` | the FULL verification pass (the P9.34 gate + the extended v2 checks; the anti-rollback counter advanced durably) | — |
| `Verifying → UpdateFailed` | verification refused | `UPDATE_FAILURE_VERIFICATION_REFUSED` (the stable refusal code rides the audit line and the P18.07 evidence record) |
| `VerifiedPendingInstall → Installing` | the install path writes the INACTIVE slot per `UPDATE_SLOT_DECISION` (the register is the source of truth; two-slot A/B is the RECOMMENDED shape) | — |
| `Installing → HealthCheckPending` | slot write completed | — |
| `Installing → UpdateFailed` | install failed | `UPDATE_FAILURE_INSTALL_FAILED` |
| `HealthCheckPending → Active` | health evidence confirmed (terminal) | — |
| `HealthCheckPending → RollingBack` | health evidence failed — **AUTO-ROLLBACK** (a P9.34 rollback: re-activation of a previously-activated version, never a silent downgrade) | — |
| `RollingBack → RolledBack` | rollback completed (terminal) | — |
| `RollingBack → UpdateFailed` | rollback failed | `UPDATE_FAILURE_ROLLBACK_FAILED` (operator intervention; recorded, never silent) |
| `Downloading / VerifiedPendingInstall → UpdateFailed` | explicit cancel | `UPDATE_FAILURE_CANCELLED` (never legal from `Installing` or later) |

Every illegal transition is a named refusal (`UPDATE_TRANSITION_ILLEGAL`);
terminal states are terminal. The state machine is pure bookkeeping: it
holds no bundle bytes, no manifest, no paths, no filesystem access — the
durable evidence is the P9.34 append-only register (source of truth) and
the P18.07 audit records.

## 2. Health checks and anti-rollback counters (device-side contracts)

- The health check is DEVICE-side evidence the platform SPECIFIES and
  consumes (watchdog resets, boot-counter regressions, application-level
  liveness) — the platform cannot implement device firmware (plan §4
  non-goals). The `HealthCheckPending` evidence contract: a FAILED check
  names its evidence reference; an INDETERMINATE check is not a pass
  (fail-closed mirrors the attestation rule).
- The anti-rollback counter is the DEVICE-side register
  (`update_bundle.rs` `AntiRollbackCounter`); the boot-counter
  regression check is the P18.13 attestation policy's `RefuseCounterRegression`
  gate (Agent B, record 26 — consumed by citation).

## 3. Status notifications

`UpdateStatusNotification` carries: device id, attempt id (the P18.07
evidence-chain key), the closed `UpdateProgressState` (stable
SCREAMING_SNAKE names), the explicit failure state (when failed), the
target version, and the injected-clock instant. The transport is the
composition's uplink path; nothing sends anything.

## 4. OCPP 2.0.1 L01 firmware-update flow — checklist use, UNVERIFIED register

Per the phase plan, the OCPP 2.0.1 firmware-update flow ([S-121][S-122])
is used as a CHECKLIST for flow completeness — NOT as a conformance
claim, and NOT as a source for field names. The failure-state names in
§1 are FLEXCORE names. The mapping below is the implementation-time
register item:

| # | Item | Status |
|---|---|---|
| U-1 | The enumeration of OCPP 2.0.1 firmware-update status/failure states, and a FlexCore ↔ OCPP mapping table, are to be written AT IMPLEMENTATION from the OCA spec text | **UNVERIFIED** — no OCA text was consulted in this wave; no OCPP status name is claimed anywhere in the code or this record |
| U-2 | Flow-completeness checklist (does the FlexCore flow miss an OCPP-state category, e.g. schedule-pending or authorization-required states?) | **UNVERIFIED** — the same implementation-time check |

## 5. Honest status

The flow is specified; the state-machine types are written-unrun code
with in-module tests (happy path, auto-rollback path, explicit failure
states, blocking rule, cancel legality, out-of-order refusals). No
device, no update, no hardware behavior claimed; every hardware
determination is PENDING PILOT HARDWARE SPEC (P6.34 posture;
`UPDATE_FLOW_HARDWARE_STATEMENT` pins this in code).
