// P14.08 — Secure-transport posture note (no security claims of any kind).
//
// This module records the adapter's POSTURE toward DNP3 secure transport
// — it implements no security control, validates no certificate, and
// makes no conformance, certification, or compliance claim (conventions
// §7.3; AGENTS.md §3 rule 2). The posture is an input to a security
// review (Phase 19's surface), never its conclusion.
//
// Secondary-source anchors (UNVERIFIED): DNP3 secure authentication and
// the IEEE 1815.2 secure-transport options (and what the 2025 edition
// changes) follow the reading of [S-14][S-15]; the exact profile and
// cipher requirements are checked against the standard text and the
// utility's security requirements at implementation time.

use serde::{Deserialize, Serialize};

/// The declared secure-transport posture of a deployment. Declared, never
/// inferred from the presence of a transport; there is no default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecureTransportPosture {
    /// The deployment REQUIRES a secure transport before any wire-facing
    /// design proceeds (the recommended posture; a recommendation, never a
    /// claim that the requirement is met).
    Required,
    /// The deployment operates in a closed test lab with no utility
    /// connection — the simulation/shadow ceiling (release ladder: field
    /// write authority begins at v1.0.0 with operator approval, and
    /// nothing in 0.x connects at all).
    TestLabOnly,
}

impl SecureTransportPosture {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            SecureTransportPosture::Required => "required",
            SecureTransportPosture::TestLabOnly => "test-lab-only",
        }
    }
}

/// The recorded posture note: what the deployment declares about secure
/// transport, what is NOT yet known, and what is explicitly NOT claimed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecureTransportNote {
    /// The adapter id the note belongs to.
    pub adapter_id: String,
    /// The declared posture.
    pub posture: SecureTransportPosture,
    /// The open security questions the implementation-time review must
    /// answer (non-empty; each is an honest unknown, not a claim).
    pub open_questions: Vec<String>,
    /// The recorded statement of what is NOT claimed (non-empty).
    pub not_claimed: Vec<String>,
}

/// Errors returned by the note's construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostureError {
    /// A note field is incomplete.
    Incomplete(&'static str),
}

impl std::fmt::Display for PostureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PostureError::Incomplete(field) => {
                write!(f, "secure-transport note field '{field}' is incomplete")
            }
        }
    }
}

impl std::error::Error for PostureError {}

impl SecureTransportNote {
    /// Constructs the posture note. The open-questions and not-claimed
    /// lists are mandatory and non-empty: a posture note that records no
    /// unknowns is claiming certainty, which is exactly what this module
    /// must never do.
    pub fn new(
        adapter_id: impl Into<String>,
        posture: SecureTransportPosture,
        open_questions: Vec<String>,
        not_claimed: Vec<String>,
    ) -> Result<Self, PostureError> {
        if adapter_id.into().trim().is_empty() {
            return Err(PostureError::Incomplete("adapter_id"));
        }
        if open_questions.is_empty() || open_questions.iter().any(|q| q.trim().is_empty()) {
            return Err(PostureError::Incomplete("open_questions"));
        }
        if not_claimed.is_empty() || not_claimed.iter().any(|c| c.trim().is_empty()) {
            return Err(PostureError::Incomplete("not_claimed"));
        }
        Ok(Self {
            adapter_id: adapter_id.into(),
            posture,
            open_questions,
            not_claimed,
        })
    }

    /// The canonical posture note for this adapter (record 24 §6): the
    /// recommended posture with the honest open questions, and the
    /// explicit no-claims list.
    pub fn canonical_note() -> Result<Self, PostureError> {
        Self::new(
            "dnp3-outstation",
            SecureTransportPosture::Required,
            vec![
                // UNVERIFIED anchors — each is checked against the IEEE
                // 1815.2-2025 text [S-14] and the utility's security
                // requirements at implementation time.
                "which secure-authentication profile IEEE 1815.2-2025 requires for DER-profile \
                 outstations, and what the 2025 edition changes relative to 1815-2019"
                    .to_string(),
                "whether the named utility requires TLS on the serial/TCP path in addition to \
                 secure authentication, and with which certificate hierarchy"
                    .to_string(),
                "how the utility's key-management expectations map onto the Phase 13 secrets \
                 surface (no key material is handled in this phase)"
                    .to_string(),
            ],
            vec![
                "no security, conformance, certification, or compliance claim of any kind is \
                 made by this crate or its records"
                    .to_string(),
                "no secure-transport control is implemented in this phase; the posture is an \
                 input to a security review, never its conclusion"
                    .to_string(),
                "no connection exists: no device is reached and no message is exchanged"
                    .to_string(),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_note_declares_required_posture_with_open_questions() {
        let note = SecureTransportNote::canonical_note().expect("valid note");
        assert_eq!(note.posture, SecureTransportPosture::Required);
        assert_eq!(note.open_questions.len(), 3);
        assert_eq!(note.not_claimed.len(), 3);
        assert!(note
            .not_claimed
            .iter()
            .any(|c| c.contains("no security, conformance, certification, or compliance claim")));
    }

    #[test]
    fn a_note_without_open_questions_is_rejected() {
        assert_eq!(
            SecureTransportNote::new(
                "dnp3-outstation",
                SecureTransportPosture::Required,
                Vec::new(),
                vec!["nothing is claimed".to_string()],
            ),
            Err(PostureError::Incomplete("open_questions"))
        );
        assert_eq!(
            SecureTransportNote::new(
                "dnp3-outstation",
                SecureTransportPosture::TestLabOnly,
                vec!["which profile?".to_string()],
                Vec::new(),
            ),
            Err(PostureError::Incomplete("not_claimed"))
        );
    }
}
