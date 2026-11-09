// P1.07 — The opaque backend-error value adapters report with.
//
// Part of the P1.08 error-taxonomy contract: infrastructure failures are a
// distinct error class, and they must never leak storage or broker error
// text (SQL fragments, engine codes, connection strings) into the
// application layer or beyond. The port layer therefore defines the exact
// shape an adapter may report: an opaque short `code` plus a human-safe
// `message`, both supplied deliberately by the adapter implementer.
//
// The mapping rule (recorded in the flexcore-application README): a
// `BackendError` arriving through a port error becomes an application
// `InfrastructureError` verbatim — the adapter is responsible for the
// opaqueness of both fields, and the API layer renders a generic message
// plus the code to callers. Nothing in this type can guarantee that an
// adapter behaves; the rule is a review obligation for every adapter
// (P1.13, P1.15), the same way the storage atomicity obligations are.

/// An opaque backend failure, as reported by an adapter through a port
/// error (P1.08 infrastructure-error class).
///
/// `code` is a short, stable, adapter-defined identifier (for example a
/// symbolic failure class), `message` a human-safe summary. **Neither field
/// may carry raw storage or broker text** — no statement text, no engine
/// error output, no connection details (report §17 threat hygiene; P1.08
/// error-taxonomy rule). The adapter decides both values; the application
/// layer passes them through unchanged and the API layer renders them
/// generically.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendError {
    code: String,
    message: String,
}

impl BackendError {
    /// Builds an opaque backend error from an adapter-defined code and a
    /// human-safe message.
    pub fn new(code: String, message: String) -> Self {
        Self { code, message }
    }

    /// The stable adapter-defined failure code.
    pub fn code(&self) -> &str {
        &self.code
    }

    /// The human-safe failure summary (never raw storage/broker text —
    /// an adapter review obligation, see the type docs).
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "backend error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for BackendError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_and_message_travel_verbatim() {
        let e = BackendError::new("E_STORE_UNAVAILABLE".to_string(), "backend unavailable".to_string());
        assert_eq!(e.code(), "E_STORE_UNAVAILABLE");
        assert_eq!(e.message(), "backend unavailable");
        assert_eq!(e.to_string(), "backend error E_STORE_UNAVAILABLE: backend unavailable");
    }
}
