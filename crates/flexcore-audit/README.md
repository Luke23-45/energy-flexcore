# flexcore-audit (P1.10)

Append-oriented audit evidence types and the append-only audit write port
(report §8.1, §6.5, §10.1; ledger P1.10 deliverable B; storage counterpart:
the P1.12 §4.7 `audit_records` design).

## The append-orientation rule (P1.10 step 3)

**Audit records are append-oriented; corrections are new records referencing
the old one, never an in-place edit.**

- `AuditSink::append` is the ONLY write operation on the port. There is no
  update, delete, correct-in-place, amend, revise, remove, or edit operation,
  and none may be added.
- A correction is a **new** `AuditRecord` whose `corrects` linkage
  (`record::CorrectionRecord`) references the original record's id and
  carries a mandatory reason. The original record stays exactly as it was
  written (`AuditRecord::link_correction`).
- This is layer 3 of the P1.12 §4.7 three-layer append-only enforcement:
  (1) database grants — `INSERT`/`SELECT` only, no `UPDATE`/`DELETE` grant
  to any role; (2) a defence-in-depth `BEFORE UPDATE OR DELETE` trigger that
  raises unconditionally, closing the table-owner gap; (3) this port. No
  single layer is trusted alone.

### Mechanical evidence — the port has exactly one method

P1.10 acceptance criterion: "the audit port has no update/delete method
(verified by API inspection)". Recorded evidence (this repository, P1.10):

```text
$ grep -n "fn " crates/flexcore-audit/src/sink.rs
28://   - the `grep -n "fn " src/` output recorded in the crate README.
46:    fn append(&self, record: AuditRecord) -> Result<(), AuditError>;
62:    fn sample_uuid(tag: u8) -> FlexUuid {            <- #[cfg(test)] from here on
...
```

Outside the `#[cfg(test)]` module, the only function in the port's source is
line 46 — the trait's single `append` method. The mutation-method search:

```text
$ grep -rnE "fn (update|delete|correct|amend|revise|remove|edit)\(" crates/flexcore-audit/src/
(no matches; grep exit 1)
```

The source-text test in `src/sink.rs` (P1.10 test) re-asserts both facts on
every `cargo test` run: it extracts the trait declaration, requires exactly
one method in it, and forbids mutation-named methods anywhere in the port
source. A reviewer can also simply read `src/sink.rs` — one method.

## The port lives here, not in flexcore-ports (recorded exception)

The single-home rule for **repository** traits (P1.10 step 1) puts
repository-abstraction traits in `flexcore-ports`; `flexcore-storage`
defines none. `AuditSink` is defined in this crate because it is **not a
repository trait**: it abstracts no aggregate lifecycle, has no read path,
and exposes exactly one write operation. This placement is the P1.10 step 2
deliverable, recorded here so it is not mistaken for a duplicate-ports
violation at the P1.30 boundary audit.

## The §10.1 AuditRecord, field by field

| §10.1 field (via P0.10) | Type here |
|---|---|
| actor | `ActorIdentity` — authenticated actor identity from claims (P0.16 rule 5), never from untrusted request fields |
| action | `AuditAction` |
| object | `AuditObject` (type + id) |
| before/after references | `before_ref` / `after_ref: Option<RecordReference>` — references (id/version pointers), never full payload copies (P1.12 §4.7) |
| decision | `Option<AuditDecision>` — outcome + reason reference, never a bare allow/deny; `Denied` outcomes are how cross-tenant access denials get audited (P0.16 rule 6) |
| evidence | `Option<EvidenceReference>` |
| timestamp | `event_time: EventTime` + `ingest_time: IngestTime` — the distinct P0.09 types, never merged or substituted; construction and deserialization both enforce ingest ≥ event |
| correlation ID | `correlation_id: FlexUuid` (report §21.3 trace propagation) |
| — | `tenant_id: Option<TenantId>` — tenant context where applicable; `None` marks platform-level actions (P1.12 §4.7) |
| — | `audit_id: FlexUuid` — supplied by the caller (generation is the P1.07 id-generation port) |
| — | `corrects: Option<CorrectionRecord>` — the correction linkage |

## Selective event sourcing context (report §6.5, via P0.16 §5)

The audit trail is the append-oriented event history class of the §6.5
storage strategy: command lifecycle, operator approvals, plugin changes,
configuration changes, and security-relevant actions are recorded
append-oriented. Current-state tables, time-series telemetry, and object
storage follow their own strategies — this crate is deliberately not a
general event log, and JetStream (P1.09/P1.15) carries these facts as
transport, never as the authoritative store (P0.16 §5 consistency note).

## Tests (P1.10 step 4)

- append succeeds into an in-memory fake (`sink::tests`), including via
  `&dyn AuditSink`; backend failures propagate as `AuditError`.
- the port defines exactly one method named `append`; no mutation-named
  method exists anywhere in the port source (source-text test).
- a correction record links to the original with a mandatory reason; the
  original is untouched (`record::tests`).
- record validation: empty fields rejected; ingest-before-event rejected;
  serde round trips revalidate every rule; platform-level (null-tenant)
  records round trip.

## Dependency boundary

`flexcore-contracts` (FlexUuid, P0.09 time types) + `flexcore-domain`
(`TenantId`) + `serde` — no sqlx, no tokio, no `time` crate of its own
(`tools/check_dependency_rule.py` enforces the edge set). The PostgreSQL
adapter (P1.13) implements `AuditSink` against the `audit_records` table
(P1.12 §4.7). The record serializes as validated JSON for adapters; there is
no runtime JSON dependency in this crate (`serde_json` is dev-only).

## Scope and control status

Audit evidence types for the design baseline only. The entire 0.x line is
simulation-only; nothing here authorises field control or device writes.
FlexCore is a supervisory orchestration layer and performs no primary
frequency response, synthetic inertia, relay protection, inverter inner-loop
control, or sub-cycle control.
