// P1.10 — The append-only audit write port (`AuditSink`).
//
// Part of the flexcore-audit P1.10 deliverable (P1.10 step 2; report §6.5,
// §10.1; P1.12 §4.7 layer 3 of the append-only enforcement).
//
// This port lives in flexcore-audit and NOT in flexcore-ports, by recorded
// decision (P1.10): it is **not a repository trait**. A repository trait
// abstracts an aggregate's read/write lifecycle; this port is a one-way
// write valve for evidence — it has no read path, no query path, and
// exactly one write operation. The single-home rule for repository traits
// (P1.10 step 1) is untouched by this exception.
//
// THE APPEND-ONLY RULE, as a type-level fact:
//
// `append` is the ONLY method on this trait. There is no update method, no
// delete method, no correction-in-place method, no "rewrite" or "amend"
// operation, and none may be added: a correction is a NEW record whose
// `corrects` linkage references the original (see `record::CorrectionRecord`
// and `AuditRecord::link_correction`). This is layer 3 of the P1.12 §4.7
// three-layer append-only enforcement (1: grants — INSERT/SELECT only; 2:
// defence-in-depth trigger raising on row mutation attempts; 3: this port).
// The mechanical evidence for the P1.10 acceptance criterion "the audit
// port has no update/delete method (verified by API inspection)" is:
//
//   - the source-text test below (it asserts the trait body contains
//     exactly one method and forbids mutation-named methods in the port's
//     source), and
//   - the `grep -n "fn " src/` output recorded in the crate README.
//
// Reviewers: read the trait below — one method, `append`, taking an
// immutable reference and returning only an error possibility.

use crate::record::{AuditError, AuditRecord};

/// The append-only audit write port (P1.10 step 2).
///
/// Append is the only write operation. An implementation stores the record
/// as given and returns an error only if the backend rejects the write
/// (storage failure, policy, capacity) — it may never mutate, reorder,
/// redact, or drop a record it has accepted, and it may never expose update
/// or delete operations through this trait.
pub trait AuditSink {
    /// Appends one audit record. Exactly-once ordering per implementation
    /// is the backend's concern; the port guarantees only that a successful
    /// return means the record was accepted for durable append.
    fn append(&self, record: AuditRecord) -> Result<(), AuditError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::{
        ActorIdentity, AuditAction, AuditObject, AuditRecordFields,
    };
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_contracts::time_model::{EventTime, IngestTime};
    use flexcore_domain::ids::TenantId;
    use std::sync::Mutex;

    const SINK_SOURCE: &str = include_str!("sink.rs");

    fn sample_uuid(tag: u8) -> FlexUuid {
        let mut bytes = [0u8; 16];
        bytes[15] = tag;
        FlexUuid::from_bytes(bytes)
    }

    fn sample_record(tag: u8) -> AuditRecord {
        AuditRecord::new(AuditRecordFields {
            audit_id: sample_uuid(tag),
            tenant_id: Some(TenantId::new(sample_uuid(2))),
            actor: ActorIdentity::new("operator-1".to_string()).expect("valid actor"),
            action: AuditAction::new("asset.register".to_string()).expect("valid action"),
            object: AuditObject::new("asset".to_string(), "asset-1".to_string())
                .expect("valid object"),
            before_ref: None,
            after_ref: None,
            decision: None,
            evidence: None,
            event_time: EventTime::from_unix_millis(1_000).expect("valid event time"),
            ingest_time: IngestTime::from_unix_millis(2_000).expect("valid ingest time"),
            correlation_id: sample_uuid(3),
        })
        .expect("valid record")
    }

    /// An in-memory fake sink: the reference implementation for tests, and
    /// the proof that a working `AuditSink` needs nothing but `append`.
    #[derive(Default)]
    struct MemorySink {
        records: Mutex<Vec<AuditRecord>>,
    }

    impl AuditSink for MemorySink {
        fn append(&self, record: AuditRecord) -> Result<(), AuditError> {
            self.records
                .lock()
                .expect("poisoned mutex in test fake")
                .push(record);
            Ok(())
        }
    }

    /// A sink whose backend fails, to prove errors propagate unchanged.
    struct FailingSink;

    impl AuditSink for FailingSink {
        fn append(&self, _record: AuditRecord) -> Result<(), AuditError> {
            Err(AuditError::Rejected {
                reason: "backend unavailable".to_string(),
            })
        }
    }

    #[test]
    fn append_stores_the_record_in_an_in_memory_sink() {
        // P1.10 test: audit append succeeds into an in-memory fake.
        let sink = MemorySink::default();
        let record = sample_record(1);
        sink.append(record.clone()).expect("append succeeds");
        sink.append(sample_record(4)).expect("append succeeds");
        let stored = sink.records.lock().unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0], record);
    }

    #[test]
    fn append_via_a_trait_object() {
        // The port is object-safe in practice: application code holds it as
        // `dyn AuditSink` behind the application layer.
        let sink: &dyn AuditSink = &MemorySink::default();
        sink.append(sample_record(5)).expect("append succeeds");
    }

    #[test]
    fn a_backend_failure_propagates_as_an_error() {
        let sink = FailingSink;
        assert!(sink.append(sample_record(6)).is_err());
    }

    #[test]
    fn the_port_defines_exactly_one_method_named_append() {
        // P1.10 acceptance: "the audit port has no update/delete method
        // (verified by API inspection)". Mechanical form: within the trait
        // declaration there is exactly one `fn `, and the port source
        // defines no mutation-named method anywhere.
        let start = SINK_SOURCE
            .find("pub trait AuditSink")
            .expect("the port is declared in this file");
        let end = start + SINK_SOURCE[start..]
            .find("\n}")
            .expect("the trait declaration closes at column 0");
        let trait_body = &SINK_SOURCE[start..end];
        assert_eq!(
            trait_body.matches("fn ").count(),
            1,
            "the port must define exactly one method (append): {trait_body}"
        );
        assert!(
            trait_body.contains("fn append(&self, record: AuditRecord)"),
            "the single method is append over an immutable record"
        );

        // No mutation-named method may appear anywhere in the port's source.
        // The tokens are assembled with format! so this test module does not
        // itself contain the forbidden source strings.
        for name in ["update", "delete", "correct", "amend", "revise", "remove", "edit"] {
            assert!(
                !SINK_SOURCE.contains(&format!("fn {name}")),
                "the audit port must never define a mutation method ({name})"
            );
        }
    }
}
