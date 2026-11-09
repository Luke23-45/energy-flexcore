// P1.08 — The error taxonomy (P1.08 step 5).
//
// Three distinct error classes, never mixed, never flattened into a bare
// string (report §6.1–§6.2; ledger P1.08 step 5):
//
// 1. **Domain / validation** — [`ValidationError`]: an input violated a
//    frozen rule (domain entity invariants, a duplicate registration). The
//    caller can fix these; the API layer renders them as client-visible
//    rejections.
// 2. **Authorization** — [`AuthorizationError`]: the caller referenced or
//    touched something outside their tenant scope. Never a validation
//    error: scoping is a security boundary, not an input-format rule.
// 3. **Infrastructure** — [`InfrastructureError`]: a backend failed. The
//    value carries an **opaque code + message** supplied by the adapter
//    ([`BackendError`]); raw storage or broker text never crosses this
//    layer, and the API layer renders a generic failure plus the code.
//
// The port-error mapping is a recorded decision (crate README): a
// repository `NotFound` in a create flow means the referenced parent is
// outside the caller's tenant scope (or genuinely missing — the two are
// deliberately indistinguishable, no existence leak) and maps to the
// authorization class; `TenantMismatch` maps to the authorization class;
// a uniqueness `Conflict` maps to the validation class (duplicate
// registration); a `Backend` failure maps to the infrastructure class
// verbatim. Outbox errors map the same way: shape/validation failures are
// validation-class invariants, backend failures are infrastructure.
//
// No variant of this taxonomy carries a SQL fragment, a broker subject
// dump, or any backend-generated text other than the adapter-supplied
// opaque code + message.

use flexcore_domain::entities::EntityError;
use flexcore_events::envelope::EnvelopeError;
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::event_outbox::{EventOutboxError, OutboxWriteError};
use flexcore_ports::repository::RepositoryError;

/// A backend failure as the taxonomy reports it: an opaque, stable code
/// plus a human-safe message, both supplied deliberately by the adapter.
/// Never raw storage or broker text — an adapter review obligation (see
/// [`BackendError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfrastructureError {
    code: String,
    message: String,
}

impl InfrastructureError {
    /// Builds the error from an adapter-defined opaque code and a
    /// human-safe message.
    pub fn new(code: String, message: String) -> Self {
        Self { code, message }
    }

    /// The opaque, stable failure code (rendered to operators/logs; never
    /// interpreted by callers).
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The human-safe failure summary.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for InfrastructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "infrastructure failure {}: {}", self.code, self.message)
    }
}

impl std::error::Error for InfrastructureError {}

impl From<BackendError> for InfrastructureError {
    fn from(e: BackendError) -> Self {
        Self::new(e.code().to_string(), e.message().to_string())
    }
}

/// Domain / validation rejections (P1.08 class 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    /// A domain entity constructor rejected the input (empty field,
    /// non-positive rated quantity, time-rule violation, nil identifier,
    /// reactive-convention mismatch).
    Entity(EntityError),
    /// A duplicate registration: the entity's uniqueness rule already has
    /// a row. `entity` names the registry (a
    /// [`flexcore_ports::state_change::entities`] name), `key` the
    /// colliding natural key.
    DuplicateRegistration {
        /// Which registry collided.
        entity: &'static str,
        /// The colliding natural-key value.
        key: String,
    },
    /// The assembled outbox write failed its shape validation — an
    /// internal invariant (empty key/subject/payload); a use-case bug, not
    /// a caller input problem, but classified with the invariants.
    InvalidOutboxWrite(OutboxWriteError),
    /// The assembled event envelope failed construction (unsupported
    /// schema version, empty key) — an internal invariant.
    InvalidEventEnvelope(EnvelopeError),
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::Entity(e) => write!(f, "domain rule violated: {e}"),
            ValidationError::DuplicateRegistration { entity, key } => {
                write!(f, "{entity} is already registered (key: {key})")
            }
            ValidationError::InvalidOutboxWrite(e) => write!(f, "outbox write invalid: {e}"),
            ValidationError::InvalidEventEnvelope(e) => write!(f, "event envelope invalid: {e}"),
        }
    }
}

impl std::error::Error for ValidationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ValidationError::Entity(e) => Some(e),
            ValidationError::InvalidOutboxWrite(e) => Some(e),
            ValidationError::InvalidEventEnvelope(e) => Some(e),
            ValidationError::DuplicateRegistration { .. } => None,
        }
    }
}

impl From<EntityError> for ValidationError {
    fn from(e: EntityError) -> Self {
        Self::Entity(e)
    }
}

impl From<OutboxWriteError> for ValidationError {
    fn from(e: OutboxWriteError) -> Self {
        Self::InvalidOutboxWrite(e)
    }
}

impl From<EnvelopeError> for ValidationError {
    fn from(e: EnvelopeError) -> Self {
        Self::InvalidEventEnvelope(e)
    }
}

/// Authorization rejections (P1.08 class 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthorizationError {
    /// A referenced parent entity (portfolio, site) does not exist within
    /// the caller's tenant scope. A cross-tenant reference is
    /// deliberately indistinguishable from a missing one — existence is
    /// never leaked across the boundary.
    ParentOutsideTenantScope {
        /// Which entity kind was referenced.
        entity: &'static str,
    },
    /// The authenticated principal's tenant has no registry row (a stale
    /// or revoked tenant); the caller cannot proceed.
    CallerTenantUnknown,
    /// A write was refused because it crossed the tenant scope (the
    /// repository reported the mismatch explicitly — the caller held a
    /// foreign entity).
    TenantScopeViolation {
        /// Which entity kind was refused.
        entity: &'static str,
    },
}

impl std::fmt::Display for AuthorizationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthorizationError::ParentOutsideTenantScope { entity } => {
                write!(f, "{entity} not found within the caller's tenant scope")
            }
            AuthorizationError::CallerTenantUnknown => {
                write!(f, "the caller's tenant has no registry row")
            }
            AuthorizationError::TenantScopeViolation { entity } => {
                write!(f, "refused a cross-tenant write on {entity}")
            }
        }
    }
}

impl std::error::Error for AuthorizationError {}

/// The use-case error type: exactly one of the three taxonomy classes
/// (P1.08 step 5). Callers match the class, not backend details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplicationError {
    /// A frozen rule was violated; the caller can fix the input.
    Validation(ValidationError),
    /// The caller lacks (or exceeds) their tenant scope.
    Authorization(AuthorizationError),
    /// A backend failed; opaque code + message only.
    Infrastructure(InfrastructureError),
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApplicationError::Validation(e) => write!(f, "validation rejected: {e}"),
            ApplicationError::Authorization(e) => write!(f, "authorization rejected: {e}"),
            ApplicationError::Infrastructure(e) => write!(f, "infrastructure failure: {e}"),
        }
    }
}

impl std::error::Error for ApplicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ApplicationError::Validation(e) => Some(e),
            ApplicationError::Authorization(e) => Some(e),
            ApplicationError::Infrastructure(e) => Some(e),
        }
    }
}

impl From<ValidationError> for ApplicationError {
    fn from(e: ValidationError) -> Self {
        Self::Validation(e)
    }
}

impl From<AuthorizationError> for ApplicationError {
    fn from(e: AuthorizationError) -> Self {
        Self::Authorization(e)
    }
}

impl From<EntityError> for ApplicationError {
    fn from(e: EntityError) -> Self {
        Self::Validation(e.into())
    }
}

impl From<RepositoryError> for ApplicationError {
    fn from(e: RepositoryError) -> Self {
        match e {
            // Recorded mapping (module header): in the create flows a
            // missing parent is a scope question — a cross-tenant reference
            // must be indistinguishable from a missing one, and neither
            // leaks existence.
            RepositoryError::NotFound { entity } => {
                Self::Authorization(AuthorizationError::ParentOutsideTenantScope { entity })
            }
            RepositoryError::TenantMismatch { entity } => {
                Self::Authorization(AuthorizationError::TenantScopeViolation { entity })
            }
            RepositoryError::Conflict { entity, reason } => {
                Self::Validation(ValidationError::DuplicateRegistration { entity, key: reason })
            }
            RepositoryError::Backend(backend) => Self::Infrastructure(backend.into()),
        }
    }
}

impl From<EventOutboxError> for ApplicationError {
    fn from(e: EventOutboxError) -> Self {
        match e {
            // A duplicate idempotency key is a generator collision, not a
            // caller problem: infrastructure class, opaque code.
            EventOutboxError::DuplicateKey { idempotency_key } => Self::Infrastructure(
                InfrastructureError::new(
                    "OUTBOX_DUPLICATE_IDEMPOTENCY_KEY".to_string(),
                    format!("an outbox entry with key {idempotency_key} already exists"),
                ),
            ),
            EventOutboxError::UnknownKey { idempotency_key } => Self::Infrastructure(
                InfrastructureError::new(
                    "OUTBOX_UNKNOWN_IDEMPOTENCY_KEY".to_string(),
                    format!("no outbox entry matches key {idempotency_key}"),
                ),
            ),
            EventOutboxError::Backend(backend) => Self::Infrastructure(backend.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_not_found_maps_to_authorization_without_leaking_existence() {
        let e = ApplicationError::from(RepositoryError::NotFound { entity: "portfolios" });
        assert_eq!(
            e,
            ApplicationError::Authorization(AuthorizationError::ParentOutsideTenantScope {
                entity: "portfolios"
            })
        );
    }

    #[test]
    fn repository_tenant_mismatch_maps_to_authorization() {
        let e = ApplicationError::from(RepositoryError::TenantMismatch { entity: "sites" });
        assert_eq!(
            e,
            ApplicationError::Authorization(AuthorizationError::TenantScopeViolation {
                entity: "sites"
            })
        );
    }

    #[test]
    fn repository_conflict_maps_to_duplicate_registration() {
        let e = ApplicationError::from(RepositoryError::Conflict {
            entity: "tenants",
            reason: "identity provider reference already registered".to_string(),
        });
        assert_eq!(
            e,
            ApplicationError::Validation(ValidationError::DuplicateRegistration {
                entity: "tenants",
                key: "identity provider reference already registered".to_string(),
            })
        );
    }

    #[test]
    fn backend_errors_map_to_opaque_infrastructure_errors() {
        let e = ApplicationError::from(RepositoryError::Backend(BackendError::new(
            "E_STORE_DOWN".to_string(),
            "backend unavailable".to_string(),
        )));
        assert_eq!(
            e,
            ApplicationError::Infrastructure(InfrastructureError::new(
                "E_STORE_DOWN".to_string(),
                "backend unavailable".to_string()
            ))
        );
        let ApplicationError::Infrastructure(infra) = e else {
            panic!("must be infrastructure class")
        };
        assert_eq!(infra.code(), "E_STORE_DOWN");
        assert_eq!(infra.message(), "backend unavailable");
    }

    #[test]
    fn outbox_errors_map_to_the_infrastructure_class_with_opaque_codes() {
        let e = ApplicationError::from(EventOutboxError::DuplicateKey {
            idempotency_key: "k".to_string(),
        });
        let ApplicationError::Infrastructure(infra) = e else {
            panic!("must be infrastructure class")
        };
        assert_eq!(infra.code(), "OUTBOX_DUPLICATE_IDEMPOTENCY_KEY");
    }

    #[test]
    fn infrastructure_error_display_carries_code_and_message_only() {
        let e = InfrastructureError::new("E_X".to_string(), "backend unavailable".to_string());
        assert_eq!(e.to_string(), "infrastructure failure E_X: backend unavailable");
    }
}
