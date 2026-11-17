# P13.09 — Outbound webhook service (Stripe-grade signing, book-before-attempt delivery bookkeeping, dead-lettering, secret rolling)

| Field | Value |
|---|---|
| Subtask | P13.09 — Stripe-grade signing: HMAC-SHA256 over `timestamp.rawbody`, 5-minute tolerance, exponential-backoff retries with Postgres bookkeeping booked BEFORE each attempt under a delivery idempotency key, dead-letter table plus depth metric, secret rolling with overlap |
| Wave / agent | Phase 13 wave 1, Agent B |
| Status | WRITTEN-UNRUN throughout: the signing and delivery modules have never been compiled or executed; migration 0020 has never been applied; NO webhook has ever been signed, delivered, retried, or dead-lettered. The first human build/test is the gate (conventions §1); every evidence cell below is EMPTY/UNRUN. |
| Sources | [S-83] (Stripe webhooks — the DESIGN reference for the signing scheme shape, tolerance window, bounded retries, dead-lettering, and secret rolling; the FlexCore scheme is its own and no Stripe wire compatibility is claimed); plan §2.3 (the webhook delivery contract, binding); plan §2.1 (the P13.08→P13.09 secret-format seam); P11.16 (secrets posture — the landing zone, consumed never re-decided, `../../phase-11-operational-robustness/records/32_p11_16_secrets_management.md`); P11.15 (the hand-written-crypto precedent, `../../phase-11-operational-robustness/records/31_p11_15_audit_chain_upgrade.md` §3); P11.18 (migration policy, `crates/adapters/postgres/MIGRATION_POLICY.md`) |
| Files touched | `apps/control-plane/src/webhook_signing.rs` (new), `apps/control-plane/src/webhook_service.rs` (new), `crates/adapters/postgres/migrations/0020_webhook_delivery_p13_09.up.sql` + `.down.sql` (new — next free number after 0019, expand-only per P11.18), `apps/control-plane/src/metrics.rs` (ADDITIVE: the `flexcore_webhook_dead_letters` gauge + setter + render lines + header note — the single P1.17 metrics surface, no second registry), `apps/control-plane/src/lib.rs` (module declarations, marked P13.09) |

## 1. What was implemented (written, never run)

| Piece | Where | Honest status |
|---|---|---|
| Signature scheme: `FlexCore-Signature: t=<unix_seconds>,v1=<hex>`; HMAC-SHA-256 over `timestamp + "." + rawbody` with the RAW body bytes exactly as transmitted, never a re-serialized form (plan §2.3, [S-83]); 5-minute tolerance window (`TOLERANCE_SECONDS = 300`, inclusive boundary, absolute — future-stamped deliveries are equally rejected) | `apps/control-plane/src/webhook_signing.rs` | Written; verified only by the in-module unit tests (written complete, UNRUN per conventions §1) |
| Secret rolling: `WebhookSecretSet::rolling(current, previous)` — signatures are made with the CURRENT secret only; verification accepts either during the overlap; revocation is replacing the set with a single-secret one. The overlap DURATION is deployment configuration, never a constant | same | Written-unrun; the overlap-closure test pins the semantics |
| Secret consumption: `WebhookSigningKey::from_bytes` (opaque bytes, 32-byte floor, sealed construction, best-effort zeroization, no Debug/Display). The FORMAT above the floor is record 08 §4's, cited — see §2 (the seam, RESOLVED) | same | Format consumed by citation (the seam rule honored) |
| Hand-written std-only SHA-256 (FIPS 180-4) and HMAC-SHA-256 (RFC 2104), anchored to the published vectors (FIPS 180-4 "abc"/empty; RFC 4231 TC1/TC2/TC7 — the same published vectors the P11.15 chain module transcribes) | same | ASSUMPTION (hand-written crypto — §4); vector tests are the human-build correctness gate |
| Delivery bookkeeping: `WebhookDeliveryStore` port (book-delivery under idempotency key with duplicate-collapse and same-key-different-content conflict; book-attempt; record-attempt-outcome; set-delivery-schedule; list-due; promote-to-dead-letter; count-dead-letters), closed vocabularies (`PENDING`/`IN_FLIGHT`/`DELIVERED`/`DEAD_LETTERED`; `BOOKED`/`SUCCESS`/`RETRYABLE_FAILURE`/`FATAL_FAILURE`; failure classes `TIMEOUT`/`CONNECT_ERROR`/`NON_2XX`/`TLS`/`UNKNOWN` + `ATTEMPTS_EXHAUSTED`) | `apps/control-plane/src/webhook_service.rs` | Written-unrun; the in-memory `MemoryWebhookDeliveryStore` is the test path |
| Book-BEFORE-attempt discipline: the attempt row (`BOOKED`) is durably booked BEFORE the HTTP call; the crash window between booking and call leaves an honestly-stale `BOOKED` row that the recovery scan re-drives after `STALE_BOOKED_TIMEOUT_MILLIS` (60 000 ms registered default) | same | Written-unrun; the stale-recovery filter is a Postgres-implementation concern (fake limitation recorded in the module) |
| Bounded exponential backoff: `backoff_delay_secs` — attempt 1 immediate, then `2s * 2^(attempt-2)` capped at 300 s; `MAX_ATTEMPTS = 10`; beyond the bound the delivery dead-letters. All three constants REGISTERED DEFAULTS (X-8 pre-registration; §3) | same | Pure function; unit tests pin the schedule |
| Dispatcher: `WebhookDispatcher::enqueue` (idempotency-key booking) and `run_attempt` (book → sign → post → record → schedule/dead-letter) and `run_scan` (per-delivery errors recorded, never scan-aborting). HTTP transport is the `OutboundHttpPost` PORT — no HTTP-client dependency is added, no I/O is performed by this batch | same | Written-unrun |
| Dead-letter table + depth metric: `webhook_dead_letters` (migration 0020) with closed reason classes; `flexcore_webhook_dead_letters` gauge added to the single metrics registry (always rendered, zero included, NO labels — the recorded cardinality discipline), recommended alert threshold `DEAD_LETTER_ALERT_THRESHOLD_RECOMMENDED = 100` registered as a RECOMMENDATION for the operator's monitoring, never claimed as an approved operating number | migration 0020; `metrics.rs` additive edit | Written-unrun; the scanner→gauge wiring is a composition step (§5) |
| Delivery bookkeeping DDL: `webhook_endpoints` (tenant-scoped; secret provider REFERENCES + fingerprints per record 08 §4 — never material; active flag), `webhook_deliveries` (UNIQUE (tenant_id, delivery_key); tenant-scoped FK to endpoints), `webhook_delivery_attempts` (UNIQUE (delivery_id, attempt_seq); book-before-attempt columns), `webhook_dead_letters` (UNIQUE (delivery_id); resolution marks WHEN, never deletes) — RLS ENABLE + FORCE with the shared `tenant_isolation` policy, `flexcore_app` grants SELECT/INSERT/UPDATE (these four are bookkeeping/configuration tables, NOT the append-only evidence class — `audit_records` 0008+0019 untouched, the P11.18 protected set unchanged) | migration 0020 up/down | Written; never applied |

## 2. The P13.08→P13.09 secret-format seam — RECONCILED

The seam rule (plan §2.1): the webhook secret FORMAT is defined ONCE by
Agent A's P13.08 record and consumed by this signer — never redefined.

**History and outcome.** At first authoring time Agent A's phase-13
records were NOT present (the `records/` directory did not exist —
wait-check 1, 2026-11-14), so this batch derived the MINIMUM it needed
from plan §2.3: opaque bytes, ≥ 32-byte floor, storage under the P11.16
posture, references-not-format. Agent A's record
`08_p13_08_api_keys_webhook_secret_format.md` subsequently landed (wait-
check 2, before Agent B's close), and the reconciliation is RESOLVED as
follows:

| Element | Record 08 §4 (the authority) | How this batch consumes it |
|---|---|---|
| Literal form | `whsec_<43 unpadded base64url characters>` — 32 bytes CSPRNG entropy, strict parse | The literal form is NOT restated in any B artifact; the signing module consumes DECODED bytes only (`WebhookSigningKey::from_bytes`, whose `SECRET_MIN_BYTES = 32` floor equals record 08 §4's decoded length exactly — no competing floor exists) |
| Storage | "the secret lands in the P11.16 posture … the database row carries a fingerprint and a state, never the material" | **Migration 0020 was AMENDED to comply**: the endpoint columns are `secret_current_ref` / `secret_current_fingerprint` / `secret_previous_ref` / `secret_previous_fingerprint` — provider references and fingerprints, NEVER material. The material is resolved at attempt time through the new `WebhookSecretResolver` port (composition binds it to the P11.16 posture). The initial draft's material-in-DB columns are gone |
| Binding | "exactly one live secret per webhook endpoint at rest; during the rolling overlap an endpoint carries current + previous — both of the SAME format" | The `WebhookEndpointRecord` current/previous reference pair implements exactly this; the overlap semantics are plan §2.3's (record 08 §4 defers the computation to P13.09 — this module) |
| Fingerprint | "the first 8 characters after the prefix — never the secret" | The `*_fingerprint` columns carry it for the P13.18 admin surfaces; this batch renders it nowhere |

**No conflict remains.** The one CONFLICT found during reconciliation (the
initial draft stored secret material in the 0020 columns, which record 08
§4 forbids) was FIXED by amending the migration and adding the resolver
port — the amendment is part of this batch, before the close. The
byte-identical never-granted set (record 08 §5's own quote) is unaffected.

## 3. Registered values (X-8 pre-registration)

All registered before any run that could depend on them (conventions §5;
no delivery has ever been made, so no observed distribution exists):

| Constant | Value | Meaning | Change procedure |
|---|---|---|---|
| `TOLERANCE_SECONDS` | 300 | the replay window (plan §2.3 fixed) | recorded amendment to this record AND a coordinated receiver change |
| `SECRET_MIN_BYTES` | 32 | fail-closed secret floor — equals record 08 §4's decoded length exactly (§2, RESOLVED) | a record 08 §4 amendment would propagate |
| `MAX_ATTEMPTS` | 10 | the bounded attempt count | recorded amendment (registered DEFAULT pending operator tuning) |
| `BACKOFF_BASE_SECS` / `BACKOFF_FACTOR` / `BACKOFF_CAP_SECS` | 2 / 2 / 300 | the exponential schedule | recorded amendment |
| `STALE_BOOKED_TIMEOUT_MILLIS` | 60 000 | the crash-recovery window for `BOOKED` rows | recorded amendment |
| `DEAD_LETTER_ALERT_THRESHOLD_RECOMMENDED` | 100 (RECOMMENDATION) | the DLQ gauge's alert threshold — an operator decision on the monitoring side | operator tuning; the number here is a recommendation to bind, never an approved operating figure |

## 4. ASSUMPTION / UNRUN / UNSIGNED register

1. **ASSUMPTION — hand-written crypto.** The program bans new dependency
   additions, so SHA-256/HMAC-SHA-256 are hand-written (std-only), the
   exact precedent of P11.15 (record 31 §3 items 1–2, including the
   duplication rationale — consolidation of the two copies is a recorded
   follow-up for a wave that owns both manifests and the visibility
   surface). Vector tests (FIPS 180-4, RFC 4231 TC1/TC2/TC7) are the
   first-build correctness gate; a mismatch is build-blocking. Test
   vectors demonstrate algorithm conformance of a hand-written
   implementation; they are NOT a security review. An independent review
   of this module's crypto before any production secret is used is part
   of the DEPLOYMENT REQUIREMENT gate (conventions §6.6).
2. **ASSUMPTION — egress.** The `OutboundHttpPost` port performs whatever
   egress policy the deployment composes (TLS verification, proxying,
   allow-lists are deployment concerns, not this module's). The URL
   scheme is not policed here; the deployment policy owns it.
3. **UNRUN** — every function, test, the migration, the metric render,
   and every scan/attempt/dead-letter path. No webhook exists anywhere in
   any environment; `webhook_endpoints` is empty by construction.
4. **UNSIGNED / EMPTY** — no delivery key, no attempt booking, no dead
   letter, no secret (current or previous), no gauge reading above zero:
   all EMPTY by construction.
5. **RECORDED FAKE LIMITATION** — `MemoryWebhookDeliveryStore` does not
   apply the stale-booking recovery filter (a Postgres WHERE concern);
   recovery semantics are exercised by the real adapter, not the fake
   (noted in the module source).

## 5. Wiring (recorded follow-up, deliberately not edited this batch)

The Postgres implementation of `WebhookDeliveryStore` lands on
`FlexCoreStore` following the edge-enrollment pattern (P6.06: a standalone
store opening its own tenant-scoped unit of work per call, OUTSIDE the
per-request UoW bundle — delivery bookkeeping is background-store state).
This batch deliberately does NOT edit
`crates/adapters/postgres/src/store.rs` / `unit_of_work.rs` / the
composition roots further: those are shared seams where Agent A's identity
storage (P13.03/P13.07/P13.08) lands concurrently, and a conflicting edit
to the unit-of-work bundle is a higher risk than an honestly-recorded
follow-up. The named composition steps for wave 2 / the human build:

1. `crates/adapters/postgres/src/webhook_delivery.rs` implementing the
   port on `FlexCoreStore` (the 0020 DDL is its contract).
2. Composition: `FlexCoreStore::with_webhook_delivery_store` (or an
   equivalent accessor), the real `OutboundHttpPost` client, the
   dispatcher's hosting loop (scan cadence = deployment configuration),
   and the scanner that feeds `set_webhook_dead_letters`.
3. The P13.18 admin surface (wave 2) renders the endpoint/secret model
   (plan §2.1: P13.09 → P13.18).

## 6. The never-granted permission set — byte-identical

Nothing in this batch grants anything to anyone: the webhook service is
outbound notification plumbing with no authz surface. Quoted unchanged
(conventions §6.4):

`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`

No principal gains any of them through this subtask; a webhook endpoint
holds a signing secret, which is outbound-message authentication material
— it confers no permission, no scope, and no dispatch reach (Phase 13
context §3 item 5 applies to the delivery path too: a webhook is never a
dispatch path; the payloads the service carries are event notifications,
and the module imports no command/dispatch type).

## 7. Handoff notes

1. **Orchestrator**: tick P13.09 only with this record's citation; the
   subtask is implemented (written-unrun), never tested or verified.
2. **Agent A (P13.08)**: §2 records the RESOLVED reconciliation against
   your record 08 §4 (format cited; the 0020 columns amended to
   references + fingerprints; the resolver port added). If record 08 §4
   is itself amended later, the amendment propagates to the resolver
   composition only — no change to the signer or the delivery state
   machine.
3. **Wave 2 (P13.18/P13.21)**: the admin UI renders `WebhookEndpointRecord`
   (§5 step 3); the notification pipeline's webhook channel
   (`notifications.rs`) is a SEPARATE consumer that composes THIS service
   for outbound notification webhooks — one delivery mechanism, two
   entry points.
4. **Human build**: the RFC 4231/FIPS vector tests are the correctness
   gate for the hand-written crypto (build-blocking on mismatch); the
   migration policy audit trail treats 0020 as expand-only (no
   `CONCURRENTLY`/`VALIDATE` steps scheduled — all statements are new
   tables and their indexes).
