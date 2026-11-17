# P17.04 — Boot/provisioning/status/transaction mapping: the domain EVSE registry extension, the offline rule, the named-state rule

| Field | Value |
|---|---|
| Subtask | P17.04 (BootNotification, StatusNotification, Heartbeat, TransactionEvent mapped into a domain EVSE registry extension — NEW file in flexcore-domain; offline behavior recorded: connection loss is NOT inferred asset unavailability; statuses map to named states, never guessed [S-121][S-122]) |
| Wave / agent | Phase 17 wave 1, Agent A (exclusive file ownership: `crates/flexcore-domain/src/evse_registry.rs` + `crates/adapters/protocol-ocpp/src/provisioning.rs`) |
| Status | WRITTEN-UNRUN: the registry extension and the adapter mapping are written model code with unit tests, never compiled or executed; no charge point has ever connected; every protocol spelling is UNVERIFIED against the OCA text |
| Sources | [S-121] (OCA OCPP protocols), [S-122] (OCA What is new in OCPP 2.0.1); phase-17 context §2 (the registry seams) |
| Files touched | `crates/flexcore-domain/src/evse_registry.rs` (NEW); `crates/flexcore-domain/src/lib.rs` (additive module entry); `crates/adapters/protocol-ocpp/src/provisioning.rs` (NEW); `crates/adapters/protocol-ocpp/src/model.rs` (the provisioning payload types, from record 01's batch) |

## 1. The domain extension (additive, nothing frozen touched)

`crates/flexcore-domain/src/evse_registry.rs` is a NEW file: it extends the
domain with the EVSE-side registry vocabulary the CSMS adapter maps its
provisioning messages into. It does NOT touch `registry_rules.rs` (the
P2.02 REST deletion rule), `entities.rs` (P0.10), or the frozen command
lifecycle. Content:

| Type | Contract |
|---|---|
| `EvseIdentity` | The validated (charge-point id, evse-id) pair — evseId 0 = whole station (UNVERIFIED semantics [S-124]); `BTreeMap` determinism |
| `BootRecord` | The ACCEPTED boot: vendor/model/serial/firmware carried verbatim as DECLARED data (never parsed into capability claims), dual timestamps, the CSMS-ASSIGNED heartbeat interval (declared configuration, never a station inference) |
| `EvseLinkState` | `LINK_UP` / `LINK_DOWN{reason}` — the CSMS-connection view |
| `EvseOperationalState` | `REPORTED_AVAILABLE / REPORTED_OCCUPIED / REPORTED_RESERVED / REPORTED_UNAVAILABLE / REPORTED_FAULTED` — the last STATION-REPORTED state (recorded, never inferred) |
| `map_status_to_operational_state` | The DECLARED mapping; an unmapped status is a named refusal (`EvseRegistryError::UnmappedStatus`) — never a guessed state. The OCA 2.0.1 StatusNotification vocabulary is broader than the five secondary-sourced names; the implementation-time check may EXTEND the mapping, never silently reinterpret it |
| `EvseTransactionRecord` / `EvseTransactionState` | `Active` / `Ended` keyed by the station-side transaction id; updates refresh energy/timestamps without inventing states |
| `TimestampPair` | The dual stamp every entry carries: the protocol-reported EVENT time + the RECEIVE time (the injected clock) — the staleness machinery's input; a large inversion is a named error |
| `EvseRegistry` | Per-site, deterministic registry; provisioning and re-boot are EXPLICIT (no auto-create; a duplicate provision is refused, a re-boot goes through the explicit path) |

## 2. The offline rule (recorded once, enforced structurally)

**Connection loss is NOT asset unavailability.** When the CSMS link goes
down, ONLY the link state changes
(`EvseRegistry::set_charge_point_link_state` /
`provisioning::apply_link_loss`); the last station-reported operational
state STANDS with its own timestamps, and no asset judgment is produced.
The rule statement
(`CONNECTION_LOSS_IS_NOT_UNAVAILABILITY`) is surfaced verbatim wherever a
LINK_DOWN is consumed. Staleness is the telemetry-quality vocabulary's
business (STALE per the frozen six states) — never an inference from link
loss. Heartbeats are liveness evidence only: no registry call consumes
them into an operational state (absence is structural).

## 3. The adapter mapping (`provisioning.rs`)

| Message | Mapping |
|---|---|
| `BootNotification` | The `ProvisioningPolicy` decides ACCEPTED/REJECTED. Fail-closed DEFAULT: `allow_only(list)` accepts NOTHING unnamed; `allow_all` requires a RECORDED REASON that travels on every accept. `map_accepted_boot` validates the payload and builds the `BootRecord` at station scope |
| `StatusNotification` | The reported status name goes through the DOMAIN's declared table; the typed `StationStatus` vocabulary agrees with it (total over the five declared names). The link state is untouched |
| `Heartbeat` | Liveness only (see §2) |
| `TransactionEvent` | Started/Updated keep the transaction Active (an update refreshes energy/timestamps); Ended closes it. An update or an end for a transaction that never started is a named refusal — the registry never invents a transaction to absorb an event |

## 4. UNVERIFIED register (implementation-time checks)

| # | Detail | Check |
|---|---|---|
| 1 | BootNotification field spellings (vendorName, chargePointModel, chargePointSerialNumber, firmwareVersion) and the response shape (status/interval/currentTime) | OCA 2.0.1 provisioning section |
| 2 | The StatusNotification state vocabulary (the five names recorded here; the full 2.0.1 list is broader) and the evseId/connectorId structure | OCA 2.0.1 text [S-121][S-122] |
| 3 | The TransactionEvent event types (Started/Updated/Ended) and the transactionId semantics | OCA 2.0.1 transactions section |
| 4 | The heartbeat payload/response fields and the interval assignment | OCA 2.0.1 text |
| 5 | evseId 0 = whole station | [S-124] / OCA smart-charging text |

A mismatch is a code or record edit, never a silent reinterpretation.

## 5. Honest boundaries

1. No charge point has ever connected; no boot, status, heartbeat, or
   transaction message has ever been received from a real device.
2. The registry extension is additive — the frozen P0.11 lifecycle, P0.10
   entities, and P2.02 registry rules are untouched.
3. Agent B's concurrent modules (`ev_proposal_records`, `ev_soc_arbitration`,
   `ev_departure_constraints`, `ev_override_policy`, `ev_depot_load`) are
   separate files; the constraint-record and SoC surfaces consume this
   registry's vocabulary downstream (wave-1 ownership per plan §3).
