# flexcore-storage (P1.10)

Non-port persistence abstractions for Neryva FlexCore (report §8.1, §6.3,
§15.5, §16.3; ledger P1.10 deliverable A). This crate defines **no
persistence implementation**: no SQL, no connection, no async runtime — the
PostgreSQL adapter (P1.13) owns all of that.

## Single-home rule for repository traits (recorded)

**Repository-abstraction traits live in `flexcore-ports` (P1.07), not here.**
This was decided in P1.07 and recorded by P1.10 step 1, which resolves the
"may live here or in ports" ambiguity in favour of ports. One home, one
definition; the P1.30 boundary audit checks that no repository trait is
defined twice (ports vs storage). Nothing in this crate is a port or a
repository trait.

Why this crate may define the audit write port but this one may not define
repository traits: it may not — the audit port lives in `flexcore-audit`
(P1.10 step 2), which is a separate recorded exception, not a precedent for
this crate. See the `flexcore-audit` README.

## What is here

| Module | Type | Rule it encodes |
|---|---|---|
| `unit_of_work` | `UnitOfWork`, `FinalizedUnitOfWork`, `UnitOfWorkOutcome` | One transactional boundary: begin, exactly one of commit/rollback. `commit`/`rollback` consume the unit, so double-finalization is a compile error. Drop of an unfinalized unit MUST roll back — an adapter obligation the adapter's tests enforce (P1.13); a plain type cannot run a drop handler for a backend it does not know. |
| `outbox_write` | `TransactionalWrite`, `StateChangeDescriptor`, `StateChangeOperation` | The "state change + outbox record in ONE transaction" rule (report §6.3 step 1, §15.5). Private fields + one constructor make the partial write unrepresentable; the binding adapter obligation is documented on the type; **mechanical enforcement is the P1.13 atomicity test** (ledger P1.13 step 4: injected failure leaves neither state nor outbox, or both). |
| `retention` | `RetentionPolicy`, `RetentionSetting` | Per-data-class retention **configuration** (P0.16 §4, report §16.3): all seven §16.3 classes default to `Unset`; there is no universal retention period anywhere in code, and a missing class in deployment configuration deserializes as `Unset`, never as a built-in period. |

## What is deliberately not here

- **No retention logic.** Retention is configuration, not code (P0.16 §4);
  `RetentionPolicy` only carries deployment-supplied values with unset
  defaults. Nothing in this crate deletes, drops, or expires anything.
- **No repository traits** (see the single-home rule above).
- **No storage engine.** No sqlx, no tokio, no PostgreSQL, no NATS — the
  dependency rule (`tools/check_dependency_rule.py`) forbids infrastructure
  in this crate, and the manifest records the justification for every
  dependency that is present.

## Adapter obligations (binding)

An adapter that consumes these types MUST (see module headers for detail):

1. Drive each `TransactionalWrite` through a single `UnitOfWork` boundary:
   state change and outbox record in one native transaction, all-or-nothing
   on failure — proven by an atomicity test in the adapter (P1.13 for
   PostgreSQL; equivalent tests for any other store).
2. Roll back when an unfinalized `UnitOfWork` is dropped.
3. Treat `RetentionSetting::Unset` as "no retention action authorized by
   this configuration" — never as "keep forever", never as a built-in
   period.

## Scope and control status

Abstraction types for the design baseline only. The entire 0.x line is
simulation-only; nothing here touches a database, a device, or a live grid.
FlexCore is a supervisory orchestration layer and performs no primary
frequency response, synthetic inertia, relay protection, inverter inner-loop
control, or sub-cycle control.
