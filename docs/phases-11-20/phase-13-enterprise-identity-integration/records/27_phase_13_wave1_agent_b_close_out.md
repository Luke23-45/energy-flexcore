# Phase 13 wave 1 — Agent B close-out (P13.09–P13.14)

| Field | Value |
|---|---|
| Agent | Phase 13 wave 1, Agent B |
| Subtasks delivered | P13.09, P13.10, P13.11, P13.12, P13.13, P13.14 — all WRITTEN-UNRUN ("implemented, awaiting human build/test"); the ledger ticks are the orchestrator's, applied with each record as citation |
| Records written | `21_p13_09_webhook_service.md`, `22_p13_10_api_versioning_deprecation.md`, `23_p13_11_notification_pipeline.md`, `24_p13_12_audit_export.md`, `25_p13_13_gdpr_privacy.md`, `26_p13_14_jurisdiction_registry.md`, this close-out |
| Code written | `apps/control-plane/src/webhook_signing.rs`, `webhook_service.rs`, `api_versioning.rs`, `notifications.rs`, `audit_export.rs` (all new, all with complete in-module test files written-unrun); migration `crates/adapters/postgres/migrations/0020_webhook_delivery_p13_09.up.sql` + `.down.sql`; ADDITIVE metrics edit (`flexcore_webhook_dead_letters` gauge in `apps/control-plane/src/metrics.rs`); ADDITIVE module declarations in `apps/control-plane/src/lib.rs` |
| Files NOT touched | `docs/foundations/*`, `docs/plan/*`, `.agent/*`, the ledger, plan.md, context.md, `apps/control-plane/src/auth.rs`, `crates/flexcore-authz/*`, `crates/flexcore-application/*`, `crates/flexcore-storage/*`, `crates/flexcore-audit/*`, every Agent A record (01–08, 19), `crates/adapters/postgres/src/*` (store.rs/unit_of_work.rs deliberately untouched — see record 21 §5) |

## 1. Per-subtask one-line status

| Subtask | Deliverable | Record | Honest state |
|---|---|---|---|
| P13.14 (landed first — P13.13 keys on it) | Jurisdiction registry: per-tenant residency + legal-basis capture design; sign-off = human gate; cites the P9.10 record, never rewrites it | 26 | Design record; every jurisdiction/regime/basis value EMPTY; D1 stays OPEN |
| P13.09 | Webhook signing (HMAC-SHA256 over `timestamp.rawbody`, 5-min tolerance, secret rolling) + delivery service (book-before-attempt under an idempotency key, bounded exponential backoff, dead-letter table + depth gauge) + migration 0020 | 21 | Written-unrun; no webhook signed, delivered, or dead-lettered anywhere |
| P13.10 | API versioning policy: `/api/v1/` confirmed CURRENT; Deprecation/Sunset header design (RFC 8594 [S-84]; Deprecation header is a DRAFT — recorded UNVERIFIED-AT-A-DISTANCE); 180/365-day window bounds; OpenAPI breaking-change checker over the GENERATED spec | 22 | Policy registered + mechanics written-unrun; no version deprecated, no header ever attached, lint never run |
| P13.11 | Notification pipeline: provider-abstracted channel ports, validated template registry, per-tenant severity preferences, event→notification map (4 conservative alarm rows; absence = audit-only) | 23 | P13-E-2 registered OPEN with DEFAULT (email-first); NO notification ever sent; never-a-dispatch-path enforced structurally (receipt-only ports) |
| P13.12 | Audit export packs: SIEM JSONL over the SELECT-only `AuditEventReader`, tenant-scoped by construction, HMAC-signed manifest + body chain, integrity statement INHERITING the P11.15 chain with the chain-column gap stated verbatim | 24 | No export ever produced; no SIEM ever consumed a line |
| P13.13 | GDPR/privacy engineering: workspace data inventory (by citation + the Phase 13 additions), per-jurisdiction retention matrix over the 7 P1.10 classes keyed on the P13.14 registry (ALL value/basis cells EMPTY), DSAR job design, erasure incl. the backup RE-MATERIALIZATION caveat, event streams (bounded retention + erasure-as-events), ML caches (cryptographic erasure design) | 25 | No regime asserted (D1 OPEN); no DSAR exercised; no erasure run; no code changed |

## 2. The A-reconciliation (the secret-format seam) — RESOLVED

Timeline, honestly recorded:

1. **Wait-check 1** (before any B write): Agent A's phase-13 `records/`
   directory did not exist — B derived the minimum from plan §2.3 (opaque
   bytes, 32-byte floor, P11.16 posture) and marked the seam OPEN in
   record 21 §2.
2. **Wait-check 2** (before B's close): Agent A's records 01–08 and 19
   had landed, including `08_p13_08_api_keys_webhook_secret_format.md`
   §4 — THE format: `whsec_<43 unpadded base64url>` = 32 bytes, strict
   parse, and the storage rule "the database row carries a fingerprint
   and a state, never the material".
3. **Reconciliation**: one CONFLICT was found and FIXED before close —
   B's initial 0020 draft stored secret material in the endpoint columns,
   which record 08 §4 forbids. The amendment (all in B's own files):
   0020 columns became `secret_current_ref`/`secret_current_fingerprint`/
   `secret_previous_ref`/`secret_previous_fingerprint` (references +
   fingerprints, never material); a `WebhookSecretResolver` port resolves
   references to bytes at attempt time (composition binds the P11.16
   posture); `SECRET_MIN_BYTES = 32` equals record 08 §4's decoded length
   exactly; the literal format is never restated in any B artifact.
   Record 21 §2 carries the resolved table; A's close-out handoff note 1
   (cite record 08 §4) is honored. A's suggestion to propose `webhook.*`
   authz actions through the additive closed-enum rule is left to the
   wave that owns the authz seam (A's crate) — no management endpoint
   exists in this batch (P13.18, wave 2), so no action was needed.

## 3. The never-granted permission set — byte-identical

Quoted unchanged in records 21 (§6), 23 (§2), 25 (§5), per conventions
§6.4:

`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`

No principal gains any of them through this wave; no B module imports any
command/dispatch type (the notification ports return receipts only; the
webhook payloads are event notifications; the export is read-only).

## 4. Permitted verification tools — VERBATIM OUTPUTS

The three permitted tools, run at close (conventions §1 item 5; the run
below is the close-out run; the deterministic outputs are quoted exactly
as the console emitted them):

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
```

```text
$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
```

```text
$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 20 workspace members.
```

(The `ù` character in the dependency-rule line is the console's
rendering of the tool's separator character as emitted in this
environment — quoted verbatim, not normalized. 20 workspace members =
no crate was added by this wave; all B code is modules in existing
members, so the checker's edge table is untouched.)

## 5. Deviations and why (all recorded in the owning subtask's record)

| Deviation | Why | Where recorded |
|---|---|---|
| No Postgres adapter implementation of `WebhookDeliveryStore` (the in-memory fake is the test path; the 0020 DDL is the storage design) | `store.rs`/`unit_of_work.rs` are shared seams where Agent A's identity storage lands concurrently; a conflicting UoW-bundle edit was a higher risk than a recorded follow-up | record 21 §5 |
| No composition-root wiring (state.rs/main.rs untouched by B; module declarations only in lib.rs) | The dispatcher/scan hosting loop, the HTTP client, and the resolver composition are deployment decisions; nothing routes to these services yet (management UI is P13.18, wave 2) | records 21 §5, 23 §5, 24 §5 |
| Hand-written std-only SHA-256/HMAC-SHA256 in `webhook_signing.rs` (duplicating the P11.15 chain's `pub(crate)` crypto) | The batch bans forbid new dependency additions and manifest edits; the P11.15 precedent (record 31 §3) is followed exactly, with the same vector anchoring and the same consolidation follow-up | record 21 §4 item 1 |
| `metrics.rs` edited additively (one gauge + setter + render lines) | The plan §2.3 contract requires the DLQ depth to be a metric on the SINGLE registry; the P11.20 stuck-gauge precedent is the pattern | record 21 §1 |
| P13.12 built over the `AuditEventReader` port rather than `flexcore-audit` records directly | The read port is the only SELECT surface into `audit_records` (control-plane does not depend on flexcore-audit, and adding the edge would be a manifest change); the port's two-method shape is pinned and untouched; the chain-column gap is stated verbatim in every manifest | record 24 §2, §5 |

## 6. Handoff notes

1. **Orchestrator**: the six subtasks are implemented (written-unrun);
   tick each ledger row only with its record citation; the permitted-tool
   outputs above are the close evidence.
2. **Wave 2**: P13.18 renders the endpoint/secret model (references +
   fingerprints — display-never-secret per record 08 §4); P13.19's
   negative tests extend the in-module test surfaces (webhook
   signature/tolerance/replay, idempotent-handler, never-a-dispatch-path);
   P13.21 registers the closed notification vocabulary against
   `EVENT_NOTIFICATION_MAP` (rows, never logic).
3. **Human build gates outstanding**: the first workspace build (compiles
   the new modules, the migration, and every written test file — the
   RFC 4231/FIPS vector tests are build-blocking for the hand-written
   crypto); the migration 0020 application; every runtime act (first
   webhook delivery, first notification, first export) stays UNRUN.
4. **Agent A**: if record 08 §4 is ever amended, the propagation point is
   the `WebhookSecretResolver` composition only (record 21 §7 note 2).
