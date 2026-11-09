// P1.17 — HTTP error envelope and ApplicationError mapping.
//
// Rules implemented here (ledger P1.17 step 3/6):
// - JSON error envelope with a stable shape; SQL/broker error text is never
//   leaked (infrastructure errors carry only an opaque code + generic text).
// - Cross-tenant parent references return 404, not 403 — the application
//   layer deliberately makes a cross-tenant reference indistinguishable from
//   a missing one, so the API preserves that (no existence leak). Recorded
//   decision (CONTEXT.md open question 6): 404 for cross-tenant resource
//   references; 403 where the CALLER's own context is invalid; 401 when no
//   verified authentication is present; 422 for validation rejections.
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use flexcore_application::error::{ApplicationError, AuthorizationError};
use flexcore_ports::backend_error::BackendError;
use flexcore_ports::repository::RepositoryError;
use serde::Serialize;

/// The stable error envelope: `{"error": {"code": ..., "message": ...}}`.
#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
}

/// A request rejection carrying its HTTP status and stable error code.
#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    pub fn unauthenticated() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthenticated",
            message: "missing or invalid authentication".to_string(),
        }
    }

    pub fn forbidden(reason: &str) -> Self {
        Self {
            status: StatusCode::FORBIDDEN,
            code: "forbidden",
            message: format!("authorization denied: {reason}"),
        }
    }

    pub fn not_found() -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: "resource not found".to_string(),
        }
    }

    pub fn validation(message: String) -> Self {
        Self {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            code: "validation_rejected",
            message,
        }
    }

    /// A request that is well-formed but conflicts with the current state
    /// of the resource — the P2.12 registry DELETE routes map the P2.02
    /// RESTRICT rule (a non-empty parent cannot be deleted) to 409 with the
    /// stable code `non_empty_parent` (DECIDED: 409, not 422 — the request
    /// is valid; the CONFLICT is with the resource's dependents, and the
    /// standard remediation is to delete the dependents first).
    pub fn conflict(code: &'static str, message: String) -> Self {
        Self {
            status: StatusCode::CONFLICT,
            code,
            message,
        }
    }

    pub fn infrastructure() -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "infrastructure_failure",
            // Opaque by contract: no backend text is ever forwarded.
            message: "the request could not be completed; see server logs with the trace id"
                .to_string(),
        }
    }
}

impl From<RepositoryError> for ApiError {
    fn from(e: RepositoryError) -> Self {
        let _ = e; // never forwarded to the client
        ApiError::infrastructure()
    }
}

impl From<BackendError> for ApiError {
    fn from(e: BackendError) -> Self {
        let _ = e; // never forwarded to the client
        ApiError::infrastructure()
    }
}

impl From<ApplicationError> for ApiError {
    fn from(e: ApplicationError) -> Self {
        match e {
            ApplicationError::Validation(err) => ApiError::validation(err.to_string()),
            ApplicationError::Authorization(err) => match err {
                // No existence leak: a cross-tenant reference is 404.
                AuthorizationError::ParentOutsideTenantScope { .. } => ApiError::not_found(),
                // The caller's own context is the problem: 403.
                AuthorizationError::CallerTenantUnknown
                | AuthorizationError::TenantScopeViolation { .. } => {
                    ApiError::forbidden(&err.to_string())
                }
            },
            ApplicationError::Infrastructure(_) => ApiError::infrastructure(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.status;
        let code = self.code;
        let message = self.message;
        (status, Json(ErrorEnvelope { error: ErrorBody { code: code.to_string(), message } })).into_response()
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.code, self.status)
    }
}

impl std::error::Error for ApiError {}
