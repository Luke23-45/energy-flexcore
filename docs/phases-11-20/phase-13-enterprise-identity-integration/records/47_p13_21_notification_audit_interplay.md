# P13.21 — Notification/audit interplay (the closed, registered, versioned disposition vocabulary — registered against the P13.11 map, against the frozen 26-event set)

| Field | Value |
|---|---|
| Subtask | P13.21 — closed, registered vocabulary of which lifecycle transitions and alarms emit notifications vs audit-only; the mapping is registered, versioned DATA, not scattered logic |
| Wave / agent | Phase 13 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the registry module and its tests are written and never compiled or executed; **no notification has ever been sent, no disposition has ever routed a message, no registry amendment has ever been applied anywhere**. The registration is DATA in the repo; every runtime evidence cell is EMPTY/UNRUN. |
| Sources | record 23 (the P13.11 pipeline — `EVENT_NOTIFICATION_MAP`, the four initial conservative rows, the absence=audit-only default, the never-a-dispatch-path structural rule — consumed BY CITATION, the map itself untouched); `crates/flexcore-events/src/events.rs` (`FROZEN_EVENT_NAMES` — the frozen 26-event vocabulary, IMPORTED not re-listed; the frozen set stays byte-identical); plan §2 row P13.21 and §2.1 (the P13.11 → P13.21 ordering); context §3 item 5 (notifications are never a dispatch path); conventions §6.4 |
| Files touched | `apps/control-plane/src/notification_vocab.rs` (NEW), `apps/control-plane/src/lib.rs` (additive module line, marked P13.21) |

## 1. The registration (what "registered, versioned DATA" means here)

One table, one version, one home:

| Element | Value |
|---|---|
| Registry | `EVENT_DISPOSITIONS: [EventDisposition; 26]` — the disposition of EVERY frozen event, exactly once each (pinned by test) |
| Version | `INTERPLAY_REGISTRY_VERSION = "1.0.0"` — amending a disposition is a recorded amendment that bumps this constant (patch = rationale-only edit, minor = disposition change; the recorded convention) |
| Dispositions | the closed two-value vocabulary `Notify` / `AuditOnly` (extension = a recorded amendment) |
| Row shape | (event name, disposition, channels, severity, template id, rationale) — no handler, no target, no state mutation: the routing fields are the map's OWN shapes, never new logic |
| Source of truth | `FROZEN_EVENT_NAMES` is IMPORTED (the module never re-lists the 26 names) — one source of truth, byte-identical |
| Relation to the P13.11 map | the registry is the layer ABOVE `EVENT_NOTIFICATION_MAP`: the 22 absences the map expressed implicitly are now EXPLICIT rows with registered rationales; the map is untouched (rows, never logic — B's close-out handoff note 2, honored) |

## 2. The registered vocabulary (the table, summarized)

| Disposition | Events | Rationale pattern |
|---|---|---|
| **Notify** (4) | `TelemetryQualityChanged` (EMAIL/WARNING/alarm.quality_changed), `AssetAvailabilityChanged` (EMAIL/WARNING/alarm.availability_changed), `PluginFailed` (EMAIL/CRITICAL/alarm.plugin_failed), `DispatchFailed` (EMAIL/CRITICAL/alarm.dispatch_failed) | exactly the record 23 initial registration, verbatim — failure/quality facts that ALREADY happened and are ALREADY in the audit trail; the full table in `notification_vocab.rs` carries each row's registered rationale |
| **AuditOnly** (22) | every other frozen event — all 11 remaining dispatch transitions (`DispatchProposed`, `DispatchApproved`, `DispatchRejected`, `DispatchIssued`, `DispatchAcknowledged`, `DispatchTrackingUpdated`, `DispatchCompleted`, `DispatchExpired`, `DispatchCancelled`, `DispatchSuperseded`, `DispatchFallbackActivated`), the registry/planning events (`AssetRegistered`, `AssetCapabilityUpdated`, `TelemetryReceived`, `MarketSignalReceived`, `FlexibilityEnvelopeUpdated`, `ForecastPublished`, `ScheduleProposed`, `ScheduleValidated`, `FeasibilityCheckCompleted`), `PluginActivated`, `OperatorActionRecorded` | each row carries its own registered rationale (e.g. proposals are workflow state; high-frequency deltas would be spam by construction; the operator action record IS the evidence); a transition notification would be a deliberate, reviewed row in a recorded amendment — none exists |

The conservative default is now STRUCTURAL and VISIBLE: exactly four
events notify, and `DispatchFailed` notifies as an observed fact — the
notification neither causes nor alters anything (the never-a-dispatch-path
rule; the row's rationale states it and a test pins the wording).

## 3. The agreement invariant (rows, never logic)

`validate_registry()` (pure, exported) checks over the IMPORTED frozen
names and the UNTOUCHED P13.11 map:

1. every frozen event appears in the registry exactly once; no name
   outside the frozen set; registry length equals the frozen length;
2. every `Notify` row has an `EVENT_NOTIFICATION_MAP` counterpart and
   agrees with it EXACTLY (channels, severity, template id);
3. every `AuditOnly` row carries empty routing fields AND has no map row
   (a map row the registry calls audit-only would be a contradiction —
   denied);

The in-module tests pin these plus: the Notify set is exactly the record
23 four; every dispatch transition except the failure is audit-only; the
version is a recorded constant; the frozen vocabulary is consumed, not
re-listed. All UNRUN (conventions §1 item 3).

## 4. The frozen sets and the never-granted set — byte-identical

| Frozen set | How this batch touches it | State |
|---|---|---|
| The frozen 26-event vocabulary (`crates/flexcore-events/src/events.rs`) | imported (`flexcore_events::FROZEN_EVENT_NAMES`); the file was NOT opened for edit; a test pins that the registry length equals the imported constant's length (the module cannot silently diverge) | **byte-identical** |
| The frozen 14-state / 27-edge command lifecycle | referenced by NAME only in rationales; no file touched | **byte-identical** |
| The never-granted permission set (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) | quoted unchanged in the module header; this batch grants nothing to anyone — a disposition is a message-routing fact, never a permission | **byte-identical** |
| `apps/control-plane/src/notifications.rs` (record 23) | consumed by citation (typed import of `EVENT_NOTIFICATION_MAP`, `NOTIFICATION_CHANNELS`, `NOTIFICATION_SEVERITIES`); NOT edited — the map stays byte-identical | **byte-identical** |

## 5. ASSUMPTION / UNRUN register

1. **UNRUN** — every test and validation; no notification exists in any
   environment; the router has never consumed any disposition. Evidence
   cells EMPTY.
2. **ASSUMPTION (compile)** — the typed import of the three
   `notifications.rs` items and `flexcore_events::FROZEN_EVENT_NAMES`
   compiles as written (both re-export/declare the shapes as read);
   verified by reading the sources, not by building.
3. **REGISTERED, not decided** — the 22 audit-only rationales are the
   RECOMMENDED DEFAULT registrations (reviewable, like the record 05
   grant table); amending one is a version bump plus a recorded
   amendment, never a code change beyond the table.
4. **RECORDED LIMITATION** — the registry is a compile-time table; a
   deployment needing per-tenant notification OVERRIDES composes them at
   the preference layer (record 23's severity floors), never by editing
   this vocabulary.

## 6. Handoff notes

1. **Orchestrator**: tick P13.21 with this record; the vocabulary is
   implemented (written-unrun), never tested or verified.
2. **P13.22 (close)**: the cross-references to check — record 23 §3's
   "the final closed vocabulary is wave 2's P13.21 registration" is
   THIS table; the map itself stays byte-identical; the four Notify
   rows agree verbatim.
3. **Future amendments**: a disposition change = the table row + the
   version bump + a recorded amendment (and, for a new Notify row, a
   matching `EVENT_NOTIFICATION_MAP` row in the same amendment — the
   agreement invariant fails the build otherwise, which is the drift
   guard).
