// P1.10 — AuditRecord (§10.1 via P0.10) and the correction linkage.
//
// Part of the flexcore-audit P1.10 deliverable. The §10.1 field list (via
// `ledger/phase-00-product-freeze/10_asset_capability_vocabulary.md` §1,
// AuditRecord row) is: "actor, action, object, before/after references,
// decision, evidence, timestamp, correlation ID". The column-level mapping
// follows the P1.12 §4.7 `audit_records` design, which is the storage
// counterpart of this type.
//
// Field-by-field, §10.1 → this type:
//
// | §10.1 field            | Here |
// |---|---|
// | actor                  | `actor: ActorIdentity` — authenticated actor identity from claims (P0.16 rule 5: tenant context and actor identity come from authenticated claims, never from untrusted request fields) |
// | action                 | `action: AuditAction` |
// | object                 | `object: AuditObject` (type + id) |
// | before/after references| `before_ref` / `after_ref: Option<RecordReference>` — *references* (identifier/version pointers), not full payload copies (P1.12 §4.7: before_ref/after_ref are references, "not a full payload copy") |
// | decision               | `decision: Option<AuditDecision>` — a decision value plus a reason/evidence reference, never a bare allow/deny (P1.11 decision discipline mirrored here; P1.12 §4.7 decision column) |
// | evidence               | `evidence: Option<EvidenceReference>` |
// | timestamp              | `event_time: EventTime` (when the audited action occurred) + `ingest_time: IngestTime` (when the record was written) — the two are **distinct types** per the P0.09 time model and must never be merged or substituted |
// | correlation ID         | `correlation_id: FlexUuid` — workflow correlation (report §21.3 trace propagation; same representation as the event payloads) |
//
// Plus the tenant context where applicable: `tenant_id: Option<TenantId>` —
// `None` marks a platform-level action with no tenant context (P1.12 §4.7:
// NULL allowed, FK when set). Plus `audit_id: FlexUuid`, the record
// identifier, supplied by the caller (ID *generation* is the P1.07
// id-generation port's concern, not this crate's).
//
// Append-orientation (P1.10 step 3): audit records are append-oriented;
// corrections are NEW records referencing the original — see
// [`CorrectionRecord`] and the crate README. There is no in-place edit
// operation anywhere in this crate, and none may be added.

use flexcore_contracts::ids::FlexUuid;
use flexcore_contracts::time_model::{EventTime, IngestTime};
use flexcore_domain::ids::TenantId;
use serde::{Deserialize, Serialize};

/// Errors returned by audit-record construction and by the append port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditError {
    /// The actor identity was empty or all whitespace.
    EmptyActor,
    /// The action was empty or all whitespace.
    EmptyAction,
    /// The object type was empty or all whitespace.
    EmptyObjectType,
    /// The object id was empty or all whitespace.
    EmptyObjectId,
    /// A before/after reference or decision-reason reference was empty.
    EmptyRecordReference,
    /// The evidence reference was empty.
    EmptyEvidenceReference,
    /// The correction reason was empty.
    EmptyCorrectionReason,
    /// The record was written before the action it records occurred:
    /// `ingest_time` must not precede `event_time` (P0.09 ordering rule).
    IngestBeforeEvent,
    /// The audit backend rejected the append ([`crate::sink::AuditSink`]):
    /// storage failure, policy, or capacity. The record was NOT appended;
    /// the caller decides whether to retry or fail the operation that
    /// produced the evidence. A rejected append never leaves a partial or
    /// mutated trail behind.
    Rejected {
        /// The backend-supplied reason for the rejection.
        reason: String,
    },
}

impl std::fmt::Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditError::EmptyActor => write!(f, "the audit actor identity must be non-empty"),
            AuditError::EmptyAction => write!(f, "the audit action must be non-empty"),
            AuditError::EmptyObjectType => {
                write!(f, "the audit object type must be non-empty")
            }
            AuditError::EmptyObjectId => write!(f, "the audit object id must be non-empty"),
            AuditError::EmptyRecordReference => {
                write!(f, "a record reference must be non-empty")
            }
            AuditError::EmptyEvidenceReference => {
                write!(f, "the evidence reference must be non-empty")
            }
            AuditError::EmptyCorrectionReason => {
                write!(f, "a correction reason must be non-empty")
            }
            AuditError::IngestBeforeEvent => {
                write!(f, "ingest_time must not precede event_time (P0.09)")
            }
            AuditError::Rejected { reason } => {
                write!(f, "the audit backend rejected the append: {reason}")
            }
        }
    }
}

impl std::error::Error for AuditError {}

/// Builds a validated, non-empty string newtype used as one audit field.
///
/// Validation: non-empty and not all whitespace (interior whitespace is
/// legitimate, e.g. in an actor's display name). Serialization: the inner
/// string; deserialization re-runs the validation, so a parsed value always
/// satisfies the constructor rule.
macro_rules! audit_string_newtype {
    (
        $(#[$meta:meta])*
        $name:ident, $error:ident
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(into = "String", try_from = "String")]
        pub struct $name(String);

        impl $name {
            /// Validates and wraps the value: non-empty, not all whitespace.
            pub fn new(value: String) -> Result<Self, AuditError> {
                if value.trim().is_empty() {
                    return Err(AuditError::$error);
                }
                Ok(Self(value))
            }

            /// The value as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = AuditError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<$name> for String {
            fn from(v: $name) -> Self {
                v.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

audit_string_newtype! {
    /// Who performed the audited action: the authenticated actor identity
    /// (§10.1 "actor"). From claims, never from untrusted request fields
    /// (P0.16 rule 5).
    ActorIdentity, EmptyActor
}

audit_string_newtype! {
    /// What was done (§10.1 "action"), e.g. `asset.register`,
    /// `command.approve`, `plugin.install`.
    AuditAction, EmptyAction
}

audit_string_newtype! {
    /// The type of the audited object (§10.1 "object", type half), e.g.
    /// `asset`, `command`, `plugin`, `tenant`.
    ObjectType, EmptyObjectType
}

audit_string_newtype! {
    /// The identifier of the audited object (§10.1 "object", id half). A
    /// string on purpose: audit objects cross entity kinds, and the typed
    /// identifier wrappers live with their own aggregates — the audit trail
    /// records the reference, it does not re-own the entity.
    ObjectId, EmptyObjectId
}

audit_string_newtype! {
    /// A before/after *reference* (§10.1): a pointer to the previous or new
    /// state (identifier/version), never a full payload copy (P1.12 §4.7).
    RecordReference, EmptyRecordReference
}

audit_string_newtype! {
    /// An evidence reference (§10.1 "evidence"): a pointer to supporting
    /// evidence (a document, a decision record, a trace). The reference is
    /// recorded, not the evidence itself.
    EvidenceReference, EmptyEvidenceReference
}

audit_string_newtype! {
    /// Why a correction was made (P1.10 step 3: corrections carry a reason
    /// and reference the original record).
    CorrectionReason, EmptyCorrectionReason
}

/// The outcome half of a recorded authorization decision. Never a bare
/// boolean: the decision is only meaningful together with its reason
/// reference ([`AuditDecision::reason_reference`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DecisionOutcome {
    /// The action was allowed.
    Allowed,
    /// The action was denied (P0.16 rule 6: audit cross-tenant access
    /// denials).
    Denied,
}

/// A recorded authorization decision (§10.1 "decision"): outcome plus the
/// reference to the reason/evidence for it. Mirrors the P1.11 rule that a
/// decision is an explicit value with a reason, never a bare allow/deny;
/// this type records the decision in the audit trail, it does not make
/// decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditDecision {
    /// The decision outcome.
    pub outcome: DecisionOutcome,
    /// Reference to the reason/evidence behind the decision.
    pub reason_reference: RecordReference,
}

impl AuditDecision {
    /// Builds a decision with its reason reference (mandatory — there is no
    /// way to record a decision without a reason).
    pub fn new(outcome: DecisionOutcome, reason_reference: String) -> Result<Self, AuditError> {
        Ok(Self {
            outcome,
            reason_reference: RecordReference::new(reason_reference)?,
        })
    }
}

/// The audited object (§10.1 "object"): type and id.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AuditObject {
    /// The object type (e.g. `asset`, `command`).
    pub object_type: ObjectType,
    /// The object identifier.
    pub object_id: ObjectId,
}

impl AuditObject {
    /// Builds the object from type and id strings, validating both.
    pub fn new(object_type: String, object_id: String) -> Result<Self, AuditError> {
        Ok(Self {
            object_type: ObjectType::new(object_type)?,
            object_id: ObjectId::new(object_id)?,
        })
    }
}

/// The correction linkage carried by a record that corrects an earlier one
/// (P1.10 step 3): the original record's identifier and the reason.
///
/// A correction is a **new** record appended to the trail, referencing the
/// original — the original is never edited, overwritten, or deleted, at
/// this layer or below it (P1.12 §4.7: `corrected_audit_id` is a new row's
/// FK to the original; grants and a defence-in-depth trigger keep the table
/// itself append-only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorrectionRecord {
    original_audit_id: FlexUuid,
    reason: CorrectionReason,
}

impl CorrectionRecord {
    /// Builds the linkage to the original record and the reason for the
    /// correction. Both parts are mandatory.
    pub fn new(
        original_audit_id: FlexUuid,
        reason: String,
    ) -> Result<Self, AuditError> {
        Ok(Self {
            original_audit_id,
            reason: CorrectionReason::new(reason)?,
        })
    }

    /// The identifier of the original record this correction references.
    pub fn original_audit_id(&self) -> FlexUuid {
        self.original_audit_id
    }

    /// The reason the correction was made.
    pub fn reason(&self) -> &str {
        self.reason.as_str()
    }
}

#[derive(Serialize, Deserialize)]
struct CorrectionRecordWire {
    original_audit_id: FlexUuid,
    reason: String,
}

impl From<CorrectionRecord> for CorrectionRecordWire {
    fn from(c: CorrectionRecord) -> Self {
        Self {
            original_audit_id: c.original_audit_id,
            reason: c.reason.as_str().to_string(),
        }
    }
}

impl TryFrom<CorrectionRecordWire> for CorrectionRecord {
    type Error = AuditError;

    fn try_from(wire: CorrectionRecordWire) -> Result<Self, Self::Error> {
        Self::new(wire.original_audit_id, wire.reason)
    }
}

impl Serialize for CorrectionRecord {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        CorrectionRecordWire::from(self.clone()).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CorrectionRecord {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        CorrectionRecordWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

/// The field set of an [`AuditRecord`], as one constructible value.
///
/// Every component is a validated type except the identifiers, so the only
/// cross-field check left to [`AuditRecord::new`] is the P0.09 time
/// ordering (`ingest_time` must not precede `event_time`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecordFields {
    /// The record identifier. Supplied by the caller: ID generation is the
    /// P1.07 id-generation port's concern, not this crate's.
    pub audit_id: FlexUuid,
    /// Tenant context where applicable; `None` marks a platform-level
    /// action with no tenant context (P1.12 §4.7).
    pub tenant_id: Option<TenantId>,
    /// Who performed the action (from authenticated claims, P0.16 rule 5).
    pub actor: ActorIdentity,
    /// What was done.
    pub action: AuditAction,
    /// The object acted on (type + id).
    pub object: AuditObject,
    /// Reference to the previous state, where applicable.
    pub before_ref: Option<RecordReference>,
    /// Reference to the new state, where applicable.
    pub after_ref: Option<RecordReference>,
    /// The authorization decision, where one was made (outcome + reason
    /// reference, never a bare allow/deny).
    pub decision: Option<AuditDecision>,
    /// Evidence reference, where applicable.
    pub evidence: Option<EvidenceReference>,
    /// When the audited action occurred (P0.09; distinct from
    /// `ingest_time`).
    pub event_time: EventTime,
    /// When the record was written (P0.09; distinct from `event_time`).
    pub ingest_time: IngestTime,
    /// Workflow correlation ID (report §21.3 trace propagation).
    pub correlation_id: FlexUuid,
}

/// One audit evidence record (§10.1 via P0.10), the append-only trail's
/// unit.
///
/// The record is immutable by construction: no setter exists, no field can
/// be mutated through this type, and the one "change" the audit trail
/// supports is appending a correction record that references this one (see
/// [`CorrectionRecord`] and the crate README). Corrections never edit in
/// place.
///
/// Deserialization re-runs [`AuditRecord::new`] (the P0.09 ordering rule
/// holds on every parse path); the correction linkage revalidates its
/// reason through [`CorrectionRecord`]'s own validating serde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "AuditRecordWire")]
pub struct AuditRecord {
    /// The record identifier (supplied by the caller).
    pub audit_id: FlexUuid,
    /// Tenant context where applicable; `None` = platform-level action.
    #[serde(with = "serde_ids::option_tenant_id")]
    pub tenant_id: Option<TenantId>,
    /// The authenticated actor (§10.1 actor; P0.16 rule 5).
    pub actor: ActorIdentity,
    /// The action (§10.1 action).
    pub action: AuditAction,
    /// The object (§10.1 object: type + id).
    pub object: AuditObject,
    /// Before reference (§10.1 before reference), where applicable.
    pub before_ref: Option<RecordReference>,
    /// After reference (§10.1 after reference), where applicable.
    pub after_ref: Option<RecordReference>,
    /// The decision (§10.1 decision), where one was made.
    pub decision: Option<AuditDecision>,
    /// Evidence reference (§10.1 evidence), where applicable.
    pub evidence: Option<EvidenceReference>,
    /// When the action occurred (P0.09 event time).
    pub event_time: EventTime,
    /// When the record was written (P0.09 ingest time).
    pub ingest_time: IngestTime,
    /// Workflow correlation ID (§21.3).
    pub correlation_id: FlexUuid,
    /// The correction linkage, when this record corrects an earlier one. A
    /// record with this set is a NEW record referencing the original — the
    /// original record is never edited.
    pub corrects: Option<CorrectionRecord>,
}

impl AuditRecord {
    /// Builds and validates an audit record from its field set.
    ///
    /// The one cross-field rule: `ingest_time` must not precede
    /// `event_time` — a record cannot be written before the action it
    /// records occurred (P0.09 ordering rule).
    pub fn new(fields: AuditRecordFields) -> Result<Self, AuditError> {
        if fields.ingest_time.timestamp() < fields.event_time.timestamp() {
            return Err(AuditError::IngestBeforeEvent);
        }
        Ok(Self {
            audit_id: fields.audit_id,
            tenant_id: fields.tenant_id,
            actor: fields.actor,
            action: fields.action,
            object: fields.object,
            before_ref: fields.before_ref,
            after_ref: fields.after_ref,
            decision: fields.decision,
            evidence: fields.evidence,
            event_time: fields.event_time,
            ingest_time: fields.ingest_time,
            correlation_id: fields.correlation_id,
            // A plain `new` never produces a correction; corrections are
            // built through `AuditRecord::link_correction`, which sets this
            // field with a validated linkage.
            corrects: None,
        })
    }

    /// Marks this record as a correction of `original_audit_id` with a
    /// mandatory reason, returning the corrected record ready to append.
    ///
    /// The original record is not touched: the correction is a new record
    /// whose `corrects` linkage references it (P1.10 step 3; P1.12 §4.7
    /// `corrected_audit_id`).
    pub fn link_correction(
        mut self,
        original_audit_id: FlexUuid,
        reason: String,
    ) -> Result<Self, AuditError> {
        self.corrects = Some(CorrectionRecord::new(original_audit_id, reason)?);
        Ok(self)
    }
}

/// Deserialization target for [`AuditRecord`]; the `TryFrom` re-runs the
/// validating constructor so a parsed record always satisfies the
/// construction rules.
#[derive(Deserialize)]
struct AuditRecordWire {
    pub audit_id: FlexUuid,
    #[serde(with = "serde_ids::option_tenant_id")]
    pub tenant_id: Option<TenantId>,
    pub actor: ActorIdentity,
    pub action: AuditAction,
    pub object: AuditObject,
    pub before_ref: Option<RecordReference>,
    pub after_ref: Option<RecordReference>,
    pub decision: Option<AuditDecision>,
    pub evidence: Option<EvidenceReference>,
    pub event_time: EventTime,
    pub ingest_time: IngestTime,
    pub correlation_id: FlexUuid,
    pub corrects: Option<CorrectionRecord>,
}

impl TryFrom<AuditRecordWire> for AuditRecord {
    type Error = AuditError;

    fn try_from(wire: AuditRecordWire) -> Result<Self, Self::Error> {
        let mut record = AuditRecord::new(AuditRecordFields {
            audit_id: wire.audit_id,
            tenant_id: wire.tenant_id,
            actor: wire.actor,
            action: wire.action,
            object: wire.object,
            before_ref: wire.before_ref,
            after_ref: wire.after_ref,
            decision: wire.decision,
            evidence: wire.evidence,
            event_time: wire.event_time,
            ingest_time: wire.ingest_time,
            correlation_id: wire.correlation_id,
        })?;
        record.corrects = wire.corrects;
        Ok(record)
    }
}

/// Field-level (de)serialization helpers for the domain typed identifiers.
///
/// The domain crate deliberately takes no serde dependency (same situation
/// as the flexcore-events crate, P1.09), so this module delegates to
/// `FlexUuid`'s serde impl and wraps/reads the typed ID newtype around it.
mod serde_ids {
    use super::TenantId;
    use flexcore_contracts::ids::FlexUuid;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub mod option_tenant_id {
        use super::*;

        pub fn serialize<S: Serializer>(
            tenant: &Option<TenantId>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match tenant {
                Some(t) => FlexUuid::serialize(&t.uuid(), serializer),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<TenantId>, D::Error> {
            let uuid = Option::<FlexUuid>::deserialize(deserializer)?;
            Ok(uuid.map(TenantId::new))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_uuid(tag: u8) -> FlexUuid {
        let mut bytes = [0u8; 16];
        bytes[15] = tag;
        FlexUuid::from_bytes(bytes)
    }

    fn sample_fields() -> AuditRecordFields {
        AuditRecordFields {
            audit_id: sample_uuid(1),
            tenant_id: Some(TenantId::new(sample_uuid(2))),
            actor: ActorIdentity::new("operator-1".to_string()).expect("valid actor"),
            action: AuditAction::new("asset.register".to_string()).expect("valid action"),
            object: AuditObject::new("asset".to_string(), "asset-1".to_string())
                .expect("valid object"),
            before_ref: None,
            after_ref: Some(RecordReference::new("asset-1:v2".to_string()).expect("valid ref")),
            decision: None,
            evidence: None,
            event_time: EventTime::from_unix_millis(1_000).expect("valid event time"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid ingest time"),
            correlation_id: sample_uuid(3),
        }
    }

    fn sample_record() -> AuditRecord {
        AuditRecord::new(sample_fields()).expect("valid record")
    }

    #[test]
    fn record_carries_the_full_s10_1_field_set() {
        let record = sample_record();
        assert_eq!(record.actor.as_str(), "operator-1");
        assert_eq!(record.action.as_str(), "asset.register");
        assert_eq!(record.object.object_type.as_str(), "asset");
        assert_eq!(record.object.object_id.as_str(), "asset-1");
        assert_eq!(
            record.after_ref.as_ref().map(|r| r.as_str()),
            Some("asset-1:v2")
        );
        assert!(record.before_ref.is_none());
        assert!(record.decision.is_none());
        assert!(record.evidence.is_none());
        assert_eq!(record.tenant_id.map(|t| t.uuid()), Some(sample_uuid(2)));
        assert_eq!(record.correlation_id, sample_uuid(3));
        // The two P0.09 timestamps are present and distinct types.
        assert_eq!(record.event_time.unix_millis(), 1_000);
        assert_eq!(record.ingest_time.unix_millis(), 2_000);
    }

    #[test]
    fn ingest_before_event_is_rejected() {
        // A record cannot be written before the action occurred (P0.09).
        let mut fields = sample_fields();
        fields.event_time = EventTime::from_unix_millis(3_000).unwrap();
        fields.ingest_time = IngestTime::from_unix_millis(2_000).unwrap();
        assert_eq!(AuditRecord::new(fields), Err(AuditError::IngestBeforeEvent));
    }

    #[test]
    fn empty_field_values_are_rejected() {
        assert_eq!(
            ActorIdentity::new(String::new()),
            Err(AuditError::EmptyActor)
        );
        assert_eq!(
            ActorIdentity::new("   ".to_string()),
            Err(AuditError::EmptyActor)
        );
        assert_eq!(
            AuditAction::new(String::new()),
            Err(AuditError::EmptyAction)
        );
        assert_eq!(
            AuditObject::new(String::new(), "x".to_string()),
            Err(AuditError::EmptyObjectType)
        );
        assert_eq!(
            AuditObject::new("asset".to_string(), String::new()),
            Err(AuditError::EmptyObjectId)
        );
        assert_eq!(
            RecordReference::new(String::new()),
            Err(AuditError::EmptyRecordReference)
        );
        assert_eq!(
            EvidenceReference::new(String::new()),
            Err(AuditError::EmptyEvidenceReference)
        );
        assert_eq!(
            CorrectionReason::new(String::new()),
            Err(AuditError::EmptyCorrectionReason)
        );
    }

    #[test]
    fn a_decision_always_carries_a_reason_reference() {
        let allowed = AuditDecision::new(DecisionOutcome::Allowed, "policy-ref-1".to_string())
            .expect("valid decision");
        assert_eq!(allowed.outcome, DecisionOutcome::Allowed);
        assert_eq!(allowed.reason_reference.as_str(), "policy-ref-1");

        // No reason, no decision value.
        assert_eq!(
            AuditDecision::new(DecisionOutcome::Denied, String::new()),
            Err(AuditError::EmptyRecordReference)
        );
    }

    #[test]
    fn correction_links_to_the_original_without_touching_it() {
        // P1.10 test: a correction record links to the original.
        let original = sample_record();
        let correction = sample_record()
            .link_correction(original.audit_id, "wrong after-ref recorded".to_string())
            .expect("valid correction");
        let link = correction.corrects.as_ref().expect("linkage set");
        assert_eq!(link.original_audit_id(), original.audit_id);
        assert_eq!(link.reason(), "wrong after-ref recorded");
        // The original is untouched: same id, no linkage on it.
        assert!(original.corrects.is_none());
        // A correction needs a reason.
        assert_eq!(
            sample_record().link_correction(original.audit_id, String::new()),
            Err(AuditError::EmptyCorrectionReason)
        );
    }

    #[test]
    fn record_round_trips_through_serde_json() {
        let record = sample_record()
            .link_correction(sample_uuid(9), "reason".to_string())
            .unwrap();
        let json = serde_json::to_value(&record).unwrap();
        let back: AuditRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, record);
        // The correction linkage survives the round trip, revalidated.
        assert_eq!(
            back.corrects.as_ref().map(|c| c.original_audit_id()),
            Some(sample_uuid(9))
        );
    }

    #[test]
    fn deserialization_revalidates_the_time_ordering() {
        let record = sample_record();
        let mut json = serde_json::to_value(&record).unwrap();
        json["event_time"] = serde_json::json!("1970-01-01T00:10:00Z");
        json["ingest_time"] = serde_json::json!("1970-01-01T00:00:02Z");
        let err = serde_json::from_value::<AuditRecord>(json).unwrap_err();
        assert!(
            err.to_string().contains("ingest_time"),
            "the ordering rule must fail the parse: {err}"
        );
    }

    #[test]
    fn platform_level_actions_deserialize_with_null_tenant() {
        let mut fields = sample_fields();
        fields.tenant_id = None;
        let record = AuditRecord::new(fields).unwrap();
        let json = serde_json::to_value(&record).unwrap();
        assert_eq!(json["tenant_id"], serde_json::Value::Null);
        let back: AuditRecord = serde_json::from_value(json).unwrap();
        assert_eq!(back, record);
        assert!(back.tenant_id.is_none());
    }

    #[test]
    fn string_newtypes_serde_round_trip_and_revalidate() {
        let actor = ActorIdentity::new("svc:worker".to_string()).unwrap();
        let json = serde_json::to_value(&actor).unwrap();
        assert_eq!(json, serde_json::json!("svc:worker"));
        let back: ActorIdentity = serde_json::from_value(json).unwrap();
        assert_eq!(back, actor);
        // An empty actor never parses back.
        assert!(serde_json::from_value::<ActorIdentity>(serde_json::json!("")).is_err());
    }

    #[test]
    fn decision_outcome_serde_uses_the_frozen_names() {
        for outcome in [DecisionOutcome::Allowed, DecisionOutcome::Denied] {
            let back: DecisionOutcome =
                serde_json::from_value(serde_json::to_value(&outcome).unwrap()).unwrap();
            assert_eq!(back, outcome);
        }
        assert!(serde_json::from_value::<DecisionOutcome>(serde_json::json!("MAYBE")).is_err());
    }
}
