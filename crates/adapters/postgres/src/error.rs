// P1.13 — Error mapping: sqlx failures to opaque port errors.
//
// Rule (P1.07 `backend_error`; P1.08 error taxonomy; report §17 hygiene):
// port errors never carry raw storage text — no statement fragments, no
// engine error output, no connection details. The adapter therefore maps
// every sqlx failure to a stable, adapter-defined short code plus a fixed,
// human-safe message derived from the error CLASS only. Constraint and
// object names (schema identifiers) are the one deliberate exception kept
// out of messages entirely: classification is by error kind, not text.
//
// sqlx error kinds used here (sqlx::error::DatabaseErrorKind) are matched,
// never string-parsed, so the mapping survives driver version changes.

use flexcore_audit::record::AuditError;
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::event_outbox::EventOutboxError;
use flexcore_ports::repository::RepositoryError;

/// Stable adapter codes reported through port errors.
pub(crate) mod codes {
    pub const UNIQUE: &str = "E_STORE_UNIQUE";
    pub const CHECK: &str = "E_STORE_CHECK";
    pub const FOREIGN_KEY: &str = "E_STORE_FOREIGN_KEY";
    pub const NOT_NULL: &str = "E_STORE_NOT_NULL";
    pub const UNAVAILABLE: &str = "E_STORE_UNAVAILABLE";
    pub const TIMEOUT: &str = "E_STORE_TIMEOUT";
    pub const READ_ONLY: &str = "E_STORE_READ_ONLY";
    pub const AUTH: &str = "E_STORE_AUTH";
    pub const REJECTED: &str = "E_STORE_REJECTED";
    pub const DECODE: &str = "E_STORE_DECODE";
    pub const REFERENTIAL: &str = "E_STORE_REFERENTIAL";
    pub const MIGRATION: &str = "E_STORE_MIGRATION";
    pub const OUTBOX_PAYLOAD: &str = "E_OUTBOX_PAYLOAD";
    pub const OUTBOX_REQUIRES_UOW: &str = "E_OUTBOX_REQUIRES_UNIT_OF_WORK";
    pub const UOW_FINALIZED: &str = "E_UNIT_OF_WORK_FINALIZED";
    pub const RUNTIME: &str = "E_RUNTIME";
}

/// Classifies a sqlx failure into a stable code plus a fixed human-safe
/// message. Never inspects error text.
pub(crate) fn classify(err: &sqlx::Error) -> (&'static str, String) {
    match err {
        sqlx::Error::Database(db) => match db.kind() {
            sqlx::error::DatabaseErrorKind::UniqueViolation => (
                codes::UNIQUE,
                "a uniqueness rule was violated".to_string(),
            ),
            sqlx::error::DatabaseErrorKind::CheckViolation => (
                codes::CHECK,
                "a data constraint was violated".to_string(),
            ),
            sqlx::error::DatabaseErrorKind::ForeignKeyViolation => (
                codes::FOREIGN_KEY,
                "a referenced record is missing or still in use".to_string(),
            ),
            sqlx::error::DatabaseErrorKind::NotNullViolation => (
                codes::NOT_NULL,
                "a required field was missing".to_string(),
            ),
            sqlx::error::DatabaseErrorKind::ClosedConnection => (
                codes::UNAVAILABLE,
                "the storage backend closed the connection".to_string(),
            ),
            sqlx::error::DatabaseErrorKind::QueryTimeout => (
                codes::TIMEOUT,
                "the storage backend timed out".to_string(),
            ),
            sqlx::error::DatabaseErrorKind::ReadOnlyTransaction => (
                codes::READ_ONLY,
                "the transaction is read-only".to_string(),
            ),
            sqlx::error::DatabaseErrorKind::InvalidPassword => (
                codes::AUTH,
                "storage credentials were rejected".to_string(),
            ),
            _ => (
                codes::REJECTED,
                "the storage backend rejected the operation".to_string(),
            ),
        },
        sqlx::Error::ColumnDecode { index, .. } => (
            codes::DECODE,
            format!("a stored value could not be decoded (column {index})"),
        ),
        sqlx::Error::ColumnNotFound(name) => (
            codes::DECODE,
            format!("a queried column is missing ({name})"),
        ),
        sqlx::Error::RowNotFound => (
            codes::DECODE,
            "an expected row was not found".to_string(),
        ),
        _ => (
            codes::UNAVAILABLE,
            "the storage backend is unavailable".to_string(),
        ),
    }
}

/// Maps a sqlx failure to the opaque [`BackendError`] shape.
pub(crate) fn map_backend(err: sqlx::Error) -> BackendError {
    let (code, message) = classify(&err);
    BackendError::new(code.to_string(), message)
}

/// `true` only for a database-level uniqueness violation (the durable form
/// of the outbox idempotency rule and the repository Conflict class).
pub(crate) fn is_unique_violation(err: &sqlx::Error) -> bool {
    matches!(
        err,
        sqlx::Error::Database(db)
            if matches!(db.kind(), sqlx::error::DatabaseErrorKind::UniqueViolation)
    )
}

/// Maps a sqlx failure to a repository port error.
pub(crate) fn map_repository_error(entity: &'static str, err: sqlx::Error) -> RepositoryError {
    if is_unique_violation(&err) {
        return RepositoryError::Conflict {
            entity,
            reason: "a uniqueness rule was violated (identifier or tenant-scoped natural key)"
                .to_string(),
        };
    }
    RepositoryError::Backend(map_backend(err))
}

/// Maps a sqlx failure to an outbox port error. A uniqueness violation on
/// the insert path is the durable duplicate-key signal (P1.12 §4.6 unique
/// constraint); the colliding key is the one the caller supplied.
pub(crate) fn map_outbox_error(err: sqlx::Error, idempotency_key: &str) -> EventOutboxError {
    if is_unique_violation(&err) {
        return EventOutboxError::DuplicateKey {
            idempotency_key: idempotency_key.to_string(),
        };
    }
    EventOutboxError::Backend(map_backend(err))
}

/// Maps a sqlx failure to the audit sink's rejection error. The reason is
/// the fixed human-safe class message, never raw storage text.
pub(crate) fn map_audit_rejection(err: sqlx::Error) -> AuditError {
    let (_, message) = classify(&err);
    AuditError::Rejected { reason: message }
}

/// A decode failure for one named column, as an opaque backend error.
pub(crate) fn decode_failure(column: &'static str) -> BackendError {
    BackendError::new(
        codes::DECODE.to_string(),
        format!("stored value in column {column} could not be decoded into the domain model"),
    )
}

/// A referential inconsistency (a parent row referenced by a child row is
/// missing or invisible) as an opaque backend error.
pub(crate) fn referential_failure(what: &'static str) -> BackendError {
    BackendError::new(
        codes::REFERENTIAL.to_string(),
        format!("{what}: the referenced record is missing or invisible in the caller's scope"),
    )
}

/// The error for a port call on an already-finalized unit of work.
pub(crate) fn unit_of_work_finalized() -> BackendError {
    BackendError::new(
        codes::UOW_FINALIZED.to_string(),
        "the unit of work was already committed or rolled back; open a new one with FlexCoreStore::begin".to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_database_errors_classify_as_unavailable() {
        // A transport-level failure (no database error object exists) maps
        // to the availability class, never to a data-integrity class.
        let err = sqlx::Error::PoolClosed;
        let (code, message) = classify(&err);
        assert_eq!(code, codes::UNAVAILABLE);
        assert_eq!(message, "the storage backend is unavailable");
        assert!(!is_unique_violation(&err));
    }

    #[test]
    fn mapped_errors_carry_stable_codes_only() {
        let backend = map_backend(sqlx::Error::PoolTimedOut);
        assert_eq!(backend.code(), codes::UNAVAILABLE);
        // The message is the fixed class text; no raw storage text can be
        // attached because classify never reads engine output.
        assert!(!backend.message().contains("postgres://"));

        let rejection = map_audit_rejection(sqlx::Error::PoolClosed);
        assert!(matches!(rejection, AuditError::Rejected { .. }));
        if let AuditError::Rejected { reason } = rejection {
            assert_eq!(reason, "the storage backend is unavailable");
        }
    }

    #[test]
    fn unit_of_work_finalized_error_is_stable() {
        let e = unit_of_work_finalized();
        assert_eq!(e.code(), codes::UOW_FINALIZED);
        assert_eq!(e.code(), "E_UNIT_OF_WORK_FINALIZED");
    }
}
