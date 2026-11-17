# DNP3 Outstation Adapter Guide (IEEE 1815.2-2025 DER profile)

| Field | Value |
|---|---|
| Scope | The FlexCore `protocol-dnp3` adapter (`crates/adapters/protocol-dnp3/`): the DNP3 OUTSTATION role — FlexCore presenting its DER fleet to a utility SCADA master |
| Owner record | `docs/phases-11-20/phase-14-protocol-device-connectivity/records/24_p14_08_dnp3_der_profile.md` |
| Implementation status | **WRITTEN-UNRUN.** Data-object vocabulary, outstation model, master-role assessment, secure-transport posture note, and the transport SEAM exist as written code; nothing has compiled, no TCP session or serial link was opened, no master polled this outstation, no control was received (`UnwiredTransport` refuses every report with `SeamNotWired`) |
| Conformance status | **No conformance, certification, compliance, or security claim of any kind.** DNP Users Group testing is a human-gated lab process — see the evidence plan (records 101) |
| Control status | Supervisory relay only. A DNP3 control arriving from the master enters the frozen 14-state command lifecycle at PROPOSED — never an issued setpoint (record 24 §3) |

## 1. What the adapter is (and is not)

| Aspect | Recorded state |
|---|---|
| Role | OUTSTATION (the passive end; common SERVER role) — the MASTER role is DEFERRED and structurally refused (`assert_role_available()` refuses it by name; there is no master-role type to construct) |
| Data objects | The `Dnp3ObjectGroup` vocabulary (BinaryInput, Counter, AnalogInput, AnalogOutputStatus, AnalogOutput, ControlOutput — group numbers 1/20/30/40/41/12, ALL UNVERIFIED) with the reviewed quantity→object mapping for fleet quantities |
| Event classes | Class0 static plus Classes 1–3 events (semantics UNVERIFIED); class-0 points do not queue |
| Point inventory | DECLARED per deployment from the utility's authoritative point list — index conflicts refused; an empty inventory is refused |

## 2. Deployment (what a deployment would configure)

| Configuration | Honesty note |
|---|---|
| `LinkConfig` (Tcp / Serial with declared addresses and parameters) | the well-known DNP3 TCP port is a configuration choice, never a default; link-address semantics UNVERIFIED |
| The point list → point-index allocation | the utility's signed point list is authoritative per deployment; a mutation outside it is refused at the model layer |
| `DeclaredScaling` per point (canonical MW ↔ source units) | declared per profile, never inferred; reactive points REQUIRE a declared reactive-sign convention (the units.rs rule) |
| Integrity-poll / unsolicited-reporting posture | semantics UNVERIFIED — recorded as design, not behavior |

## 3. Security (posture, no claims)

| Topic | Design position |
|---|---|
| Secure transport | The posture note declares `Required` / `TestLabOnly` and carries a MANDATORY non-empty open-questions list: the 1815.2-2025 secure-authentication profile requirement, utility TLS expectations, and key-management mapping to the Phase 13 secrets surface — all UNVERIFIED anchors to resolve before any security-bearing implementation |
| Claims | The posture note structurally carries a mandatory not-claimed list (no security/conformance/certification/compliance claim; no control implemented; no connection exists). A posture note without recorded unknowns is a construction error |
| Secrets | Phase 13 secret REFERENCES only; `secret.read` stays in the never-granted set |

## 4. Operations (budgets, observability, failure modes)

| Topic | Design position |
|---|---|
| Reporting | `OutstationReport` (static points + drained event queue) — the record the transport seam would report; a design record, not observed behavior |
| Control arrival | Controls for non-control points refused; nil command ids refused; the commanded value converts into canonical MW through the point's declared scaling; the select-before-operate arming sequence is an UNVERIFIED implementation-time design point |
| Connection states | The common `ProtocolConnection` vocabulary — vocabulary only: no adapter implements the port in this baseline (the recorded FRV1-R6-001 deferral; the shipped default refuses fail-closed by name), so connection reporting rides the adapter's own seam surface (`SeamNotWired`) until the first adapter composition. The P11 taxonomy consumes the names, never redefines them |
| Failure modes | Index conflicts refused; empty inventory refused; unwired transport → `SeamNotWired`; the master role refused at the scope gate AND by name |

## 5. Honest boundary and certification pointer

No SCADA connection exists, no master polled, no control was received,
no message was exchanged. The DER-profile object model is a design
vocabulary for the platform side — not a claim that any utility SCADA
accepted it. DNP Users Group testing is the human-gated path described
in the evidence plan (records 101). The master-role deferral reopens only
on a named utility SCADA integration (record 24 §1, `master.rs`).
