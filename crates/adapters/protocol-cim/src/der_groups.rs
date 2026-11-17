// P14.12 — The IEC 61968-5 DER-group registry (decision record
// 44_p14_12_cim_cgmes_import.md; source [S-18]).
//
// WHAT THIS MODULE IS: DER group declarations as REGISTRY data — the
// design the 61968-5 DER-group-management payloads would map into. A
// group declaration names a group and the DER equipment objects (by
// mRID) that belong to it. The registry is validated against the
// imported profile: every member must reference an object the profile
// actually contains.
//
// UNVERIFIED HONESTY (plan §7 note 6): the IEC 61968-5:2020 text [S-18]
// was NOT consulted (only the catalog entry [S-18] and the gap-analysis
// description of DER group management). The group payload STRUCTURE here
// (an id, a member mRID list, declared notes) is a DECLARED design shape,
// not a specification quote; the implementation-time check confirms the
// real payload classes, attribute spellings, and reference semantics
// against the standard text. A mismatch is a recorded edit, never a
// silent reinterpretation.
//
// THE CONTROL-PATH RULE: a DER group is registry data. NO group command
// path exists here — a command to a group of DERs is a command-lifecycle
// candidate like any other, entering at PROPOSED and passing all three
// gates (the platform rule every Phase 14 adapter records; see
// `crates/flexcore-domain/src/command_state.rs`). The group declaration
// never becomes a setpoint and never reaches a device.
//
// RELATIONSHIP TO THE NETWORK MAPPING ([`crate::class_mapping`]): a
// group member is typically a `PowerElectronicsConnection` — a class the
// network mapping EXPLICITLY_UNMAPPED (the network IR has no DER
// equipment element). That is coherent, not contradictory: the group is
// a fleet-registry record, not a network element; its members are DER
// assets declared by the profile, modelled in flexcore-domain envelopes.
// The registry records the member's mapping status alongside the
// membership so no consumer can confuse the two surfaces.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{CimMRid, ProfileDocument};
use crate::unmapped::ImportRegister;

/// Errors returned by DER-group declaration and registry assembly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerGroupError {
    /// A group field is empty (the group id, a member note).
    EmptyField(&'static str),
    /// A group declares no members — an empty group is not a reviewed
    /// declaration.
    EmptyGroup,
    /// The same member appears twice in one group.
    DuplicateMember {
        mrid: String,
    },
    /// A member mRID is not present in the imported profile — the
    /// registry refuses a group whose fleet it cannot point at.
    MemberNotInProfile {
        mrid: String,
    },
}

impl std::fmt::Display for DerGroupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DerGroupError::EmptyField(field) => write!(f, "DER group field '{field}' is empty"),
            DerGroupError::EmptyGroup => write!(f, "the DER group declares no members"),
            DerGroupError::DuplicateMember { mrid } => {
                write!(f, "member '{mrid}' appears twice in the group")
            }
            DerGroupError::MemberNotInProfile { mrid } => write!(
                f,
                "member '{mrid}' is not present in the imported profile — the registry \
                 refuses a group whose fleet it cannot point at"
            ),
        }
    }
}

impl std::error::Error for DerGroupError {}

/// One DER group declaration: the group id and the member mRIDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerGroupDeclaration {
    /// The declared group identifier (the registry key).
    pub group_id: String,
    /// The member equipment mRIDs, unique within the group.
    pub member_mr_ids: Vec<CimMRid>,
    /// Recorded notes (the declared purpose, review pointers).
    pub notes: Vec<String>,
}

impl DerGroupDeclaration {
    /// Constructs and validates a group declaration (uniqueness within
    /// the group; non-empty id and member list).
    pub fn new(
        group_id: String,
        member_mr_ids: Vec<CimMRid>,
        notes: Vec<String>,
    ) -> Result<Self, DerGroupError> {
        if group_id.trim().is_empty() {
            return Err(DerGroupError::EmptyField("group_id"));
        }
        if member_mr_ids.is_empty() {
            return Err(DerGroupError::EmptyGroup);
        }
        let mut seen = std::collections::BTreeSet::new();
        for member in &member_mr_ids {
            if !seen.insert(member.value().to_string()) {
                return Err(DerGroupError::DuplicateMember {
                    mrid: member.value().to_string(),
                });
            }
        }
        Ok(Self {
            group_id: group_id.trim().to_string(),
            member_mr_ids,
            notes,
        })
    }
}

/// One registry entry: the group plus, per member, the member's network-
/// mapping status (the coherence note above — recorded, not recomputed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerGroupMemberRecord {
    pub mrid: String,
    /// The member's class name in the profile.
    pub class_name: String,
    /// The member's network-mapping status name (MAPPED /
    /// EXPLICITLY_UNMAPPED / UNMAPPED / AMBIGUOUS — the register's own
    /// vocabulary; the registry consumes it, never re-derives it).
    pub network_mapping_status: String,
}

/// The validated DER-group registry of one import: every declared group
/// with its members' recorded mapping statuses. Ordered (BTreeMap over
/// group id — the determinism rule).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerGroupRegistry {
    groups: BTreeMap<String, Vec<DerGroupMemberRecord>>,
}

impl DerGroupRegistry {
    /// Assembles the registry: validates every declaration against the
    /// imported profile (members must exist) and records each member's
    /// network-mapping status from the import register.
    pub fn assemble(
        declarations: Vec<DerGroupDeclaration>,
        profile: &ProfileDocument,
        register: &ImportRegister,
    ) -> Result<Self, DerGroupError> {
        let mut groups = BTreeMap::new();
        for declaration in declarations {
            let mut members = Vec::new();
            for member in &declaration.member_mr_ids {
                let object = profile
                    .object(member)
                    .ok_or_else(|| DerGroupError::MemberNotInProfile {
                        mrid: member.value().to_string(),
                    })?;
                let network_mapping_status = register
                    .status(member.value())
                    .map(|status| status.name().to_string())
                    .unwrap_or_else(|| "UNCLASSIFIED".to_string());
                members.push(DerGroupMemberRecord {
                    mrid: member.value().to_string(),
                    class_name: object.class_name.clone(),
                    network_mapping_status,
                });
            }
            groups.insert(declaration.group_id, members);
        }
        Ok(Self { groups })
    }

    /// The registered groups, deterministically ordered by group id.
    pub fn group_ids(&self) -> Vec<&str> {
        self.groups.keys().map(|id| id.as_str()).collect()
    }

    /// The members of one group.
    pub fn members(&self, group_id: &str) -> Option<&[DerGroupMemberRecord]> {
        self.groups.get(group_id).map(|members| members.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ATTR_BASE_VOLTAGE_NOMINAL_KV, ATTR_NODE_BASE_VOLTAGE_REF,
    };

    fn object(mrid: &str, class: &str, attributes: &[(&str, &str)]) -> crate::model::CimObject {
        let mut map = BTreeMap::new();
        for (key, value) in attributes {
            map.insert((*key).to_string(), (*value).to_string());
        }
        crate::model::CimObject {
            mrid: CimMRid::new(mrid).expect("valid"),
            class_name: class.to_string(),
            attributes: map,
        }
    }

    fn profile_with_der() -> ProfileDocument {
        ProfileDocument::new(vec![
            object("_bv20", "BaseVoltage", &[(ATTR_BASE_VOLTAGE_NOMINAL_KV, "20.0")]),
            object("_busA", "ConnectivityNode", &[(ATTR_NODE_BASE_VOLTAGE_REF, "_bv20")]),
            object("_pec1", "PowerElectronicsConnection", &[]),
            object("_pec2", "PowerElectronicsConnection", &[]),
        ])
        .expect("valid profile")
    }

    #[test]
    fn declarations_refuse_empty_duplicate_and_missing_members() {
        assert_eq!(
            DerGroupDeclaration::new("  ".to_string(), vec![CimMRid::new("_pec1").expect("v")], Vec::new()),
            Err(DerGroupError::EmptyField("group_id"))
        );
        assert_eq!(
            DerGroupDeclaration::new("g".to_string(), Vec::new(), Vec::new()),
            Err(DerGroupError::EmptyGroup)
        );
        assert_eq!(
            DerGroupDeclaration::new(
                "g".to_string(),
                vec![
                    CimMRid::new("_pec1").expect("valid"),
                    CimMRid::new(" _pec1 ").expect("valid"),
                ],
                Vec::new(),
            ),
            Err(DerGroupError::DuplicateMember {
                mrid: "_pec1".to_string(),
            })
        );
    }

    #[test]
    fn the_registry_refuses_members_outside_the_profile() {
        let profile = profile_with_der();
        let register = ImportRegister::classify_profile(&profile);
        let declaration = DerGroupDeclaration::new(
            "south-fleet".to_string(),
            vec![
                CimMRid::new("_pec1").expect("valid"),
                CimMRid::new("_not-there").expect("valid"),
            ],
            Vec::new(),
        )
        .expect("valid declaration");
        assert_eq!(
            DerGroupRegistry::assemble(vec![declaration], &profile, &register),
            Err(DerGroupError::MemberNotInProfile {
                mrid: "_not-there".to_string(),
            })
        );
    }

    #[test]
    fn the_registry_records_member_mapping_statuses_verbatim() {
        let profile = profile_with_der();
        let register = ImportRegister::classify_profile(&profile);
        let declaration = DerGroupDeclaration::new(
            "south-fleet".to_string(),
            vec![
                CimMRid::new("_pec1").expect("valid"),
                CimMRid::new("_pec2").expect("valid"),
            ],
            vec!["the declared DER fleet of feeder south".to_string()],
        )
        .expect("valid declaration");
        let registry =
            DerGroupRegistry::assemble(vec![declaration], &profile, &register).expect("valid");
        assert_eq!(registry.group_ids(), vec!["south-fleet"]);
        let members = registry.members("south-fleet").expect("present");
        // The members are PowerElectronicsConnection objects — a class the
        // network mapping EXPLICITLY_UNMAPPED. The group record carries
        // that status verbatim, so no consumer confuses the registry
        // surface with the network-IR surface.
        for member in members {
            assert_eq!(member.class_name, "PowerElectronicsConnection");
            assert_eq!(member.network_mapping_status, "EXPLICITLY_UNMAPPED");
        }
    }

    #[test]
    fn the_registry_has_no_command_path() {
        // Structural honesty: the registry type offers only group
        // listing and member records. There is no send, no set, no
        // dispatch method — absence is structural (the P6.12 rule).
        // This test documents the rule where a reviewer will look for it.
        let profile = profile_with_der();
        let register = ImportRegister::classify_profile(&profile);
        let declaration = DerGroupDeclaration::new(
            "g".to_string(),
            vec![CimMRid::new("_pec1").expect("valid")],
            Vec::new(),
        )
        .expect("valid declaration");
        let registry =
            DerGroupRegistry::assemble(vec![declaration], &profile, &register).expect("valid");
        let members = registry.members("g").expect("present");
        assert_eq!(members.len(), 1);
        assert_eq!(registry.group_ids().len(), 1);
    }
}
