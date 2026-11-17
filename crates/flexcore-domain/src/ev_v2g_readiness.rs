// P17.13 — V2G readiness modeling (ADDITIVE; wave 2 Agent A).
//
// Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/42_p17_13_v2g_readiness_modeling.md`.
//
// What this module IS, stated plainly:
//
// - **A readiness RECORD layer, never an authority.** ADR-010 discipline
//   holds and extends (program conventions §6.3): readiness records are
//   inputs, records, and proposals — never commands. Nothing in this
//   module returns a `CommandState`, a `DispatchCommand`, or any type
//   that could enter the lifecycle above PROPOSED; the structural shape
//   of the ledger makes the violation unrepresentable. Readiness never
//   authorizes dispatch — a Readied verdict is a documented capability
//   ASSESSMENT, and every dispatch still passes the frozen 14-state
//   lifecycle's three gates.
// - **The five-condition gate is CONSUMED, never redefined.** The
//   §11.5 five-condition `V2xConfig` gate in `ev_fleet_model` (EV, EVSE,
//   protocol, interconnection agreement, market rules — OFF by default,
//   decided once) is byte-identical and untouched by this module. A
//   readiness verdict of `Readied` requires that gate to be ENABLED
//   ([`flexcore_domain::ev_fleet_model::V2xConfig::is_enabled`]) AND a
//   per-device bidirectional capability declaration with recorded
//   evidence. This module is additive on top of the gate; it adds no
//   condition, removes none, and has no constructor that could enable
//   the gate itself.
// - **Per-EVSE and per-vehicle.** The record is keyed per subject (an
//   EVSE or a vehicle of the fleet), because the §11.5 conditions and
//   the device capability are per-device truths, not fleet-wide ones.
//
// What this module is NOT:
//
// - Not an OCPP 2.1 implementation. The OCPP 2.1 bidirectional/DER
//   hooks ([`Ocpp21V2xHook`]) are DESIGN RECORDS: names, scope
//   citations, and UNVERIFIED labels — never exercised by any adapter
//   in this program (the phase-17 risk-1 treatment: 2.1-only features
//   are design records, never half-implementations).
// - Not the North-American AC path. IEEE 2030.5 V2G-AC / SAE J3072 /
//   UL 1741 SC is recorded ([`NA_AC_PATH_STATEMENT`]) as a SEPARATE
//   future integration, outside the OCPP CSMS adapter's scope
//   ([S-4][S-131][S-144]).
//
// Honest status: WRITTEN-UNRUN (the entire program's hard status
// boundary). No V2G session ever existed, no bidirectional energy was
// ever transferred, no device was ever readied in the field, and no
// conformance, certification, or safety claim is made anywhere.

use std::collections::BTreeMap;

use crate::ev_fleet_model::V2xConfig;

/// The recorded ADR-010 discipline statement for this module (conventions
/// §6.3; the EXT-01 pattern the proposal-record modules carry).
pub const READINESS_NEVER_AUTHORIZES_DISPATCH: &str =
    "ADR-010 discipline, extended to V2G readiness: a readiness record is an \
input, a record, and a proposal — never a command. A Readied verdict is a \
documented capability assessment; it authorizes nothing. Every dispatch \
(including any future bidirectional dispatch) enters the frozen 14-state \
command lifecycle at PROPOSED and passes the validation, feasibility, and \
authorization gates. No readiness surface in this module can produce a \
lifecycle state.";

/// The recorded five-condition-gate statement (the gate is consumed, never
/// redefined; decided once in `ev_fleet_model`, §11.5).
pub const FIVE_CONDITION_GATE_CONSUMED_STATEMENT: &str =
    "The five-condition V2xConfig gate (EV, EVSE, protocol, interconnection \
agreement, AND market rules; OFF by default) is CONSUMED here, never \
redefined: this module is additive on top of it. A Readied verdict requires \
that gate to be ENABLED plus a per-device bidirectional capability \
declaration with recorded evidence. No condition was added, removed, or \
relaxed by the P17.13 extension.";

/// The recorded OCPP 2.1 hook statement (the hooks are design records, not
/// implementations; the phase-17 plan risk-1 treatment).
pub const OCPP21_HOOKS_ARE_DESIGN_RECORDS_STATEMENT: &str =
    "The OCPP 2.1 bidirectional/DER hooks (NotifyAllowedEnergyTransfer, \
AFRRSignal, SetDERControl) are DESIGN RECORDS: scope citations with \
UNVERIFIED labels, never exercised by any adapter in this program. OCPP 2.1 \
is cited for capability SCOPE only — never as a claim that any EVSE, \
vehicle, or device implements or conforms to it; device support must be \
verified per §11.5.";

/// The recorded North-American AC path statement: a SEPARATE future
/// integration, recorded so it is neither silently promised nor silently
/// omitted. Sources: the SunSpec IEEE 2030.5 V2G-AC Profile v1.0 [S-4], the
/// QualityLogic V2G-AC implementation guidance [S-144], and the CharIN V2G
/// landscape [S-131]. SAE J3072 is named in the phase-17 plan §2 row for
/// P17.13 but has NO §11 source entry — its citation here is therefore an
/// ASSUMPTION pending a source, explicitly labelled.
pub const NA_AC_PATH_STATEMENT: &str =
    "The North-American AC bidirectional path (IEEE 2030.5 with the SunSpec \
V2G-AC profile [S-4], the SAE J3072 AC bidirectional interface named in the \
phase-17 plan (no §11 source entry — UNVERIFIED/ASSUMPTION), and UL 1741 SC \
equipment listings) is a SEPARATE FUTURE INTEGRATION, recorded here so it \
is neither silently promised nor silently omitted. It is OUT OF SCOPE for \
the OCPP CSMS adapter: the adapter's not_modelled record carries \
no_bidirectional_v2g_control, and no 2030.5 V2G-AC server, J3072 profile, \
or UL 1741 SC interplay is designed, implemented, or claimed anywhere in \
this program. Sources for the profile landscape: [S-4][S-131][S-144].";

// --- Errors -------------------------------------------------------------------

/// Errors of readiness-record construction. Every failure names the
/// offending field/value — never a bare "invalid input" (the P3.04
/// error-catalogue style).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum V2gReadinessError {
    /// A required field is empty. Silence is never a readiness record.
    EmptyField(&'static str),
    /// A capable device declaration carried no recorded evidence — a
    /// capability claim without traceability is refused (the §11.5
    /// evidence discipline, applied per device).
    EmptyEvidence {
        subject: String,
    },
}

impl std::fmt::Display for V2gReadinessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            V2gReadinessError::EmptyField(field) => write!(f, "field '{field}' is empty"),
            V2gReadinessError::EmptyEvidence { subject } => write!(
                f,
                "the bidirectional capability declaration for '{subject}' carries no \
                 recorded evidence; a capability claim without traceability is refused"
            ),
        }
    }
}

impl std::error::Error for V2gReadinessError {}

// --- The readiness record -------------------------------------------------------

/// The subject kind of a readiness record (per-EVSE or per-vehicle — the
/// §11.5 conditions and the device capability are per-device truths).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SubjectKind {
    /// An EVSE (charging station / charge point).
    Evse,
    /// A vehicle of the fleet.
    Vehicle,
}

impl SubjectKind {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            SubjectKind::Evse => "EVSE",
            SubjectKind::Vehicle => "VEHICLE",
        }
    }
}

/// The per-device bidirectional capability declaration (distinct from the
/// five-condition gate: this is the DEVICE-side capability truth; the gate
/// is the SITE/AGGREGATE enablement decision). `capable` without non-empty
/// `evidence` is a construction error — the §11.5 evidence discipline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BidirectionalDeclaration {
    /// Whether the device declares bidirectional power-flow capability.
    pub capable: bool,
    /// The recorded evidence for the capability (a document/model reference
    /// label; audit-safe; must be non-empty when `capable` is true).
    pub evidence: String,
}

/// The derived readiness verdict. Closed vocabulary; `Readied` requires
/// BOTH the per-device capability AND the enabled five-condition gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum V2gReadinessLevel {
    /// The device does not declare bidirectional capability. Terminal for
    /// this record: no gate state can make it Readied (the device truth
    /// governs).
    NotCapable,
    /// The device declares capability but the five-condition gate is
    /// disabled (the §11.5 default). The path to Readied runs through the
    /// gate's own enable procedure (`V2xConfig::enable`) — this module has
    /// no enable path of its own.
    CapableButGateDisabled,
    /// The device declares capability WITH evidence and the five-condition
    /// gate is ENABLED. A capability ASSESSMENT only — it authorizes
    /// nothing ([`READINESS_NEVER_AUTHORIZES_DISPATCH`]).
    Readied,
}

impl V2gReadinessLevel {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            V2gReadinessLevel::NotCapable => "NOT_CAPABLE",
            V2gReadinessLevel::CapableButGateDisabled => "CAPABLE_BUT_GATE_DISABLED",
            V2gReadinessLevel::Readied => "READIED",
        }
    }
}

/// One V2G readiness record, per EVSE or per vehicle. It CONSUMES the
/// five-condition `V2xConfig` (held by value; the gate type is the frozen
/// `ev_fleet_model` type, untouched) and the device-side bidirectional
/// declaration; the verdict is derived, never stored as free data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct V2gReadinessRecord {
    /// The subject kind.
    pub subject_kind: SubjectKind,
    /// The subject key (the EVSE or vehicle key; non-empty).
    pub subject_key: String,
    /// The device-side bidirectional declaration.
    pub bidirectional: BidirectionalDeclaration,
    /// The five-condition gate snapshot (the frozen `V2xConfig`; consumed,
    /// never redefined).
    pub v2x_gate: V2xConfig,
}

impl V2gReadinessRecord {
    /// Constructs the record. Fails closed on an empty subject key or a
    /// capability claim without evidence. The verdict is derived from the
    /// inputs ([`V2gReadinessRecord::readiness`]) — never declared as free
    /// data by the caller.
    pub fn new(
        subject_kind: SubjectKind,
        subject_key: String,
        bidirectional: BidirectionalDeclaration,
        v2x_gate: V2xConfig,
    ) -> Result<Self, V2gReadinessError> {
        if subject_key.trim().is_empty() {
            return Err(V2gReadinessError::EmptyField("subject_key"));
        }
        if bidirectional.capable && bidirectional.evidence.trim().is_empty() {
            return Err(V2gReadinessError::EmptyEvidence {
                subject: subject_key,
            });
        }
        Ok(Self {
            subject_kind,
            subject_key,
            bidirectional,
            v2x_gate,
        })
    }

    /// The derived readiness verdict (the classification rule, decided
    /// once):
    ///
    /// | Device declares capability | Five-condition gate enabled | Verdict |
    /// |---|---|---|
    /// | no | either | `NotCapable` |
    /// | yes | no | `CapableButGateDisabled` |
    /// | yes (with evidence) | yes | `Readied` |
    pub const fn readiness(&self) -> V2gReadinessLevel {
        if !self.bidirectional.capable {
            return V2gReadinessLevel::NotCapable;
        }
        if !self.v2x_gate.is_enabled() {
            return V2gReadinessLevel::CapableButGateDisabled;
        }
        V2gReadinessLevel::Readied
    }
}

// --- The readiness ledger ---------------------------------------------------------

/// The V2G readiness ledger: deterministic `BTreeMap` keyed by
/// `"<kind>:<subject_key>"` (the BTreeMap determinism kernel pattern).
/// The ledger records assessments; it has NO method that produces a
/// command, a lifecycle state, or a dispatch proposal above PROPOSED —
/// the structural ADR-010 guarantee.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct V2gReadinessLedger {
    records: BTreeMap<String, V2gReadinessRecord>,
}

impl V2gReadinessLedger {
    /// An empty ledger.
    pub fn new() -> Self {
        Self::default()
    }

    fn key(kind: SubjectKind, subject_key: &str) -> String {
        format!("{}:{subject_key}", kind.name())
    }

    /// Records (or replaces) the readiness assessment for a subject.
    /// Replacement is a RECORD update, never a lifecycle transition — the
    /// ledger holds assessments, not states.
    pub fn record(&mut self, record: V2gReadinessRecord) -> Result<(), V2gReadinessError> {
        if record.subject_key.trim().is_empty() {
            return Err(V2gReadinessError::EmptyField("subject_key"));
        }
        let key = Self::key(record.subject_kind, &record.subject_key);
        self.records.insert(key, record);
        Ok(())
    }

    /// The recorded assessment, when one exists.
    pub fn assessment(&self, kind: SubjectKind, subject_key: &str) -> Option<&V2gReadinessRecord> {
        self.records.get(&Self::key(kind, subject_key))
    }

    /// The derived readiness verdict for a subject, when one is recorded.
    pub fn readiness_of(
        &self,
        kind: SubjectKind,
        subject_key: &str,
    ) -> Option<V2gReadinessLevel> {
        self.assessment(kind, subject_key)
            .map(|record| record.readiness())
    }

    /// The ledger snapshot in key order (deterministic diagnostics).
    pub fn snapshot_keys(&self) -> Vec<String> {
        self.records.keys().cloned().collect()
    }

    /// The number of recorded assessments.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Whether the ledger is empty.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

// --- The OCPP 2.1 hook design records ------------------------------------------------

/// The OCPP 2.1 bidirectional/DER hooks, as DESIGN RECORDS ([S-125];
/// UNVERIFIED — every name and behavior note below is secondary-sourced
/// and the OCA OCPP 2.1 text is the implementation-time check). These are
/// NOT actions of the CSMS adapter's closed vocabulary (crate::framing in
/// `flexcore-ocpp-adapter`): no adapter in this program sends, receives,
/// or parses any of them. Widening the adapter's action vocabulary or
/// subprotocol set to reach them would be a reviewed scope change
/// (P17-E-1 stays OPEN; D5 stays human-closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ocpp21V2xHook {
    /// The station reports the energy-transfer modes the EV is allowed to
    /// use (the bidirectional-aware evolution of the 2.0.1 charging-needs
    /// family; UNVERIFIED name and semantics — [S-125]).
    NotifyAllowedEnergyTransfer,
    /// The aFRR (automatic Frequency Restoration Reserve) signal surface
    /// the 2.1 DER/bidirectional blocks introduce (UNVERIFIED name and
    /// semantics — [S-125]); a frequency-service INPUT record class, never
    /// a protection-grade or primary-response claim (foundations §0.3).
    AfrrSignal,
    /// The 2.1 DER control functional block's control object (UNVERIFIED
    /// name and semantics — [S-125]); if it ever lands, it enters the
    /// command lifecycle at PROPOSED like every external control object —
    /// never as an adapter-side write.
    SetDerControl,
}

impl Ocpp21V2xHook {
    /// The hook's recorded name (UNVERIFIED spelling — the OCA OCPP 2.1
    /// text is the implementation-time check).
    pub const fn name(self) -> &'static str {
        match self {
            Ocpp21V2xHook::NotifyAllowedEnergyTransfer => "NotifyAllowedEnergyTransfer",
            Ocpp21V2xHook::AfrrSignal => "AFRRSignal",
            Ocpp21V2xHook::SetDerControl => "SetDERControl",
        }
    }

    /// The hook's design-record note (each note carries its UNVERIFIED
    /// label and the lifecycle rule that would govern it).
    pub const fn note(self) -> &'static str {
        match self {
            Ocpp21V2xHook::NotifyAllowedEnergyTransfer => {
                "UNVERIFIED [S-125]: the energy-transfer permission report in the \
                 OCPP 2.1 bidirectional block. Design-record depth: never sent, \
                 received, or parsed by any adapter in this program. If it ever \
                 lands, it is an INPUT record (a vehicle proposal surface, the \
                 P17.06 pattern) — never an auto-command."
            }
            Ocpp21V2xHook::AfrrSignal => {
                "UNVERIFIED [S-125]: the aFRR signal surface of the OCPP 2.1 \
                 DER/bidirectional blocks. Design-record depth only. FlexCore is a \
                 supervisory orchestration layer: no primary frequency response, \
                 no synthetic inertia, no sub-cycle control — an aFRR record would \
                 be an optimization INPUT feeding scheduling, never a real-time \
                 control path."
            }
            Ocpp21V2xHook::SetDerControl => {
                "UNVERIFIED [S-125]: the DER control object of the OCPP 2.1 DER \
                 control functional block. Design-record depth only. If it ever \
                 lands, it is a COMMAND candidate: it enters the frozen 14-state \
                 lifecycle at PROPOSED and passes all three gates — the adapter \
                 relays only ISSUED output (the P17.05 relay rule)."
            }
        }
    }
}

/// The declared hook set, in the recorded order (the matrix the record 42
/// §4 carries).
pub const DECLARED_OCPP21_HOOKS: [Ocpp21V2xHook; 3] = [
    Ocpp21V2xHook::NotifyAllowedEnergyTransfer,
    Ocpp21V2xHook::AfrrSignal,
    Ocpp21V2xHook::SetDerControl,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ev_fleet_model::{V2xConditionEvidence, V2xEvidence};

    fn evidence(text: &str) -> V2xConditionEvidence {
        V2xConditionEvidence {
            supported: true,
            evidence: text.to_string(),
        }
    }

    /// A complete five-condition evidence set (the §11.5 pattern; the
    /// evidence strings are the recorded labels of the named synthetic
    /// fixture discipline — SYNTHETIC labels, never field claims).
    fn full_evidence() -> V2xEvidence {
        V2xEvidence {
            ev: evidence("ev-capability-evidence"),
            evse: evidence("evse-capability-evidence"),
            protocol: evidence("protocol-scope-evidence"),
            interconnection_agreement: evidence("interconnection-evidence"),
            market_rules: evidence("market-rules-evidence"),
        }
    }

    fn capable_decl() -> BidirectionalDeclaration {
        BidirectionalDeclaration {
            capable: true,
            evidence: "device-model-bidirectional-evidence".to_string(),
        }
    }

    fn charge_only_decl() -> BidirectionalDeclaration {
        BidirectionalDeclaration {
            capable: false,
            evidence: String::new(),
        }
    }

    #[test]
    fn the_gate_enable_procedure_is_untouched_and_refuses_incomplete_evidence() {
        // The five-condition gate is consumed, never redefined: its own
        // enable procedure still refuses an incomplete set (this module
        // adds no enable path of its own).
        let mut incomplete = full_evidence();
        incomplete.market_rules.supported = false;
        assert!(V2xConfig::enable(incomplete).is_err());
        let mut no_evidence = full_evidence();
        no_evidence.protocol.evidence = "  ".to_string();
        assert!(V2xConfig::enable(no_evidence).is_err());
        // The complete set enables.
        assert!(V2xConfig::enable(full_evidence()).is_ok());
    }

    #[test]
    fn construction_is_fail_closed() {
        // An empty subject key is refused.
        assert_eq!(
            V2gReadinessRecord::new(
                SubjectKind::Evse,
                "  ".to_string(),
                capable_decl(),
                V2xConfig::disabled()
            ),
            Err(V2gReadinessError::EmptyField("subject_key"))
        );
        // A capability claim without evidence is refused (the §11.5
        // evidence discipline, applied per device).
        let error = V2gReadinessRecord::new(
            SubjectKind::Evse,
            "evse-7".to_string(),
            BidirectionalDeclaration {
                capable: true,
                evidence: " ".to_string(),
            },
            V2xConfig::disabled(),
        )
        .expect_err("refused");
        assert!(matches!(error, V2gReadinessError::EmptyEvidence { .. }));
        assert!(error.to_string().contains("evse-7"));
        // A charge-only declaration may carry an empty evidence field
        // (there is no claim to trace).
        V2gReadinessRecord::new(
            SubjectKind::Vehicle,
            "veh-1".to_string(),
            charge_only_decl(),
            V2xConfig::disabled(),
        )
        .expect("charge-only with empty evidence is fine");
    }

    #[test]
    fn the_readiness_matrix_is_derived_never_declared() {
        // (capability, gate) -> verdict, the full matrix.
        // Charge-only + disabled gate: NotCapable.
        let record = V2gReadinessRecord::new(
            SubjectKind::Evse,
            "a".to_string(),
            charge_only_decl(),
            V2xConfig::disabled(),
        )
        .expect("valid");
        assert_eq!(record.readiness(), V2gReadinessLevel::NotCapable);
        // Charge-only + enabled gate: STILL NotCapable (the device truth
        // governs; no gate state can make a charge-only device Readied).
        let gate = V2xConfig::enable(full_evidence()).expect("enabled");
        let record = V2gReadinessRecord::new(
            SubjectKind::Evse,
            "b".to_string(),
            charge_only_decl(),
            gate.clone(),
        )
        .expect("valid");
        assert_eq!(record.readiness(), V2gReadinessLevel::NotCapable);
        // Capable + disabled gate (the §11.5 DEFAULT): CapableButGateDisabled.
        let record = V2gReadinessRecord::new(
            SubjectKind::Vehicle,
            "c".to_string(),
            capable_decl(),
            V2xConfig::disabled(),
        )
        .expect("valid");
        assert_eq!(record.readiness(), V2gReadinessLevel::CapableButGateDisabled);
        // Capable + enabled gate: Readied (an assessment, never an
        // authorization).
        let record = V2gReadinessRecord::new(
            SubjectKind::Vehicle,
            "d".to_string(),
            capable_decl(),
            gate,
        )
        .expect("valid");
        assert_eq!(record.readiness(), V2gReadinessLevel::Readied);
    }

    #[test]
    fn the_ledger_is_deterministic_and_records_assessments_only() {
        let mut ledger = V2gReadinessLedger::new();
        assert!(ledger.is_empty());
        ledger
            .record(V2gReadinessRecord::new(
                SubjectKind::Evse,
                "evse-2".to_string(),
                charge_only_decl(),
                V2xConfig::disabled(),
            )
            .expect("valid"))
            .expect("recorded");
        ledger
            .record(V2gReadinessRecord::new(
                SubjectKind::Vehicle,
                "veh-1".to_string(),
                capable_decl(),
                V2xConfig::disabled(),
            )
            .expect("valid"))
            .expect("recorded");
        // Key-order determinism: EVSE before VEHICLE, then key order.
        assert_eq!(
            ledger.snapshot_keys(),
            vec!["EVSE:evse-2".to_string(), "VEHICLE:veh-1".to_string()]
        );
        assert_eq!(ledger.len(), 2);
        // Per-subject verdicts derive through the ledger.
        assert_eq!(
            ledger.readiness_of(SubjectKind::Evse, "evse-2"),
            Some(V2gReadinessLevel::NotCapable)
        );
        assert_eq!(
            ledger.readiness_of(SubjectKind::Vehicle, "veh-1"),
            Some(V2gReadinessLevel::CapableButGateDisabled)
        );
        assert_eq!(ledger.readiness_of(SubjectKind::Evse, "ghost"), None);
        // Re-recording replaces the ASSESSMENT (a record update, never a
        // lifecycle transition — the ledger holds no states).
        let gate = V2xConfig::enable(full_evidence()).expect("enabled");
        ledger
            .record(V2gReadinessRecord::new(
                SubjectKind::Vehicle,
                "veh-1".to_string(),
                capable_decl(),
                gate,
            )
            .expect("valid"))
            .expect("re-recorded");
        assert_eq!(
            ledger.readiness_of(SubjectKind::Vehicle, "veh-1"),
            Some(V2gReadinessLevel::Readied)
        );
        assert_eq!(ledger.len(), 2, "replacement, not accumulation");
    }

    #[test]
    fn the_ocpp21_hooks_are_design_records_with_unverified_labels() {
        // The hook set and names (UNVERIFIED spellings pinned so a change
        // is a conscious edit).
        let names: Vec<&str> = DECLARED_OCPP21_HOOKS
            .iter()
            .map(|hook| hook.name())
            .collect();
        assert_eq!(
            names,
            vec![
                "NotifyAllowedEnergyTransfer",
                "AFRRSignal",
                "SetDERControl"
            ]
        );
        // Every note carries its UNVERIFIED label and the lifecycle rule.
        for hook in DECLARED_OCPP21_HOOKS {
            let note = hook.note();
            assert!(note.contains("UNVERIFIED"), "hook {}", hook.name());
            assert!(
                note.contains("PROPOSED") || note.contains("record") || note.contains("INPUT"),
                "hook {} must name its record/proposal disposition: {note}",
                hook.name()
            );
        }
        // The statement names the scope-only citation discipline.
        assert!(OCPP21_HOOKS_ARE_DESIGN_RECORDS_STATEMENT.contains("DESIGN RECORDS"));
        assert!(OCPP21_HOOKS_ARE_DESIGN_RECORDS_STATEMENT.contains("capability SCOPE only"));
    }

    #[test]
    fn the_na_ac_path_is_a_separate_future_integration() {
        // The statement carries the §11 sources and the separation.
        assert!(NA_AC_PATH_STATEMENT.contains("[S-4]"));
        assert!(NA_AC_PATH_STATEMENT.contains("[S-131]"));
        assert!(NA_AC_PATH_STATEMENT.contains("[S-144]"));
        assert!(NA_AC_PATH_STATEMENT.contains("SEPARATE FUTURE INTEGRATION"));
        assert!(NA_AC_PATH_STATEMENT.contains("OUT OF SCOPE"));
        // The J3072 citation is honestly labelled (no §11 source entry).
        assert!(NA_AC_PATH_STATEMENT.contains("UNVERIFIED/ASSUMPTION"));
    }

    #[test]
    fn the_adr010_statement_travels_and_no_authority_surface_exists() {
        // The statement is recorded and complete.
        assert!(READINESS_NEVER_AUTHORIZES_DISPATCH.contains("never a command"));
        assert!(READINESS_NEVER_AUTHORIZES_DISPATCH.contains("PROPOSED"));
        // Structural check: the public surface of this module exposes only
        // records, verdicts, and statements — there is no function here
        // that returns a CommandState, a DispatchCommand, or a lifecycle
        // transition. Pinned by the absence of any such call in this test:
        // the strongest thing the ledger returns is a
        // V2gReadinessLevel (an enum with no authority semantics).
        let ledger = V2gReadinessLedger::new();
        assert_eq!(ledger.readiness_of(SubjectKind::Evse, "x"), None);
        // The gate-consumed statement names the additive discipline.
        assert!(FIVE_CONDITION_GATE_CONSUMED_STATEMENT.contains("CONSUMED"));
        assert!(FIVE_CONDITION_GATE_CONSUMED_STATEMENT.contains("never redefined"));
    }

    #[test]
    fn verdict_names_are_canonical() {
        assert_eq!(V2gReadinessLevel::NotCapable.name(), "NOT_CAPABLE");
        assert_eq!(
            V2gReadinessLevel::CapableButGateDisabled.name(),
            "CAPABLE_BUT_GATE_DISABLED"
        );
        assert_eq!(V2gReadinessLevel::Readied.name(), "READIED");
        assert_eq!(SubjectKind::Evse.name(), "EVSE");
        assert_eq!(SubjectKind::Vehicle.name(), "VEHICLE");
    }
}
