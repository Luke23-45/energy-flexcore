# P4.30 — Audit, events, persistence, observability (§15.2–§15.5, §16.1–§16.3, §21.3)

Status: **implemented, awaiting human build/test**. Nothing was built, run,
installed, or executed in this subtask (loop bans hold). All migrations are
written but NEVER APPLIED; all tests are written but NEVER EXECUTED (the
DB-gated suite is env-gated on `FLEXCORE_TEST_DATABASE_URL` per the recorded
harness pattern and skips cleanly when unset). Every acceptance criterion
below is therefore implemented-and-unverified, not passed.

## 1. What was wired

### 1.1 Persistence (step 1; §16.1–§16.2)

Already persisted since P4.10 (NOT rebuilt here): migration 0016
`dispatch_commands` + `dispatch_command_transitions` — the command records
and the append-only state-change journal, whose rows carry the
validation/feasibility/authorization EVIDENCE REFERENCES (the three gates
are distinct, §14.1), actor, timestamp, reason, and gate name. RLS
ENABLE+FORCE, `tenant_id` on every row, INSERT/SELECT grants plus the
column-level transition UPDATE grant.

**Added — migration 0017** (`0017_feasibility_checks_p4_30.{up,down}.sql`):
one new append-only, tenant-owned table.

- `feasibility_check_records` — one immutable row per completed §12.5
  feasibility check: `check_id` (the attempt id or the pipeline-level
  `gate:<candidate>` id — the recorded `FeasibilityCheckCompleted`
  check-id convention of the P4.37 contract; TEXT, not a UUID), nullable
  `command_id` (a candidate check may precede command creation) with a
  composite tenant-scoped FK to `dispatch_commands`, portfolio/site/asset
  + schedule/revision composite FKs, `model_id`/`model_version`/
  `model_content_hash`, closed outcome vocabulary (`PASSED` |
  `LIMIT_VIOLATED` | `FAIL_CLOSED`), verdict-shape CHECKs (PASSED names no
  failure and binds THE MODEL HASH, §17.5 invariant 3; LIMIT_VIOLATED
  names the constraint; FAIL_CLOSED names the reason), margins/coverage
  summaries, actor, `correlation_id`, `checked_at_unix_millis`.
  Immutability by three layers (the 0008 pattern): INSERT+SELECT grants
  only for `flexcore_app`, an unconditional BEFORE UPDATE OR DELETE
  trigger, and no update path anywhere in the store contract. RLS
  ENABLE+FORCE with the shared `tenant_isolation` policy; owner
  `flexcore_migrator`. No retention value, no TTL (P0.16 §4).
- Deliberately NOT added — a second authorization-decision table
  (**D-P4.30-2**, DEFAULT): the authorization evidence already persists in
  two places — the APPROVED/REJECTED journal rows (0016) carry the
  decision reference, the acting identity, and the instant, and the
  immutable §14.3 command payload carries the authorization
  scope/limits/expiry fields (§14.3 field 14). A third table would need a
  new write path through seams whose fakes are shared with the recorded
  rejection suites; the journal-is-the-audit-record reading satisfies
  §16.1. Flagged for Phase 5 review if a standalone authorization ledger
  is ever required.
- Audit events (step 1 "audit events") persist in the outbox (migration
  0007) written in the SAME transaction as the state change (§15.5) — no
  new table.

### 1.2 Storage contracts + PostgreSQL implementations (§16.1)

- **flexcore-storage** (NEW module `feasibility_check.rs`,
  `FeasibilityCheckStore` trait + `FeasibilityCheckRow` +
  `FeasibilityCheckFilter` + `FeasibilityCheckStoreError`): append-only
  record contract; the validated constructor enforces the closed outcome
  vocabulary and the verdict-shape rules BEFORE anything is written (the
  database CHECKs are defence in depth); adapter obligations on the trait
  (one unit of work, tenant-scoped reads, opaque backend errors). Same
  recorded home deviation as `schedule_store` (D-P3.21-4). Registered
  additively in the crate lib.
- **flexcore-storage** (`command_store.rs`, additive): new
  `CommandListFilter` + `CommandRecordStore::list_commands` (newest-first
  keyset over the UUIDv7 `command_id`, optional closed-vocabulary state
  filter, caller-bounded page) — the §15.2 listing surface's store method.
  The only implementor of the trait is the Postgres adapter (verified by
  grep), so the trait extension breaks nothing.
- **crates/adapters/postgres** (NEW module `feasibility_check.rs`):
  `FeasibilityCheckStore for PgUnitOfWork` on the exact command-store
  pattern — one native transaction per unit of work, bound parameters
  everywhere, explicit tenant filter on top of RLS, decode through the row
  constructor (fail closed). `command_store.rs` gains the `list_commands`
  implementation (parameterized dynamic WHERE; the limit is ALWAYS
  applied).

### 1.3 Event emission (step 2; §15.3, §15.5)

**Coverage result (frozen payload set, flexcore-events):** all 26 frozen
§15.3 payloads ALREADY EXISTED — every `Dispatch*` payload
(Proposed/Approved/Rejected/Issued/Acknowledged/TrackingUpdated/Completed/
Failed/Expired/Cancelled/Superseded/FallbackActivated) plus
`FeasibilityCheckCompleted` and `ScheduleValidated`. ZERO payloads were
added; the frozen set is untouched (the P0.14 freeze holds).

**Emission sites verified (all pre-existing):** `DispatchProposed` +
`DispatchRejected` (validation/feasibility/authorization rejections) +
`DispatchApproved` (command_gates); `DispatchIssued`, `DispatchAcknowledged`,
`DispatchTrackingUpdated`, `DispatchCompleted`, `DispatchFailed`,
`DispatchExpired` (dispatch_simulator); `DispatchExpired`,
`DispatchCancelled`, `DispatchSuperseded` (command_expiry);
`DispatchFallbackActivated` (fallback_policy); `ScheduleProposed` +
`ScheduleValidated` (the Phase 3 REST composition, routes_phase3 —
asset/time-coupled validation ONLY, the recorded caveat).

**Gap filled (D-P4.30-1, DEFAULT):** `FeasibilityCheckCompleted` was never
emitted anywhere. It is now emitted in `command_gates::run_feasibility_gate`
for EVERY verdict the §12.5 runner completes — pass, limit-violated, and
fail-closed (a fail-closed verdict is still a completed check; the §12.5
closing rule makes it never a pass) — through the transactional outbox,
paired with the feasibility transition's store receipt. The
source-revision-usability pre-check rejection emits NOTHING extra (no check
ran; `DispatchRejected` is the fact of record). Recorded choices:

- the event's `correlation_id` is the COMMAND id (the deterministic,
  replay-safe convention of this module's events; the outbox key is
  `feasibility_check_completed:<command uuid>` — the gate admits at most
  ONE runner-verdict check per command, so the key is unique per event
  instance);
- the frozen payload carries identity + time only (recorded payload-fit
  gap, the P3.12/P3.21 convention): the verdict detail travels in the
  journal's feasibility evidence reference and the migration-0017 record;
- one receipt descriptor may pair with more than one event of the SAME
  state change (the recorded one-receipt-per-event reading of the P3.28
  routes) — on a rejecting verdict `DispatchRejected` and
  `FeasibilityCheckCompleted` share the transition receipt.

**Additive consequence for existing suites:** the emission adds one outbox
row per feasibility verdict, so the exact event-list expectations in
`crates/flexcore-application/tests/command_gates.rs` (3 assertion sites +
the event-name helper) and
`crates/flexcore-application/tests/dispatch_simulator.rs` (the event-name
helper + 1 assertion site) were updated for the new `FeasibilityCheckCompleted`
entry. The rejection suites (P4.20–P4.26) and `dispatch_security.rs` were
NOT touched — they assert per-event-name counts or no-event windows, which
the additive emission does not change (verified by reading their assertion
shapes; UNRUN, so this is reasoning, not a run result).

### 1.4 REST resources (step 3; §15.2)

- `GET /api/v1/dispatch-commands` — tenant-scoped listing, newest first
  (keyset over `command_id`), optional closed-vocabulary `state` filter.
- `GET /api/v1/dispatch-commands/{command_id}` — the record plus its FULL
  transition journal (the reconstruction view; every transition, never
  rewritten).
- `GET /api/v1/feasibility-checks` — tenant-scoped listing ordered by
  `(checked_at, check_id)`, optional `command_id` / `schedule_id` filters.
- `GET /api/v1/feasibility-checks/{check_id}` — one evidence record.

**READ-ONLY — NO execute-command endpoint (§15.2).** Both resources are
GET-only. The §14.1 transitions happen exclusively through the
application-layer gates (`command.create` / `command.validate` /
`command.approve` at the gates; `dispatch.authorize` / `dispatch.issue`
do not exist on the action surface, §5.7). Exposing gate operations over
REST was deliberately NOT added (**D-P4.30-3**, DEFAULT): P4.30's surface
is the read/evidence surface; a REST write path into the protected command
lifecycle is a new decision for a later subtask. The contract tests assert
the structural guarantee (GET-only operations in the generated OpenAPI,
POST probes → 405/404).

Wiring: `routes_phase4.rs` + `dto_phase4.rs` (NEW), `phase4_fakes.rs`
(NEW — Test-backend fakes honouring tenant scoping, immutability
conflicts, CAS transitions, keyset listing), `openapi.rs` (additive
path/schema registration), `auth.rs` (additive `phase4_default_policies`
— the cumulative Phase 3 set + `command.read` + `feasibility.read` for
every role at tenant scope), `state.rs` (additive bundle fields +
`for_tests_with_phase4`), `lib.rs`/`routes.rs` (additive registration +
router merge), `main.rs` (the composition root now uses
`phase4_default_policies`).

Authorization actions: `command.read` (pre-existing, P4.13) for the
dispatch-command resource; **NEW additive action** `feasibility.read`
(`Action::FeasibilityRead` — variant + id arm + parse arm + a
`phase4_policies` grant for Operator/TenantAdmin/Viewer at all scope
kinds + the action.rs round-trip test list). Least-privilege: no write or
execute action exists on this REST surface.

Contract tests: `apps/control-plane/tests/api_phase4.rs` (NEW, NOT
DB-gated): 401 unauthenticated; 403 under a deny-by-default (empty) policy
set; tenant-scoped listings (another tenant's records absent); cross-tenant
by-id reads → 404 no leak; OpenAPI-shape validation of every 200 body
against the GENERATED schemas; correlation-field assertions (§21.3);
malformed filter/cursor/limit → 422; the no-execute structural test.

### 1.5 Correlation (step 4; §21.3)

Coverage verified per record class (mapping table; nothing was added to
frozen types — the gaps closed were the migration-0017 record and the DTOs,
which carry the full set):

| Field (§21.3) | Command record (0016) | Journal row (0016) | Check record (0017) | Outbox event |
|---|---|---|---|---|
| tenant / portfolio / site / asset | yes (columns) | via command FK | yes (columns) | yes (payload) |
| schedule + revision | yes | via command FK | yes | via journal/records |
| command id | yes (PK) | yes | yes (nullable, candidate checks) | via correlation (dispatch payloads carry command_id) |
| plugin / solver / model versions | §14.3 payload (SourceOrigin) | — | model_id + model_version + model_content_hash | — |
| telemetry snapshot + grid-model version | §14.3 payload (EvidenceReferences fields 12–13) | — | model binding | — |
| trace / correlation id | outbox envelope correlation | — | correlation_id column | correlation_id + idempotency key |
| actor identity | — | actor column | actor column | — |

No secrets and no unnecessary personal/customer data in any record or log
field: the actor is an identity reference (`svc:…` / `user:<subject>`);
the dev auth token format is documented dev-only (auth.rs); Config redacts
the database URL. Assumption recorded: "trace ID" is served by the
correlation id convention above (the outbox envelope's `correlation_id`
and the 0017 column); a W3C trace-context propagation layer does not exist
in 0.x and is not invented here.

### 1.6 Retention per data class (step 5; §16.3)

Already implemented and REUSED, not rebuilt: `flexcore-storage::retention::
RetentionPolicy` (P1.10) — the seven §16.3 data classes, every class
defaulting to `Unset`, `#[serde(default)]` partial documents, no universal
period anywhere in code (its own tests pin the rule). The partition
maintenance surface already wires the telemetry classes from deployment
configuration (`FLEXCORE_RETENTION_*`, partition_maintenance.rs).

Phase 4 class assignment (documentation — the classes are the frozen
seven): dispatch commands, the transition journal, the feasibility-check
evidence records (migration 0017), and the outbox audit trail are **class 4
— command/audit evidence** (`RetentionPolicy.command_audit_evidence`);
their actual retention is a deployment/legal decision, and migration 0017
deliberately carries no TTL (matching 0016's "no retention value" rule).
The append-only rule governs WRITES, never deletion; any future deletion
path must be authorized by a configured `Configured { period }` — never by
a default. No code changes were needed (the configuration type and the
unset defaults already satisfy the acceptance criterion "retention is
configurable").

## 2. Tests written (all UNRUN)

- `crates/adapters/postgres/tests/persistence_wiring.rs` (NEW, DB-gated on
  `FLEXCORE_TEST_DATABASE_URL`, skipping cleanly when unset): (a) a
  command transition, its two DISTINCT evidence references, and the
  feasibility-check record commit in ONE transaction and read back
  (including the new `list_commands` and `list_feasibility_checks`
  surfaces); (b) check records are append-only (duplicate → named
  Conflict; raw UPDATE/DELETE refused; closed outcome CHECK) and
  tenant-scoped (foreign reads None, listings empty); (c) the P4.24
  outbox semantics on the real outbox table — a redelivered
  idempotency key is refused with the durable `DuplicateKey` within a
  transaction and across transactions, exactly one effective row per key.
- `apps/control-plane/tests/api_phase4.rs` (NEW, not DB-gated — see 1.4).
- Updated expectations in `tests/command_gates.rs` (3 sites + helper) and
  `tests/dispatch_simulator.rs` (helper + 1 site) for the additive
  `FeasibilityCheckCompleted` emission — recorded in 1.3.

## 3. CI / migration inventory

The `migration-tests` CI job needs NO edit: the up path is the
`sqlx::migrate!` compile-time glob (a new migration is picked up
automatically). The migration enumeration that DID exist was the
down-file table in `crates/adapters/postgres/tests/integration.rs`
(`DOWN_FILES`), which had lagged at 0010 since the P2/P3 waves. Extended
additively to 0011–0017 so the down/up replay covers the full set (the
down files replay in reverse, newest first). The migration-tests job
comment in `.github/workflows/ci.yml` already documents that later
migrations need no CI edit — verified, no workflow change made.

## 4. Honest limitations

- **Nothing was built, run, verified, or passed.** All Rust changes are
  UNCOMPILED; all tests (contract + DB-gated) are UNRUN; migration 0017
  was NEVER APPLIED to any database. The loop bans prohibit build/test/
  docker; the human gate runs them.
- **Compile risk recorded** (VERIFY-BUILD conventions, same as prior
  waves): sibling-module private-field access in the new adapter module
  follows the `command_store.rs` precedent; the utoipa schema names used
  by the contract tests (`DataEnvelope_<T>` rendering) follow the
  api_phase3 fallback lookup; `Backend::Test` gained two fields — all
  `Backend::Test` construction goes through `state.rs`'s helpers (verified
  by grep), so no test composition breaks.
- The `FeasibilityCheckCompleted` payload-fit gap (identity + time only,
  no command/attempt field) is the frozen vocabulary's recorded shape; the
  attempt identity is reconstructable from the journal evidence reference
  and the 0017 record, not from the event payload alone.
- The check-id key uniqueness argument (at most one runner-verdict check
  per command) rests on the frozen-edge table (a check applies only from
  VALIDATED and moves the command to FEASIBLE/REJECTED terminally for that
  gate); a future re-validation feature would need a new key scheme.
- Authorization evidence remains journal-borne (D-P4.30-2) — see 1.1.
- Trace sampling (the acceptance criterion "correlation IDs are present on
  a sampled end-to-end trace") is implemented at the record level
  (correlation columns + outbox envelope correlation); an actual sampled
  trace over NATS/HTTP is P4.31/P4.36 integration territory and was not
  demonstrated here.

## 5. Decisions index

| ID | Decision | Kind |
|---|---|---|
| D-P4.30-1 | `FeasibilityCheckCompleted` is emitted for every §12.5 runner verdict (pass/limit/fail-closed), not for the usability pre-check; correlation = command id; one receipt may pair both the rejection and the completed-check event | DEFAULT |
| D-P4.30-2 | No second authorization-decision table: the 0016 journal rows + the §14.3 payload carry the authorization evidence (reference, actor, scope, limits, expiry) | DEFAULT |
| D-P4.30-3 | The Phase 4 REST surface is READ-ONLY; gate operations are not exposed over REST in P4.30 | DEFAULT |
| D-P4.30-4 | Phase 4 persisted records map to §16.3 class 4 (command/audit evidence) in `RetentionPolicy`; no TTL in any Phase 4 migration | DEFAULT |
