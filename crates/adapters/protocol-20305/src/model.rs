// P14.02 §2 — The CSIP resource-tree model types (decision record
// 02_p14_02_aggregator_server.md §2).
//
// The IEEE 2030.5 resource hierarchy CSIP requires of an aggregator
// [S-1][S-3]: EndDeviceList, FunctionSetAssignmentsList, DERProgramList
// (with DefaultDERControl and DERControlList), ResponseList, pricing
// resources, and log-event resources. These are the platform-side value
// types: serde-serializable design records that the server skeleton
// ([`crate::server`]) stores and serves. They are NOT wire types — the
// 2030.5 wire encoding (EXI/XML per the standard, UNVERIFIED) is a
// transport-seam concern; this module's serde spellings are the JSON-facing
// design surface.
//
// UNVERIFIED-ASSUMPTION register (the `adapters::openems` pattern, plan §7.6
// — every point below is cited from SECONDARY sources [S-1][S-3][S-5] and
// must be checked against the IEEE 2030.5-2018 text or a lab Edge before
// any implementation-time use; a mismatch is a code or record edit, never a
// silent reinterpretation):
//
// 1. Attribute spellings (camelCase, e.g. `opModGenLimW`) follow the 2030.5
//    XSD as transcribed in the QualityLogic/CSIP secondary material
//    [S-3][S-5] — UNVERIFIED against the standard's schema.
// 2. `TimeType` is modelled as i64 seconds since the Unix epoch with an
//    optional timezone offset — the 2030.5 TimeType/TimeOffsetType pair
//    UNVERIFIED (sign conventions, epoch base).
// 3. SFDI/LFDI derivation and formats (decimal vs hex, hash input) are
//    UNVERIFIED — the security record 03 §3 registers the implementation-
//    time check; the types here only enforce shape.
// 4. List semantics (`all`/`results` counts, `pollRate`) UNVERIFIED.
// 5. Response status codes (0–7) UNVERIFIED.
// 6. DERControlBase operator limits are carried in NATIVE WATTS (the
//    protocol's unit) — canonical MW/MVAr conversion happens at the
//    anti-corruption layer per `flexcore-contracts/src/units.rs`, declared
//    per device profile, never inferred (AGENTS.md §4 units rule).

use serde::{Deserialize, Serialize};

use crate::addressing::ResourceHref;

/// A model-level validation error (mRID/href/SFDI shape). Every failure is
/// named; nothing is silently repaired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// The mRID (the 2030.5 resource identifier, UNVERIFIED format) is
    /// empty or whitespace.
    EmptyMrid(&'static str),
    /// A required href is missing where the resource kind requires one.
    MissingHref(&'static str),
    /// A numeric field violates its recorded shape (e.g. SFDI zero).
    InvalidValue { field: &'static str, reason: String },
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::EmptyMrid(where_) => write!(f, "mRID of {where_} is empty"),
            ModelError::MissingHref(where_) => write!(f, "required href of {where_} is missing"),
            ModelError::InvalidValue { field, reason } => {
                write!(f, "field '{field}' is invalid: {reason}")
            }
        }
    }
}

impl std::error::Error for ModelError {}

/// The 2030.5 `TimeType` analogue (UNVERIFIED: i64 seconds since the Unix
/// epoch; sign conventions and the epoch base must be checked against the
/// standard text at implementation time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TimeType(pub i64);

impl TimeType {
    /// Canonical name for records and error messages.
    pub const NAME: &'static str = "TimeType";
}

/// SFDI — the Short Form Device Identifier (UNVERIFIED: the 2030.5 short
/// device identifier derived from the LFDI; the exact derivation is the
/// security record 03 §3 implementation-time check). Modelled as a
/// non-zero decimal integer; the API boundary never infers one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Sfdi(pub u64);

impl Sfdi {
    /// Validates the recorded SFDI shape: non-zero (a zero SFDI is an
    /// enrollment error, not a wildcard — UNVERIFIED whether the standard
    /// reserves zero; the fail-closed reading is enforced here until
    /// verified).
    pub fn new(value: u64) -> Result<Self, ModelError> {
        if value == 0 {
            return Err(ModelError::InvalidValue {
                field: "sfdi",
                reason: "zero SFDI is not permitted (fail-closed; UNVERIFIED whether the \
                         standard reserves zero)"
                    .to_string(),
            });
        }
        Ok(Sfdi(value))
    }
}

/// LFDI — the Long Form Device Identifier (UNVERIFIED: modelled as a
/// lowercase hex string; the exact bit length and hash input are the
/// security record 03 §3 implementation-time check).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Lfdi(pub String);

impl Lfdi {
    /// Validates the recorded LFDI shape: non-empty lowercase hex.
    pub fn new(value: String) -> Result<Self, ModelError> {
        if value.trim().is_empty() {
            return Err(ModelError::InvalidValue {
                field: "lfdi",
                reason: "empty LFDI".to_string(),
            });
        }
        if !value
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(ModelError::InvalidValue {
                field: "lfdi",
                reason: "LFDI must be lowercase hex (UNVERIFIED format; recorded shape only)"
                    .to_string(),
            });
        }
        Ok(Lfdi(value))
    }
}

/// The list metadata every 2030.5 list carries (UNVERIFIED: `all` and
/// `results` counts per the List schema transcribed in [S-3][S-5]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListMeta {
    /// The total number of resources in the list.
    pub all: u32,
    /// The number of results in THIS response (after paging).
    pub results: u32,
}

impl ListMeta {
    /// Builds metadata for an unpaged list of `count` resources.
    pub const fn unpaged(count: u32) -> Self {
        ListMeta {
            all: count,
            results: count,
        }
    }
}

// --- EndDevice (enrollment surface) ------------------------------------------

/// An enrolled end device (the 2030.5 EndDevice resource, CSIP enrollment
/// surface [S-3]; UNVERIFIED attribute spellings). SFDI/LFDI handling and
/// the certificate lifecycle are the security record's subject (03), not
/// this type's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndDevice {
    /// The device SFDI.
    pub sfdi: Sfdi,
    /// The device LFDI, when the enrollment record carries it.
    pub lfdi: Option<Lfdi>,
    /// The FunctionSetAssignmentsList href for this device, when assigned.
    pub function_set_assignments_link: Option<ResourceHref>,
    /// The DERProgramList href for this device, when assigned.
    pub der_program_list_link: Option<ResourceHref>,
}

/// The EndDeviceList the aggregator serves (the CSIP enrollment surface
/// [S-3]; spellings UNVERIFIED).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndDeviceList {
    /// List metadata (all/results).
    pub meta: ListMeta,
    /// The declared poll rate in seconds, when the list declares one.
    pub poll_rate: Option<u32>,
    /// The enrolled devices.
    pub end_devices: Vec<EndDevice>,
}

// --- FunctionSetAssignments ---------------------------------------------------

/// A FunctionSetAssignments resource: the 2030.5 mechanism binding
/// function-set instances (control, reading, pricing) to a device or group
/// [S-1]; CSIP requires the aggregator to serve it [S-3].
///
/// The `primacy` field (UNVERIFIED semantics: CSIP carries a primacy value
/// whose ordering convention must be confirmed against the standard; the
/// declared resolution rules in [`crate::addressing`] treat the DECLARED
/// value as authoritative whatever the convention) is what the primacy
/// tests (P14.04) exercise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSetAssignments {
    /// The resource href, when served.
    pub href: Option<ResourceHref>,
    /// The resource mRID (validated non-empty in [`FunctionSetAssignments::new`]).
    pub mrid: String,
    /// The declared primacy value for this assignment (UNVERIFIED ordering
    /// convention; the resolution rules declare, never assume).
    pub primacy: Option<u8>,
    /// The DERProgramList link this assignment binds (CSIP: the aggregator
    /// serves DERP under FSA — UNVERIFIED path relationship).
    pub der_program_list_link: Option<ResourceHref>,
    /// The ResponseList link this assignment binds (UNVERIFIED).
    pub response_list_link: Option<ResourceHref>,
}

impl FunctionSetAssignments {
    /// Constructs an FSA with a validated mRID.
    pub fn new(
        mrid: String,
        primacy: Option<u8>,
        der_program_list_link: Option<ResourceHref>,
        response_list_link: Option<ResourceHref>,
    ) -> Result<Self, ModelError> {
        if mrid.trim().is_empty() {
            return Err(ModelError::EmptyMrid("FunctionSetAssignments"));
        }
        Ok(Self {
            href: None,
            mrid,
            primacy,
            der_program_list_link,
            response_list_link,
        })
    }
}

/// The FunctionSetAssignmentsList (CSIP resource [S-3]; spellings
/// UNVERIFIED).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunctionSetAssignmentsList {
    /// List metadata (all/results).
    pub meta: ListMeta,
    /// The declared poll rate in seconds, when the list declares one
    /// (UNVERIFIED field spelling).
    pub poll_rate: Option<u32>,
    /// The assignments.
    pub function_set_assignments: Vec<FunctionSetAssignments>,
}

// --- DERProgram / DefaultDERControl / DERControlList ---------------------------

/// A DERProgram resource: the container binding a DER control program to
/// devices [S-1][S-3] (spellings UNVERIFIED).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DERProgram {
    /// The resource href, when served.
    pub href: Option<ResourceHref>,
    /// The resource mRID (validated non-empty in [`DERProgram::new`]).
    pub mrid: String,
    /// The DefaultDERControl link (the program's default control base).
    pub default_der_control_link: Option<ResourceHref>,
    /// The DERControlList link (the program's active control list).
    pub der_control_list_link: Option<ResourceHref>,
}

impl DERProgram {
    /// Constructs a DER program with a validated mRID.
    pub fn new(
        mrid: String,
        default_der_control_link: Option<ResourceHref>,
        der_control_list_link: Option<ResourceHref>,
    ) -> Result<Self, ModelError> {
        if mrid.trim().is_empty() {
            return Err(ModelError::EmptyMrid("DERProgram"));
        }
        Ok(Self {
            href: None,
            mrid,
            default_der_control_link,
            der_control_list_link,
        })
    }
}

/// The DERProgramList (CSIP resource [S-3]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DERProgramList {
    /// List metadata (all/results).
    pub meta: ListMeta,
    /// The declared poll rate in seconds, when the list declares one.
    pub poll_rate: Option<u32>,
    /// The programs.
    pub der_programs: Vec<DERProgram>,
}

/// The DERControlBase: the operator-limit payload a control carries
/// (spellings UNVERIFIED against the 2030.5 schema). Values are NATIVE
/// protocol units (watts) — canonical-unit conversion is the
/// anti-corruption layer's declared mapping, never an inference.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DERControlBase {
    /// `opModEnergize` — energize/disconnect operator request (UNVERIFIED
    /// spelling and semantics; a protection-grade action this platform
    /// NEVER relays directly — it enters the lifecycle at PROPOSED like
    /// everything else).
    pub op_mod_energize: Option<bool>,
    /// `opModGenLimW` — generation limit in W (native units; UNVERIFIED).
    pub op_mod_gen_lim_w: Option<f64>,
    /// `opModLoadLimW` — load limit in W (native units; UNVERIFIED).
    pub op_mod_load_lim_w: Option<f64>,
    /// `opModStoreLimW` — storage limit in W (native units; UNVERIFIED).
    pub op_mod_store_lim_w: Option<f64>,
    /// `opModTargetW` — target power in W (native units; UNVERIFIED).
    pub op_mod_target_w: Option<f64>,
}

impl DERControlBase {
    /// Whether the base carries any limit at all (an all-None base is the
    /// no-op shape the DefaultDERControl fallback uses).
    pub const fn is_no_op(&self) -> bool {
        self.op_mod_energize.is_none()
            && self.op_mod_gen_lim_w.is_none()
            && self.op_mod_load_lim_w.is_none()
            && self.op_mod_store_lim_w.is_none()
            && self.op_mod_target_w.is_none()
    }
}

/// The DefaultDERControl resource: the program's default control base that
/// applies when no DERControl is active (CSIP resource [S-3]; spellings
/// UNVERIFIED).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefaultDERControl {
    /// The resource mRID.
    pub mrid: String,
    /// The default control base.
    pub der_control_base: DERControlBase,
}

/// A DERControl resource: a timed control carrying setpoints and targets
/// for one or more DERs — the shape external DR schedules arrive in
/// (phase-14 context §4; [S-1][S-3]; spellings UNVERIFIED).
///
/// The timing fields live in [`crate::scheduling::DerControlWindow`] form at
/// the mapping layer; this resource type records them verbatim.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DERControl {
    /// The resource href, when served.
    pub href: Option<ResourceHref>,
    /// The resource mRID (validated non-empty in [`DERControl::new`]).
    pub mrid: String,
    /// The creation timestamp (TimeType; UNVERIFIED field presence).
    pub created_date_time: Option<TimeType>,
    /// The interval start (TimeType seconds; UNVERIFIED).
    pub start: Option<TimeType>,
    /// The interval duration in seconds (UNVERIFIED).
    pub duration: Option<u32>,
    /// The randomized start offset in seconds (UNVERIFIED semantics; the
    /// scheduling module treats it as DECLARED data, never inferred).
    pub randomized_start: Option<u32>,
    /// The randomized duration offset in seconds (UNVERIFIED).
    pub randomized_duration: Option<u32>,
    /// The control payload.
    pub der_control_base: DERControlBase,
}

impl DERControl {
    /// Constructs a DER control with a validated mRID and a no-op-free
    /// payload check: a control with an all-None base AND no timing window
    /// is refused (an empty control is not a schedule).
    pub fn new(
        mrid: String,
        created_date_time: Option<TimeType>,
        start: Option<TimeType>,
        duration: Option<u32>,
        randomized_start: Option<u32>,
        randomized_duration: Option<u32>,
        der_control_base: DERControlBase,
    ) -> Result<Self, ModelError> {
        if mrid.trim().is_empty() {
            return Err(ModelError::EmptyMrid("DERControl"));
        }
        if der_control_base.is_no_op() && start.is_none() {
            return Err(ModelError::InvalidValue {
                field: "der_control_base",
                reason: "a DERControl with no payload and no timing window is not a schedule"
                    .to_string(),
            });
        }
        Ok(Self {
            href: None,
            mrid,
            created_date_time,
            start,
            duration,
            randomized_start,
            randomized_duration,
            der_control_base,
        })
    }

    /// The scheduling window of this control, if it declares one.
    pub fn window(&self) -> Option<crate::scheduling::DerControlWindow> {
        Some(crate::scheduling::DerControlWindow {
            start: self.start?,
            duration_seconds: self.duration?,
            randomized_start: self.randomized_start,
            randomized_duration: self.randomized_duration,
        })
    }
}

/// The DERControlList (CSIP resource [S-3]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DERControlList {
    /// List metadata (all/results).
    pub meta: ListMeta,
    /// The declared poll rate in seconds, when the list declares one.
    pub poll_rate: Option<u32>,
    /// The controls.
    pub der_controls: Vec<DERControl>,
}

// --- ResponseList ---------------------------------------------------------------

/// A Response resource: the device's acknowledgement record for a function
/// set or control (CSIP requires the aggregator to collect responses
/// [S-3]; spellings and the status vocabulary UNVERIFIED — the recorded
/// shape here carries a numeric status 0–255 until verified).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    /// The resource href, when served.
    pub href: Option<ResourceHref>,
    /// The responding device LFDI, when carried (UNVERIFIED field).
    pub end_device_lfdi: Option<Lfdi>,
    /// The response creation timestamp (UNVERIFIED).
    pub created_date_time: Option<TimeType>,
    /// The numeric status value (UNVERIFIED vocabulary; recorded as
    /// declared data, never interpreted).
    pub status: Option<u8>,
    /// The subject href the response refers to (UNVERIFIED).
    pub subject: Option<ResourceHref>,
}

/// The ResponseList the aggregator serves and the devices populate (CSIP
/// resource [S-3]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResponseList {
    /// List metadata (all/results).
    pub meta: ListMeta,
    /// The declared poll rate in seconds, when the list declares one.
    pub poll_rate: Option<u32>,
    /// The responses (bounded by the server's declared cap — P14.02 §4).
    pub responses: Vec<Response>,
}

// --- PricingList ----------------------------------------------------------------

/// A pricing resource placeholder (CSIP requires pricing resources in the
/// aggregator's tree [S-3]; the 2030.5 pricing classes (TariffProfile and
/// relatives) are modelled MINIMALLY here — the class inventory and
/// attribute spellings are UNVERIFIED and must be confirmed against the
/// standard at implementation time before any pricing flow is designed in
/// detail).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pricing {
    /// The resource href, when served.
    pub href: Option<ResourceHref>,
    /// The resource mRID (validated non-empty in [`Pricing::new`]).
    pub mrid: String,
    /// A human-safe rate label (the platform's own record; the protocol's
    /// price payload shape is UNVERIFIED and intentionally not guessed).
    pub rate_label: String,
}

impl Pricing {
    /// Constructs a pricing record with a validated mRID.
    pub fn new(mrid: String, rate_label: String) -> Result<Self, ModelError> {
        if mrid.trim().is_empty() {
            return Err(ModelError::EmptyMrid("Pricing"));
        }
        if rate_label.trim().is_empty() {
            return Err(ModelError::InvalidValue {
                field: "rate_label",
                reason: "empty rate label".to_string(),
            });
        }
        Ok(Self {
            href: None,
            mrid,
            rate_label,
        })
    }
}

/// The PricingList (CSIP resource [S-3]; contents UNVERIFIED — see
/// [`Pricing`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricingList {
    /// List metadata (all/results).
    pub meta: ListMeta,
    /// The declared poll rate in seconds, when the list declares one.
    pub poll_rate: Option<u32>,
    /// The pricing records.
    pub pricings: Vec<Pricing>,
}

// --- LogEventList -----------------------------------------------------------------

/// A log-event resource: the device's audit/log record the aggregator
/// collects (CSIP requires log-event collection [S-3]; field spellings and
/// the function-set vocabulary UNVERIFIED).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    /// The resource href, when served.
    pub href: Option<ResourceHref>,
    /// The event creation timestamp (UNVERIFIED).
    pub created_date_time: Option<TimeType>,
    /// The human-safe event details as recorded (raw protocol text is
    /// carried, but consumers must treat it as untrusted data — the P1.08
    /// rule applies at the presentation layer).
    pub details: String,
    /// The function set the event belongs to (UNVERIFIED vocabulary).
    pub function_set: Option<String>,
    /// Extended data, when carried (UNVERIFIED shape).
    pub extended_data: Option<String>,
}

impl LogEvent {
    /// Constructs a log event with a validated (non-empty) details field.
    pub fn new(
        created_date_time: Option<TimeType>,
        details: String,
        function_set: Option<String>,
        extended_data: Option<String>,
    ) -> Result<Self, ModelError> {
        if details.trim().is_empty() {
            return Err(ModelError::InvalidValue {
                field: "details",
                reason: "empty log event details".to_string(),
            });
        }
        Ok(Self {
            href: None,
            created_date_time,
            details,
            function_set,
            extended_data,
        })
    }
}

/// The LogEventList the aggregator serves and the devices populate (CSIP
/// resource [S-3]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEventList {
    /// List metadata (all/results).
    pub meta: ListMeta,
    /// The declared poll rate in seconds, when the list declares one.
    pub poll_rate: Option<u32>,
    /// The log events (bounded by the server's declared cap — P14.02 §4).
    pub log_events: Vec<LogEvent>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn href(path: &str) -> ResourceHref {
        ResourceHref::new(path.to_string()).expect("valid href")
    }

    #[test]
    fn sfdi_rejects_zero_fail_closed() {
        assert_eq!(
            Sfdi::new(0),
            Err(ModelError::InvalidValue {
                field: "sfdi",
                reason: "zero SFDI is not permitted (fail-closed; UNVERIFIED whether the \
                         standard reserves zero)"
                    .to_string(),
            })
        );
        assert_eq!(Sfdi::new(1_234_567_891), Ok(Sfdi(1_234_567_891)));
    }

    #[test]
    fn lfdi_rejects_empty_and_uppercase() {
        assert!(Lfdi::new(String::new()).is_err());
        assert!(Lfdi::new("  ".to_string()).is_err());
        assert!(Lfdi::new("ABC123".to_string()).is_err());
        assert_eq!(
            Lfdi::new("0a1b2c3d".to_string()),
            Ok(Lfdi("0a1b2c3d".to_string()))
        );
    }

    #[test]
    fn fsa_and_program_and_control_validate_mrids() {
        assert_eq!(
            FunctionSetAssignments::new(
                "  ".to_string(),
                None,
                Some(href("/derp")),
                Some(href("/rsps")),
            ),
            Err(ModelError::EmptyMrid("FunctionSetAssignments"))
        );
        assert_eq!(
            DERProgram::new(String::new(), None, None),
            Err(ModelError::EmptyMrid("DERProgram"))
        );
        assert_eq!(
            DERControl::new(
                String::new(),
                None,
                Some(TimeType(1000)),
                Some(300),
                None,
                None,
                DERControlBase {
                    op_mod_gen_lim_w: Some(5000.0),
                    ..DERControlBase::default()
                },
            ),
            Err(ModelError::EmptyMrid("DERControl"))
        );
        // A control with no payload and no timing window is refused.
        assert_eq!(
            DERControl::new(
                "ctrl".to_string(),
                None,
                None,
                None,
                None,
                None,
                DERControlBase::default(),
            ),
            Err(ModelError::InvalidValue {
                field: "der_control_base",
                reason: "a DERControl with no payload and no timing window is not a schedule"
                    .to_string(),
            })
        );
    }

    #[test]
    fn control_base_no_op_detection_and_window_access() {
        let base = DERControlBase::default();
        assert!(base.is_no_op());
        let payload = DERControlBase {
            op_mod_gen_lim_w: Some(4000.0),
            ..DERControlBase::default()
        };
        assert!(!payload.is_no_op());

        let control = DERControl::new(
            "ctrl-1".to_string(),
            Some(TimeType(100)),
            Some(TimeType(200)),
            Some(300),
            None,
            None,
            payload,
        )
        .expect("valid control");
        let window = control.window().expect("timing window present");
        assert_eq!(window.start, TimeType(200));
        assert_eq!(window.duration_seconds, 300);
        assert_eq!(window.active_at(TimeType(350)), Some(true));
        assert_eq!(window.active_at(TimeType(550)), Some(false));
        // A control without timing has no window.
        let timeless = DERControl::new(
            "ctrl-2".to_string(),
            None,
            None,
            None,
            None,
            None,
            payload,
        )
        .expect("valid control (payload present)");
        assert!(timeless.window().is_none());
    }

    #[test]
    fn pricing_and_log_event_validate_fields() {
        assert_eq!(
            Pricing::new(" ".to_string(), "rate".to_string()),
            Err(ModelError::EmptyMrid("Pricing"))
        );
        assert_eq!(
            LogEvent::new(None, "  ".to_string(), None, None),
            Err(ModelError::InvalidValue {
                field: "details",
                reason: "empty log event details".to_string(),
            })
        );
        let event = LogEvent::new(
            Some(TimeType(1)),
            "device enrolled".to_string(),
            Some("fs-control".to_string()),
            None,
        )
        .expect("valid event");
        assert_eq!(event.details, "device enrolled");
    }

    #[test]
    fn resource_tree_types_round_trip_through_json_with_camel_case() {
        // The serde spellings are the design surface; camelCase is
        // UNVERIFIED against the 2030.5 schema (module header, assumption 1).
        let control = DERControl::new(
            "ctrl-json".to_string(),
            None,
            Some(TimeType(10)),
            Some(20),
            None,
            None,
            DERControlBase {
                op_mod_gen_lim_w: Some(1000.0),
                ..DERControlBase::default()
            },
        )
        .expect("valid control");
        let json = serde_json::to_string(&control).expect("serializes");
        assert!(json.contains("\"mrid\":\"ctrl-json\""), "mrid key: {json}");
        assert!(json.contains("opModGenLimW"), "camelCase payload key: {json}");
        let back: DERControl = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back, control);
    }
}
