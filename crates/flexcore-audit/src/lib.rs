//! Neryva FlexCore — append-oriented audit evidence types and port (P1.10).
//!
//! This crate turns the §10.1 `AuditRecord` (via P0.10,
//! `ledger/phase-00-product-freeze/10_asset_capability_vocabulary.md`) into
//! a typed Rust record and defines the append-only audit write port
//! (P1.10 step 2; report §6.5, §10.1; P1.12 §4.7 `audit_records` design).
//!
//! | Module | Content |
//! |---|---|
//! | [`record`] | `AuditRecord` (actor, action, object, before/after references, decision, evidence, event/ingest timestamps, correlation ID, tenant context) plus `CorrectionRecord`, the new-record-referencing-the-original linkage |
//! | [`sink`] | `AuditSink` — the append-only write port: `append` is its ONLY method |
//! | [`chain`] | P11.15 keyed HMAC-SHA-256 linear chain (the P11-E-1 DEFAULT) with the per-tenant chain namespace and the periodic anchor-export design — a DEPLOYMENT REQUIREMENT: implemented code, never keyed, deployed, or exercised here |
//!
//! # The append-orientation rule (P1.10 step 3)
//!
//! Audit records are append-oriented. A correction is a **new** record
//! whose `corrects` linkage references the original record's id, with a
//! mandatory reason — never an in-place edit, never a delete-and-reinsert.
//! The original record stays exactly as it was written. This is layer 3 of
//! the P1.12 §4.7 three-layer append-only enforcement (grants; a
//! defence-in-depth database trigger; this port). The port's single-method
//! shape is the type-level half of that rule; the mechanical checks are the
//! source-text test in [`sink`] and the `grep` output recorded in the crate
//! README.
//!
//! # Selective event sourcing context (report §6.5, via P0.16 §5)
//!
//! The audit trail participates in the §6.5 storage strategy's
//! **append-oriented event history** class, which covers: command lifecycle,
//! operator approvals, plugin changes, configuration changes, and
//! security-relevant actions. Those histories are append-oriented by design;
//! current-state tables, time-series telemetry, and object storage follow
//! their own strategies (never a single global append-only log for
//! everything). This crate carries the evidence records for that
//! append-oriented class; it is not a general event log and not a
//! repository.
//!
//! # Port placement (single-home rule)
//!
//! Repository-abstraction traits live in `flexcore-ports` (P1.07 decision,
//! P1.10 step 1). `AuditSink` lives HERE because it is not a repository
//! trait: it has no read path and one write operation. This is a recorded
//! exception, not a precedent — see the crate README.
//!
//! # Scope and control status
//!
//! Audit evidence types for the design baseline only. The entire 0.x line is
//! simulation-only; nothing here authorises field control or device writes.
//! FlexCore is a supervisory orchestration layer and performs no primary
//! frequency response, synthetic inertia, relay protection, inverter
//! inner-loop control, or sub-cycle control.

pub mod chain;
pub mod record;
pub mod sink;
// P18.07 (phase 18, wave 1, Agent A) — per-device update EVIDENCE
// records (the ledger's five closed kinds: attempt / verify / install /
// result / rollback), a per-attempt keyed HMAC-SHA-256 linear chain
// (domain-separated from the P11.15 audit chain — the SAME std-only
// primitives, consumed not duplicated), an exportable bundle shape, and
// the §10.1 `AuditRecord` mapping that rides each record through the
// EXISTING `AuditSink` port and the P11.15 production chain (consumed,
// never redefined; LR-203 honored — the test-grade FNV path untouched).
// No update evidence record exists anywhere: no device, no fleet, no
// update (decision record
// docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/
// 07_p18_07_update_evidence_audit_pack.md).
pub mod update_evidence;
