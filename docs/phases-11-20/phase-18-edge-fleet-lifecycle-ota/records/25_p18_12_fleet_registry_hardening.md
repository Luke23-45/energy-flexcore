# P18.12 — Fleet device-registry hardening: migration 0023 (expand-only) + the fleet-state schema types

| Field | Value |
|---|---|
| Subtask | P18.12 — Fleet device-registry hardening (0018 registry extension: firmware state, cohort membership, attestation state, last-seen health) |
| Wave / agent | Phase 18 wave 1, Agent B |
| Status | **IMPLEMENTED, AWAITING HUMAN BUILD/TEST** (WRITTEN-UNRUN). Migration spec + schema types written; **no migration runs** — execution is a human gate |
| Sources | Seams: `crates/adapters/postgres/migrations/0018_edge_device_registry_p6_06.up.sql` (READ FULLY — append-only per term, INSERT+SELECT grants, RLS), `crates/adapters/postgres/MIGRATION_POLICY.md` (P11.18 expand/contract + lock-risk rules); the P11.18 lock-risk audit record; consumers: P18.06 cohort membership (Agent A), P18.11 eSIM fields, P18.10 deregistration |
| Files touched | `crates/adapters/postgres/migrations/0023_edge_fleet_registry_p18_12.up.sql` (new), `crates/adapters/postgres/migrations/0023_edge_fleet_registry_p18_12.down.sql` (new), `crates/edge-agent/src/fleet_registry.rs` (new), `crates/edge-agent/src/lib.rs` (additive module declaration only) |

## 1. The design decision: a companion state JOURNAL, not registry columns

The 0018 `edge_device_registry` is append-only per identity term: its
UPDATE/DELETE trigger refuses unconditionally, `flexcore_app` holds INSERT +
SELECT only, and a lifecycle change appends a NEW term row. Firmware state,
attestation state, and last-seen health are HIGH-FREQUENCY observations;
putting them on the registry would demand either an UPDATE grant (forbidden
by the 0018 design) or a new registry term per telemetry-like observation
(churning the one-ACTIVE-term invariant and the identity evidence).

The expansion is therefore a **new companion table** —
`edge_fleet_device_state`, an append-only state journal keyed to the SAME
`(device_id, tenant_id)` identity scope:

- One row per state observation: `(event_id, tenant_id, device_id,
  state_kind, state_value, detail, correlation_id, observed_at_unix_millis,
  recorded_at)`.
- `state_kind` CHECK set (exactly five): `FIRMWARE_STATE`,
  `COHORT_MEMBERSHIP`, `ATTESTATION_STATE`, `LAST_SEEN_HEALTH`,
  `ESIM_PROFILE_STATE`.
- A device's CURRENT fleet state = its latest row per
  `(device_id, state_kind)` — the same event-sourced shape 0018 uses for
  the identity itself. The read pattern is a `DISTINCT ON` walk, served by
  `edge_fleet_device_state_current_idx` (created in-transaction on the
  brand-new, empty table — the MIGRATION_POLICY lock-risk rules bind only
  populated tables; this one is new in the same migration).
- Defence-in-depth identical to 0018: append-only guard trigger (raises for
  UPDATE and DELETE, including for the owner), `flexcore_app` INSERT +
  SELECT only, RLS ENABLE + FORCE with the shared `tenant_isolation`
  policy, owner `flexcore_migrator`.
- No secret, no key material, no certificate columns anywhere.
- **No foreign key to the registry**, deliberately — the same recorded
  rationale as the 0018 enrollment journal (an observation may name a
  device in states the registry's term shape does not join to); the only
  FK is the tenant.

## 2. Expand/contract per P11.18 (MIGRATION_POLICY.md)

| Stage | Status for this change |
|---|---|
| 1. Expand | THIS migration: `CREATE TABLE` + index + trigger + grants + RLS. No ALTER touches any existing table; no constraint scans a populated table; the 0018 registry and journal stay byte-identical |
| 2. Dual-write / backfill | NOT APPLICABLE — nothing is replaced; the journal starts empty (no fleet exists) |
| 3. Switch | NOT APPLICABLE — consumers (P18.06/P18.10/P18.11) read the journal as a NEW surface; no read moves off an old shape |
| 4. Contract | NOT APPLICABLE — the down migration drops the new table whole; nothing else is touched |

Lock-risk statement (per the policy's honesty rule): the table is created
EMPTY in the same transaction as its index, so no populated-table lock risk
arises; the policy's CONCURRENTLY/VALIDATE runbook scheduling is therefore
not triggered by this migration. No lock was observed — the whole migration
is WRITTEN-UNRUN.

## 3. The schema types (`crates/edge-agent/src/fleet_registry.rs`)

The Rust mirror of the SQL contract — the closed `state_value` domains per
kind, the validated row shape, and the BTreeMap current-state projection:

| Kind | Vocabulary (exact strings) | Owner |
|---|---|---|
| `FIRMWARE_STATE` | `IN_SYNC`, `UPDATE_PENDING`, `UPDATING`, `UPDATE_FAILED`, `ROLLED_BACK`, `UNKNOWN` | This module. `UNKNOWN` is a real state, never healthy-by-default (the P6.17 discipline); an UNKNOWN device is NOT an eligible cohort-progression target — the orchestrator decides, this module refuses to claim the state is known |
| `COHORT_MEMBERSHIP` | `IN:<cohort>:<block>` / `OUT` (leaving a cohort is a NEW row, never a deletion) | This module; **consumed by Agent A's P18.06** (`current_cohort`) |
| `ATTESTATION_STATE` | `NOT_REPORTED`, `CURRENT`, `STALE`, `FAILED`, `REVOKED` | `crate::attestation` (P18.13) |
| `LAST_SEEN_HEALTH` | `SEEN_HEALTHY`, `SEEN_DEGRADED`, `NOT_SEEN` | This module |
| `ESIM_PROFILE_STATE` | `NO_PROFILE`, `ENABLED`, `DISABLED`, `UNKNOWN`, `DELETED` | `crate::esim_lifecycle` (P18.11) |

Every unknown kind/value is a named refusal (`FLEET_STATE_KIND_UNKNOWN` /
`FLEET_STATE_VALUE_UNKNOWN`) — the vocabulary is closed, never guessed open.
The projection (`FleetStateView`) validates each row before it enters
(fail-closed), keeps the full history count, and resolves the latest
observed row per kind (out-of-order arrival from an offline device is legal
and does not displace the latest).

## 4. Handoff to Agent A (P18.06 consumes this — landed first for that reason)

- Cohort membership read: `FleetStateView::current_cohort(device_id)` →
  `CohortMembership { cohort_id, block_index, observed_at_unix_millis }`;
  `None` = never assigned. The SQL shape is documented in the migration
  header.
- Firmware state read: `FleetStateView::current_firmware_state(device_id)`;
  `None` = no observation ever — treat exactly like `UNKNOWN`, never as
  healthy.
- Agent A CONSUMES these vocabularies and the journal schema; A never
  redefines a value, adds a kind, or writes an UPDATE path (none exists —
  the journal is append-only by trigger).

## 5. Compliance mapping (mapping, never a certification claim)

| Deliverable aspect | Mapped requirement | Basis |
|---|---|---|
| Append-only fleet-state evidence with tenant isolation | IEC 62443-4-1 SAR 3.4 (process evidence) [S-164]; IEC 62443-4-2 CR 2.1 (authorization enforcement — RLS/INSERT-only grants) [S-163] | As cited by the phase-18 plan; clause-content interpretation **UNVERIFIED** |

## 6. Honesty register

1. **WRITTEN-UNRUN** — the migration has never been executed against any
   database; the schema types have never been built or their tests run.
2. **No fleet** — the journal has never held a row; the projection is
   exercised (in UNRUN tests) against in-memory doubles only.
3. **Kernel untouched** — the plugin-rollout policy register is
   byte-identical; this migration adds no rollout mechanism at all.
4. **Never-granted set byte-identical** — the journal's grants are
   INSERT + SELECT on the new table only; no new permission exists.
