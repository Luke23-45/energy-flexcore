<!-- P9.10 — Data residency, retention, and privacy design (design record). TEMPLATE only: every jurisdiction/contract/period value EMPTY. No pilot exists; no tenancy test run; no log inspection run; no export/deletion executed. -->
# P9.10 — Data Residency, Retention, and Privacy Design (Design Template)

Date: 2026-11-14. Subtask: P9.10 (`ledger_phase_9.md` §P9.10).
Owner role: Product + legal (retention/residency); platform engineering
(tenancy test). Report trace: §23 Stage 9; §16.2–§16.3; §25.2 (D1).
Inputs: P9.01 freeze (TEMPLATE, UNSIGNED — `01_pilot_scope_freeze.md`);
D1 position (OPEN — §2 below); the customer contract and its applicable
privacy/security requirements — **which do not exist**; the market rules for
the region (P9.11 capture — TEMPLATE, no market contact,
`11_market_program_requirements.md`).

## 1. Honest status — the plain statement

**No pilot exists, no jurisdiction is recorded (D1 OPEN), no customer contract
exists, and no production data of any pilot customer has ever been stored.**
This record delivers the data-governance TEMPLATE that the real pilot
data-governance note must fill: the hosting/residency cells (§4), the
data inventory of what the platform actually stores today (§5, filled from
the real schema and edge code — the only filled part of this record),
the per-class retention schedule (§6, every period and basis cell EMPTY),
the privacy-obligations structure (§8), the tenancy-confirmation design with
the negative test WRITTEN-UNRUN (§9), and the export/deletion procedure as a
DESIGN with the execution gate recorded (§10).

The ledger's acceptance criteria for P9.10 are NOT met and are NOT claimed:
(a) every §16.3 data class has a row, but no retention period and no basis
can be recorded without the contract and market rules; (b) the tenancy
negative test is designed but UNRUN (there is no pilot environment to run it
in); (c) the log inspection is designed but UNRUN (there are no pilot logs);
(d) the export/deletion procedure is written as a design whose execution is a
human gate (ENTRY_NOTE boundary 1). The template's job is to make criteria
(a)–(d) mechanically checkable when a human fills and runs it — not to
simulate their satisfaction.

## 2. D1 position — carried OPEN with a DEFAULT/RECOMMENDATION label

D1 ("target jurisdiction and market") drives interconnection, the market
adapter, **data residency**, and liability (`.agent/DECISIONS.md`, read-only —
this record does not edit it). D1 is **OPEN** per ENTRY_NOTE boundary 2;
the pilot's jurisdiction cell is EMPTY until P9.01 §3.H is filled by the
human signatories.

| Field | Status |
|---|---|
| D1 status | **OPEN** — agent-undecided; the pilot's residency jurisdiction is `<EMPTY>` (`.agent/DECISIONS.md`; P9.01 §3.H) |
| RECOMMENDATION (DEFAULT label) | Residency, retention, and privacy obligations are treated as **deployment configuration per data class, never a universal default baked into code** — report §16.3 forbids a universal retention period in code, and the implemented storage already follows that rule (migration 0010 D-P5: "NO retention value, no TTL trigger, and no scheduled-delete rule appear anywhere in this DDL"; `crates/adapters/postgres/migrations/0010_telemetry_storage_p2_06.up.sql`) |
| What would change it | The D1 closure naming the jurisdiction; the customer contract clause(s) on residency/retention; the P9.11 capture of market retention rules — then §4/§6/§8 cells fill and the note is re-issued |
| Consequence while OPEN | No residency constraint can be stated; no retention period or basis can be recorded; §6 stays a template; P9.14 (lab integration) receives no retention configuration from this record |

## 3. Scope rule — a value enters this note only from a primary source

Every filled value in §4, §6, and §8 traces to exactly one of: the signed
P9.01 freeze, the customer contract, the applicable privacy/security
requirement for the D1-named jurisdiction, or the P9.11 capture from program
documentation. A value filled from a neighbouring jurisdiction's rules, from
an assumed privacy regime, or from this template's own structure is treated
as UNVERIFIED and the reviewer rejects it (§11, check 3).

## 4. Hosting mode and residency — ALL SELECTION CELLS EMPTY

Report §21.1 hosting modes (customer private cloud, customer on-premises,
hybrid) are the candidate modes; the selection is the deployment's, not a
default. Cells stay EMPTY until the signed freeze and contract name them.

| Field | Value |
|---|---|
| Hosting mode (§21.1) | `<EMPTY — one of: customer private cloud / customer on-premises / hybrid>` |
| Deployment jurisdiction (D1) | `<EMPTY>` |
| Residency constraint the mode must satisfy | `<EMPTY — from the contract / applicable requirement, cited by clause>` |
| May central data (PostgreSQL, object storage, JetStream) leave the jurisdiction? | `<EMPTY>` |
| Edge-local data location (the edge host is AT the site — see §5 edge rows) | `<EMPTY — site address scope per signed freeze §3.B>` |
| Cross-border transfer/sub-processor constraints | `<EMPTY>` |

## 5. Data inventory — what the platform actually stores (filled, from the real schema and edge code)

This section is the one filled part of the record: it inventories real stores
by citation. It records what the code stores; it does NOT assert that any
pilot data exists. Canonical units throughout: MW, MVAr, MVA, MWh; SOC a
fraction in [0,1] at the API boundary (`asset_state.soc` CHECK 0..1,
migration 0006); `P > 0` = export at the PCC.

### 5.A Central PostgreSQL — `crates/adapters/postgres/migrations/`

| Store (migration) | Contents | Personal-data candidates | Tenancy |
|---|---|---|---|
| `tenants` (0002) | tenant name, description, `idp_reference` (customer IdP link) | `idp_reference` (an identity-provider reference), `name` | tenant ROOT; no RLS by design (the table IS the tenant root, P1.12 §5.2) |
| `portfolios` (0003), `sites` (0004), `assets` (0005) | registry rows; assets carry rated MW/MWh; NO SOC column (D-S5); NO reactive-power sign column (declared per grid model/device profile, never defaulted) | customer-identifying site/location data if filled | `tenant_id` + `tenant_isolation` RLS (ENABLE + FORCE) |
| `asset_state` (0006) | per-asset SOC [0,1], MW/MVAr, availability, operating_mode, health, event/ingest time | none expected | `tenant_id` + RLS |
| `outbox` (0007) | event payloads (jsonb) for §15.5 transactional outbox | payloads may embed actor refs | no tenant column (platform-level delivery records) |
| `audit_records` (0008) | append-only audit: actor, action, object refs, decision (value + reason reference), evidence jsonb, event/ingest time, correlation_id; corrections are NEW rows (`corrected_audit_id`), never edits | **`actor` text NOT NULL — operator/system identities**; decision/evidence payloads | `tenant_id` (NULL = platform-level, invisible to `flexcore_app`); RLS; UPDATE/DELETE refused by trigger + grants (no DELETE grant to ANY role) |
| `telemetry` (0010, extended 0012) | partitioned monthly (`telemetry_yYYYYmMM`, PARTITION BY RANGE(event_time)); one row per (asset_id, event_time, quantity); quality enum; `telemetry_quarantine`; corrections are new rows with `correction_reason`, `corrected_by`, `corrected_at` | **`corrected_by` on correction rows (0012)** | `tenant_id` + RLS; `flexcore_ingest`/`flexcore_query` NOLOGIN runtime roles, both under RLS, neither owns a table |
| `schedule_results` (0013), `schedule_revisions` (0014), `flexibility_envelopes` (0015) | optimization outputs, rolling-horizon schedules, portfolio envelopes | none expected | `tenant_id` + RLS |
| `dispatch_commands` + `dispatch_command_transitions` (0016) | command rows (immutable per tenant), closed 14-state §14.2 vocabulary, `valid_until_millis`, supersede links | approvals/actor identity if recorded in transition metadata | `tenant_id` in PK + RLS |
| `feasibility_check_records` (0017) | feasibility evidence: model id/version, results | none expected | `tenant_id` + RLS |
| `edge_device_registry` + `edge_enrollment_journal` (0018) | edge device identities: `device_id`, enrollment terms (ACTIVE/REVOKED/EXPIRED), `approved_asset_scope`; append-only lifecycle journal (ENROLLED/ROTATED/REVOKED/EXPIRED/REJECTED) with `actor` | **`device_id`, `actor`** — device and approver identity | `tenant_id` + RLS; at most ONE ACTIVE identity term per (device_id, tenant_id) via partial unique index; UPDATE/DELETE refused by trigger |
| user credentials | **no local user-credential store appears in the reviewed migrations** — §16.1/§17.3 place users/roles as references to the customer identity provider (OIDC/OAuth2); `tenants.idp_reference` is the only central identity link | n/a (by design) | n/a |

### 5.B Edge-local stores — `crates/edge-agent/src/`

| Store | Contents | Personal-data candidates | Notes |
|---|---|---|---|
| telemetry journal (`buffer_file.rs`, P6.09; path `FLEXCORE_EDGE_BUFFER_PATH`) | append-only CRC-checked store-and-forward journal of canonical samples; truncate-and-audit recovery; audited drop-of-OLDEST overflow; **age retention** is a configured bound | none expected (measurements + identities) | **encryption at rest is a recorded OPEN item (§18.3)** — the residency/privacy note must not assume it exists |
| audit journal (`audit_buffer.rs`, P6.18; magic `FLEXCORE-EDGE-AUDIT-JOURNAL`) | bounded append journal of audit records; in-order at-least-once upload on reconnect | **actor/subject fields carried in `BufferedAuditRecord`** | lives ON the edge host at the site — the edge rows are the residency-relevant stores for site-local data |
| command-id binding journal (`pilot_rejection_proof.rs`, P9.31) | restart-surviving idempotency bindings (command_id ↔ payload); refuses a conflicting payload under a used id | none expected | §14.3 edge-stand-in control |
| enrollment identity-lifecycle register (`enrollment_ceremony.rs`, P9.33) | append-only register of enrollment ceremony events; a revoked identity can never authorize its successor | **device identity, ceremony actors** | CRC-32 framing, truncate-and-audit |
| update/rollback register (`update_verification.rs`, P9.34) | append-only signed-update and rollback register | signer key references | never-silent downgrade rule |

### 5.C Stores that DO NOT EXIST yet (design only — no code cited, none claimed)

| §16.3 class | Store status |
|---|---|
| Aggregated intervals | No implemented store. (`flexibility_envelopes` holds forecast envelope intervals, not measured aggregates.) |
| Market records | No implemented store; existence and shape depend on P9.11/D1 (§2). |
| Simulation artifacts / exports / model files | §16.1 recommends S3-compatible object storage; no implementation is cited here. |
| Forecast outputs | No forecast table; `ForecastPublished` exists as a §15.3 event only. |
| NATS JetStream streams | §15.4 requires retention/limits defined **per stream**; stream configs are deployment artefacts, none recorded. |

## 6. Retention schedule — §16.3 TEMPLATE, EVERY PERIOD AND BASIS CELL EMPTY

Filling rule: each period cell is filled ONLY from a stated basis — the
customer contract clause, a market rule captured by P9.11, or an applicable
privacy/security requirement for the D1-named jurisdiction. A period with no
cited basis is rejected by the reviewer. **No universal retention period is
chosen here, and none may be baked into code (§16.3; migration 0010 D-P5).**
Where the basis is a market rule, that rule does not exist yet: it arrives
through the P9.11 capture (`11_market_program_requirements.md` §4).

| §16.3 data class | Store(s) today (§5 refs) | Retention period | Basis (cited) | Deletion mechanism (pre-printed where the design exists) |
|---|---|---|---|---|
| Raw high-frequency telemetry | `telemetry` monthly partitions; edge telemetry journal | `<EMPTY>` | `<EMPTY>` | central: detach-then-drop (or archive-first) of whole monthly partitions by the P2.20 maintenance worker under the migrator/maintenance role, one audit record per action (migration 0010 D-P5); edge: journal age retention bound, then audited drop-of-oldest (P6.09) |
| Normalized telemetry | `telemetry` (+ versioned corrections, 0012) | `<EMPTY>` | `<EMPTY>` | as above; a correction never mutates the original row — retention applies to ALL versions |
| Aggregated intervals | none implemented | `<EMPTY>` | `<EMPTY>` | `<EMPTY — store does not exist; define when/if implemented>` |
| Command/audit evidence | `audit_records`, `dispatch_commands`, `dispatch_command_transitions`, `feasibility_check_records`; edge audit journal, binding journal | `<EMPTY>` | `<EMPTY>` | **append-only by design**: no runtime DELETE path exists (0008/0018 triggers, grants); deletion is §10's human-gated procedure, never a runtime operation |
| Market records | none implemented | `<EMPTY>` | `<EMPTY — P9.11 capture feeds this cell>` | `<EMPTY>` |
| Simulation artifacts | §16.1 object storage (design) | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Model/forecast outputs | `schedule_results`/`schedule_revisions`/`flexibility_envelopes`; forecasts event-only | `<EMPTY>` | `<EMPTY>` | as the class's store dictates when implemented |
| Registry/lifecycle records | `tenants`, `portfolios`, `sites`, `assets`, `asset_state`, `edge_device_registry`, `edge_enrollment_journal` | `<EMPTY>` | `<EMPTY>` | registry tables have no Phase 1 delete path ("a future delete needs an explicit, reviewed migration", migration 0002); lifecycle journal is append-only (0018) |
| Edge-local journals (site host) | §5.B rows | `<EMPTY>` | `<EMPTY>` | configured age bound / capacity bound with audited drop-of-oldest; the VALUE is deployment configuration, `<EMPTY>` here |
| JetStream streams | §15.4 (design) | `<EMPTY>` | `<EMPTY>` | per-stream retention/limits, defined at deployment, never universal |

The ledger's named failure mode — "telemetry retained indefinitely because
no class was ever assigned a period" — is structurally gated by this table:
a retention configuration that cannot cite a row of §6 is not deployable
(§11, check 4).

## 7. Residency requirements — jurisdiction-dependent placeholders

The pilot's jurisdiction is EMPTY (D1 OPEN, §2). No privacy regime is named,
and none is presupposed: the template does not assume that any particular
data-protection law applies, nor that none does. The requirement rows below
are the structure a filled note must answer for whatever regime the
jurisdiction actually imposes.

| Requirement area | The filled note must state | Value |
|---|---|---|
| Processing location | which jurisdictions may hold/process central stores (§5.A) and edge stores (§5.B, at the site) | `<EMPTY>` |
| Applicable privacy/security regime(s) | named from the jurisdiction + contract, with the citation | `<EMPTY>` |
| Roles | controller/processor (or jurisdiction-equivalent) split between customer and Neryva, per contract | `<EMPTY>` |
| Personal-data register | the §8 table, completed for the named regime | `<EMPTY>` |
| Cross-border transfer | constraints on central-store location and support access | `<EMPTY>` |
| Breach notification | deadlines and recipients per the named regime | `<EMPTY>` |
| Subject/customer rights handling | access/erasure portability procedure mapping to §10 | `<EMPTY>` |
| Retention overrides | any regime rule that overrides contract periods (the ledger's named risk: a market rule requiring longer retention than the contract allows) | `<EMPTY>` |

## 8. Privacy obligations structure — what personal data could exist, and minimization by design

### 8.A Personal-data candidate inventory (from the real schemas, §5)

| Candidate | Where it appears | Why it may be personal | Handling rule |
|---|---|---|---|
| Operator/approver identity | `audit_records.actor` (0008); `edge_enrollment_journal.actor` (0018); `telemetry.corrected_by` (0012); decision/evidence payloads | a named person's actions | stored because audit requires it (§17.3 "audit logging for all control-plane and dispatch actions"); access scoped by RLS; retention per §6 row 4 — NOT deletable by runtime |
| Identity-provider references | `tenants.idp_reference` (0002) | links a tenant to a customer IdP subject | stored as a reference, never a credential; no local credential store (§5.A last row) |
| Edge device identity | `edge_device_registry.device_id`, enrollment register | device identity tied to a customer site | scoped to one tenant and approved site/asset set (§16.2); lifecycle is append-only |
| Customer/site identifiers | freeze §3.A/§3.B values propagated to `sites`, `assets` rows | customer-identifying data | filled only from the signed freeze; tenancy-scoped |
| Log-borne personal data | application and edge logs | anything operators put in messages | **minimization rule is pre-printed: §21.3 "Never put secrets or unnecessary personal/customer data in logs"**; verified by log inspection, UNRUN (§9.B) |

### 8.B Minimization by design (pre-printed; not fillable)

1. Tenant context is derived from authenticated claims/authorized context,
   never from untrusted request-body fields (§16.2) — personal identifiers
   are not accepted as scoping inputs.
2. Logs carry correlation IDs and object references, not free-text personal
   data or secrets (§21.3; §15.3 events "describe facts that occurred").
3. The platform stores measurement and identity data it needs to operate;
   it has no marketing/analytics store and adds none in the pilot.
4. Every personal-data candidate above exists because a required control
   (audit §17.3, tenancy §16.2, edge identity §16.2) needs it — the filled
   note must justify any NEW personal-data field against this list.

### 8.C Obligation owners — ALL CELLS EMPTY

| Obligation | Owner (named human) | Status |
|---|---|---|
| Privacy regime determination + controller/processor split | `<EMPTY>` | NOT-STARTED |
| Personal-data register maintenance | `<EMPTY>` | NOT-STARTED |
| Breach notification procedure | `<EMPTY>` | NOT-STARTED |
| Rights-handling procedure | `<EMPTY>` | NOT-STARTED |

## 9. Tenancy enforcement — what exists, and the negative test (WRITTEN-UNRUN)

### 9.A What the schema already enforces (filled, cited)

- `tenant_id` on every tenant-owned record (§16.2) — migrations 0002–0018.
- RLS as defence in depth: `ENABLE ROW LEVEL SECURITY` + `FORCE ROW LEVEL
  SECURITY` with the `tenant_isolation` policy keyed on
  `current_setting('flexcore.tenant_id', true)` (e.g. migrations 0008, 0010,
  0016, 0018).
- Application runtime roles do not own tenant tables and cannot bypass RLS
  (§16.2, [R25]): `flexcore_ingest`/`flexcore_query` are created NOLOGIN with
  no table ownership (migration 0010 D-P6); `flexcore_app` holds only
  INSERT/SELECT(/UPDATE where designed) grants, no DELETE on registry tables,
  INSERT/SELECT only on `audit_records` (migrations 0002, 0008). The table
  owner is `flexcore_migrator` — **the §16.2/[R25] owner-bypass warning
  stands: the deployment must never run the application as the table owner;
  this is a deployment requirement the filled note must verify against the
  running roles, not assume**.
- Cross-tenant access denials are audited (§16.2): isolation denials carry
  actor/object/timestamp in the edge audit record shape (P6.18/P6.21) and
  platform-side denials land in `audit_records`.

### 9.B What does NOT exist — the tests are UNRUN

| Check (ledger P9.10 verification method) | Status | Where it runs |
|---|---|---|
| Tenancy negative test: cross-tenant access denied AND audited | **UNRUN** — designed here; runs in the pilot environment (§12) | pilot deployment |
| Retention configuration inspected against §6 | **UNRUN** — nothing to inspect (no retention configuration exists while §6 is empty) | pilot deployment |
| Log sample inspected for secrets/unnecessary personal/customer data (§21.3) | **UNRUN** — no pilot logs exist | pilot deployment |

Designed test shape (WRITTEN-UNRUN; the human executes it in the pilot
environment and files the evidence named in `CHECKLIST.md`):

1. Provision two tenants A and B with assets under each.
2. As role `flexcore_query` with `flexcore.tenant_id` set to A, query B-scoped
   rows — expected: zero rows returned (RLS), and the attempt audited.
3. Attempt an UPDATE/DELETE on `audit_records` and on
   `edge_device_registry` — expected: refused by trigger and by grants.
4. Attempt the same under the table-owner role — expected: `audit_records`
   and `edge_device_registry` still refuse (trigger layer 2); tenant-scoped
   tables remain RLS-covered under FORCE even for the owner.
5. Record per-check pass/fail with the SQL transcript; a failure blocks
   P9.14.

## 10. Export/deletion procedure — DESIGN with the execution gate recorded

**Execution gate (pre-printed):** every step below is executed by humans in
the pilot environment, each action producing an audit record; this record
executes none of it and the procedure runs only after (i) the triggering
event exists (end of pilot, customer request, contract expiry, or a
regulatory request), (ii) §6 is filled with cited bases, and (iii) the D1
jurisdiction's obligations are named (§7). Until all three hold, the
procedure is DESIGN-ONLY by construction.

### 10.A Export (design)

| Step | Content | Open design point |
|---|---|---|
| E1 | Define export scope per tenant (registry, telemetry, schedules/commands, audit evidence, edge journals in scope) | `<EMPTY — scope per contract>` |
| E2 | Export format + integrity: audit exports carry the integrity anchor of the P9.32 audit-proof surface (`crates/edge-agent/src/audit_proof.rs` authority-review export bundle), so exported evidence is verifiable offline | format cell `<EMPTY>` |
| E3 | Authorization: export is an operator action, step-up-approved where the customer requires it (§17.3), and recorded in `audit_records` | approver roles `<EMPTY>` |
| E4 | Delivery and its residency constraint (§7 cross-border row) | `<EMPTY>` |

### 10.B Deletion (design) — the append-only conflict, stated honestly

Deletion is NOT a runtime operation in this codebase: `audit_records`,
`edge_device_registry`, `edge_enrollment_journal`, and `telemetry` have no
UPDATE/DELETE path for any runtime role (trigger + grants; migration 0002's
registry note: "a future delete needs an explicit, reviewed migration"). The
designed deletion procedure is therefore:

| Step | Content | Open design point |
|---|---|---|
| D1 | Enumerate the data classes to delete from §6 rows, with the cited basis for each deletion obligation | obligations `<EMPTY>` |
| D2 | Central partitioned telemetry: whole-month partition detach-then-drop (or archive-first) by the P2.20 maintenance worker under the migrator/maintenance role, one audit record per action (migration 0010 D-P5) | none — mechanism exists in the design |
| D3 | Append-only evidence tables: **legal erasure obligations can conflict with append-only audit evidence**. The designed resolution paths are (a) archive-then-withdraw-access (evidence retained, access revoked), (b) a reviewed, explicit migration authored for the specific deletion, or (c) pseudonymization of the personal-data fields within otherwise-retained evidence — WHICH path suffices is a question of the named jurisdiction's obligations and is `<EMPTY>` until D1 closes | carried as an open risk to P9.27 (§13) |
| D4 | Edge-local journals: configured age/capacity bounds drive audited drop; a manual purge at pilot end is a human edge-host action with an audit record | `<EMPTY — purge command per deployment>` |
| D5 | Verification: post-deletion inspection per class, recorded against §6 | procedure cell `<EMPTY>` |

## 11. Reviewer verification procedure

1. Confirm §1's plain statement is unchanged and no residency constraint,
   retention period, or privacy-regime claim exists anywhere in the record.
2. Confirm every `<EMPTY>` cell in §4, §6, §7, §8.C, §10 is still EMPTY and
   every §6 period cell has an EMPTY basis cell beside it.
3. Spot-check §5 citations against the named files: migration 0008's
   append-only trigger and grants, migration 0010's D-P5/D-P6 notes, the
   `corrected_by` columns of 0012, the partial unique index and append-only
   guard of 0018, and the edge journal headers in `buffer_file.rs` /
   `audit_buffer.rs`.
4. Confirm §9.B marks all three checks UNRUN and no "tenancy verified" or
   "logs inspected" claim exists.
5. Confirm §10's execution gate is intact and no deletion/export step is
   described as performed.
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the design is written; no tenancy test or inspection ran.

## 12. Filling commands (for the human executor)

- Fill §4/§7 after the D1 closure and from the customer contract (primary
  source, cited by clause); never from a neighbouring jurisdiction's rules.
- Fill §6 period+basis per class from the contract, the P9.11 capture
  (`11_market_program_requirements.md`), and the applicable requirements;
  where a market rule and the contract disagree, record BOTH and stop —
  the conflict is a human/legal decision, not a template fill.
- Fill §8.C with named owners; fill §10 open design points.
- Run the §9.B checks in the pilot environment; file the transcript as the
  P9.10 evidence named in `CHECKLIST.md` ("Data governance defined").
- Re-issue this note (version bump + revision note) when D1, the contract,
  or the P9.11 capture changes any basis.
- Downstream unblock order: this record feeds P9.14 (lab integration) and
  P9.24 (backup/restore rehearsal takes the P9.10 retention design as an
  input — `ledger_phase_9.md` §P9.24).

## 13. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| D1 OPEN — no jurisdiction, therefore no residency constraint, no named privacy regime, and no basis for any retention period | §2, §4, §7; `.agent/DECISIONS.md` |
| A market rule requiring longer retention than the contract allows, discovered after data was deleted | ledger P9.10 risk row; §7 last row; the conflict-stop rule in §12 |
| RLS relied on while the application runs as table owner — a deployment misconfiguration the schema does not prevent | §9.A; §16.2, [R25] |
| Telemetry retained indefinitely because a class was never assigned a period | ledger P9.10 risk row; gated by the §6 no-basis-no-deploy rule, but the rule itself is unenforced until a human reviews it |
| Edge telemetry journal encryption at rest is a recorded OPEN item — site-local residency cannot assume confidentiality at rest | §5.B; §18.3; `crates/edge-agent/src/lib.rs` module note |
| Append-only audit evidence vs erasure obligations — the resolution path is jurisdiction-dependent and EMPTY | §10.B step D3 |
| Tenancy negative test, retention inspection, and log inspection all UNRUN — the pilot inherits them as open verification debt | §9.B |

Status: **WRITTEN-UNRUN** — a design template cannot be "run"; the only
executable steps (the §9.B checks, the log inspection, any §10 execution)
have not happened, cannot happen without a pilot deployment, and are not
scheduled. No pilot data exists and none is claimed.
