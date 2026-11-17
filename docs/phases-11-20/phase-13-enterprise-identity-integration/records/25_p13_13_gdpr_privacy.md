# P13.13 — GDPR/privacy engineering (data inventory, per-jurisdiction retention matrix, DSAR export design, erasure incl. backups, event streams, ML caches)

| Field | Value |
|---|---|
| Subtask | P13.13 — data inventory across the workspace, per-table retention matrix over the P1.10 seven data classes (per jurisdiction — no universal default in code), DSAR export job design, erasure strategy including backups (documented re-materialization caveat), event streams (bounded retention plus erasure events), ML caches (cryptographic erasure) |
| Wave / agent | Phase 13 wave 1, Agent B |
| Status | WRITTEN-UNRUN — an engineering record. NO DSAR has ever been exercised, no erasure has ever run, no export job exists, no jurisdiction is named (D1 OPEN, LR-062 direction). Every retention VALUE and legal-basis cell below is EMPTY by design; the matrix is the mechanical structure a jurisdiction decision fills. |
| Sources | [S-86] (GDPR Art. 17 — right to erasure; the ARTICLE FRAME is cited as the reference the regime determination will name, never asserted as applicable: no jurisdiction is recorded, so no regime is claimed to apply — the P9.10 scope rule, `ledger/phase-09-pilot-qualification/10_data_residency_retention.md` §3); [S-87] (GDPR erasure exceptions and limits — same conditional status); [S-88] (immutable/hash-chained audit logs — the append-only-vs-erasure conflict's reference); `crates/flexcore-storage/src/retention.rs` (the P1.10 seven data classes, `Unset` semantics — consumed, never edited); `ledger/phase-09-pilot-qualification/10_data_residency_retention.md` (the P9.10 record — the §5 inventory base and the §10.B append-only-vs-erasure analysis, consumed never rewritten); record `26_p13_14_jurisdiction_registry.md` (THE KEY this matrix fills against — P13.14 → P13.13, plan §2.1) |
| Files touched | This record only (no code, no migration — the DSAR job is a design; the retention matrix is registry data, §2) |

## 1. The data inventory — what the workspace actually stores

The inventory is BY CITATION (one source of truth per store; this record
adds only the NEW Phase 13 stores and the cross-cutting surfaces). The
central/edge inventory base is the P9.10 record §5 (its rows are the
authoritative mapping of migrations 0002–0018 and the edge journals —
re-listing them here would fork the inventory). This record registers:

### 1.A The Phase 13 additions to the inventory (this wave's own stores)

| Store | Contents | Personal-data candidates | Data class (P1.10) |
|---|---|---|---|
| `webhook_endpoints` (migration 0020) | delivery URL, opaque signing secrets (current + rolling previous), active flag | the URL (customer endpoint); the secrets are credentials-adjacent material (never personal data itself) | none of the seven exactly — bookkeeping/configuration; the closest class is registry/lifecycle records (P9.10 §6 last row's treatment) |
| `webhook_deliveries` (0020) | delivery key, event subject, RAW body, status machine | event payload text may embed actor/subject references | as the payload's event class dictates (a `DispatchFailed` payload is command/audit evidence adjacent) |
| `webhook_delivery_attempts` (0020) | attempt bookkeeping: outcome, response code, failure class | none expected (closed failure classes, never payloads) | none — bookkeeping |
| `webhook_dead_letters` (0020) | exhausted deliveries with reasons | as `webhook_deliveries` | as the payload's class |
| The P13.09 signing secrets (`secret_current`/`secret_previous`) | opaque secret material (P13.08's format — the reconciliation item of record `21_p13_09_webhook_service.md` §2) | NOT personal data; credential material under the P11.16 posture | n/a |
| The P13.11 notification pipeline (`notifications.rs`) | templates, preferences (severity floors per channel), receipts (message ids) | an email RECIPIENT (an operator contact) travels through the email port at send time; the in-module fake stores none; a real composition's persistence is a NEW store that must be added to THIS inventory row when it exists (none exists) | none — configuration until proven otherwise |
| The P13.12 export packs (`audit_export.rs`) | derived JSONL copies of audit records + manifests | DERIVED PERSONAL DATA: a pack contains every personal-data candidate the source records contain (actor identities, references) — a pack is a COPY that inherits the source row's retention/erasure obligations (§4) | inherits the source class (command/audit evidence) |
| The P13.14 jurisdiction registry (record 26 §2 draft shape) | per-tenant residency + legal-basis capture, sign-off state | the signatory name (`signed_off_by` — an identity reference) | registry/lifecycle records |

### 1.B The cross-cutting surfaces (verified, cited)

| Surface | Personal-data candidates | Source |
|---|---|---|
| `TenantContext` / `AuthenticatedClaims` | tenant id + subject (the authenticated principal, P0.16 rule 5 — from verified claims, never request bodies) | `crates/flexcore-domain/src/tenant_context.rs` |
| `audit_records.actor` and every audit actor field | operator/system identities | `crates/flexcore-audit/src/record.rs`; migration 0008 (the P9.10 §8.A row 1 analysis stands) |
| Application logs | anything operators put in messages — the minimization rule (§21.3: no secrets, no unnecessary personal/customer data; correlation ids and object references only) is pre-printed and the inspection remains UNRUN | P9.10 §8.A last row; phase-11 log schema record 07 |
| Backups | EVERYTHING (a backup is a full copy) | the Phase 12 backup architecture (record `../../phase-12-backup-dr-resilience/records/01_p12_01_backup_architecture.md`) — consumed, never re-decided |
| JetStream streams | event payloads (§15.3 facts; payload-embedded actor refs) | `crates/flexcore-events/`; the P9.10 §5.C stream row |

## 2. The per-table retention matrix — per jurisdiction, keyed on the P13.14 registry

THE RULE (P0.16 §4 / report §16.3 / the P1.10 type's header, binding): **no
universal retention period in code — ever.** The matrix below is therefore
not a schedule; it is the MAPPING from stores to (data class, registry
key), with the VALUE cells EMPTY until the P13.14 jurisdiction registry
carries a SIGNED row for the tenant's jurisdiction (record 26 §2: an
UNSIGNED row authorizes no retention action; an absent row is `UNSET` —
never "keep forever", never a default).

| Store (migration) | P1.10 data class | Central / edge | Erasure mechanism that exists today | Retention value (per jurisdiction) | Basis (cited, per jurisdiction) |
|---|---|---|---|---|---|
| `telemetry` monthly partitions (0010/0012) | raw high-frequency telemetry; normalized telemetry (corrections included — ALL versions) | central | partition detach-then-drop by the P2.20 maintenance worker (0010 D-P5); edge journal age bound (P6.09) | EMPTY | EMPTY |
| `audit_records` (0008, chain 0019) | command/audit evidence | central | NONE (append-only: trigger + grants; deletion is the §4 procedure, never runtime) | EMPTY | EMPTY |
| `dispatch_commands` + transitions (0016) | command/audit evidence | central | NONE (append-only) | EMPTY | EMPTY |
| `feasibility_check_records` (0017) | command/audit evidence | central | NONE (append-only) | EMPTY | EMPTY |
| `edge_device_registry` + `edge_enrollment_journal` (0018) | registry/lifecycle (the P9.10 §6 row-8 treatment) | central | NONE (append-only lifecycle) | EMPTY | EMPTY |
| `tenants`/`portfolios`/`sites`/`assets`/`asset_state` (0002–0006) | registry/lifecycle | central | NO delete path ("a future delete needs an explicit, reviewed migration" — 0002) | EMPTY | EMPTY |
| `outbox` (0007) | n/a (platform delivery bookkeeping) | central | none designed | EMPTY | EMPTY |
| `webhook_*` (0020 — this wave) | registry/lifecycle + bookkeeping | central | none designed (configuration rows are UPDATE-able; delivery history is bookkeeping) | EMPTY | EMPTY |
| JetStream streams (§15.4) | per-payload class | central | per-stream retention/limits AT DEPLOYMENT — none recorded | EMPTY | EMPTY |
| Edge telemetry journal (`buffer_file.rs`) | raw high-frequency telemetry | edge (site host) | configured age bound + audited drop-of-oldest; VALUE is deployment config; encryption at rest is a recorded OPEN item (§18.3) | EMPTY | EMPTY |
| Edge audit/binding/enrollment/update journals (P6.18/P9.31–P9.34) | command/audit evidence | edge (site host) | bounded append journals; audited drop; manual purge = human edge-host action | EMPTY | EMPTY |
| Backups (Phase 12 architecture) | inherits the source classes | backup target | restore-time; see §4 | EMPTY | EMPTY |
| ML/forecast caches (§3.E) | model/forecast outputs | central (design; no store exists) | cryptographic erasure (§5) | EMPTY | EMPTY |
| Aggregated intervals / market records / simulation artifacts | their classes | none implemented (P9.10 §5.C) | define when/if implemented | EMPTY | EMPTY |

The deployability rule (the P9.10 §11 check-4 discipline, restated as data):
a retention configuration that cannot cite a matrix row's basis is NOT
deployable, and a matrix row whose registry key has no SIGNED
jurisdiction row resolves to `UNSET` — which authorizes NO retention
action (never a default action).

## 3. DSAR (Data Subject Access Request) export job — DESIGN

A DSAR is an erasure/access exercise for a DATA SUBJECT (a person:
typically the `actor` identities in audit records and operator contacts).
No DSAR has ever been exercised; the job is design-only:

| Step | Design | Status |
|---|---|---|
| DS1 | Subject identification: resolve the subject's identity references across the inventory §1 stores — the `actor` text fields (audit_records, edge journals), `telemetry.corrected_by`, notification recipients, tenant registry rows. Identity resolution is a REVIEWED OPERATOR step (the actor string space is deployment-defined; no automatic person-matching is claimed) | DESIGN; mechanism UNBUILT |
| DS2 | Scoping: per store, resolve the store's retention/erasure obligations through the P13.14 registry for the subject's tenant(s) (SIGNED rows only — an UNSIGNED registry row blocks the obligation determination, fail closed, record 26 §4) | DESIGN |
| DS3 | Export job: the P13.12 pack builder is the export mechanism — a DSAR export is a pack (or pack set) scoped to the subject's tenant and the subject's identity filter. The CURRENT builder is range-scoped, not subject-scoped: the subject filter is the recorded extension (a `subject`-keyed walk over the audit port's `actor` filter — `AuditEventFilter.actor` exists; the job composes it) | DESIGN; job UNBUILT; no DSAR exercised |
| DS4 | Delivery: per the residency constraint (registry `cross_border_transfer` row) and the operator procedure (P9.10 §10.A E3–E4 — the approver/step-up/delivery cells are the human's to fill) | DESIGN |
| DS5 | Evidence: the DSAR exercise itself is an audited action (the DSAR's own `audit_records` trail: who requested, who approved, what was exported, when) | DESIGN |

## 4. Erasure strategy — including the backup re-materialization caveat

The P9.10 §10.B analysis (append-only evidence vs erasure obligations; the
three resolution paths a jurisdiction's obligations may select between) is
CONSUMED VERBATIM — this record does not re-litigate it. What this record
adds is the engineering strategy around it:

| Surface | Strategy | Honest caveat |
|---|---|---|
| Central mutable bookkeeping (`webhook_*` 0020, notification config) | Row-level deletion is AVAILABLE (UPDATE grants exist) and an erasure job MAY delete — but no erasure code exists in this batch; the job is design | A deletion is itself audited (one audit record per action, the 0010 D-P5 pattern) |
| Central append-only evidence (audit_records, commands, feasibility, edge registry/journal) | The P9.10 §10.B D3 paths: (a) archive-then-withdraw-access, (b) a reviewed explicit migration authored for the specific deletion, (c) pseudonymization of the personal-data fields within otherwise-retained evidence. WHICH path suffices is a jurisdiction-obligation question — EMPTY until D1 closes. Pseudonymization (replacing the actor text with an opaque token, mapping held outside the store) is the path this record RECOMMENDS for feasibility: it preserves the chain (a re-written digest would break the P11.15 chain — any row edit is a chain-breaking event that must be recorded as such) | [S-88]: an immutable/hash-chained log is fundamentally in tension with in-place erasure; the chain upgrade makes path (c) the only path that preserves verifiability. No pseudonymization code exists |
| Backups — THE RE-MATERIALIZATION CAVEAT (the subtask's named caveat, stated plainly) | **Erasure does not reach backups.** A backup taken before an erasure CONTAINS the erased data, and a RESTORE RE-MATERIALIZES it — the erased rows return. The Phase 12 backup architecture (record 01) is consumed: its retention/restore design does not currently include a post-erasure purge mechanism. The honest strategy register: (1) erasure procedures MUST record the backup coverage window (which backups contain the subject's data); (2) restores within a window containing erased subjects must RE-APPLY the erasure (a post-restore erasure sweep — design-only, unbuilt); (3) the registry's jurisdiction rows may carry backup-retention constraints (record 26 §3, Phase 12 row) — EMPTY today | This caveat is a REAL gap, not a resolved risk: no backup-purge mechanism exists anywhere in the workspace |
| Event streams (JetStream) | Bounded retention per stream (per-stream limits at deployment — §15.4, never universal) + erasure AS EVENTS: an erasure is recorded as a NEW event/audit row (an erasure fact), and stream consumers treat the erasure event as the instruction to drop/overwrite subject-bearing payloads where the stream's retention model allows. JetStream per-message deletion capability is NOT verified ([S-65]-adjacent — UNVERIFIED-AT-A-DISTANCE: the broker's deletion semantics were not fetched this batch); the design assumes bounded retention eventually expires subject-bearing payloads and records the erasure fact immediately | UNVERIFIED broker behavior; bounded retention VALUES are EMPTY (registry) |
| ML/forecast caches — CRYPTOGRAPHIC ERASURE | No ML cache store exists yet (P9.10 §5.C: no forecast table; ForecastPublished is event-only). The DESIGN for when one exists: derived/cached personal data that cannot be row-erased (denormalized copies, embeddings, aggregates) is stored ENCRYPTED under per-tenant (or per-cache-generation) key material; erasure DESTROYS THE KEY MATERIAL — the ciphertext remains but is unrecoverable ([S-87]-adjacent practice; the term is defined in the phase-13 context glossary). The key management is the P11.16 posture + the P11.15 key-handling honesty (deployment-supplied, never agent-generated). A key-destruction event is itself audited | No cache store exists; the strategy is DESIGN-ONLY and claims nothing about any running system |

## 5. What this record does NOT do (honest limits)

1. No jurisdiction is named and NO REGIME IS CLAIMED TO APPLY: [S-86]
   (Art. 17) and [S-87] are cited as the REFERENCE ARTICLES the eventual
   regime determination will name — citing them here does not assert GDPR
   applicability to any deployment (the P9.10 §3 scope rule governs; D1 is
   OPEN).
2. Every retention value and basis cell is EMPTY; the matrix (§2) is
   structure, not schedule.
3. No DSAR was exercised; no erasure ran; no export job exists; no
   pseudonymization code exists; no post-restore sweep exists; no ML cache
   exists. All DESIGN.
4. The P9.10 record is consumed, never rewritten; the P1.10
   `retention.rs` type is consumed, never edited (no default added, no
   class renamed).
5. The never-granted permission set stays byte-identical — privacy
   engineering adds no authority: `dispatch.authorize`, `dispatch.issue`,
   `secret.read`, `network.any` (quoted unchanged per conventions §6.4).

## 6. Reviewer verification

1. Confirm §2's value/basis cells are all EMPTY and no retention period
   appears anywhere in this record.
2. Confirm §4's re-materialization caveat is present and no backup-purge
   capability is claimed.
3. Confirm §5's limits hold (no regime asserted, no code changed).
4. Confirm the P9.10 record and `retention.rs` are byte-identical
   (`git diff` on both).
