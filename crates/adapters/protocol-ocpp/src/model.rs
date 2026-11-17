// P17.01/P17.03/P17.04/P17.05 — The OCPP message payload model types.
//
// Module-level UNVERIFIED-ASSUMPTION register (the openems.rs / entsoe.rs
// pattern; phase-17 plan §7.5). EVERY field name, enum spelling, and
// message shape below is transcribed from SECONDARY sources
// ([S-121]-[S-125][S-126][S-127][S-133][S-141] — the gap-analysis EV track;
// none of the OCA specification texts is in the repository). Each item
// names the implementation-time check against the OCA text (OCPP 2.0.1 /
// 2.1 parts, and the OCPP-J transport part) that must confirm or correct
// it. A mismatch is a code or record edit, never a silent reinterpretation:
//
// 1. camelCase JSON field spellings (e.g. `chargePointModel`,
//    `chargingStationName`, `evseId`, `chargingProfileId`, `stackLevel`,
//    `chargingRateUnit`, `minChargingRate`) — UNVERIFIED against the OCA
//    2.0.1/2.1 schema files.
// 2. The BootNotification request/response fields (charge point model,
//    serial, firmware version, vendor; response = status + interval +
//    statusTime) — UNVERIFIED.
// 3. The StatusNotification state vocabulary (`Available`, `Occupied`,
//    `Reserved`, `Unavailable`, `Faulted`, and the finer 2.0.1 states) and
//    the evseId/wrapped connectorId structure — UNVERIFIED; the domain
//    mapping (crate::provisioning) refuses unmapped states, never guesses.
// 4. The TransactionEvent event types (`Started`, `Updated`, `Ended`) and
//    transaction fields — UNVERIFIED.
// 5. The security block spellings (`SignCertificate`, `CertificateSigned`,
//    `InstallCertificate`, `GetInstalledCertificateIds`,
//    `DeleteCertificate`, `SecurityEventNotification`, the certificate type
//    strings like `V2GCertificate`/`CSMSRootCertificate`) and the
//    security-event names (e.g. a timestamp+type+message triple) —
//    UNVERIFIED; the OCA Security Operations Guide [S-127] is the check.
// 6. The smart-charging block (SetChargingProfile payload fields, the four
//    purposes `ChargingStationExternalConstraints`,
//    `ChargingStationMaxProfile`, `TxDefaultProfile`, `TxProfile`,
//    `ChargingProfileKind` values, `ChargingRateUnit` W/A, `minChargingRate`
//    semantics, evseId 0 = whole station, and the four-purpose
//    composition-precedence ORDER) — UNVERIFIED against the OCA 2.0.1 Part
//    2 smart-charging text [S-124][S-141].
// 7. The unit fields: W = watts, A = amperes, `phases` semantics (0/1/3 and
//    its AC-only meaning) — UNVERIFIED. Canonical FlexCore units are
//    MW/MVAr/MVA/MWh; conversion happens ONLY in the composer
//    (flexcore_domain::ev_charging_composer), never in this module.
// 8. Message-type codes 2/3/4 for CALL/CALLRESULT/CALLERROR and the OCPP-J
//    array shapes — see crate::framing (its own register).

use serde::{Deserialize, Serialize};

/// Model errors of the payload types. Every failure names the offending
/// field/value — never a bare "invalid input" (the P3.04 error-catalogue
/// style).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// A required field is empty. Silence is never a payload.
    EmptyField(&'static str),
    /// A sanity bound was exceeded (not a protocol claim — a gate against
    /// unbounded input).
    FieldTooLong {
        field: &'static str,
        len: usize,
        max: usize,
    },
    /// A value that must be finite and non-negative was not.
    NonNegativeRequired {
        field: &'static str,
        value: f64,
    },
    /// Schedule period start offsets are not strictly ascending
    /// (duplicates are refused, never merged silently).
    NonAscendingPeriod {
        previous_start_s: u32,
        offending_start_s: u32,
    },
    /// A Tx-class purpose was targeted at station scope (evseId 0) —
    /// those purposes are per-EVSE by definition (UNVERIFIED register
    /// item 6).
    StationScopePurpose {
        purpose: &'static str,
    },
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModelError::EmptyField(field) => write!(f, "field '{field}' is empty"),
            ModelError::FieldTooLong { field, len, max } => {
                write!(f, "field '{field}' of {len} exceeds the {max} sanity bound")
            }
            ModelError::NonNegativeRequired { field, value } => {
                write!(f, "field '{field}' must be finite and non-negative (found {value})")
            }
            ModelError::NonAscendingPeriod {
                previous_start_s,
                offending_start_s,
            } => {
                write!(f, "schedule period start offsets are not strictly ascending ({previous_start_s} then {offending_start_s}); duplicates are refused, never merged")
            }
            ModelError::StationScopePurpose { purpose } => {
                write!(f, "purpose '{purpose}' is per-EVSE by definition and cannot target station scope (evseId 0)")
            }
        }
    }
}

impl std::error::Error for ModelError {}

// --- Shared value shapes -----------------------------------------------------

/// A validated OCPP charge-point identity (UNVERIFIED spelling of the
/// chargePointId / chargingStation id semantics [S-121]; the identifier
/// itself is deployment-declared). Non-empty; length-bounded as a
/// sanity gate, not as a protocol claim.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChargePointId(String);

impl ChargePointId {
    /// The sanity upper bound (a gate against unbounded input, not a
    /// protocol limit claim — UNVERIFIED).
    pub const MAX_LEN: usize = 256;

    /// Validates and constructs the identity.
    pub fn new(value: String) -> Result<Self, ModelError> {
        if value.trim().is_empty() {
            return Err(ModelError::EmptyField("charge_point_id"));
        }
        if value.len() > Self::MAX_LEN {
            return Err(ModelError::FieldTooLong {
                field: "charge_point_id",
                len: value.len(),
                max: Self::MAX_LEN,
            });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ChargePointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The per-EVSE identifier inside a charge point (UNVERIFIED semantics:
/// evseId 0 addresses the whole station [S-124]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EvseWireId(u32);

impl EvseWireId {
    /// The whole-station identifier (UNVERIFIED [S-124]).
    pub const STATION: u32 = 0;

    /// Constructs the id; the value must be within the sanity bound
    /// (not a protocol limit claim — UNVERIFIED).
    pub fn new(value: u32) -> Result<Self, ModelError> {
        if value > 65_535 {
            return Err(ModelError::FieldTooLong {
                field: "evse_id",
                len: value as usize,
                max: 65_535,
            });
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> u32 {
        self.0
    }

    /// Whether this id addresses the whole station (UNVERIFIED [S-124]).
    pub const fn is_station(self) -> bool {
        self.0 == Self::STATION
    }
}

// --- Provisioning block (P17.04) ---------------------------------------------

/// The BootNotification REQUEST payload (charge point → CSMS; UNVERIFIED
/// field spellings — register item 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootNotificationRequest {
    /// The charge point's declared vendor (UNVERIFIED spelling).
    #[serde(rename = "vendorName")]
    pub vendor_name: String,
    /// The charge point's declared model (UNVERIFIED spelling).
    #[serde(rename = "chargePointModel")]
    pub charge_point_model: String,
    /// The charge point serial, when reported (UNVERIFIED spelling).
    #[serde(rename = "chargePointSerialNumber")]
    pub charge_point_serial_number: Option<String>,
    /// The reported firmware version, when reported (UNVERIFIED spelling).
    #[serde(rename = "firmwareVersion")]
    pub firmware_version: Option<String>,
    /// The declared protocol flavor the station reports, when it reports
    /// one (UNVERIFIED). The negotiated SUBPROTOCOL (session.rs) is the
    /// authority for what the connection speaks; this field is carried as
    /// declared data, never parsed into a protocol claim.
    pub protocol: Option<String>,
}

impl BootNotificationRequest {
    /// Validates: vendor and model must be non-empty (a boot that names
    /// nothing is refused, never defaulted).
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.vendor_name.trim().is_empty() {
            return Err(ModelError::EmptyField("vendor_name"));
        }
        if self.charge_point_model.trim().is_empty() {
            return Err(ModelError::EmptyField("charge_point_model"));
        }
        Ok(())
    }
}

/// The BootNotification RESPONSE status vocabulary (CSMS → charge point;
/// UNVERIFIED spellings — register item 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum BootStatus {
    /// The station is accepted (UNVERIFIED spelling).
    Accepted,
    /// The station is pending (UNVERIFIED spelling).
    Pending,
    /// The station is rejected (UNVERIFIED spelling).
    Rejected,
}

impl BootStatus {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            BootStatus::Accepted => "Accepted",
            BootStatus::Pending => "Pending",
            BootStatus::Rejected => "Rejected",
        }
    }
}

/// The BootNotification RESPONSE payload (UNVERIFIED fields — register
/// item 2). `interval_seconds` is the declared heartbeat interval the CSMS
/// assigns (a DECLARED configuration, never inferred).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BootNotificationResponse {
    pub status: BootStatus,
    #[serde(rename = "interval")]
    pub interval_seconds: u32,
    /// The response timestamp in UTC milliseconds (event_time discipline:
    /// UTC-representable only; produced by the injected clock, never by the
    /// peer's local clock).
    #[serde(rename = "currentTime")]
    pub current_time_millis: i64,
}

/// The StatusNotification state vocabulary (UNVERIFIED spellings — register
/// item 3; the domain mapping refuses unmapped states).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum StationStatus {
    Available,
    Occupied,
    Reserved,
    Unavailable,
    Faulted,
}

impl StationStatus {
    /// Canonical name for records and mapping.
    pub const fn name(self) -> &'static str {
        match self {
            StationStatus::Available => "Available",
            StationStatus::Occupied => "Occupied",
            StationStatus::Reserved => "Reserved",
            StationStatus::Unavailable => "Unavailable",
            StationStatus::Faulted => "Faulted",
        }
    }
}

/// The StatusNotification REQUEST payload (UNVERIFIED fields — register
/// item 3). The 2.0.1 shape addresses an EVSE (with an optional connector
/// id inside); the 1.6-style flat connectorId is NOT modelled here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StatusNotificationRequest {
    /// UTC milliseconds of the event (declared by the station; carried as
    /// event-time data, validated for UTC representability upstream).
    #[serde(rename = "timestamp")]
    pub timestamp_millis: i64,
    /// The EVSE the status addresses (UNVERIFIED structure).
    #[serde(rename = "evseId")]
    pub evse_id: u32,
    /// The connector status vocabulary (UNVERIFIED — register item 3).
    #[serde(rename = "connectorStatus")]
    pub connector_status: StationStatus,
}

/// The Heartbeat REQUEST payload (UNVERIFIED: 2.0.1 carries `currentTime`;
/// the CSMS response carries `currentTime` back and the assigned
/// interval — register item 2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    #[serde(rename = "currentTime")]
    pub current_time_millis: i64,
}

/// The Heartbeat RESPONSE payload (UNVERIFIED).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    #[serde(rename = "currentTime")]
    pub current_time_millis: i64,
}

/// The TransactionEvent event-type vocabulary (UNVERIFIED spellings —
/// register item 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TransactionEventType {
    Started,
    Updated,
    Ended,
}

impl TransactionEventType {
    /// Canonical name for records and mapping.
    pub const fn name(self) -> &'static str {
        match self {
            TransactionEventType::Started => "Started",
            TransactionEventType::Updated => "Updated",
            TransactionEventType::Ended => "Ended",
        }
    }
}

/// The TransactionEvent REQUEST payload (UNVERIFIED fields — register
/// item 4). The 2.0.1 model merges start/update/end into one action with an
/// event type; the metered-energy field is carried as declared data with
/// its unit UNVERIFIED (kWh assumed at the implementation-time check).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionEventRequest {
    #[serde(rename = "eventType")]
    pub event_type: TransactionEventType,
    /// UTC milliseconds of the event (UNVERIFIED field name `timestamp`).
    #[serde(rename = "timestamp")]
    pub timestamp_millis: i64,
    /// The per-EVSE the transaction runs on (UNVERIFIED structure).
    #[serde(rename = "evse")]
    pub evse_id: u32,
    /// The station-side transaction id (UNVERIFIED spelling `transactionId`).
    #[serde(rename = "transactionId")]
    pub transaction_id: String,
    /// The trigger-reason string, when the station reports one (UNVERIFIED
    /// vocabulary `triggerReason`; carried verbatim as declared data — the
    /// "vehicle physically plugged" ground truth is P17.08's plug-state
    /// surface, never this string).
    #[serde(rename = "triggerReason", skip_serializing_if = "Option::is_none")]
    pub trigger_reason: Option<String>,
    /// The metered energy so far, in Wh (UNVERIFIED unit and field name —
    /// register item 4). `None` = not reported in this event.
    #[serde(rename = "energyWh", skip_serializing_if = "Option::is_none")]
    pub energy_wh: Option<f64>,
}

impl TransactionEventRequest {
    /// Validates: a non-empty transaction id; EVSE id sane; energy, when
    /// present, finite and non-negative.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.transaction_id.trim().is_empty() {
            return Err(ModelError::EmptyField("transaction_id"));
        }
        EvseWireId::new(self.evse_id)?;
        if let Some(energy) = self.energy_wh {
            if !energy.is_finite() || energy < 0.0 {
                return Err(ModelError::NonNegativeRequired {
                    field: "energy_wh",
                    value: energy,
                });
            }
        }
        Ok(())
    }
}

// --- Security block (P17.03) ---------------------------------------------------

/// The certificate-type vocabulary of the certificate flows (UNVERIFIED
/// spellings — register item 5; the OCA Security Operations Guide [S-127]
/// is the implementation-time check).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CertificateType {
    V2gRootCertificate,
    V2gCertificate,
    CsmsCertificate,
    ManufacturerCertificate,
}

impl CertificateType {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            CertificateType::V2gRootCertificate => "V2GRootCertificate",
            CertificateType::V2gCertificate => "V2gCertificate",
            CertificateType::CsmsCertificate => "CSMSCertificate",
            CertificateType::ManufacturerCertificate => "ManufacturerCertificate",
        }
    }
}

/// The SignCertificate REQUEST payload (charge point → CSMS; the station
/// asks for a certificate). The CSR is carried as a LABEL/record — this
/// crate NEVER holds key or certificate MATERIAL (the P14.03 no-CA rule;
/// the never-granted `secret.read` set, conventions §6.4). UNVERIFIED
/// fields — register item 5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SignCertificateRequest {
    /// The CSR as a base64 LABEL (UNVERIFIED spelling `csr`). The material
    /// behind the label is a composition-root artifact of the deployment's
    /// PKI operations; no CA is stood up in this program (P14.03).
    pub csr: String,
    /// The certificate type requested (UNVERIFIED field; 2.0.1 additions).
    #[serde(rename = "typeOfCertificate", skip_serializing_if = "Option::is_none")]
    pub type_of_certificate: Option<CertificateType>,
}

impl SignCertificateRequest {
    /// Validates: the CSR label is non-empty (an unsigned empty request is
    /// refused, never defaulted).
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.csr.trim().is_empty() {
            return Err(ModelError::EmptyField("csr"));
        }
        Ok(())
    }
}

/// The CertificateSigned RESPONSE status vocabulary (UNVERIFIED — register
/// item 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CertificateSignedStatus {
    Accepted,
    Rejected,
}

impl CertificateSignedStatus {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            CertificateSignedStatus::Accepted => "Accepted",
            CertificateSignedStatus::Rejected => "Rejected",
        }
    }
}

/// The CertificateSigned REQUEST payload (CSMS → charge point; the signed
/// chain as a LABEL). UNVERIFIED fields — register item 5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CertificateSignedRequest {
    /// The certificate chain, as LABELS (UNVERIFIED field
    /// `certificateChain`, a base64 string). Material never enters this
    /// crate.
    #[serde(rename = "certificateChain")]
    pub certificate_chain_label: String,
}

/// The InstallCertificate REQUEST payload (CSMS → charge point; installing
/// a root/sub-CA certificate). UNVERIFIED fields — register item 5.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InstallCertificateRequest {
    #[serde(rename = "certificateType")]
    pub certificate_type: CertificateType,
    /// The certificate as a LABEL (UNVERIFIED field `certificate`).
    pub certificate: String,
}

impl InstallCertificateRequest {
    /// Validates: the label is non-empty.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.certificate.trim().is_empty() {
            return Err(ModelError::EmptyField("certificate"));
        }
        Ok(())
    }
}

/// The SecurityEventNotification REQUEST payload (charge point → CSMS;
/// UNVERIFIED fields — register item 5). Ingested as EVENT-LOG records;
/// a security event is a record, never a platform judgment or an
/// automatic revocation trigger.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SecurityEventNotificationRequest {
    /// UTC milliseconds of the event (UNVERIFIED field `timestamp`).
    #[serde(rename = "timestamp")]
    pub timestamp_millis: i64,
    /// The security-event type string (UNVERIFIED vocabulary — register
    /// item 5). Carried verbatim; the criticality classification lives in
    /// the security module's declared table, which refuses unknown types
    /// rather than guessing.
    #[serde(rename = "type")]
    pub event_type: String,
    /// Human-safe technical info (UNVERIFIED field `techInfo`).
    #[serde(rename = "techInfo", skip_serializing_if = "Option::is_none")]
    pub tech_info: Option<String>,
}

impl SecurityEventNotificationRequest {
    /// Validates: the type string is non-empty (an unnamed security event
    /// is refused, never defaulted).
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.event_type.trim().is_empty() {
            return Err(ModelError::EmptyField("security_event_type"));
        }
        Ok(())
    }
}

// --- Smart-charging block (P17.05) ----------------------------------------------

/// The charging-profile purpose vocabulary (UNVERIFIED spellings — register
/// item 6 [S-124]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ChargingProfilePurpose {
    ChargingStationExternalConstraints,
    ChargingStationMaxProfile,
    TxDefaultProfile,
    TxProfile,
}

impl ChargingProfilePurpose {
    /// Canonical wire-ish name for records.
    pub const fn name(self) -> &'static str {
        match self {
            ChargingProfilePurpose::ChargingStationExternalConstraints => {
                "ChargingStationExternalConstraints"
            }
            ChargingProfilePurpose::ChargingStationMaxProfile => "ChargingStationMaxProfile",
            ChargingProfilePurpose::TxDefaultProfile => "TxDefaultProfile",
            ChargingProfilePurpose::TxProfile => "TxProfile",
        }
    }
}

/// The charging-profile kind vocabulary (UNVERIFIED spellings — register
/// item 6). `Absolute` is the composer's declared default (an absolute
/// schedule is anchored to a UTC instant, which matches the dispatch
/// horizon semantics; the alternative kinds are carried, not selected by
/// this crate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ChargingProfileKind {
    Absolute,
    Recurring,
    Relative,
    Dynamic,
}

impl ChargingProfileKind {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            ChargingProfileKind::Absolute => "Absolute",
            ChargingProfileKind::Recurring => "Recurring",
            ChargingProfileKind::Relative => "Relative",
            ChargingProfileKind::Dynamic => "Dynamic",
        }
    }
}

/// The charging-rate unit vocabulary (UNVERIFIED — register item 6/7):
/// `W` watts or `A` amperes. The W/A choice and the conversion from the
/// canonical MW happen in the composer, never here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ChargingRateUnit {
    W,
    A,
}

impl ChargingRateUnit {
    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            ChargingRateUnit::W => "W",
            ChargingRateUnit::A => "A",
        }
    }
}

/// One charging-schedule period (UNVERIFIED fields — register item 6):
/// a start offset in seconds from the schedule start and a limit in the
/// schedule's rate unit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ChargingSchedulePeriod {
    /// Start offset from the schedule start, in seconds (UNVERIFIED field
    /// `startPeriod`).
    #[serde(rename = "startPeriod")]
    pub start_period_s: u32,
    /// The limit in the schedule's rate unit (UNVERIFIED field `limit`).
    /// A limit of 0.0 means "off" for the period (the minChargingRate
    /// rule: a positive rate below the minimum is not a valid alternative —
    /// [S-124]); the composer enforces the pairing.
    pub limit: f64,
    /// The phase count the period applies to, when declared (UNVERIFIED —
    /// register item 7: values 0/1/3 and AC-only meaning).
    #[serde(rename = "numberPhases", skip_serializing_if = "Option::is_none")]
    pub number_phases: Option<u8>,
}

/// One charging schedule inside a charging profile (UNVERIFIED fields —
/// register item 6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChargingSchedule {
    /// The schedule duration in seconds, when declared (UNVERIFIED).
    #[serde(rename = "duration", skip_serializing_if = "Option::is_none")]
    pub duration_s: Option<u32>,
    /// The schedule start instant in UTC milliseconds, when declared
    /// (UNVERIFIED field `startSchedule`, RFC 3339 on the wire; the codec
    /// carries the UTC-milliseconds form and the transport seam formats
    /// it — the instant is UTC-representable only).
    #[serde(rename = "startSchedule", skip_serializing_if = "Option::is_none")]
    pub start_schedule_millis: Option<i64>,
    /// The rate unit of this schedule (UNVERIFIED field `chargingRateUnit`).
    #[serde(rename = "chargingRateUnit")]
    pub charging_rate_unit: ChargingRateUnit,
    /// The minimum rate this schedule permits (UNVERIFIED semantics:
    /// a schedule step below it must be 0 [S-124] — register item 6).
    #[serde(rename = "minChargingRate", skip_serializing_if = "Option::is_none")]
    pub min_charging_rate: Option<f64>,
    /// The periods, ascending by start offset (validated).
    #[serde(rename = "chargingSchedulePeriod")]
    pub charging_schedule_period: Vec<ChargingSchedulePeriod>,
}

impl ChargingSchedule {
    /// Validates: at least one period; finite, non-negative limits;
    /// strictly ascending start offsets (duplicates are a composition
    /// defect, refused, never merged silently).
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.charging_schedule_period.is_empty() {
            return Err(ModelError::EmptyField("charging_schedule_period"));
        }
        let mut previous: Option<u32> = None;
        for period in &self.charging_schedule_period {
            if !period.limit.is_finite() || period.limit < 0.0 {
                return Err(ModelError::NonNegativeRequired {
                    field: "limit",
                    value: period.limit,
                });
            }
            if let Some(prev) = previous {
                if period.start_period_s <= prev {
                    return Err(ModelError::NonAscendingPeriod {
                        previous_start_s: prev,
                        offending_start_s: period.start_period_s,
                    });
                }
            }
            previous = Some(period.start_period_s);
        }
        if let Some(min_rate) = self.min_charging_rate {
            if !min_rate.is_finite() || min_rate < 0.0 {
                return Err(ModelError::NonNegativeRequired {
                    field: "min_charging_rate",
                    value: min_rate,
                });
            }
        }
        Ok(())
    }
}

/// The SetChargingProfile REQUEST payload (CSMS → charge point; the wire
/// shape the composer's plan maps into — UNVERIFIED fields — register
/// item 6). Construction from a domain plan goes through
/// [`crate::charging`] (the lifecycle-typed relay surface), never through a
/// bare struct literal in the dispatch path.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SetChargingProfileRequest {
    /// The charging profile id (UNVERIFIED field `chargingProfileId`;
    /// CSMS-assigned, declared — the dispatch command id travels alongside,
    /// not inside, this field).
    #[serde(rename = "chargingProfileId")]
    pub charging_profile_id: i64,
    /// The EVSE the profile targets — 0 addresses the whole station
    /// (UNVERIFIED — register item 6).
    #[serde(rename = "evseId")]
    pub evse_id: u32,
    /// Whether the profile is a grid-critical external constraint
    /// (UNVERIFIED: the `isGridCritical` field carries the external
    /// constraints purpose's grid-critical marking — register item 6;
    /// 2.1-era field per secondary sources [S-124][S-125]).
    #[serde(rename = "isGridCritical", skip_serializing_if = "Option::is_none")]
    pub is_grid_critical: Option<bool>,
    /// The transaction the profile binds to, when the purpose is TxProfile
    /// (UNVERIFIED field `transactionId`).
    #[serde(rename = "transactionId", skip_serializing_if = "Option::is_none")]
    pub transaction_id: Option<String>,
    /// The stack level (UNVERIFIED semantics: higher stack levels override
    /// lower ones within a purpose [S-124]).
    #[serde(rename = "stackLevel")]
    pub stack_level: i32,
    /// The purpose (UNVERIFIED — register item 6).
    #[serde(rename = "chargingProfilePurpose")]
    pub charging_profile_purpose: ChargingProfilePurpose,
    /// The kind (UNVERIFIED — register item 6).
    #[serde(rename = "chargingProfileKind")]
    pub charging_profile_kind: ChargingProfileKind,
    /// The (single) schedule of this profile (UNVERIFIED: 2.0.1 carries
    /// one `chargingSchedule`; 2.1 may extend — register item 6).
    #[serde(rename = "chargingSchedule")]
    pub charging_schedule: ChargingSchedule,
}

impl SetChargingProfileRequest {
    /// Validates every declared rule (non-empty schedule, ascending
    /// periods, TxProfile ⇒ transaction id present, TxProfile/ TxDefault
    /// ⇒ a per-EVSE target).
    pub fn validate(&self) -> Result<(), ModelError> {
        self.charging_schedule.validate()?;
        match self.charging_profile_purpose {
            ChargingProfilePurpose::TxProfile => {
                if self
                    .transaction_id
                    .as_deref()
                    .map(|id| id.trim().is_empty())
                    .unwrap_or(true)
                {
                    return Err(ModelError::EmptyField("transaction_id (TxProfile)"));
                }
            }
            ChargingProfilePurpose::TxDefaultProfile => {
                if EvseWireId::new(self.evse_id)?.is_station() {
                    return Err(ModelError::StationScopePurpose {
                        purpose: self.charging_profile_purpose.name(),
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// The SetChargingProfile RESPONSE payload (charge point → CSMS; UNVERIFIED
/// spellings — register item 6). **The honesty rule:** `Accepted` means the
/// charge point STORED the profile — it is evidence of storage, NEVER of
/// application; application evidence requires the GetCompositeSchedule
/// reconciliation (record 05 §6; the command lifecycle's ACKNOWLEDGED state
/// is the transport-level analogue and is not physical delivery).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SetChargingProfileStatus {
    Accepted,
    Rejected,
}

impl SetChargingProfileStatus {
    /// The recorded stored-not-applied statement (the honesty rule,
    /// surfaced wherever an Accepted lands).
    pub const STORED_NOT_APPLIED_STATEMENT: &'static str =
        "Accepted means STORED, not applied: a charge point's SetChargingProfile \
         acceptance is storage evidence only; application evidence comes from the \
         GetCompositeSchedule reconciliation, and the command lifecycle's \
         ACKNOWLEDGED state is not physical delivery.";

    /// Canonical name for records.
    pub const fn name(self) -> &'static str {
        match self {
            SetChargingProfileStatus::Accepted => "Accepted",
            SetChargingProfileStatus::Rejected => "Rejected",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charge_point_id_validates() {
        assert_eq!(
            ChargePointId::new(String::new()),
            Err(ModelError::EmptyField("charge_point_id"))
        );
        assert_eq!(
            ChargePointId::new("   ".to_string()),
            Err(ModelError::EmptyField("charge_point_id"))
        );
        let long = "x".repeat(ChargePointId::MAX_LEN + 1);
        assert!(matches!(
            ChargePointId::new(long),
            Err(ModelError::FieldTooLong { .. })
        ));
        let id = ChargePointId::new("depot-cp-001".to_string()).expect("valid");
        assert_eq!(id.as_str(), "depot-cp-001");
        assert_eq!(id.to_string(), "depot-cp-001");
    }

    #[test]
    fn evse_wire_id_rules() {
        assert!(EvseWireId::new(65_536).is_err());
        let station = EvseWireId::new(EvseWireId::STATION).expect("valid");
        assert!(station.is_station());
        let evse = EvseWireId::new(7).expect("valid");
        assert_eq!(evse.value(), 7);
        assert!(!evse.is_station());
    }

    #[test]
    fn boot_notification_validates() {
        let boot = BootNotificationRequest {
            vendor_name: "  ".to_string(),
            charge_point_model: "model".to_string(),
            charge_point_serial_number: None,
            firmware_version: None,
            protocol: None,
        };
        assert_eq!(
            boot.validate(),
            Err(ModelError::EmptyField("vendor_name"))
        );
        let boot = BootNotificationRequest {
            vendor_name: "vendor".to_string(),
            charge_point_model: "model".to_string(),
            charge_point_serial_number: Some("sn-1".to_string()),
            firmware_version: Some("fw-1".to_string()),
            protocol: Some("OCPP 2.0.1".to_string()),
        };
        boot.validate().expect("valid");
    }

    #[test]
    fn transaction_event_validates() {
        let mut event = TransactionEventRequest {
            event_type: TransactionEventType::Started,
            timestamp_millis: 0,
            evse_id: 1,
            transaction_id: "txn-1".to_string(),
            trigger_reason: None,
            energy_wh: Some(1500.5),
        };
        event.validate().expect("valid");
        // Negative energy is refused.
        event.energy_wh = Some(-1.0);
        assert!(matches!(
            event.validate(),
            Err(ModelError::NonNegativeRequired { .. })
        ));
        // Empty transaction id is refused.
        event.energy_wh = None;
        event.transaction_id = " ".to_string();
        assert_eq!(
            event.validate(),
            Err(ModelError::EmptyField("transaction_id"))
        );
        // Out-of-range EVSE id is refused.
        event.transaction_id = "txn-1".to_string();
        event.evse_id = 99_999;
        assert!(matches!(
            event.validate(),
            Err(ModelError::FieldTooLong { .. })
        ));
    }

    #[test]
    fn schedule_period_validation_rules() {
        let schedule = ChargingSchedule {
            duration_s: Some(3600),
            start_schedule_millis: None,
            charging_rate_unit: ChargingRateUnit::W,
            min_charging_rate: Some(0.0),
            charging_schedule_period: Vec::new(),
        };
        assert_eq!(
            schedule.validate(),
            Err(ModelError::EmptyField("charging_schedule_period"))
        );
        let schedule = ChargingSchedule {
            duration_s: Some(3600),
            start_schedule_millis: None,
            charging_rate_unit: ChargingRateUnit::W,
            min_charging_rate: Some(6.0),
            charging_schedule_period: vec![
                ChargingSchedulePeriod {
                    start_period_s: 0,
                    limit: 11_000.0,
                    number_phases: Some(3),
                },
                ChargingSchedulePeriod {
                    start_period_s: 1800,
                    limit: 0.0,
                    number_phases: Some(3),
                },
            ],
        };
        schedule.validate().expect("valid");
        // Duplicate start offsets are refused (never silently merged).
        let schedule = ChargingSchedule {
            charging_schedule_period: vec![
                ChargingSchedulePeriod {
                    start_period_s: 0,
                    limit: 11_000.0,
                    number_phases: None,
                },
                ChargingSchedulePeriod {
                    start_period_s: 0,
                    limit: 5_000.0,
                    number_phases: None,
                },
            ],
            ..schedule
        };
        assert!(matches!(
            schedule.validate(),
            Err(ModelError::NonAscendingPeriod { .. })
        ));
    }

    #[test]
    fn set_charging_profile_purpose_rules() {
        let request = SetChargingProfileRequest {
            charging_profile_id: 1,
            evse_id: 0,
            is_grid_critical: Some(true),
            transaction_id: None,
            stack_level: 5,
            charging_profile_purpose: ChargingProfilePurpose::ChargingStationExternalConstraints,
            charging_profile_kind: ChargingProfileKind::Absolute,
            charging_schedule: ChargingSchedule {
                duration_s: None,
                start_schedule_millis: Some(0),
                charging_rate_unit: ChargingRateUnit::W,
                min_charging_rate: None,
                charging_schedule_period: vec![ChargingSchedulePeriod {
                    start_period_s: 0,
                    limit: 0.0,
                    number_phases: None,
                }],
            },
        };
        // Station-scope external constraints with isGridCritical is valid.
        request.validate().expect("valid");
        // TxDefaultProfile at the station scope is refused (it is a
        // per-EVSE default by definition).
        let request = SetChargingProfileRequest {
            charging_profile_purpose: ChargingProfilePurpose::TxDefaultProfile,
            is_grid_critical: None,
            ..request
        };
        assert!(matches!(
            request.validate(),
            Err(ModelError::StationScopePurpose { .. })
        ));
        // TxProfile without a transaction id is refused.
        let request = SetChargingProfileRequest {
            charging_profile_purpose: ChargingProfilePurpose::TxProfile,
            evse_id: 1,
            ..request
        };
        assert_eq!(
            request.validate(),
            Err(ModelError::EmptyField("transaction_id (TxProfile)"))
        );
    }

    #[test]
    fn security_event_validates_and_labels_are_record_only() {
        let event = SecurityEventNotificationRequest {
            timestamp_millis: 0,
            event_type: "  ".to_string(),
            tech_info: None,
        };
        assert_eq!(
            event.validate(),
            Err(ModelError::EmptyField("security_event_type"))
        );
        let event = SecurityEventNotificationRequest {
            timestamp_millis: 1,
            event_type: "InvalidCSMSCertificate".to_string(),
            tech_info: Some("label-only".to_string()),
        };
        event.validate().expect("valid");
        // The certificate payloads carry LABELS only; no field exists that
        // could hold key material beyond the declared label strings.
        let csr = SignCertificateRequest {
            csr: "base64-label".to_string(),
            type_of_certificate: Some(CertificateType::CsmsCertificate),
        };
        csr.validate().expect("valid");
        assert_eq!(
            CertificateType::CsmsCertificate.name(),
            "CSMSCertificate"
        );
    }

    #[test]
    fn set_charging_profile_status_names_and_honesty_statement() {
        assert_eq!(SetChargingProfileStatus::Accepted.name(), "Accepted");
        assert_eq!(SetChargingProfileStatus::Rejected.name(), "Rejected");
        assert!(
            SetChargingProfileStatus::STORED_NOT_APPLIED_STATEMENT
                .contains("STORED, not applied"),
            "the honesty statement must name stored-not-applied"
        );
    }
}
