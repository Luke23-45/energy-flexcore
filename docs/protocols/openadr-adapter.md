# OpenADR 3.0 VTN Adapter Guide

| Field | Value |
|---|---|
| Scope | The FlexCore `protocol-openadr` adapter (`crates/adapters/protocol-openadr/`): the OpenADR 3.0 VTN role — event publication, report collection, VEN management |
| Owner record | `docs/phases-11-20/phase-14-protocol-device-connectivity/records/21_p14_05_openadr_vtn.md` (+ record 22 for the 2.0b assessment) |
| Implementation status | **WRITTEN-UNRUN.** Model types, endpoint design data, VEN registry, delivery planner, and the event-to-PROPOSED mapping exist as written code; nothing has compiled, no HTTP endpoint has served, no VEN has been reached. Serving is a designed seam — no HTTP framework dependency exists |
| Conformance status | **No conformance, certification, or compliance claim of any kind.** OpenADR Alliance certification is a human-gated lab process — see the evidence plan (records 101) |
| Control status | Supervisory relay only. An OpenADR event is an external control object: it enters the frozen 14-state command lifecycle at PROPOSED and passes all three gates — never an issued setpoint (record 21 §3) |

## 1. What the adapter is (and is not)

| Aspect | Recorded state |
|---|---|
| Role | VTN (the OpenADR server role [S-10]); VENs poll or receive push |
| Surface | `/registration`, `/poll/{ven_id}`, `/push/{ven_id}`, `/report/{ven_id}`, `/vens` as ROUTE DESIGN DATA (`RouteSpec`) — every path literal UNVERIFIED against the 3.0 specification |
| Payloads | REST/JSON model types (`EventDescriptor`, `EventSignal`, `Interval`, `OpenAdrEvent`, `Ven`, `ReportSpec`) — field sets UNVERIFIED |
| Out of scope | The 2.0b XML/SOAP legacy profile (P14-E-2 OPEN, DEFAULT "3.0-first, 2.0b on program demand" — record 22); no 2.0b code exists |
| Direction | The VTN publishes events and collects reports; the mapping to the command lifecycle is one-way into PROPOSED |

## 2. Deployment (what a deployment would configure)

| Configuration | Source | Honesty note |
|---|---|---|
| The endpoint surface | the declared `VtnEndpointDesign` | serving is the transport seam; no framework lands until a human-gated implementation decision |
| Delivery mode per VEN | the declared `DeliveryPlanner` preference order (polling vs push) | poll-frequency agreement semantics UNVERIFIED |
| Report handling | `ReportSpec` (structurally telemetry-only) | report request/submission shapes UNVERIFIED |

## 3. Security and enrollment (VEN management)

| Topic | Design position |
|---|---|
| VEN enrollment | Operator-mediated: `VtnRegistry` resolves targets ONLY for known and ACCEPTED VENs — acceptance is an operator decision, never an unattended registration |
| Credentials | VEN credential handling is not designed in record 21 beyond the transport posture; 3.0 deploys over HTTPS with standard web TLS (record 22 §1 row 2 context); the platform's Phase 13/18 identity surfaces apply |
| Secrets | Secret REFERENCES per the deployment configuration — `secret.read` stays in the never-granted set |
| VEN management surface | Recorded as PLATFORM-SPECIFIC, not protocol-specified (record 21 §5 item 5 — UNVERIFIED) |

## 4. Operations (budgets, observability, failure modes)

| Topic | Design position |
|---|---|
| Delivery planning | The planner produces a per-VEN PLAN (a record); execution belongs to the serving seam |
| Connection states | The common `ProtocolConnection` vocabulary (CLOSED/CONNECTING/CONNECTED/DEGRADED/UNREACHABLE) — consumed by the P11 taxonomy, never redefined |
| Failure modes | Test events refused by name; ambiguous power signals refused; non-power units refused; nil command ids refused; out-of-scope mappings refused; unknown/not-accepted VENs refused |
| Unit handling | kW/MW payloads convert to canonical `ActivePowerMW` at the adapter boundary (frozen P > 0 = export); reactive power is deliberately NOT mapped — OpenADR events carry no reactive-sign convention, and the platform never assumes one |

## 5. Honest boundary and certification pointer

No VTN exists, no VEN was registered, no event was published, no report
was collected, no message was exchanged. OpenADR certification is the
human-gated alliance process described in the evidence plan (records 101);
FlexCore is not certified, and the 2.0b track is deferred while P14-E-2
stands (record 22 §3).
