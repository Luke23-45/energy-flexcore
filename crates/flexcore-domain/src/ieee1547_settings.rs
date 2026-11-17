// P14.13 — IEEE 1547-2018 settings-profile repository (decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 61_p14_13_settings_profile_repository.md`).
//
// Sources: [S-19] MA TSRG Inverter Source Requirements Document
// (mass.gov), [S-20] IEEE 1547-2018 (standards.ieee.org/standard/
// 1547-2018.html). **Neither standard text was consulted** — every
// function name, category name, and table-shape detail below is
// transcribed from the secondary sources the gap analysis cites and
// carries an UNVERIFIED label at its definition. The implementation-time
// human gate verifies them against the purchased [S-20] text and the
// [S-19] document; a confirmed mismatch is a recorded edit to this
// module, never a silent reinterpretation (plan §7 note 6).
//
// THE DOCUMENT-STORE POSTURE (the load-bearing rule of this module):
//
//   This module is a **repository of declared settings DATA**. It records
//   what a jurisdiction's default table says and what a device's settings
//   state is. It NEVER claims that any device accepts any function, that
//   any device's settings are writable through any protocol, or that any
//   recorded value is the standard's correct value. Device function
//   support is per-device DECLARED data entered from an interconnection
//   record or a device profile — there is no "default function set" a
//   device is assumed to support [S-19][S-20].
//
// Placement (conventions §6.1): settings vocabulary is domain data — the
// repository types live in flexcore-domain and depend on nothing
// infrastructure (the crate takes flexcore-contracts only; no serde, no
// protocol-crate types). Protocol adapters (2030.5 / Modbus settings
// surfaces) map their native spellings INTO these types at their own
// boundary, exactly as every adapter maps into existing domain models
// (phase-14 context §2: "adapters never define parallel models").
//
// IEEE 1547-2018 SCOPE STATEMENT (the P3.08 inverter_pq_model.rs scope
// statement carried forward): IEEE 1547-2018 defines DER interconnection
// and interoperability performance requirements. This repository claims
// NO conformance with IEEE 1547-2018, IEEE 1547.1-2020, or UL 1741 SB;
// certification is a human-gated lab process (conventions §7.3). FlexCore
// is a supervisory orchestration layer: settings management here is
// record-keeping and workflow data, never inverter inner-loop control
// (AGENTS.md rules 2–3).
//
// Control status: pure domain vocabulary for the simulation-only 0.x
// line. No I/O, no device write path, no protocol coupling. A settings
// CHANGE is a lifecycle command (P14.14, `settings_management`), never a
// direct adapter write [S-20][S-156].

use std::collections::BTreeMap;

use flexcore_contracts::capability::Provenance;

use crate::ids::AssetId;

// --- The function catalog ----------------------------------------------------

/// The IEEE 1547-2018 function catalog as FlexCore models it (plan §2
/// P14.13: "volt-var, volt-watt, freq-watt, constant-PF, ramp rates,
/// enter/exit service, ride-through categories").
///
/// **UNVERIFIED (secondary-source transcription):** the function set and
/// these names are cited from [S-19][S-20] secondary material (the MA TSRG
/// Inverter SRD and gap-analysis Track F2). The implementation-time check
/// against the [S-20] text must confirm: the exact clause-5/6 function
/// names, whether additional functions belong in the catalog (e.g. reactive-
/// power-priority, voltage-reference), and the category vocabulary. A
/// confirmed mismatch is an enum rename or an additive variant with a
/// record edit — never a silent reinterpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SettingFunctionId {
    /// Constant power factor (the "constant-PF" row of plan §2 P14.13).
    /// UNVERIFIED against [S-20] Clause 5.
    ConstantPf,
    /// Volt-var curve function.
    /// UNVERIFIED against [S-20] Clause 5.
    VoltVar,
    /// Volt-watt curve function.
    /// UNVERIFIED against [S-20] Clause 5.
    VoltWatt,
    /// Frequency-watt curve function.
    /// UNVERIFIED against [S-20] Clause 5.
    FreqWatt,
    /// Ramp-rate limits (rate of change of active power).
    /// UNVERIFIED against [S-20] Clause 5/6.
    RampRate,
    /// Enter-service settings.
    /// UNVERIFIED against [S-20] Clause 5/6.
    EnterService,
    /// Exit-service settings.
    /// UNVERIFIED against [S-20] Clause 5/6.
    ExitService,
    /// Frequency ride-through category selection.
    /// UNVERIFIED against [S-20] Clause 5/7.
    FrequencyRideThrough,
    /// Voltage ride-through category selection.
    /// UNVERIFIED against [S-20] Clause 5/7.
    VoltageRideThrough,
}

impl SettingFunctionId {
    /// All catalog entries, in declaration order.
    pub const ALL: [SettingFunctionId; 9] = [
        SettingFunctionId::ConstantPf,
        SettingFunctionId::VoltVar,
        SettingFunctionId::VoltWatt,
        SettingFunctionId::FreqWatt,
        SettingFunctionId::RampRate,
        SettingFunctionId::EnterService,
        SettingFunctionId::ExitService,
        SettingFunctionId::FrequencyRideThrough,
        SettingFunctionId::VoltageRideThrough,
    ];

    /// The canonical snake_case name used in records and jurisdiction
    /// tables (the stored key spelling; the catalog enum is the closed
    /// vocabulary, never free text).
    pub const fn name(self) -> &'static str {
        match self {
            SettingFunctionId::ConstantPf => "constant_pf",
            SettingFunctionId::VoltVar => "volt_var",
            SettingFunctionId::VoltWatt => "volt_watt",
            SettingFunctionId::FreqWatt => "freq_watt",
            SettingFunctionId::RampRate => "ramp_rate",
            SettingFunctionId::EnterService => "enter_service",
            SettingFunctionId::ExitService => "exit_service",
            SettingFunctionId::FrequencyRideThrough => "frequency_ride_through",
            SettingFunctionId::VoltageRideThrough => "voltage_ride_through",
        }
    }

    /// Whether the function is a curve function (carries curve-point
    /// values). Declared, never inferred from the name.
    pub const fn is_curve(self) -> bool {
        matches!(
            self,
            SettingFunctionId::VoltVar
                | SettingFunctionId::VoltWatt
                | SettingFunctionId::FreqWatt
        )
    }
}

/// The ride-through category vocabulary (Category I / II / III).
///
/// **UNVERIFIED:** the category set is cited from [S-19][S-20] secondary
/// material; the implementation-time check against [S-20] must confirm the
/// category letters/count and which categories are mandatory per level.
/// A category is recorded DATA on a setting value — this module never
/// interprets what a category implies for device behaviour (that is the
/// device's and the interconnection record's business).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RideThroughCategory {
    /// Category I. UNVERIFIED against [S-20].
    CategoryI,
    /// Category II. UNVERIFIED against [S-20].
    CategoryII,
    /// Category III. UNVERIFIED against [S-20].
    CategoryIII,
}

impl RideThroughCategory {
    /// All categories, in declaration order.
    pub const ALL: [RideThroughCategory; 3] = [
        RideThroughCategory::CategoryI,
        RideThroughCategory::CategoryII,
        RideThroughCategory::CategoryIII,
    ];

    /// Canonical name (the stored spelling).
    pub const fn name(self) -> &'static str {
        match self {
            RideThroughCategory::CategoryI => "category_i",
            RideThroughCategory::CategoryII => "category_ii",
            RideThroughCategory::CategoryIII => "category_iii",
        }
    }
}

/// The declared axis quantity of a curve point (the x or y axis of one
/// curve point). **Units are declared, never inferred** — the repository
/// stores the declaration next to the number.
///
/// Voltage axes are per-unit on an explicit declared base (AGENTS.md §4:
/// voltage is kV at the nominal base, per-unit in solver representation);
/// frequency axes are per-unit on nominal frequency; power/response axes
/// are percent-of-nameplate or declared W/var magnitudes. Every variant
/// here is UNVERIFIED against the [S-20] curve definitions — the
/// implementation-time check confirms which quantities each curve's axes
/// actually carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CurveQuantity {
    /// Per-unit voltage (on the declared nominal base).
    PerUnitVoltage,
    /// Per-unit frequency (on the declared nominal frequency).
    PerUnitFrequency,
    /// Percent of nameplate active power.
    ActivePowerPercent,
    /// Percent of nameplate reactive power.
    ReactivePowerPercent,
    /// Percent of nameplate apparent power.
    ApparentPowerPercent,
}

impl CurveQuantity {
    /// Canonical name (the stored spelling).
    pub const fn name(self) -> &'static str {
        match self {
            CurveQuantity::PerUnitVoltage => "per_unit_voltage",
            CurveQuantity::PerUnitFrequency => "per_unit_frequency",
            CurveQuantity::ActivePowerPercent => "active_power_percent",
            CurveQuantity::ReactivePowerPercent => "reactive_power_percent",
            CurveQuantity::ApparentPowerPercent => "apparent_power_percent",
        }
    }
}

// --- Setting values ----------------------------------------------------------

/// One declared curve point: an x/y pair whose axis quantities are carried
/// alongside the numbers (declared, never inferred). `NaN`/infinite values
/// are construction errors.
#[derive(Debug, Clone, PartialEq)]
pub struct CurvePoint {
    /// The x-axis quantity declaration.
    pub x_quantity: CurveQuantity,
    /// The x-axis value (finite).
    pub x: f64,
    /// The y-axis quantity declaration.
    pub y_quantity: CurveQuantity,
    /// The y-axis value (finite).
    pub y: f64,
}

/// One declared setting value for one function. The repository stores
/// values AS DECLARED — it never interprets what a value means for device
/// behaviour (the scope statement above).
#[derive(Debug, Clone, PartialEq)]
pub enum SettingValue {
    /// A curve, as an ordered list of declared points. A curve value must
    /// carry at least one point (the checked constructor refuses an empty
    /// curve — a curve with no points is not a curve).
    Curve(Vec<CurvePoint>),
    /// A scalar quantity (ramp rate, delay, reference, limit — the units
    /// are declared in the setting entry's `quantity_note`).
    Scalar(f64),
    /// A ride-through category selection.
    Category(RideThroughCategory),
    /// An enable/disable declaration (whether the function is enabled at
    /// the device — recorded, never inferred).
    Enabled(bool),
}

/// A single value entry within one function's setting record: the value
/// plus its declared quantity note (e.g. "percent per second", "seconds").
/// The note is mandatory — a number without a declared quantity is not a
/// reviewed setting.
#[derive(Debug, Clone, PartialEq)]
pub struct SettingValueEntry {
    /// The declared quantity note for this value (non-empty). Examples the
    /// record register expects: `"percent_of_nameplate_per_second"`,
    /// `"seconds"`, `"per_unit_voltage"`. The spelling is recorded data —
    /// the module never parses it.
    pub quantity_note: String,
    /// The declared value.
    pub value: SettingValue,
}

/// Errors returned by settings-record construction. Every variant names
/// the offending field (the P3.04 style this crate's validators use).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// A numeric value was NaN or infinite.
    NonFiniteValue {
        /// The offending field.
        field: &'static str,
    },
    /// A required text field was empty (or whitespace-only).
    EmptyField {
        /// The offending field.
        field: &'static str,
    },
    /// A curve value carried no points.
    EmptyCurve {
        /// The offending function.
        function: &'static str,
    },
    /// A setting entry for a curve function carried no curve value.
    CurveFunctionWithoutCurve {
        /// The offending function.
        function: &'static str,
    },
    /// The same function appeared twice in one function set.
    DuplicateFunction {
        /// The duplicated function name.
        function: &'static str,
    },
    /// An entry quantity note was empty.
    EmptyQuantityNote,
    /// A function set declared no functions.
    EmptyFunctionSet,
    /// The jurisdiction id was already registered.
    DuplicateJurisdiction {
        /// The duplicated jurisdiction id.
        jurisdiction: String,
    },
    /// The device settings state already carries a record of this kind and
    /// the write path refused to overwrite it silently.
    SettingsStateConflict {
        /// The asset whose state conflicts.
        asset: String,
        /// The conflicting record kind.
        kind: &'static str,
    },
}

impl std::fmt::Display for SettingsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingsError::NonFiniteValue { field } => {
                write!(f, "settings value '{field}' must be finite")
            }
            SettingsError::EmptyField { field } => {
                write!(f, "settings field '{field}' must be non-empty")
            }
            SettingsError::EmptyCurve { function } => {
                write!(f, "curve value for function '{function}' must carry at least one point")
            }
            SettingsError::CurveFunctionWithoutCurve { function } => {
                write!(f, "setting for curve function '{function}' must carry a curve value")
            }
            SettingsError::DuplicateFunction { function } => {
                write!(f, "function '{function}' appears twice in one function set")
            }
            SettingsError::EmptyQuantityNote => {
                write!(f, "a setting value entry must declare its quantity note")
            }
            SettingsError::EmptyFunctionSet => {
                write!(f, "a function set must declare at least one function")
            }
            SettingsError::DuplicateJurisdiction { jurisdiction } => {
                write!(f, "jurisdiction '{jurisdiction}' is already registered")
            }
            SettingsError::SettingsStateConflict { asset, kind } => {
                write!(
                    f,
                    "device '{asset}' already carries {kind}; the state write path refuses silent overwrites"
                )
            }
        }
    }
}

impl std::error::Error for SettingsError {}

/// Validates one setting value: finite numbers where numbers appear; a
/// curve carries points; a curve function carries a curve.
fn validate_value(
    function: SettingFunctionId,
    entry: &SettingValueEntry,
) -> Result<(), SettingsError> {
    if entry.quantity_note.trim().is_empty() {
        return Err(SettingsError::EmptyQuantityNote);
    }
    match &entry.value {
        SettingValue::Curve(points) => {
            if points.is_empty() {
                return Err(SettingsError::EmptyCurve { function: function.name() });
            }
            for point in points {
                if !point.x.is_finite() || !point.y.is_finite() {
                    return Err(SettingsError::NonFiniteValue { field: function.name() });
                }
            }
        }
        SettingValue::Scalar(v) => {
            if !v.is_finite() {
                return Err(SettingsError::NonFiniteValue { field: function.name() });
            }
        }
        SettingValue::Category(_) | SettingValue::Enabled(_) => {}
    }
    Ok(())
}

/// One function's setting record as entered into the repository: the
/// function, its value entries, where the record came from, and the source
/// citation. A setting without a citation is not a reviewed record.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSetting {
    /// The function this record sets.
    pub function: SettingFunctionId,
    /// The declared value entries (at least one).
    pub entries: Vec<SettingValueEntry>,
    /// Where this record came from (the P14.14 authority vocabulary:
    /// certified baseline, jurisdiction default, aggregator schedule,
    /// utility override). Construction-time data, never inferred.
    pub source: SettingSource,
    /// The provenance flag from the §11.1 vocabulary (a jurisdiction
    /// default is `Configured`; an unknown-origin record is `Unknown` and
    /// the P3.02 narrowing rule applies wherever it feeds an envelope).
    pub provenance: Provenance,
    /// The source citation (non-empty): a document id, a standard clause
    /// reference, or an interconnection-record reference. Never raw wire
    /// text (the P1.08 rule).
    pub source_citation: String,
}

impl FunctionSetting {
    /// Constructs and validates one function setting. Every entry is
    /// validated; a curve function must carry a curve value; the citation
    /// must be non-empty.
    pub fn new(
        function: SettingFunctionId,
        entries: Vec<SettingValueEntry>,
        source: SettingSource,
        provenance: Provenance,
        source_citation: String,
    ) -> Result<Self, SettingsError> {
        if source_citation.trim().is_empty() {
            return Err(SettingsError::EmptyField { field: "source_citation" });
        }
        if entries.is_empty() {
            return Err(SettingsError::EmptyFunctionSet);
        }
        let mut has_curve = false;
        for entry in &entries {
            validate_value(function, entry)?;
            if matches!(entry.value, SettingValue::Curve(_)) {
                has_curve = true;
            }
        }
        if function.is_curve() && !has_curve {
            return Err(SettingsError::CurveFunctionWithoutCurve {
                function: function.name(),
            });
        }
        Ok(Self { function, entries, source, provenance, source_citation })
    }
}

// --- Sources and authorities -------------------------------------------------

/// Where a settings record came from. The P14.14 workflow
/// (`settings_management`) defines the precedence over these sources; this
/// module only records the declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SettingSource {
    /// The utility-owned certified baseline (the settings the device was
    /// certified/interconnected with — [S-19][S-20]; the P14.14 record
    /// carries the Clause 10 ownership statement). UNVERIFIED detail.
    CertifiedBaseline,
    /// A jurisdiction default table entry (e.g. the MA TSRG table [S-19]).
    JurisdictionDefault,
    /// An aggregator schedule (the DERControl-shaped schedules of P14.14).
    AggregatorSchedule,
    /// A utility override action (1547-2018 Clause 10 — UNVERIFIED
    /// transcription; the override is a precedence declaration, never a
    /// gate skip).
    UtilityOverride,
}

impl SettingSource {
    /// Canonical name (the stored spelling).
    pub const fn name(self) -> &'static str {
        match self {
            SettingSource::CertifiedBaseline => "certified_baseline",
            SettingSource::JurisdictionDefault => "jurisdiction_default",
            SettingSource::AggregatorSchedule => "aggregator_schedule",
            SettingSource::UtilityOverride => "utility_override",
        }
    }
}

// --- Jurisdiction default tables (the document store) ------------------------

/// A jurisdiction default table as a DOCUMENT: a named jurisdiction, its
/// source document citation, and the function defaults transcribed from it.
///
/// This is the [S-19] posture: "MA TSRG first" — but the repository ships
/// **no transcribed values**. Every value in a jurisdiction table is
/// ENTERED DATA with its citation, recorded at implementation time from
/// the primary document ([S-19] for Massachusetts; the named jurisdiction
/// document for every other table). Shipping pre-filled numbers transcribed
/// from a secondary source would be exactly the hallucinated-fact risk
/// AGENTS.md §3 rule 1 forbids; an empty registered table with its source
/// citation is the honest starting state.
///
/// **UNVERIFIED:** the MA TSRG table SHAPE (which functions the SRD
/// specifies, which categories it selects, the curve families) is cited
/// from [S-19] secondary material and from gap-analysis Track F2; the
/// implementation-time transcription pass verifies it against the primary
/// document.
#[derive(Debug, Clone, PartialEq)]
pub struct JurisdictionDefaultTable {
    /// The jurisdiction id (non-empty; e.g. `"ma-tsrg"` — a registry key,
    /// not a claim about the jurisdiction's legal status).
    pub jurisdiction_id: String,
    /// The source document citation (non-empty; e.g. the [S-19] document
    /// reference). Every transcribed entry cites this document.
    pub source_citation: String,
    /// The transcribed defaults, keyed by function. May be empty — a
    /// registered table with no transcribed entries is an honest state,
    /// never a claim that the jurisdiction specifies nothing.
    pub defaults: BTreeMap<SettingFunctionId, FunctionSetting>,
}

impl JurisdictionDefaultTable {
    /// Constructs and validates a jurisdiction table document.
    pub fn new(
        jurisdiction_id: String,
        source_citation: String,
        defaults: Vec<FunctionSetting>,
    ) -> Result<Self, SettingsError> {
        if jurisdiction_id.trim().is_empty() {
            return Err(SettingsError::EmptyField { field: "jurisdiction_id" });
        }
        if source_citation.trim().is_empty() {
            return Err(SettingsError::EmptyField { field: "source_citation" });
        }
        let mut map = BTreeMap::new();
        for setting in defaults {
            let function = setting.function;
            if map.insert(function, setting).is_some() {
                return Err(SettingsError::DuplicateFunction {
                    function: function.name(),
                });
            }
        }
        for setting in map.values() {
            if setting.source != SettingSource::JurisdictionDefault {
                return Err(SettingsError::EmptyField {
                    field: "defaults.source (a jurisdiction-table entry must carry SettingSource::JurisdictionDefault)",
                });
            }
        }
        Ok(Self { jurisdiction_id, source_citation, defaults: map })
    }
}

/// The settings document store: the registered jurisdiction tables keyed
/// by jurisdiction id (BTreeMap determinism, the house rule).
///
/// The store is INSERT-ONLY for a given jurisdiction id: registering the
/// same id twice is a named error (a revised table is a new registration
/// under a versioned id, decided at the composition layer — this module
/// records, it does not silently overwrite).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SettingsDocumentStore {
    tables: BTreeMap<String, JurisdictionDefaultTable>,
}

impl SettingsDocumentStore {
    /// An empty store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one jurisdiction table. A duplicate jurisdiction id is a
    /// named error.
    pub fn register(
        &mut self,
        table: JurisdictionDefaultTable,
    ) -> Result<(), SettingsError> {
        if self.tables.contains_key(&table.jurisdiction_id) {
            return Err(SettingsError::DuplicateJurisdiction {
                jurisdiction: table.jurisdiction_id,
            });
        }
        self.tables.insert(table.jurisdiction_id.clone(), table);
        Ok(())
    }

    /// Looks up a jurisdiction table by id.
    pub fn table(&self, jurisdiction_id: &str) -> Option<&JurisdictionDefaultTable> {
        self.tables.get(jurisdiction_id)
    }

    /// The registered jurisdiction ids (sorted — BTreeMap order).
    pub fn jurisdiction_ids(&self) -> Vec<&str> {
        self.tables.keys().map(String::as_str).collect()
    }

    /// The default setting for one function under one jurisdiction, when
    /// the table is registered and the function is transcribed. A missing
    /// entry is `None` — never a fabricated default.
    pub fn default_setting(
        &self,
        jurisdiction_id: &str,
        function: SettingFunctionId,
    ) -> Option<&FunctionSetting> {
        self.tables
            .get(jurisdiction_id)
            .and_then(|t| t.defaults.get(&function))
    }
}

// --- Per-device settings state -----------------------------------------------

/// The certified baseline record for one device: the utility-owned
/// function settings the device carries from interconnection ([S-19][S-20];
/// the ownership statement is enforced by the P14.14 workflow — this
/// record stores the declaration).
#[derive(Debug, Clone, PartialEq)]
pub struct CertifiedBaselineRecord {
    /// The device (asset) the baseline belongs to.
    pub asset: AssetId,
    /// The baseline function settings (at least one).
    pub functions: Vec<FunctionSetting>,
    /// The interconnection/certification document citation (non-empty).
    pub citation: String,
}

/// One aggregator schedule record in a device's state: the DERControl-
/// shaped schedule reference (the P14.14 workflow produces the lifecycle
/// command; this is the state record a device carries once a change is
/// applied — in simulation, only).
#[derive(Debug, Clone, PartialEq)]
pub struct AggregatorScheduleRecord {
    /// The device (asset) the schedule targets.
    pub asset: AssetId,
    /// The command id of the lifecycle command that carried the change
    /// (the schedule reaches device state ONLY through the lifecycle —
    /// the P14.14 rule; the id is the audit anchor).
    pub command_id: String,
    /// The scheduled function settings.
    pub functions: Vec<FunctionSetting>,
}

/// One utility override record in a device's state (1547-2018 Clause 10 —
/// UNVERIFIED transcription; the override enters the device's state only
/// through the lifecycle like every other change).
#[derive(Debug, Clone, PartialEq)]
pub struct UtilityOverrideRecord {
    /// The device (asset) the override targets.
    pub asset: AssetId,
    /// The command id of the lifecycle command that carried the override.
    pub command_id: String,
    /// The overridden function settings.
    pub functions: Vec<FunctionSetting>,
}

/// The per-device settings state: a document-store record of what a device
/// is DECLARED to carry. It is NOT a device-read mirror (no device was
/// read — nothing here is evidence about a real device) and NOT a claim
/// that the device accepts any function (the scope statement above).
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceSettingsState {
    /// The device (asset) this state belongs to.
    pub asset: AssetId,
    /// The device's declared function support: which functions the device
    /// is declared to accept at all (entered from an interconnection record
    /// or device profile). A function NOT in this set is refused by the
    /// P14.14 workflow's construction gate — the "never a claim that any
    /// device accepts any function" rule [S-19][S-20], enforced here as
    /// declared data.
    pub declared_function_support: Vec<SettingFunctionId>,
    /// The certified baseline, when recorded.
    pub baseline: Option<CertifiedBaselineRecord>,
    /// The aggregator schedules recorded for the device (BTreeMap
    /// discipline: kept in command-id order by the workflow's writes).
    pub aggregator_schedules: Vec<AggregatorScheduleRecord>,
    /// The utility overrides recorded for the device.
    pub utility_overrides: Vec<UtilityOverrideRecord>,
}

impl DeviceSettingsState {
    /// A new, empty state for one device with its declared function
    /// support. The support set may be empty — a device with no declared
    /// function support accepts nothing, which is the honest default (the
    /// workflow refuses every settings change until support is declared).
    pub fn new(asset: AssetId, declared_function_support: Vec<SettingFunctionId>) -> Self {
        Self {
            asset,
            declared_function_support,
            baseline: None,
            aggregator_schedules: Vec::new(),
            utility_overrides: Vec::new(),
        }
    }

    /// Whether the device is DECLARED to support one function. Declared
    /// never inferred [S-19][S-20].
    pub fn declares_support(&self, function: SettingFunctionId) -> bool {
        self.declared_function_support.contains(&function)
    }

    /// Records the certified baseline. A baseline already present is a
    /// named conflict — replacing it is a utility action through the P14.14
    /// workflow (a lifecycle command), never a silent state overwrite.
    pub fn record_baseline(
        &mut self,
        record: CertifiedBaselineRecord,
    ) -> Result<(), SettingsError> {
        if record.asset != self.asset {
            return Err(SettingsError::SettingsStateConflict {
                asset: format!("{:?}", record.asset),
                kind: "a baseline for a different asset",
            });
        }
        if self.baseline.is_some() {
            return Err(SettingsError::SettingsStateConflict {
                asset: format!("{:?}", self.asset),
                kind: "a certified baseline",
            });
        }
        self.baseline = Some(record);
        Ok(())
    }

    /// Appends one aggregator schedule record (the workflow is the only
    /// writer; duplicate command ids are refused — every record is one
    /// lifecycle command's outcome).
    pub fn record_aggregator_schedule(
        &mut self,
        record: AggregatorScheduleRecord,
    ) -> Result<(), SettingsError> {
        if record.asset != self.asset {
            return Err(SettingsError::SettingsStateConflict {
                asset: format!("{:?}", record.asset),
                kind: "a schedule for a different asset",
            });
        }
        if self
            .aggregator_schedules
            .iter()
            .any(|r| r.command_id == record.command_id)
        {
            return Err(SettingsError::SettingsStateConflict {
                asset: format!("{:?}", self.asset),
                kind: "a schedule with this command id",
            });
        }
        self.aggregator_schedules.push(record);
        self.aggregator_schedules.sort_by(|a, b| a.command_id.cmp(&b.command_id));
        Ok(())
    }

    /// Appends one utility override record (same discipline).
    pub fn record_utility_override(
        &mut self,
        record: UtilityOverrideRecord,
    ) -> Result<(), SettingsError> {
        if record.asset != self.asset {
            return Err(SettingsError::SettingsStateConflict {
                asset: format!("{:?}", record.asset),
                kind: "an override for a different asset",
            });
        }
        if self.utility_overrides.iter().any(|r| r.command_id == record.command_id) {
            return Err(SettingsError::SettingsStateConflict {
                asset: format!("{:?}", self.asset),
                kind: "an override with this command id",
            });
        }
        self.utility_overrides.push(record);
        self.utility_overrides.sort_by(|a, b| a.command_id.cmp(&b.command_id));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(sample: &str) -> AssetId {
        AssetId::new(flexcore_contracts::ids::FlexUuid::parse(sample).expect("valid uuid"))
    }

    const ASSET_1: &str = "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b";
    const ASSET_2: &str = "0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8c";

    fn curve_entry(points: &[(f64, f64)]) -> SettingValueEntry {
        SettingValueEntry {
            quantity_note: "per_unit_voltage_to_reactive_power_percent".to_string(),
            value: SettingValue::Curve(
                points
                    .iter()
                    .map(|&(x, y)| CurvePoint {
                        x_quantity: CurveQuantity::PerUnitVoltage,
                        x,
                        y_quantity: CurveQuantity::ReactivePowerPercent,
                        y,
                    })
                    .collect(),
            ),
        }
    }

    fn scalar_entry(v: f64) -> SettingValueEntry {
        SettingValueEntry {
            quantity_note: "percent_of_nameplate_per_second".to_string(),
            value: SettingValue::Scalar(v),
        }
    }

    fn volt_var_setting() -> FunctionSetting {
        FunctionSetting::new(
            SettingFunctionId::VoltVar,
            vec![curve_entry(&[(0.9, 44.0), (1.0, 0.0), (1.1, -44.0)])],
            SettingSource::JurisdictionDefault,
            Provenance::Unknown,
            "UNVERIFIED transcription; verify against the primary document".to_string(),
        )
        .expect("valid setting")
    }

    #[test]
    fn catalog_is_closed_and_named() {
        assert_eq!(SettingFunctionId::ALL.len(), 9);
        for function in SettingFunctionId::ALL {
            assert!(!function.name().is_empty());
        }
        // Curve-function declarations are explicit data, not inferred.
        assert!(SettingFunctionId::VoltVar.is_curve());
        assert!(!SettingFunctionId::RampRate.is_curve());
    }

    #[test]
    fn curve_function_requires_a_curve_value() {
        // A curve function with only a scalar entry is a named refusal.
        assert_eq!(
            FunctionSetting::new(
                SettingFunctionId::VoltVar,
                vec![scalar_entry(1.0)],
                SettingSource::JurisdictionDefault,
                Provenance::Unknown,
                "citation".to_string(),
            ),
            Err(SettingsError::CurveFunctionWithoutCurve { function: "volt_var" })
        );
        // An empty curve is a named refusal.
        assert_eq!(
            FunctionSetting::new(
                SettingFunctionId::VoltWatt,
                vec![curve_entry(&[])],
                SettingSource::JurisdictionDefault,
                Provenance::Unknown,
                "citation".to_string(),
            ),
            Err(SettingsError::EmptyCurve { function: "volt_watt" })
        );
        // NaN is a named refusal.
        assert_eq!(
            FunctionSetting::new(
                SettingFunctionId::VoltWatt,
                vec![curve_entry(&[(f64::NAN, 1.0)])],
                SettingSource::JurisdictionDefault,
                Provenance::Unknown,
                "citation".to_string(),
            ),
            Err(SettingsError::NonFiniteValue { field: "volt_watt" })
        );
    }

    #[test]
    fn store_is_insert_only_and_returns_no_fabricated_defaults() {
        let mut store = SettingsDocumentStore::new();
        let table = JurisdictionDefaultTable::new(
            "ma-tsrg".to_string(),
            "[S-19] MA TSRG Inverter SRD (UNVERIFIED transcription pending)".to_string(),
            vec![volt_var_setting()],
        )
        .expect("valid table");
        store.register(table.clone()).expect("first registration");
        // A duplicate jurisdiction id is a named error.
        assert_eq!(
            store.register(table),
            Err(SettingsError::DuplicateJurisdiction {
                jurisdiction: "ma-tsrg".to_string(),
            })
        );
        // A registered + transcribed function resolves; an unregistered
        // jurisdiction and an untranscribed function resolve to None —
        // never a fabricated default.
        assert!(store
            .default_setting("ma-tsrg", SettingFunctionId::VoltVar)
            .is_some());
        assert!(store
            .default_setting("ma-tsrg", SettingFunctionId::FreqWatt)
            .is_none());
        assert!(store
            .default_setting("unregistered-jurisdiction", SettingFunctionId::VoltVar)
            .is_none());
        assert_eq!(store.jurisdiction_ids(), vec!["ma-tsrg"]);
    }

    #[test]
    fn jurisdiction_table_entries_must_declare_their_source() {
        let wrong_source = FunctionSetting::new(
            SettingFunctionId::VoltWatt,
            vec![curve_entry(&[(0.2, 100.0), (1.0, 0.0)])],
            SettingSource::AggregatorSchedule,
            Provenance::Unknown,
            "citation".to_string(),
        )
        .expect("valid setting");
        assert!(JurisdictionDefaultTable::new(
            "some-jurisdiction".to_string(),
            "citation".to_string(),
            vec![wrong_source],
        )
        .is_err(), "a jurisdiction-table entry must carry the jurisdiction source");
    }

    #[test]
    fn device_state_refuses_undeclared_functions_and_conflicting_writes() {
        let mut state = DeviceSettingsState::new(asset(ASSET_1), vec![SettingFunctionId::VoltVar]);
        assert!(state.declares_support(SettingFunctionId::VoltVar));
        assert!(
            !state.declares_support(SettingFunctionId::FreqWatt),
            "support is declared, never assumed [S-19][S-20]"
        );
        // Empty support accepts nothing.
        let mut empty = DeviceSettingsState::new(asset(ASSET_2), Vec::new());
        assert!(!empty.declares_support(SettingFunctionId::ConstantPf));
        // One baseline only; a second is a named conflict.
        let baseline = CertifiedBaselineRecord {
            asset: asset(ASSET_1),
            functions: vec![volt_var_setting()],
            citation: "interconnection record (declared data)".to_string(),
        };
        state.record_baseline(baseline).expect("first baseline");
        assert_eq!(
            state.record_baseline(CertifiedBaselineRecord {
                asset: asset(ASSET_1),
                functions: vec![volt_var_setting()],
                citation: "second".to_string(),
            }),
            Err(SettingsError::SettingsStateConflict {
                asset: format!("{:?}", asset(ASSET_1)),
                kind: "a certified baseline",
            })
        );
        // Duplicate command ids are refused.
        let schedule = AggregatorScheduleRecord {
            asset: asset(ASSET_1),
            command_id: "cmd-1".to_string(),
            functions: vec![volt_var_setting()],
        };
        state.record_aggregator_schedule(schedule).expect("first schedule");
        assert_eq!(
            state.record_aggregator_schedule(AggregatorScheduleRecord {
                asset: asset(ASSET_1),
                command_id: "cmd-1".to_string(),
                functions: vec![volt_var_setting()],
            }),
            Err(SettingsError::SettingsStateConflict {
                asset: format!("{:?}", asset(ASSET_1)),
                kind: "a schedule with this command id",
            })
        );
        // Schedules are kept in command-id order (deterministic).
        state
            .record_aggregator_schedule(AggregatorScheduleRecord {
                asset: asset(ASSET_1),
                command_id: "cmd-0".to_string(),
                functions: vec![volt_var_setting()],
            })
            .expect("second schedule");
        let ids: Vec<&str> = state
            .aggregator_schedules
            .iter()
            .map(|r| r.command_id.as_str())
            .collect();
        assert_eq!(ids, vec!["cmd-0", "cmd-1"]);
    }
}
