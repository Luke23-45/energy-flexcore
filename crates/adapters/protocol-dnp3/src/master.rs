// P14.08 — The master-role assessment (a record-level verdict, not a
// role implementation).
//
// The phase plan (P14.08) is explicit: OUTSTATION role first
// (representing the fleet to utility SCADA), master role assessed as a
// record. The master role — a FlexCore-side DNP3 master polling utility
// SCADA or RTUs — needs a named integration counterparty and a utility's
// own point list and security requirements before any design can be
// honest; without one, protocol code has nothing to talk to (the same
// reasoning the phase applies to 2.0b and the EU surfaces). This module
// records that assessment and provides the named refusal that keeps the
// role from being implemented by drift.

/// The master-role assessment verdict. Exactly one variant exists — the
/// honest one for this phase — so no code path can pretend the role is
/// available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MasterRoleVerdict {
    /// DEFER: the master role is assessed as a record only. What reopens
    /// it is named in [`MasterRoleAssessment::reopen_evidence`].
    Defer,
}

/// The master-role assessment record (P14.08; record 24 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterRoleAssessment {
    /// The adapter id the assessment belongs to.
    pub adapter_id: String,
    /// The verdict (DEFER in this phase).
    pub verdict: MasterRoleVerdict,
    /// The recorded reasons for the verdict (non-empty; reviewed).
    pub reasons: Vec<String>,
    /// The evidence that would reopen the role (what a named integration
    /// would have to provide — the E-decision closure shape of
    /// conventions §4).
    pub reopen_evidence: Vec<String>,
}

/// Errors returned by the assessment and the role gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MasterRoleError {
    /// The assessment fields are incomplete.
    IncompleteAssessment(&'static str),
    /// The master role was requested: the verdict is DEFER, so the
    /// request is refused by name.
    MasterRoleDeferred,
}

impl std::fmt::Display for MasterRoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MasterRoleError::IncompleteAssessment(field) => {
                write!(f, "master-role assessment field '{field}' is incomplete")
            }
            MasterRoleError::MasterRoleDeferred => {
                write!(
                    f, "the DNP3 master role is assessed DEFER (P14.08): outstation first; a \
                        master implementation needs a named integration counterparty, its \
                        point list, and its security requirements"
                )
            }
        }
    }
}

impl std::error::Error for MasterRoleError {}

impl MasterRoleAssessment {
    /// Constructs the assessment record (reasons and reopen evidence are
    /// mandatory — an assessment that says nothing records nothing).
    pub fn new(
        adapter_id: impl Into<String>,
        reasons: Vec<String>,
        reopen_evidence: Vec<String>,
    ) -> Result<Self, MasterRoleError> {
        // Convert once: the emptiness check and the struct field both need
        // the owned String (converting twice moves `adapter_id`).
        let adapter_id: String = adapter_id.into();
        if adapter_id.trim().is_empty() {
            return Err(MasterRoleError::IncompleteAssessment("adapter_id"));
        }
        if reasons.is_empty() || reasons.iter().any(|r| r.trim().is_empty()) {
            return Err(MasterRoleError::IncompleteAssessment("reasons"));
        }
        if reopen_evidence.is_empty() || reopen_evidence.iter().any(|r| r.trim().is_empty()) {
            return Err(MasterRoleError::IncompleteAssessment("reopen_evidence"));
        }
        Ok(Self {
            adapter_id,
            verdict: MasterRoleVerdict::Defer,
            reasons,
            reopen_evidence,
        })
    }

    /// The role gate: any attempt to act in the master role is refused by
    /// name while the verdict is DEFER. This is the code-level form of
    /// the record — there is no master-role type anywhere in this crate
    /// to construct.
    pub fn assert_role_available(&self) -> Result<(), MasterRoleError> {
        Err(MasterRoleError::MasterRoleDeferred)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assessment() -> MasterRoleAssessment {
        MasterRoleAssessment::new(
            "dnp3-outstation",
            vec![
                "no named integration counterparty exists; protocol code without a counterparty rots".to_string(),
                "the master role duplicates the outstation security surface in reverse and needs its own review".to_string(),
            ],
            vec![
                "a named utility SCADA integration with its point list".to_string(),
                "the utility's security requirements (secure authentication profile choice)".to_string(),
            ],
        )
        .expect("valid assessment")
    }

    #[test]
    fn assessment_is_defer_with_recorded_evidence() {
        let a = assessment();
        assert_eq!(a.verdict, MasterRoleVerdict::Defer);
        assert_eq!(a.reasons.len(), 2);
        assert_eq!(a.reopen_evidence.len(), 2);
    }

    #[test]
    fn the_master_role_is_refused_by_name() {
        assert_eq!(
            assessment().assert_role_available(),
            Err(MasterRoleError::MasterRoleDeferred)
        );
    }

    #[test]
    fn incomplete_assessments_are_construction_errors() {
        assert_eq!(
            MasterRoleAssessment::new("dnp3", Vec::new(), vec!["x".to_string()]),
            Err(MasterRoleError::IncompleteAssessment("reasons"))
        );
        assert_eq!(
            MasterRoleAssessment::new("dnp3", vec!["x".to_string()], Vec::new()),
            Err(MasterRoleError::IncompleteAssessment("reopen_evidence"))
        );
    }
}
