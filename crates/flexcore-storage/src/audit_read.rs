// P4.36 — Audit-events READ contract (report §15.2, §15.3, §16.2, §10.1,
//         §21.3; ledger P4.36; decision record:
//         `ledger/phase-04-grid-feasibility-dispatch/36_audit_read_api.md`).
//
// The read-side persistence contract for the §10.1 audit trail: the
// append-only evidence that P1.10's `AuditSink` writes (PostgreSQL table
// `audit_records`, migration 0008 — RLS + INSERT/SELECT grants + the
// append-only trigger). This module adds the ONE missing half of that
// table's contract — the tenant-scoped SELECT — so the §15.2
// `/api/v1/audit-events` read resource (P4.36) and the Phase 5 Audit
// workspace (P5.09) read from a CONTRACT instead of an unspecified source.
//
// ## Home (recorded)
//
// NOT in flexcore-audit: the P1.10 `AuditSink` port is, by its recorded
// single-method shape, a one-way write valve ("no read path, no query
// path"), and its source-text test forbids a second method on that trait.
// This read contract is a record-store contract like
// `feasibility_check::FeasibilityCheckStore` (P4.30), so it lives in the
// SAME recorded home deviation (D-P3.21-4: storage-side record-store
// contracts; see the crate lib docs). The verify-first step (ledger P4.36
// step 1) confirmed NO read API existed anywhere before this module —
// there is exactly one audit-events read contract.
//
// ## The append-only rule, applied to the read port
//
// This trait is SELECT-only by construction: exactly two read methods, no
// insert, no update, no delete, no export/dump method, and none may be
// added. Writes stay exclusively on the P1.10 `AuditSink` (append only);
// corrections are NEW records referencing the original (`corrected_audit_id`),
// never in-place edits. The in-module source-text test pins this shape.
//
// ## Tenant scoping (report §16.2, P0.16 rule 5)
//
// Every method takes the caller's [`TenantContext`]; the adapter MUST
// filter by that tenant explicitly on top of the `tenant_isolation` RLS
// policy (migration 0008) and MUST bound every listing (no unbounded
// query — the recorded P2.12/P3.28/P4.30 listing convention). A
// foreign-tenant record reads as `None` / is absent from listings —
// existence never leaks across a tenant boundary. Platform-level rows
// (`tenant_id IS NULL`, P1.12 §4.7/§5.2) are invisible to this port: under
// the 0008 RLS policy they match no tenant context, and they are reachable
// only through the owner/ops path — a deliberate least-privilege boundary
// this contract does not widen.
//
// ## Correlation and data discipline (report §21.3, §10.1)
//
// Every served record carries the §10.1 field set end to end — actor,
// action, object (type + id), before/after REFERENCES, the decision
// (outcome + reason reference, never a bare allow/deny), the evidence
// reference, the distinct P0.09 event/ingest timestamps, the §21.3
// correlation id, and the correction linkage. References are pointers, not
// payload copies (P1.12 §4.7); no secret and no unnecessary personal or
// customer data appears in any field (the actor is an identity reference).
//
// Control status: persistence bookkeeping for the simulation-only 0.x
// line. Reading audit evidence authorises nothing.

use flexcore_domain::ids::TenantId;

/// The stored text of the `ALLOWED` decision outcome (the flexcore-audit
/// `DecisionOutcome` serde rendering, P1.10).
pub const AUDIT_DECISION_ALLOWED: &str = "ALLOWED";
/// The stored text of the `DENIED` decision outcome.
pub const AUDIT_DECISION_DENIED: &str = "DENIED";

/// The closed decision-outcome vocabulary of a stored audit record.
pub const AUDIT_DECISION_OUTCOMES: [&str; 2] = [AUDIT_DECISION_ALLOWED, AUDIT_DECISION_DENIED];

// --- Errors -------------------------------------------------------------------

/// Errors of the audit-events read port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEventReadError {
    /// A query argument is malformed (`field` names the problem). Nothing
    /// was read.
    InvalidFilter {
        field: &'static str,
    },
    /// The storage backend failed. `code` and `message` are opaque and
    /// human-safe — never raw storage text.
    Backend {
        code: String,
        message: String,
    },
}

impl std::fmt::Display for AuditEventReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditEventReadError::InvalidFilter { field } => {
                write!(f, "the audit-events query was rejected: '{field}' is missing or malformed")
            }
            AuditEventReadError::Backend { code, message } => {
                write!(f, "storage backend failure ({code}): {message}")
            }
        }
    }
}

impl std::error::Error for AuditEventReadError {}

// --- Rows ---------------------------------------------------------------------

/// One audit evidence record as read from the `audit_records` table
/// (migration 0008) — the §10.1 field set, end to end.
///
/// The row is immutable by construction (append-only trail): no setter
/// exists and the read port offers no mutation. Construction goes
/// exclusively through [`AuditEventRow::new`], which enforces the
/// storage-level rules BEFORE a stored row can be served (the adapter
/// re-validates every decode through it — fail closed on a malformed row).
///
/// Identifier/reference fields are text exactly as stored: `audit_id` and
/// `corrected_audit_id` are canonical hyphenated UUID text (the recorded
/// FlexUuid ↔ PG uuid mapping), and the reference fields are the pointer
/// strings the writer recorded (P1.12 §4.7 — references, not payload
/// copies).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditEventRow {
    /// The record identifier (canonical UUID text; the P1.07 id-generation
    /// port's UUIDv7, so id order is a stable time order).
    pub audit_id: String,
    /// The owning tenant; `None` marks a platform-level record (which this
    /// port never serves — see the module docs; carried as `Option` so the
    /// type mirrors the column honestly, P1.12 §4.7).
    pub tenant_id: Option<TenantId>,
    /// Who performed the audited action (an identity reference, never a
    /// credential — §21.3).
    pub actor: String,
    /// The audited action (§10.1; the event-class discriminator, e.g.
    /// `command.approve`, `asset.register`).
    pub action: String,
    /// The audited object type (§10.1, e.g. `command`, `asset`).
    pub object_type: String,
    /// The audited object identifier (§10.1).
    pub object_id: String,
    /// Reference to the previous state, where applicable (a pointer, never
    /// a payload copy — P1.12 §4.7).
    pub before_ref: Option<String>,
    /// Reference to the new state, where applicable.
    pub after_ref: Option<String>,
    /// The decision outcome (`ALLOWED` | `DENIED`), where a decision was
    /// recorded. Present exactly together with
    /// [`AuditEventRow::decision_reason_reference`] — a decision is an
    /// explicit outcome plus its reason reference, never a bare allow/deny
    /// (P1.11; P1.12 §4.7).
    pub decision_outcome: Option<String>,
    /// The reason/evidence reference behind the recorded decision.
    pub decision_reason_reference: Option<String>,
    /// The evidence reference (§10.1 "evidence"), where applicable. NEVER
    /// dropped by a read: the §15.2 resource serves references, not
    /// summaries (P5.09 consumes them).
    pub evidence: Option<String>,
    /// When the audited action occurred (UTC unix millis from the P0.09
    /// `event_time`; distinct from `ingest_time`).
    pub event_time_unix_millis: i64,
    /// When the record was written (UTC unix millis from the P0.09
    /// `ingest_time`; must not precede `event_time`).
    pub ingest_time_unix_millis: i64,
    /// The workflow correlation (trace) id (§21.3; canonical UUID text as
    /// written by the P1.13 mapping).
    pub correlation_id: String,
    /// The correction linkage: the original record this record corrects
    /// (canonical UUID text), when this record is a NEW correction row —
    /// the original was never edited (P1.10 step 3, migration 0008).
    pub corrected_audit_id: Option<String>,
}

impl AuditEventRow {
    /// Builds and validates one audit-event read row. Rules: every
    /// identity field non-empty; the decision halves present together with
    /// the outcome inside the closed vocabulary; `ingest_time` not before
    /// `event_time` (P0.09); a correction linkage, when present, non-empty.
    pub fn new(
        audit_id: String,
        tenant_id: Option<TenantId>,
        actor: String,
        action: String,
        object_type: String,
        object_id: String,
        before_ref: Option<String>,
        after_ref: Option<String>,
        decision_outcome: Option<String>,
        decision_reason_reference: Option<String>,
        evidence: Option<String>,
        event_time_unix_millis: i64,
        ingest_time_unix_millis: i64,
        correlation_id: String,
        corrected_audit_id: Option<String>,
    ) -> Result<Self, AuditEventReadError> {
        for (field, value) in [
            ("audit_id", audit_id.as_str()),
            ("actor", actor.as_str()),
            ("action", action.as_str()),
            ("object_type", object_type.as_str()),
            ("object_id", object_id.as_str()),
            ("correlation_id", correlation_id.as_str()),
        ] {
            if value.trim().is_empty() {
                return Err(AuditEventReadError::InvalidFilter { field });
            }
        }
        for (field, value) in [
            ("before_ref", before_ref.as_deref()),
            ("after_ref", after_ref.as_deref()),
            ("decision_reason_reference", decision_reason_reference.as_deref()),
            ("evidence", evidence.as_deref()),
            ("corrected_audit_id", corrected_audit_id.as_deref()),
        ] {
            if let Some(value) = value {
                if value.trim().is_empty() {
                    return Err(AuditEventReadError::InvalidFilter { field });
                }
            }
        }
        // A decision is recorded as outcome + reason reference, never as a
        // bare allow/deny (P1.11; P1.12 §4.7): the two halves are present
        // together or not at all, and the outcome is in the closed
        // vocabulary.
        match (decision_outcome.as_deref(), decision_reason_reference.as_deref()) {
            (None, None) => {}
            (Some(outcome), Some(_)) => {
                if !AUDIT_DECISION_OUTCOMES.contains(&outcome) {
                    return Err(AuditEventReadError::InvalidFilter {
                        field: "decision_outcome",
                    });
                }
            }
            (Some(_), None) => {
                return Err(AuditEventReadError::InvalidFilter {
                    field: "decision_reason_reference",
                })
            }
            (None, Some(_)) => {
                return Err(AuditEventReadError::InvalidFilter { field: "decision_outcome" })
            }
        }
        // P0.09: a record cannot have been written before the action it
        // records occurred.
        if ingest_time_unix_millis < event_time_unix_millis {
            return Err(AuditEventReadError::InvalidFilter {
                field: "ingest_time_unix_millis",
            });
        }
        Ok(Self {
            audit_id,
            tenant_id,
            actor,
            action,
            object_type,
            object_id,
            before_ref,
            after_ref,
            decision_outcome,
            decision_reason_reference,
            evidence,
            event_time_unix_millis,
            ingest_time_unix_millis,
            correlation_id,
            corrected_audit_id,
        })
    }
}

/// The listing filter of [`AuditEventReader::list_audit_events`]. Every
/// field is optional; an absent filter lists the tenant's records in
/// `(event_time, audit_id)` order. There is NO unbounded query: the page
/// size is always applied by the caller (the recorded
/// P2.12/P3.28/P4.30 listing convention: default page 200, maximum 1000).
///
/// The `action` field is the event-class discriminator (the exact §10.1
/// audited action, e.g. `command.approve`); the object pair
/// (`object_type` + `object_id`) covers the command/schedule/entity
/// filters of ledger P4.36 step 2 (a command read names `object_type =
/// "command"` and the command id). Time bounds are INCLUSIVE on both ends
/// (DEFAULT decision, recorded in the P4.36 decision record).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AuditEventFilter {
    /// Restrict to one exact audited action (e.g. `command.approve`).
    pub action: Option<String>,
    /// Restrict to one acting identity (exact identity reference).
    pub actor: Option<String>,
    /// Restrict to one audited object type (e.g. `command`, `schedule`).
    pub object_type: Option<String>,
    /// Restrict to one audited object identifier (the entity-id filter).
    pub object_id: Option<String>,
    /// Inclusive lower bound on the event time (UTC unix millis).
    pub from_event_unix_millis: Option<i64>,
    /// Inclusive upper bound on the event time (UTC unix millis).
    pub to_event_unix_millis: Option<i64>,
    /// Keyset cursor: return records strictly AFTER this `audit_id` in the
    /// listing order (the opaque cursor of the previous page; canonical
    /// UUID text).
    pub after_audit_id: Option<String>,
}

// --- The port -----------------------------------------------------------------

/// The audit-events READ port (ledger P4.36 step 2; report §15.2, §16.2).
/// A SELECT-only record-store contract over the append-only `audit_records`
/// table (migration 0008):
///
/// - **read-only by construction**: exactly the two methods below — no
///   insert, no update, no delete, no export/dump, and none may be added
///   (the in-module source-text test pins the shape; writes stay on the
///   P1.10 `AuditSink`, append only);
/// - **tenant-scoped**: every method takes the caller's
///   [`flexcore_domain::tenant_context::TenantContext`]; the adapter MUST
///   filter by that tenant explicitly on top of the `tenant_isolation` RLS
///   policy, and a foreign-tenant record reads as `None` / is absent from
///   listings — existence never leaks (P0.16 rule 5, §16.2);
/// - **always bounded**: the caller ALWAYS bounds the listing (default
///   page 200, maximum 1000 — no unbounded query);
/// - **references preserved**: every record is served with its §10.1
///   references end to end — never a summary that drops the evidence or
///   decision references (P5.09 consumes them);
/// - storage failures surface as [`AuditEventReadError::Backend`] with an
///   opaque code + message only.
pub trait AuditEventReader {
    /// Looks up one audit record in the caller's tenant scope. A
    /// foreign-tenant (or platform-level) record reads as `None` — no
    /// existence leak.
    fn find_audit_event(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        audit_id: &str,
    ) -> Result<Option<AuditEventRow>, AuditEventReadError>;

    /// Lists audit records in the caller's tenant scope, ordered by
    /// `(event_time_unix_millis, audit_id)` ascending, applying the
    /// filter. The caller ALWAYS bounds the result (default page 200,
    /// maximum 1000 — no unbounded query).
    fn list_audit_events(
        &self,
        ctx: &flexcore_domain::tenant_context::TenantContext,
        filter: &AuditEventFilter,
        limit: u32,
    ) -> Result<Vec<AuditEventRow>, AuditEventReadError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    fn tenant(seed: u8) -> TenantId {
        let mut bytes = [0u8; 16];
        bytes[15] = seed;
        TenantId::new(FlexUuid::from_bytes(bytes))
    }

    fn base_row() -> AuditEventRow {
        AuditEventRow::new(
            "0192a5e0-0000-7000-8000-0000000000a1".to_string(),
            Some(tenant(7)),
            "user:operator-1".to_string(),
            "command.approve".to_string(),
            "command".to_string(),
            "0192a5e0-0000-7000-8000-0000000000c1".to_string(),
            Some("command:0192a5e0-0000-7000-8000-0000000000c1:state=FEASIBLE".to_string()),
            Some("command:0192a5e0-0000-7000-8000-0000000000c1:state=APPROVED".to_string()),
            Some(AUDIT_DECISION_ALLOWED.to_string()),
            Some("approval-record:apr-1".to_string()),
            Some("evidence:approval/apr-1".to_string()),
            1_700_000_100_000,
            1_700_000_100_500,
            "0192a5e0-0000-7000-8000-0000000000f1".to_string(),
            None,
        )
        .expect("base row builds")
    }

    #[test]
    fn the_row_carries_the_full_s10_1_field_set() {
        let row = base_row();
        assert_eq!(row.actor, "user:operator-1");
        assert_eq!(row.action, "command.approve");
        assert_eq!(row.object_type, "command");
        assert_eq!(row.decision_outcome.as_deref(), Some(AUDIT_DECISION_ALLOWED));
        assert_eq!(row.decision_reason_reference.as_deref(), Some("approval-record:apr-1"));
        assert_eq!(row.evidence.as_deref(), Some("evidence:approval/apr-1"));
        assert!(row.corrected_audit_id.is_none());
        // The two P0.09 timestamps are present and distinct.
        assert_eq!(row.event_time_unix_millis, 1_700_000_100_000);
        assert_eq!(row.ingest_time_unix_millis, 1_700_000_100_500);
    }

    #[test]
    fn the_decision_vocabulary_is_closed_and_both_halves_travel_together() {
        assert_eq!(AUDIT_DECISION_OUTCOMES, ["ALLOWED", "DENIED"]);
        // An outcome outside the closed vocabulary is rejected.
        let corrupt = |outcome: Option<String>, reason: Option<String>| {
            AuditEventRow::new(
                "0192a5e0-0000-7000-8000-0000000000a2".to_string(),
                Some(tenant(7)),
                "user:operator-1".to_string(),
                "command.approve".to_string(),
                "command".to_string(),
                "0192a5e0-0000-7000-8000-0000000000c1".to_string(),
                None,
                None,
                outcome,
                reason,
                None,
                1_700_000_100_000,
                1_700_000_100_500,
                "0192a5e0-0000-7000-8000-0000000000f1".to_string(),
                None,
            )
            .map(|_| ())
        };
        assert_eq!(
            corrupt(Some("MAYBE".to_string()), Some("r".to_string())),
            Err(AuditEventReadError::InvalidFilter { field: "decision_outcome" })
        );
        // A bare allow/deny without the reason reference is rejected
        // (P1.11 — never a bare decision).
        assert_eq!(
            corrupt(Some(AUDIT_DECISION_DENIED.to_string()), None),
            Err(AuditEventReadError::InvalidFilter {
                field: "decision_reason_reference"
            })
        );
        // A reason reference without the outcome is rejected.
        assert_eq!(
            corrupt(None, Some("r".to_string())),
            Err(AuditEventReadError::InvalidFilter { field: "decision_outcome" })
        );
    }

    #[test]
    fn ingest_before_event_is_rejected_and_identity_fields_are_validated() {
        // P0.09 ordering on the read shape too.
        let mut row = base_row();
        row.event_time_unix_millis = 1_700_000_101_000;
        row.ingest_time_unix_millis = 1_700_000_100_500;
        let rebuilt = AuditEventRow::new(
            row.audit_id.clone(),
            row.tenant_id,
            row.actor.clone(),
            row.action.clone(),
            row.object_type.clone(),
            row.object_id.clone(),
            row.before_ref.clone(),
            row.after_ref.clone(),
            row.decision_outcome.clone(),
            row.decision_reason_reference.clone(),
            row.evidence.clone(),
            row.event_time_unix_millis,
            row.ingest_time_unix_millis,
            row.correlation_id.clone(),
            row.corrected_audit_id.clone(),
        );
        assert_eq!(
            rebuilt,
            Err(AuditEventReadError::InvalidFilter {
                field: "ingest_time_unix_millis"
            })
        );
        // An empty actor or correlation id is rejected (§21.3 correlation
        // is mandatory on the served record).
        let build = |actor: &str, correlation: &str| {
            AuditEventRow::new(
                "0192a5e0-0000-7000-8000-0000000000a3".to_string(),
                Some(tenant(7)),
                actor.to_string(),
                "command.approve".to_string(),
                "command".to_string(),
                "0192a5e0-0000-7000-8000-0000000000c1".to_string(),
                None,
                None,
                None,
                None,
                None,
                1_700_000_100_000,
                1_700_000_100_500,
                correlation.to_string(),
                None,
            )
            .map(|_| ())
        };
        assert_eq!(
            build("  ", "0192a5e0-0000-7000-8000-0000000000f1"),
            Err(AuditEventReadError::InvalidFilter { field: "actor" })
        );
        assert_eq!(
            build("user:operator-1", ""),
            Err(AuditEventReadError::InvalidFilter { field: "correlation_id" })
        );
    }

    #[test]
    fn a_correction_linkage_references_the_original() {
        let row = AuditEventRow::new(
            "0192a5e0-0000-7000-8000-0000000000a4".to_string(),
            Some(tenant(7)),
            "svc:audit-worker".to_string(),
            "audit.correct".to_string(),
            "audit_record".to_string(),
            "0192a5e0-0000-7000-8000-0000000000a1".to_string(),
            None,
            None,
            None,
            None,
            None,
            1_700_000_200_000,
            1_700_000_200_500,
            "0192a5e0-0000-7000-8000-0000000000f1".to_string(),
            Some("0192a5e0-0000-7000-8000-0000000000a1".to_string()),
        )
        .expect("the correction row builds");
        assert_eq!(
            row.corrected_audit_id.as_deref(),
            Some("0192a5e0-0000-7000-8000-0000000000a1"),
            "the correction is a NEW record referencing the original (P1.10)"
        );
        // An empty linkage is rejected.
        let bad = AuditEventRow::new(
            "0192a5e0-0000-7000-8000-0000000000a5".to_string(),
            Some(tenant(7)),
            "svc:audit-worker".to_string(),
            "audit.correct".to_string(),
            "audit_record".to_string(),
            "0192a5e0-0000-7000-8000-0000000000a1".to_string(),
            None,
            None,
            None,
            None,
            None,
            1_700_000_200_000,
            1_700_000_200_500,
            "0192a5e0-0000-7000-8000-0000000000f1".to_string(),
            Some(" ".to_string()),
        );
        assert_eq!(bad, Err(AuditEventReadError::InvalidFilter { field: "corrected_audit_id" }));
    }

    #[test]
    fn the_trait_is_object_safe_for_the_unit_of_work() {
        fn takes_dyn_reader(_: &dyn AuditEventReader) {}
        let _ = takes_dyn_reader as fn(&dyn AuditEventReader);
    }

    #[test]
    fn the_read_port_defines_exactly_two_read_methods_and_no_mutation() {
        // P4.36 step 3 (the P1.10 port rule applied to the API surface):
        // the read port is SELECT-only. Mechanical form: within the trait
        // declaration there are exactly two `fn ` declarations (both
        // reads), and the port's source defines no write- or mutation-named
        // method anywhere. The tokens are assembled with format! so this
        // test module does not itself contain the forbidden source strings.
        let source = include_str!("audit_read.rs");
        let start = source
            .find("pub trait AuditEventReader")
            .expect("the port is declared in this file");
        let end = start
            + source[start..]
                .find("\n}")
                .expect("the trait declaration closes at column 0");
        let trait_body = &source[start..end];
        assert_eq!(
            trait_body.matches("fn ").count(),
            2,
            "the port must define exactly the two read methods: {trait_body}"
        );
        assert!(trait_body.contains("fn find_audit_event"));
        assert!(trait_body.contains("fn list_audit_events"));
        for name in ["insert", "update", "delete", "append", "write", "amend", "revise", "edit", "export", "dump"] {
            assert!(
                !trait_body.contains(&format!("fn {name}")),
                "the audit read port must never define a write/mutation method ({name})"
            );
        }
    }
}
