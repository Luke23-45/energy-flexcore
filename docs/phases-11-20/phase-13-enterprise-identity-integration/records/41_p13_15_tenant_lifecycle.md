# P13.15 — Tenant lifecycle operations (onboarding checklist, offboarding/decommission procedure, tenant data export and reconciliation under RLS)

| Field | Value |
|---|---|
| Subtask | P13.15 — onboarding checklist, offboarding/decommission procedure, tenant data export and reconciliation design under RLS [S-70] |
| Wave / agent | Phase 13 wave 2, Agent A |
| Status | DESIGN RECORD (procedure) — **no tenant is created, modified, exported, or deleted anywhere**; every checklist step below is written as the procedure a deployment executes, not as an executed act. Every evidence cell (tenant count, export size, reconciliation result) is EMPTY/UNRUN. |
| Sources | [S-70] (AWS multi-tenant RLS guide, gap analysis §11 — the RLS discipline this procedure runs under; the pooling-interaction caveat [S-69] is stated, never resolved here); the repo's RLS posture: `crates/adapters/postgres/migrations/0008_audit_records.up.sql` (RLS ENABLE + FORCE, the shared `tenant_isolation` policy) and `0020_webhook_delivery_p13_09.up.sql` (the same policy on the four webhook bookkeeping tables); consumed wave-1 records: 01 (per-tenant SSO connections as tenant data), 03 (SCIM connections and the group→role mapping), 08 (API keys), 21 (webhook endpoints), 23 (notification preferences), 24 (audit export packs), 25 (GDPR/retention matrix), 26 (jurisdiction registry); the P1.10 `RetentionPolicy` classes (`crates/flexcore-storage/src/retention.rs`); report §20.1 item 12 (the Administration surface — cited, never edited) |
| Files touched | none (design record only) |

## 1. Position and scope

Tenant lifecycle operations are the OPERATOR PROCEDURES around the tenant
records the registry already models (`GET /api/v1/tenants`, the P2.12
read-only tenant-scoped read; creation/deletion grants exist on the authz
surface — `tenant.create` PlatformAdmin, `tenant.delete` PlatformAdmin —
but this record executes neither). The procedure lives UNDER the RLS
posture, never around it: every data-touching step is a tenant-scoped
operation through the existing ports, and RLS ENABLE + FORCE with the
shared `tenant_isolation` policy is the outer gate the procedure relies
on and verifies. Nothing here creates a second data path, a migration,
or an endpoint — those are named as specified-not-implemented where the
procedure needs them (§5).

## 2. The onboarding checklist (the procedure, in order)

Each step names the actor, the consumed record/seam, and its completion
evidence. The checklist is a GATED sequence: a step starts only when the
previous step's evidence exists.

| # | Step | Actor | Consumed seam / record | Completion evidence (audited fact) |
|---|---|---|---|---|
| O-1 | Commercial/contractual onboarding decision | human owner (never an agent) | plan §4 E-decision discipline | signed onboarding record (external to the repo) |
| O-2 | Tenant registry record created | PlatformAdmin operator | `tenant.create` grant; the registry surface | tenant UUID; the creation is an audit-record fact |
| O-3 | Jurisdiction + residency + legal basis captured | tenant admin + legal (human sign-off) | record 26 (the jurisdiction registry; legal sign-off is a HUMAN GATE — LR-062 D1 stays OPEN) | the per-tenant residency/legal-basis rows, signed off |
| O-4 | Per-tenant SSO connection registered | tenant admin (IdP side) + operator | record 01 §4 (SSO connections are TENANT DATA, not platform config); record 02 (issuer→tenant binding) | the issuer→tenant binding row; a first lab-token mapping check (P13.20-gated, UNRUN) |
| O-5 | SCIM connection + group→role mapping table | tenant admin (IdP side) + operator | record 03 §3 (the SINGLE mapping vocabulary consumed by SCIM and login) | the mapping table (group → role id, optional scope subset); the mapping is tenant data |
| O-6 | Retention matrix rows for the tenant's jurisdiction | operator + legal | records 25 §matrix + 26 (per-jurisdiction values over the seven P1.10 classes — never a universal default) | the per-class values/bases, jurisdiction-qualified |
| O-7 | Notification preferences + webhook endpoints | tenant admin | record 23 (per-tenant severity floors; EMAIL default per the P13-E-2 DEFAULT), records 21/08 (endpoint registration; the secret shown ONCE, the row carries a fingerprint) | preference rows; endpoint rows with fingerprints; `whsec_` material displayed once and never stored retrievable |
| O-8 | API keys (only if the tenant needs the simple tier) | tenant admin (`apikey.create`, tenant scope) | record 08 §2 (key id + digest-at-rest + scope subset) | key id; scopes; the secret displayed once |
| O-9 | RLS isolation verification for the new tenant | operator | [S-70]; the migration-0008/0020 `tenant_isolation` policy | a negative check: a tenant-scoped read for tenant A returns no tenant-B rows through the app ports (the check result is a test OUTCOME, never a guarantee — §13.3 semantics) |
| O-10 | First authenticated login + role sanity check | tenant admin | records 02/03 (the role vocabulary is ONE set in both paths) | the resolved `Caller` (tenant, subject, role) recorded in the audit trail |

Steps O-2..O-10 are written-unrun: no tenant has ever passed this
checklist, and no step has ever been executed by this program.

## 3. The offboarding / decommission procedure (in order; revocation FIRST, data LAST)

The order is deliberate: cut the access surfaces before touching data, so
an offboarded tenant cannot authenticate during or after the data phase.
Every step is audit-recorded; the audit trail itself is NEVER deleted
(append-only; the P11.15 chain inherits when keyed).

| # | Step | Actor | Consumed seam / record | Honest state |
|---|---|---|---|---|
| D-1 | Offboarding decision recorded | human owner | — | external decision; never an agent act |
| D-2 | Revoke all API keys of the tenant | PlatformAdmin/TenantAdmin | record 08 §2 (revocation is immediate: state → `revoked`, the resolver consults on every use) | procedure text; no key has ever been revoked |
| D-3 | Revoke all active sessions of the tenant | PlatformAdmin | record 42 (P13.16: revocation denies at the RESOLVER — every session of the tenant is revoked, and the next request 401s regardless of token validity) | procedure text; no session has ever been revoked |
| D-4 | Disable webhook endpoints | PlatformAdmin/TenantAdmin | record 21 (endpoint `active` flag; delivery stops; the DLQ is NOT auto-purged — its remaining depth is a metric) | procedure text |
| D-5 | Deactivate the SCIM connection and SSO connection | PlatformAdmin | records 01/03 (the connections are tenant data; deactivation means the issuer→tenant binding no longer resolves — a still-valid token for that issuer REJECTS fail-closed, record 02 §5) | procedure text |
| D-6 | Final data export (the tenant's own evidence) | operator | §4 (the export-and-reconcile design) + record 24 (the signed export pack format) + record 25 (DSAR export job design) | procedure text; no export ever produced |
| D-7 | Data disposition per the retention matrix | operator + legal | records 25/26 (per-class, per-jurisdiction: erase / retain-for-legal-hold / expire-naturally). Erasure follows record 25's strategy INCLUDING the backup re-materialization caveat and the derived-store (ML cache) cryptographic-erasure path | procedure text; D1 OPEN — no regime is asserted |
| D-8 | Tenant registry record deactivated/deleted | PlatformAdmin (`tenant.delete`) | the registry surface; the P2.02 RESTRICT rule applies to dependent records | procedure text; no tenant has ever been deleted |
| D-9 | Audit evidence PRESERVED | operator | record 24 (the signed export pack is the offboarded tenant's evidence outside the live system; the in-place `audit_records` rows are never deleted — the append-only trail outlives the tenant) | procedure text |

The never-a-dispatch-path rule and the never-granted permission set are
unaffected: offboarding grants and revokes nothing outside the closed
`Action` surface.

## 4. Tenant data export and reconciliation design (under RLS)

The export is a READ-ONLY job design. It introduces no new data path:
every read goes through the existing tenant-scoped ports, and RLS is the
outer gate [S-70].

| Element | Design |
|---|---|
| Scoping | the export job opens a tenant-scoped context (the same single claims path — `AuthenticatedClaims::from_verified` — never a raw tenant parameter on a privileged connection) and reads through the existing SELECT ports. The transaction-scoped RLS discipline (`SET LOCAL` per transaction, the gap-analysis §3 RLS row [S-69][S-70]) is the posture the job inherits; the pooling-interaction caveat [S-69] is RECORDED, not resolved: the job MUST run in the same transaction-scoped pattern the adapters use, and its first run is the human-gated verification of that pattern |
| Content | the tenant's registry rows, telemetry-derived records, command/feasibility evidence, audit events, webhook bookkeeping, notification preferences — inventoried per record 25's data inventory. Secrets are NEVER exported as material: API keys are exported as key ids + fingerprints, webhook secrets as fingerprints (record 08 §4 display-never-secret) |
| Format | the record 24 export-pack shape (SIEM-friendly JSONL + signed manifest) is REUSED for the tenant export — one export mechanism, two entry points (the audit export and the tenant export), never a second pack format |
| Reconciliation | after export, a reconciliation pass compares, per inventoried table: source row count (tenant-scoped SELECT COUNT through the port) vs. exported row count, plus a per-table digest of a deterministic canonical serialization. A mismatch is a FAILED reconciliation — the export is not marked complete, and the failure is an audit-record fact. Reconciliation proves COVERAGE of the export, never correctness of the data (a wrong-but-complete export reconciles) — the honesty line the record states |
| Execution gates | the export run, the reconciliation result, and any erasure (D-7) are HUMAN-GATED acts (conventions §5): every result cell (row counts, digests, mismatch findings) is EMPTY/UNRUN in this program |

## 5. Specified-not-implemented register (plan risk 6 discipline)

| Need the procedure names | State |
|---|---|
| A tenant-scoped bulk-export REST endpoint (`GET /api/v1/tenants/{id}/export` or equivalent) | does NOT exist — the export runs as a designed job over the existing SELECT ports; the endpoint is specified-not-implemented and P13.18 renders it as such |
| A session-revocation REST endpoint (D-3) | does NOT exist — P13.16 designs the resolver-side semantics; the management endpoint is specified-not-implemented |
| A webhook-endpoint management endpoint (D-4) | does NOT exist (record 21 §5) — specified-not-implemented (P13.18 renders it as such) |
| An SCIM service (O-5/D-5) | does NOT exist (record 03 §5) — lab-gated on P13.20 |
| A bulk erasure executor (D-7) | does NOT exist — record 25 designs the strategy; execution is a human-gated deployment act |

## 6. ASSUMPTION / UNRUN / EMPTY register

1. **UNRUN** — no tenant onboarded, no checklist step executed, no
   offboarding performed, no export produced, no reconciliation run, no
   RLS verification executed against a live tenant split. All evidence
   cells EMPTY.
2. **ASSUMPTION** — the transaction-scoped `SET LOCAL` RLS pattern the
   export job inherits is the pattern the existing adapters use (the gap
   analysis §3 RLS row [S-69][S-70] records the pooling caveat as
   UNAUDITED); the first real export run is its verification gate.
3. **RECORDED LIMITATION** — reconciliation proves coverage, not data
   correctness (§4); and the RLS negative check (O-9) is a finite test
   outcome, never an isolation guarantee (conventions §5 §13.3
   semantics).
4. **OPEN** — LR-062 / D1: no retention value or legal basis is asserted
   here; the checklist captures them as data with human legal sign-off.

## 7. Handoff notes

1. **P13.18 (record 44)**: the Administration UI renders the onboarding
   checklist as procedure TEXT and the export action as
   specified-not-implemented (§5) — no write-path fabrication.
2. **P13.19 (record 45)**: no test file is generated by this subtask
   (design-only); the RLS posture is asserted by the existing adapter
   tests, which this record does not touch.
3. **P13.22 (close)**: the export mechanism reuses record 24's pack
   format (one mechanism, two entry points) — the consistency audit
   should check that statement against record 24.
