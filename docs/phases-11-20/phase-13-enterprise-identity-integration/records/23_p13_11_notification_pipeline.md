# P13.11 — Notification pipeline (channels over provider-abstracted ports, template registry, per-tenant preferences, event→notification mapping)

| Field | Value |
|---|---|
| Subtask | P13.11 — email/SMS/webhook channels, template registry, per-tenant preferences, event→notification mapping — alarm/audit-driven ONLY; channel providers are P13-E-2; no channel is ever a dispatch path |
| Wave / agent | Phase 13 wave 1, Agent B |
| Status | WRITTEN-UNRUN — the pipeline is design-as-code that has never been compiled or executed. NO channel is configured, NO provider account exists, NO notification has ever been sent. Every evidence cell is EMPTY/UNRUN. |
| Sources | [S-83] (Stripe webhooks — the webhook channel composes the P13.09 signed delivery service rather than sending unsigned); plan §4 P13-E-2 (the channel-provider E-decision, OPEN); `crates/flexcore-events/src/events.rs` (`FROZEN_EVENT_NAMES` — exactly the 26 frozen §15.3 events, consumed as the closed mapping source, never re-listed or extended); Phase 13 context §3 item 5 (the never-a-dispatch-path rule, binding) |
| Files touched | `apps/control-plane/src/notifications.rs` (new), `apps/control-plane/src/lib.rs` (module declaration, marked P13.11) |

## 1. What was implemented (written, never run)

| Piece | Where | Honest status |
|---|---|---|
| Closed vocabularies: `NOTIFICATION_CHANNELS` (EMAIL/SMS/WEBHOOK), `NOTIFICATION_SEVERITIES` (INFO/WARNING/CRITICAL) | `notifications.rs` | Written-unrun; adding a channel is a recorded amendment |
| Channel ports: `EmailNotificationPort`, `SmsNotificationPort`, `WebhookNotificationPort` — provider-abstracted (the composition binds vendors); every port accepts rendered TEXT and returns a `NotificationReceipt` ONLY | same | Written-unrun; no provider exists behind any of them |
| Template registry: validated, versioned templates (closed placeholder vocabulary enforced at registration; an unknown `{{placeholder}}` fails registration; an unset value renders visibly as `<unset:NAME>`, never silently blank) | same | Written-unrun; the pipeline SHIPS with zero templates (an unregistered key fails closed with `TemplateNotFound` — never a half-templated send) |
| Per-tenant preferences: severity floors per channel; the P13-E-2 DEFAULT as data (EMAIL at WARNING+, SMS off, WEBHOOK off — webhook delivery stays per-endpoint explicit per the P13.09 model) | same | Written-unrun |
| Event→notification mapping: `EVENT_NOTIFICATION_MAP` — registered, versioned DATA (not scattered logic), initial rows email-only and alarm/audit-driven only: `TelemetryQualityChanged`, `AssetAvailabilityChanged`, `PluginFailed`, `DispatchFailed` | same | Initial DATA; the final closed vocabulary is wave 2's P13.21 registration AGAINST this table (plan §2.1: P13.11 → P13.21) |
| The router: one decision point (`NotificationRouter::route`) — mapping lookup (absence = audit-only, empty receipts), template render, preference floor, port send | same | Written-unrun; the hosting task that invokes the router is a recorded composition step (§5) |
| In-memory email fake (recording) as the test path | same | Written-unrun |

## 2. The never-a-dispatch-path rule — enforced structurally

The rule (context §3 item 5) is not a comment here; it is the module's
shape:

1. **The ports cannot carry control.** Every port signature is
   `(recipient, text...) → Result<NotificationReceipt>`; the receipt is a
   channel/message-id/accepted-at triple. No command id, approval, lifecycle
   state, or dispatch type appears in any port, receipt, template, or
   mapping row — a channel provider physically has nothing to call and
   nothing to return. P13-E-2 selects VENDORS, not POWERS.
2. **The mapping cannot advance a lifecycle.** `EVENT_NOTIFICATION_MAP`
   maps event NAMES to (channels, severity, template id). It carries no
   transition logic, no state mutation, no target handler. The dispatch
   lifecycle's transitions are the frozen 14-state/27-edge set
   (`crates/flexcore-domain/src/command_state.rs`) — untouched,
   byte-identical, and referenced by this module as a name list only.
3. **The router is invoked, never self-driving.** The router holds no
   event-source subscription; the pipeline's hosting task (composition,
   §5) calls it with facts that already happened. A notification that
   fails to send is a pipeline outcome (a logged failure), never a
   lifecycle event: the fact it observed has already occurred and is
   already in the audit trail — notification delivery success or failure
   changes nothing about the command's state.
4. **The never-granted permission set stays byte-identical** (conventions
   §6.4). Quoted unchanged:

   `dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`

   Channel adapters receive scope SUBSETS at most (plan risk row 7) — and
   since the ports carry no authz surface at all, this batch grants
   nothing to anyone.

## 3. The mapping discipline (alarm/audit-driven ONLY)

The initial mapping names only failure/quality events that already landed
in the audit trail and the derived alarm surface. The conservative default
is structural: **absence from the table = audit-only** — an event that is
not a row notifies nobody, and the undecided lifecycle transitions
(`DispatchProposed`, `DispatchApproved`, `DispatchIssued`,
`DispatchAcknowledged`, `DispatchCompleted`, and the rest of the frozen 26)
are deliberately absent until wave 2's P13.21 registers the closed
vocabulary against this table. A test pins the absence of the named
transitions. Nothing in this batch registers a dispatch-adjacent
notification.

## 4. P13-E-2 — registered OPEN (agents never close it)

| Field | Value |
|---|---|
| Question | Notification channel providers: which email/SMS vendors? |
| Options | self-hosted SMTP only / commercial email+SMS providers / email-only first |
| DEFAULT (RECOMMENDATION) | **email-first with provider-abstracted ports** (adapters depend on ports); SMS and webhook channels designed but vendor selection deferred; no channel is a dispatch path |
| Implemented here | The DEFAULT (the subtask says so — conventions §4): the email port is the only composed-by-default channel; SMS/webhook ports exist as designed types with no vendor; the preference defaults express email-first |
| What would close it | The human owner's vendor selection and budget (a provider account, its contract, and its secrets posture under P11.16) |
| Status | **OPEN** — this batch records the dependence, never closes it |

## 5. Wiring (recorded composition steps, deliberately not edited this batch)

1. Composition binds the email port (SMTP or, post-P13-E-2-closure, the
   selected vendor adapter) and, when operators opt in, the SMS port.
2. The webhook port composes the P13.09 delivery service — ONE signed,
   booked, retried, dead-lettered outbound path; this pipeline adds no
   second, unsigned sender.
3. The hosting task subscribes to the event stream (the frozen vocabulary's
   subjects) and invokes the router per fact; its cadence/backpressure is a
   deployment decision. Until composed, nothing notifies anywhere — the
   honest silent state (a test pins it: no port composed → empty receipts,
   no send).
4. Templates for the four initial mapping keys are registered by the
   deployment (the registry ships empty by design).

## 6. ASSUMPTION / UNRUN / EMPTY register

1. **UNRUN** — every port, the router, the registry, every test. No
   notification exists in any environment: no recipient, no message id,
   no receipt, no template, no preference row. All EMPTY by construction.
2. **ASSUMPTION** — the initial four-row mapping is this batch's
   conservative registration, pending P13.21's final vocabulary (wave 2);
   it is DATA and amending it is a recorded change, never logic drift.
3. **ASSUMPTION** — placeholder-rendering via string replacement is
   intentionally minimal (no external templating dependency is added);
   templates needing conditional logic are out of scope (a template that
   needs logic is a design smell the registry's closed placeholders
   resist).
4. **EMPTY** — P13-E-2's vendor cell; every `message_id`; every
   `accepted_at`; the receipt count of every channel.

## 7. Handoff notes

1. **Orchestrator**: tick P13.11 with this record's citation; the pipeline
   is implemented (written-unrun), never tested or verified.
2. **Wave 2 (P13.21)**: register the closed vocabulary against
   `EVENT_NOTIFICATION_MAP` (§3) — amend ROWS, never scatter logic; keep
   the dispatch-lifecycle absences until each transition's registration is
   a deliberate, reviewed row.
3. **Wave 2 (P13.19)**: the idempotent-handler and never-a-dispatch-path
   negative tests assert against this module's ports (the receipt-only
   return type is the assertion surface).
