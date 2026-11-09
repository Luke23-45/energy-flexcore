// P2.11 — The `TelemetrySource` port: the real telemetry source contract.
//
// History, recorded honestly: P1.07 defined a **shape-only** seam here (a
// minimal `subscribe`/`decode` pair with a `DecodedSample`) so the Phase 2
// adapter could not invent an application-coupled signature later. Phase 2
// (this change, ledger P2.11) replaces that shape with the real contract per
// report §9.1 and the P2.04 canonical telemetry model. Nothing consumed the
// P1.07 shape (no Stage 1 use case touches telemetry; verified by grep at
// change time), so the superseded `DecodedSample`/`TelemetryScope` types are
// gone — the transition is recorded in the crate README and in
// `ledger/phase-02-asset-telemetry/11_telemetry_source_contract.md`.
//
// The contract, precisely:
//
// - [`TelemetryQuery`] names what a consumer wants: a **tenant-scoped asset
//   set** (P0.16 rule 7), a `from`/`until` `event_time` range, and the
//   quantities of interest. Construction **requires a domain `TenantContext`**
//   — it is the only constructor argument carrying the tenant, so a query
//   without a tenant context is a **compile error**, not a runtime check
//   (P0.16 rule 5, same structural pattern as the repository ports).
// - [`NormalizedTelemetry`] is the **canonical record** of P2.04 §3: one
//   quantity of one asset at one `event_time` (P2.04 D-T2), in the canonical
//   unit newtypes of `flexcore-contracts` (P0.07), with exactly one canonical
//   quality state (P0.08), mandatory source identity
//   (`source_id` + `source_sequence`, P0.09 §1; P2.04 §8), the distinct
//   `event_time`/`ingest_time` pair (P0.09), and a recorded
//   [`ClockQuality`] (P2.04 D-T5). Construction is a [`Result`]: a record
//   missing anything canonical (empty `source_id`, a measured record without
//   a value, a quantity/value mismatch, `ingest_time` before `event_time`,
//   a misused `flexcore:` namespace) is **rejected, never partially built**.
//   A provider that cannot produce a complete canonical record fails
//   decoding — it does not emit a partial record (P2.11 step 3).
// - [`TelemetrySource`] is the port: `subscribe` a query (opaque handle),
//   `decode` one opaque raw frame into the **complete set** of canonical
//   records that frame carries (a multi-quantity sample is a set of records
//   sharing `source_id`, `source_sequence`, and `event_time`, P2.04 D-T2),
//   plus `source_id()` and `health()` for the provider, and
//   `provider_declaration()` — the structural form of the **provider rule**
//   (P2.11 step 4): every provider declares its cadence, its quality-state
//   mapping onto the six canonical states (P0.08 §3), and its clock-quality
//   behaviour.
// - Decode failure is `Err` — never an `Ok` carrying a partial or empty
//   record set, and never a silently degraded record (the no-silent-skip
//   rule of P2.07 applies at the port boundary too).
//
// Sync choice (house rule, crate README "Sync traits"): the port is
// deliberately synchronous. `subscribe` returns an opaque handle rather
// than a stream — delivery, buffering, and ordering are the adapter's
// concern, bridged by the adapter (blocking hand-off, worker thread, or an
// async wrapper around this sync trait). Keeping the port sync keeps the
// application layer dependent on nothing runtime-shaped.
//
// Source-identity namespaces (P2.04 D-T6): the `flexcore:` prefix is
// reserved for platform-derived records; derived records use
// `flexcore:derived:<detector>`. A [`TelemetrySource`] provider is a
// measured source and must never declare or emit a `flexcore:`-namespaced
// `source_id`. Only the platform's gap/derivation detectors (P2.08) may.
//
// Phase 2 provider set (P2.11 step 5): **exactly one** — the simulator
// telemetry source (P2.14). Real adapters (MQTT, Modbus, OpenEMS) are later
// phases. No implementation ships in this crate; the contract tests below
// use a minimal fake.

use flexcore_contracts::quality::TelemetryQuality;
use flexcore_contracts::time_model::{
    validate_ingest_after_event, ClockQuality, EventTime, IngestTime, TimeModelError,
};
use flexcore_contracts::units::{
    ActivePowerMW, EnergyMWh, ReactivePowerMVAr, ReactiveSignConvention, StateOfCharge,
};
use flexcore_domain::ids::{AssetId, TenantId};
use flexcore_domain::tenant_context::TenantContext;
use std::time::Duration;

/// The reserved platform source-namespace prefix (P2.04 D-T6). Measured
/// provider records must never use it.
const RESERVED_SOURCE_NAMESPACE: &str = "flexcore:";

/// The reserved namespace for **platform-derived records** (gap markers and
/// explicitly configured derivations, P2.04 D-T6; P2.08).
const DERIVED_SOURCE_NAMESPACE: &str = "flexcore:derived:";

// --- Quantities and values --------------------------------------------------

/// The frozen Phase 2 quantity vocabulary (P2.04 §4; extension is an
/// explicit migration, never a silent widening). Every variant documents its
/// canonical unit (P0.07 §1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TelemetryQuantity {
    /// Active power — canonical unit **MW**; P > 0 = export at the point of
    /// connection, P < 0 = import (P0.07 §2).
    ActivePower,
    /// Reactive power — canonical unit **MVAr**; the sign is interpreted
    /// only under the declared convention carried on the record (P0.07 §3;
    /// P2.04 D-T4) — never inferred from sign alone.
    ReactivePower,
    /// State of charge — a **fraction in [0,1]** (dimensionless unit `1`;
    /// P0.07 §4). Percent is never canonical.
    Soc,
    /// Interval energy — canonical unit **MWh** (P0.07 §1). SOC is not
    /// energy.
    Energy,
    /// Asset availability — boolean (dimensionless `1`).
    Availability,
    /// Operating mode — source-declared text state, carried verbatim
    /// (P2.04 §3 field 12; no Phase 2 canonical mode vocabulary).
    OperatingMode,
    /// Health — source-declared text state, carried verbatim (P2.04 §3
    /// field 12).
    Health,
    /// EVSE connection state — boolean (dimensionless `1`).
    ConnectionState,
    /// Flexible-load baseline reference — canonical unit **MW**; present
    /// only where a baseline is explicitly configured (P2.04 §4; D1/D2 stay
    /// open — no baseline method is assumed).
    BaselineReference,
}

/// The value of one canonical record, in the `flexcore-contracts` canonical
/// unit newtypes (P0.07). A bare number or bare string never appears: the
/// unit travels in the type (or, for reactive power, together with its
/// declared sign convention).
#[derive(Debug, Clone, PartialEq)]
pub enum QuantityValue {
    /// An active-power value in MW under the frozen sign rule
    /// (P > 0 = export). Used by `ActivePower` and `BaselineReference`.
    Power(ActivePowerMW),
    /// A reactive-power value in MVAr **together with** its declared sign
    /// convention (P0.07 §3) — the value is uninterpretable without it.
    Reactive {
        /// The reactive-power magnitude in MVAr.
        power: ReactivePowerMVAr,
        /// The declared convention governing the sign of `power`.
        convention: ReactiveSignConvention,
    },
    /// A state-of-charge fraction in [0,1] (P0.07 §4).
    Fraction(StateOfCharge),
    /// An energy value in MWh.
    Energy(EnergyMWh),
    /// A boolean state (availability, EVSE connection state).
    Flag(bool),
    /// A source-declared text state (operating mode, health), carried
    /// verbatim and non-empty (P2.04 §3 field 12).
    State(String),
}

impl TelemetryQuantity {
    /// Whether this quantity accepts the given value kind (the
    /// quantity/value pairing rule of P2.04 §3–§4: the value's kind and
    /// canonical unit are determined by the quantity).
    pub fn accepts(self, value: &QuantityValue) -> bool {
        use TelemetryQuantity as Q;
        match (self, value) {
            (Q::ActivePower | Q::BaselineReference, QuantityValue::Power(_)) => true,
            (Q::ReactivePower, QuantityValue::Reactive { .. }) => true,
            (Q::Soc, QuantityValue::Fraction(_)) => true,
            (Q::Energy, QuantityValue::Energy(_)) => true,
            (Q::Availability | Q::ConnectionState, QuantityValue::Flag(_)) => true,
            (Q::OperatingMode | Q::Health, QuantityValue::State(_)) => true,
            _ => false,
        }
    }
}

// --- The query --------------------------------------------------------------

/// Errors returned when building a [`TelemetryQuery`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryQueryError {
    /// The asset set is empty — a query names at least one asset.
    EmptyAssetSet,
    /// The quantity set is empty — a query names the quantities it wants;
    /// there is no implicit "all quantities" wildcard.
    EmptyQuantitySet,
    /// The time range is inverted (`from` after `until`).
    InvertedTimeRange,
}

impl std::fmt::Display for TelemetryQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TelemetryQueryError::EmptyAssetSet => write!(f, "a telemetry query must name at least one asset"),
            TelemetryQueryError::EmptyQuantitySet => {
                write!(f, "a telemetry query must name at least one quantity (no implicit wildcard)")
            }
            TelemetryQueryError::InvertedTimeRange => {
                write!(f, "the query time range is inverted (from after until)")
            }
        }
    }
}

impl std::error::Error for TelemetryQueryError {}

/// What a consumer wants from a [`TelemetrySource`]: a **tenant-scoped
/// asset set**, a `from <= until` range over `event_time`, and the named
/// quantities.
///
/// Tenant scoping is structural (P0.16 rules 1 and 5): the **only**
/// constructor, [`TelemetryQuery::new`], takes the domain `TenantContext`
/// and derives the tenant from it. There is no constructor without one, no
/// `From<TenantId>`, and no public field — a query without a tenant context
/// is a **compile error**, not a runtime check that can be forgotten.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryQuery {
    tenant_id: TenantId,
    assets: Vec<AssetId>,
    from: EventTime,
    until: EventTime,
    quantities: Vec<TelemetryQuantity>,
}

impl TelemetryQuery {
    /// Builds a query scoped to the tenant of `tenant`. The tenant ID is
    /// taken **only** from the context — never from a parameter, so the
    /// query cannot disagree with the authenticated scope it came from.
    ///
    /// Rejects an empty asset set, an empty quantity set, and an inverted
    /// time range.
    pub fn new(
        tenant: &TenantContext,
        assets: Vec<AssetId>,
        from: EventTime,
        until: EventTime,
        quantities: Vec<TelemetryQuantity>,
    ) -> Result<Self, TelemetryQueryError> {
        if assets.is_empty() {
            return Err(TelemetryQueryError::EmptyAssetSet);
        }
        if quantities.is_empty() {
            return Err(TelemetryQueryError::EmptyQuantitySet);
        }
        if from > until {
            return Err(TelemetryQueryError::InvertedTimeRange);
        }
        Ok(Self {
            tenant_id: tenant.tenant_id(),
            assets,
            from,
            until,
            quantities,
        })
    }

    /// The tenant the query is scoped to (derived from the `TenantContext`).
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The queried assets (non-empty).
    pub fn assets(&self) -> &[AssetId] {
        &self.assets
    }

    /// Inclusive range start over `event_time`.
    pub fn from(&self) -> EventTime {
        self.from
    }

    /// Inclusive range end over `event_time`.
    pub fn until(&self) -> EventTime {
        self.until
    }

    /// The queried quantities (non-empty; no implicit wildcard).
    pub fn quantities(&self) -> &[TelemetryQuantity] {
        &self.quantities
    }
}

// --- The canonical record ---------------------------------------------------

/// Errors returned when building a [`NormalizedTelemetry`] record. Every
/// variant is a **rejection of the whole record** — there is no partial
/// record (P2.11 step 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedTelemetryError {
    /// `source_id` is empty; source identity is mandatory (P2.04 §8).
    EmptySourceId,
    /// The source ID used the reserved `flexcore:` platform namespace
    /// outside the `flexcore:derived:` sub-namespace — measured records must
    /// never use it (P2.04 D-T6).
    ReservedSourceNamespace,
    /// A `flexcore:derived:` record claimed a non-derived quality state
    /// (only `MISSING` gap markers and explicitly configured `ESTIMATED`
    /// records are platform-derived, P2.04 D-T6).
    DerivedSourceNamespaceMisused,
    /// A `MISSING` record did not come from the derived namespace —
    /// `MISSING` markers are platform-derived records, not provider output
    /// (P2.04 D-T6; P2.08 step 4).
    MissingRequiresDerivedNamespace,
    /// A `MISSING` record carried a value — a gap marker has no value
    /// (P2.04 §3 value-field totality rule).
    ValueOnMissingRecord,
    /// A measured record carried no value — a provider that cannot produce
    /// the canonical value fails decoding instead of emitting a partial
    /// record (P2.11 step 3).
    MissingValue,
    /// The value's kind does not match the quantity (P2.04 §3–§4 pairing).
    ValueQuantityMismatch {
        /// The declared quantity.
        quantity: TelemetryQuantity,
    },
    /// A text state value was empty or whitespace-only (P2.04 §3 field 12:
    /// carried verbatim and non-empty).
    EmptyStateText,
    /// `ingest_time` precedes `event_time` — a value cannot be received
    /// before it occurred (P0.09).
    TimeOrdering(TimeModelError),
}

impl std::fmt::Display for NormalizedTelemetryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NormalizedTelemetryError::EmptySourceId => {
                write!(f, "source_id is mandatory and must be non-empty")
            }
            NormalizedTelemetryError::ReservedSourceNamespace => {
                write!(f, "the 'flexcore:' source namespace is reserved for platform-derived records; measured sources must not use it")
            }
            NormalizedTelemetryError::DerivedSourceNamespaceMisused => {
                write!(f, "a 'flexcore:derived:' record must carry quality MISSING or ESTIMATED")
            }
            NormalizedTelemetryError::MissingRequiresDerivedNamespace => {
                write!(f, "a MISSING record is a platform-derived gap marker and must use the 'flexcore:derived:' namespace")
            }
            NormalizedTelemetryError::ValueOnMissingRecord => {
                write!(f, "a MISSING record is a gap marker and must not carry a value")
            }
            NormalizedTelemetryError::MissingValue => {
                write!(f, "a measured record must carry a value; a provider that cannot produce one fails decoding instead of emitting a partial record")
            }
            NormalizedTelemetryError::ValueQuantityMismatch { quantity } => {
                write!(f, "the value kind does not match the quantity {quantity:?}")
            }
            NormalizedTelemetryError::EmptyStateText => {
                write!(f, "a text state value must be non-empty")
            }
            NormalizedTelemetryError::TimeOrdering(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for NormalizedTelemetryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NormalizedTelemetryError::TimeOrdering(e) => Some(e),
            _ => None,
        }
    }
}

/// The construction inputs of a [`NormalizedTelemetry`] record. All fields
/// are mandatory; "missing" canonical content (empty source identity, no
/// value on a measured record, …) is rejected by
/// [`NormalizedTelemetry::new`], never stored.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedTelemetryParts {
    /// The asset's tenant (P2.04 §3 field 2; P0.16 rule 1). Taken from the
    /// authenticated source context, not from the payload.
    pub tenant_id: TenantId,
    /// The measured asset (P2.04 §3 field 3).
    pub asset_id: AssetId,
    /// Which quantity this record carries (P2.04 §3 field 4; D-T2 — one
    /// quantity per record).
    pub quantity: TelemetryQuantity,
    /// The canonical value, or `None` exactly for a `MISSING` gap marker.
    pub value: Option<QuantityValue>,
    /// When the measurement occurred (P0.09).
    pub event_time: EventTime,
    /// When FlexCore received it (P0.09). Distinct type from `event_time`.
    pub ingest_time: IngestTime,
    /// Source-assigned ordering identifier, monotonically increasing per
    /// (asset, source) (P0.09 §1; P2.04 §8).
    pub source_sequence: u64,
    /// Source identity (P0.08 §7). Measured sources must not use the
    /// reserved `flexcore:` namespace (P2.04 D-T6).
    pub source_id: String,
    /// Exactly one of the six canonical quality states (P0.08).
    pub quality: TelemetryQuality,
    /// The original source quality code retained as diagnostic metadata,
    /// where the source reported a richer state (P0.08 §3; P2.04 §3
    /// field 10). Never a seventh canonical state.
    pub source_quality_code: Option<String>,
    /// Recorded clock-synchronisation status, never assumed (P0.09 §4;
    /// P2.04 D-T5).
    pub clock_quality: ClockQuality,
}

/// The canonical telemetry record (P2.04 §3) — the decode output of a
/// [`TelemetrySource`]. One record = one quantity of one asset at one
/// `event_time` (P2.04 D-T2). Constructed **only** through
/// [`NormalizedTelemetry::new`], which rejects any record that is not
/// complete and canonical — a provider emits a whole record or a decode
/// failure, never a partial record.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalizedTelemetry {
    tenant_id: TenantId,
    asset_id: AssetId,
    quantity: TelemetryQuantity,
    value: Option<QuantityValue>,
    event_time: EventTime,
    ingest_time: IngestTime,
    source_sequence: u64,
    source_id: String,
    quality: TelemetryQuality,
    source_quality_code: Option<String>,
    clock_quality: ClockQuality,
}

impl NormalizedTelemetry {
    /// Validates and builds the record. The numeric unit newtypes
    /// (`ActivePowerMW`, `StateOfCharge`, …) already reject NaN, infinity,
    /// and out-of-range values at their own constructors, so a canonical
    /// record cannot hold a non-finite or out-of-range number by
    /// construction (P0.07; P2.04 §3 field 11).
    pub fn new(parts: NormalizedTelemetryParts) -> Result<Self, NormalizedTelemetryError> {
        let NormalizedTelemetryParts {
            tenant_id,
            asset_id,
            quantity,
            value,
            event_time,
            ingest_time,
            source_sequence,
            source_id,
            quality,
            source_quality_code,
            clock_quality,
        } = parts;

        // Source identity (P2.04 §8): mandatory, non-empty, namespace rule.
        if source_id.is_empty() {
            return Err(NormalizedTelemetryError::EmptySourceId);
        }
        let derived = source_id.starts_with(DERIVED_SOURCE_NAMESPACE);
        if source_id.starts_with(RESERVED_SOURCE_NAMESPACE) && !derived {
            return Err(NormalizedTelemetryError::ReservedSourceNamespace);
        }
        if derived && !matches!(quality, TelemetryQuality::Missing | TelemetryQuality::Estimated) {
            return Err(NormalizedTelemetryError::DerivedSourceNamespaceMisused);
        }
        if quality == TelemetryQuality::Missing && !derived {
            return Err(NormalizedTelemetryError::MissingRequiresDerivedNamespace);
        }

        // Value totality (P2.04 §3): a value on a MISSING record is a gap
        // marker violation; no value on a measured record is a partial
        // record — rejected, never emitted.
        match (&value, quality) {
            (Some(_), TelemetryQuality::Missing) => {
                return Err(NormalizedTelemetryError::ValueOnMissingRecord);
            }
            (None, TelemetryQuality::Missing) => {}
            (None, _) => return Err(NormalizedTelemetryError::MissingValue),
            (Some(v), _) => {
                if !quantity.accepts(v) {
                    return Err(NormalizedTelemetryError::ValueQuantityMismatch { quantity });
                }
                if let QuantityValue::State(text) = v {
                    if text.trim().is_empty() {
                        return Err(NormalizedTelemetryError::EmptyStateText);
                    }
                }
            }
        }

        // Time ordering (P0.09): a value cannot be received before it
        // occurred.
        validate_ingest_after_event(event_time, ingest_time)
            .map_err(NormalizedTelemetryError::TimeOrdering)?;

        Ok(Self {
            tenant_id,
            asset_id,
            quantity,
            value,
            event_time,
            ingest_time,
            source_sequence,
            source_id,
            quality,
            source_quality_code,
            clock_quality,
        })
    }

    /// The record's tenant.
    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    /// The measured asset.
    pub fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    /// The quantity this record carries.
    pub fn quantity(&self) -> TelemetryQuantity {
        self.quantity
    }

    /// The canonical value, or `None` exactly for a `MISSING` gap marker.
    pub fn value(&self) -> Option<&QuantityValue> {
        self.value.as_ref()
    }

    /// When the measurement occurred.
    pub fn event_time(&self) -> EventTime {
        self.event_time
    }

    /// When FlexCore received it (excluded from replay comparison,
    /// P2.13/P2.24 determinism scope).
    pub fn ingest_time(&self) -> IngestTime {
        self.ingest_time
    }

    /// The source-assigned ordering identifier.
    pub fn source_sequence(&self) -> u64 {
        self.source_sequence
    }

    /// The source identity.
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    /// The canonical quality state (exactly one of the six, P0.08).
    pub fn quality(&self) -> TelemetryQuality {
        self.quality
    }

    /// The retained original source quality code, where present.
    pub fn source_quality_code(&self) -> Option<&str> {
        self.source_quality_code.as_deref()
    }

    /// The recorded clock-synchronisation status.
    pub fn clock_quality(&self) -> ClockQuality {
        self.clock_quality
    }
}

// --- The subscription handle ------------------------------------------------

/// The opaque handle a subscription implementation issues for one
/// subscribed query. `from_handle` exists for implementors (the value's
/// meaning is entirely the implementation's).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TelemetrySubscription(u64);

impl TelemetrySubscription {
    /// Wraps an implementation-defined handle. For implementors.
    pub const fn from_handle(handle: u64) -> Self {
        Self(handle)
    }

    /// The implementation-defined handle.
    pub const fn handle(&self) -> u64 {
        self.0
    }
}

// --- The provider rule ------------------------------------------------------

/// Errors returned when building a [`ProviderDeclaration`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderDeclarationError {
    /// The provider source ID is empty.
    EmptySourceId,
    /// The provider source ID used the reserved `flexcore:` namespace —
    /// providers are measured sources (P2.04 D-T6).
    ReservedSourceNamespace,
}

impl std::fmt::Display for ProviderDeclarationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderDeclarationError::EmptySourceId => {
                write!(f, "a provider must declare a non-empty source_id")
            }
            ProviderDeclarationError::ReservedSourceNamespace => {
                write!(f, "a provider source_id must not use the reserved 'flexcore:' namespace")
            }
        }
    }
}

impl std::error::Error for ProviderDeclarationError {}

/// One declared mapping from a verbatim source quality code to the
/// canonical state it maps to at the provider boundary (P0.08 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceStateMapping {
    /// The verbatim source-side quality code (non-empty). Retained
    /// downstream as `source_quality_code` on every record carrying it.
    pub source_code: String,
    /// The canonical state the provider maps this code to (one of the six,
    /// P0.08 §1).
    pub canonical: TelemetryQuality,
}

/// Errors returned when building a [`QualityMappingDeclaration`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QualityMappingError {
    /// The declaration is empty — a provider must declare at least one
    /// mapping.
    EmptyMapping,
    /// A source code is empty or whitespace-only.
    EmptySourceCode,
    /// The same source code is declared twice.
    DuplicateSourceCode {
        /// The duplicated code.
        source_code: String,
    },
}

impl std::fmt::Display for QualityMappingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QualityMappingError::EmptyMapping => {
                write!(f, "a provider must declare at least one quality mapping")
            }
            QualityMappingError::EmptySourceCode => {
                write!(f, "a quality mapping source code must be non-empty")
            }
            QualityMappingError::DuplicateSourceCode { source_code } => {
                write!(f, "the source code '{source_code}' is declared twice")
            }
        }
    }
}

impl std::error::Error for QualityMappingError {}

/// The provider's declared **quality-state mapping** (P2.11 step 4; P0.08
/// §3): every source-side quality code the provider can report is declared
/// with the one canonical state (of the frozen six) it maps to. The
/// original code travels on the record as `source_quality_code`; the
/// canonical vocabulary never grows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QualityMappingDeclaration {
    mappings: Vec<SourceStateMapping>,
}

impl QualityMappingDeclaration {
    /// Validates and builds the declaration: at least one mapping, no
    /// empty codes, no duplicates.
    pub fn new(mappings: Vec<SourceStateMapping>) -> Result<Self, QualityMappingError> {
        if mappings.is_empty() {
            return Err(QualityMappingError::EmptyMapping);
        }
        let mut seen: Vec<&str> = Vec::new();
        for m in &mappings {
            if m.source_code.trim().is_empty() {
                return Err(QualityMappingError::EmptySourceCode);
            }
            if seen.contains(&m.source_code.as_str()) {
                return Err(QualityMappingError::DuplicateSourceCode {
                    source_code: m.source_code.clone(),
                });
            }
            seen.push(&m.source_code);
        }
        Ok(Self { mappings })
    }

    /// The declared mappings (verbatim source code → canonical state).
    pub fn mappings(&self) -> &[SourceStateMapping] {
        &self.mappings
    }

    /// The canonical state declared for a source code, if declared.
    pub fn canonical_for(&self, source_code: &str) -> Option<TelemetryQuality> {
        self.mappings
            .iter()
            .find(|m| m.source_code == source_code)
            .map(|m| m.canonical)
    }
}

/// The **provider declaration** (P2.11 step 4): the provider rule in
/// structural form. Every provider declares — once, at construction, and
/// exposed through [`TelemetrySource::provider_declaration`] —
///
/// 1. its **cadence**: the expected emission interval between consecutive
///    samples of one asset (per-asset cadences that differ from this
///    default live in the provider's own configuration — for the simulator,
///    the scenario, P2.13 — and must not contradict it);
/// 2. its **quality-state mapping** onto the six canonical states
///    ([`QualityMappingDeclaration`], P0.08 §3);
/// 3. its **clock-quality behaviour**: the `ClockQuality` value it records
///    on its emissions (e.g. `SYNCHRONIZED` for a scenario-clock simulator;
///    `UNKNOWN` where a real source does not report synchronisation —
///    never a silent assumption of good, P2.04 D-T5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDeclaration {
    source_id: String,
    cadence: Duration,
    quality_mapping: QualityMappingDeclaration,
    clock_quality: ClockQuality,
}

impl ProviderDeclaration {
    /// Validates and builds the declaration.
    pub fn new(
        source_id: String,
        cadence: Duration,
        quality_mapping: QualityMappingDeclaration,
        clock_quality: ClockQuality,
    ) -> Result<Self, ProviderDeclarationError> {
        if source_id.is_empty() {
            return Err(ProviderDeclarationError::EmptySourceId);
        }
        if source_id.starts_with(RESERVED_SOURCE_NAMESPACE) {
            return Err(ProviderDeclarationError::ReservedSourceNamespace);
        }
        Ok(Self {
            source_id,
            cadence,
            quality_mapping,
            clock_quality,
        })
    }

    /// The provider's source identity (matches `source_id` on every record
    /// it emits).
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    /// The declared default emission cadence (per asset).
    pub fn cadence(&self) -> Duration {
        self.cadence
    }

    /// The declared quality-state mapping (P0.08 §3).
    pub fn quality_mapping(&self) -> &QualityMappingDeclaration {
        &self.quality_mapping
    }

    /// The declared clock-quality behaviour (recorded on emissions).
    pub fn clock_quality(&self) -> ClockQuality {
        self.clock_quality
    }
}

// --- Provider health --------------------------------------------------------

/// The provider's self-reported health (P2.11: the port exposes a
/// `health()` for the provider). It is the provider's honest status, not a
/// platform judgement; consumers apply their own freshness rules (P2.08).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetrySourceHealth {
    /// The provider is emitting normally.
    Healthy,
    /// The provider is up but impaired; the reason is human-safe (never raw
    /// backend text — the P1.08 rule).
    Degraded {
        /// Why the provider is degraded.
        reason: String,
    },
    /// The provider is not emitting (starting, stopped, or failed); the
    /// reason is human-safe.
    Unavailable {
        /// Why the provider is unavailable.
        reason: String,
    },
}

// --- Port errors ------------------------------------------------------------

/// Errors returned by the [`TelemetrySource`] port.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelemetrySourceError {
    /// The subscription handle does not correspond to an active
    /// subscription (expired, revoked, or never issued by this source).
    NotSubscribed,
    /// The query was rejected by the source (for example an asset outside
    /// the source's served set) — with a human-safe reason.
    InvalidQuery {
        /// A human-safe reason (never raw wire or backend content).
        reason: String,
    },
    /// The raw frame could not be decoded into complete canonical records.
    /// **This is the decode-failure rule** (P2.11 step 3): failure is
    /// reported, never papered over with a partial record.
    DecodeFailed {
        /// A human-safe reason (never raw wire content).
        reason: String,
    },
    /// The telemetry backend failed (opaque code + message; never raw
    /// backend text — the P1.08 infrastructure-error rule).
    Backend(crate::backend_error::BackendError),
}

impl std::fmt::Display for TelemetrySourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TelemetrySourceError::NotSubscribed => {
                write!(f, "the subscription handle does not match an active subscription")
            }
            TelemetrySourceError::InvalidQuery { reason } => {
                write!(f, "the telemetry query was rejected: {reason}")
            }
            TelemetrySourceError::DecodeFailed { reason } => {
                write!(f, "the raw frame could not be decoded: {reason}")
            }
            TelemetrySourceError::Backend(e) => write!(f, "telemetry backend failure: {e}"),
        }
    }
}

impl std::error::Error for TelemetrySourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TelemetrySourceError::Backend(e) => Some(e),
            _ => None,
        }
    }
}

// --- The port ---------------------------------------------------------------

/// The telemetry source port (report §9.1; ledger P2.11). The application
/// layer depends on this trait; providers (in Phase 2 exactly one — the
/// simulator, P2.14) implement it and nothing else about the ingestion
/// path.
///
/// Sync by house rule (see the module header and the crate README): no
/// future in, no future out. `subscribe` returns an opaque handle, not a
/// stream; delivery, buffering, and ordering are the adapter's concern.
pub trait TelemetrySource {
    /// The provider declaration (P2.11 step 4): cadence, quality mapping,
    /// clock-quality behaviour, and source identity. Structural so a
    /// provider cannot exist without declaring them.
    fn provider_declaration(&self) -> &ProviderDeclaration;

    /// Registers interest in the telemetry described by `query` and returns
    /// the opaque subscription handle. Implementations reject queries they
    /// cannot serve with [`TelemetrySourceError::InvalidQuery`].
    fn subscribe(
        &self,
        query: &TelemetryQuery,
    ) -> Result<TelemetrySubscription, TelemetrySourceError>;

    /// Decodes one opaque raw frame from the subscribed stream into the
    /// **complete set** of canonical records that frame carries (a
    /// multi-quantity sample is a set of records sharing `source_id`,
    /// `source_sequence`, and `event_time`, P2.04 D-T2). A successful
    /// result is non-empty and every record in it is complete and
    /// canonical; anything less is [`TelemetrySourceError::DecodeFailed`] —
    /// never a partial record, never a silently degraded quality state
    /// (P0.08 non-silencing rule).
    fn decode(
        &self,
        subscription: &TelemetrySubscription,
        payload: &[u8],
    ) -> Result<Vec<NormalizedTelemetry>, TelemetrySourceError>;

    /// The provider's source identity (matches `source_id` on every record
    /// it emits). Derived from the mandatory declaration by default.
    fn source_id(&self) -> &str {
        self.provider_declaration().source_id()
    }

    /// The provider's self-reported health.
    fn health(&self) -> TelemetrySourceHealth;
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;
    use flexcore_domain::tenant_context::{AuthenticatedClaims, VerifiedClaims};

    const TENANT_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a9b";
    const ASSET_A_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8c";
    const ASSET_B_UUID: &str = "0192a5e0-0000-7e2f-9a1b-3c4d5e6f7a8d";

    fn tenant_id() -> TenantId {
        TenantId::new(FlexUuid::parse(TENANT_UUID).unwrap())
    }

    fn asset_a() -> AssetId {
        AssetId::new(FlexUuid::parse(ASSET_A_UUID).unwrap())
    }

    fn asset_b() -> AssetId {
        AssetId::new(FlexUuid::parse(ASSET_B_UUID).unwrap())
    }

    /// The only path a query can take to a tenant: authenticated claims →
    /// tenant context → query. There is no other constructor.
    fn tenant_context() -> TenantContext {
        TenantContext::from_authenticated_claims(AuthenticatedClaims::from_verified(
            VerifiedClaims {
                tenant_id: tenant_id(),
                subject: "user:telemetry-test".to_string(),
            },
        ))
    }

    fn event(millis: i64) -> EventTime {
        EventTime::from_unix_millis(millis).unwrap()
    }

    fn ingest(millis: i64) -> IngestTime {
        IngestTime::from_unix_millis(millis).unwrap()
    }

    fn complete_record_parts() -> NormalizedTelemetryParts {
        NormalizedTelemetryParts {
            tenant_id: tenant_id(),
            asset_id: asset_a(),
            quantity: TelemetryQuantity::ActivePower,
            value: Some(QuantityValue::Power(ActivePowerMW::new(1.5).unwrap())),
            event_time: event(1_700_000_000_000),
            ingest_time: ingest(1_700_000_000_001),
            source_sequence: 1,
            source_id: "sim://exit-gate".to_string(),
            quality: TelemetryQuality::Good,
            source_quality_code: None,
            clock_quality: ClockQuality::Synchronized,
        }
    }

    #[test]
    fn the_query_requires_a_tenant_context_and_carries_its_tenant() {
        // Structural rule (P0.16 rules 1 and 5): TelemetryQuery::new is the
        // only constructor and takes &TenantContext — a query without a
        // tenant context is a compile error, not a runtime check. The
        // bypasses below do not compile (documented, same pattern as the
        // domain tenant-context negative test):
        //
        //   TelemetryQuery { tenant_id, assets, .. }  — fields are private
        //   (error[E0451])
        //   From<TenantId> for TelemetryQuery         — no impl exists
        //   (error[E0277])
        //
        // The positive path pins that the tenant comes from the context
        // and cannot disagree with it:
        let query = TelemetryQuery::new(
            &tenant_context(),
            vec![asset_a()],
            event(1_000),
            event(2_000),
            vec![TelemetryQuantity::ActivePower],
        )
        .unwrap();
        assert_eq!(query.tenant_id(), tenant_id());
        assert_eq!(query.assets(), &[asset_a()]);
        assert_eq!(query.quantities(), &[TelemetryQuantity::ActivePower]);
    }

    #[test]
    fn the_query_rejects_empty_sets_and_an_inverted_range() {
        let ctx = tenant_context();
        assert_eq!(
            TelemetryQuery::new(
                &ctx,
                Vec::new(),
                event(1_000),
                event(2_000),
                vec![TelemetryQuantity::ActivePower]
            ),
            Err(TelemetryQueryError::EmptyAssetSet)
        );
        assert_eq!(
            TelemetryQuery::new(&ctx, vec![asset_a()], event(1_000), event(2_000), Vec::new()),
            Err(TelemetryQueryError::EmptyQuantitySet)
        );
        assert_eq!(
            TelemetryQuery::new(
                &ctx,
                vec![asset_a()],
                event(2_000),
                event(1_000),
                vec![TelemetryQuantity::ActivePower]
            ),
            Err(TelemetryQueryError::InvertedTimeRange)
        );
        // A single-instant range (from == until) is valid.
        assert!(TelemetryQuery::new(
            &ctx,
            vec![asset_a()],
            event(1_000),
            event(1_000),
            vec![TelemetryQuantity::ActivePower]
        )
        .is_ok());
    }

    #[test]
    fn a_complete_measured_record_round_trips() {
        let record = NormalizedTelemetry::new(complete_record_parts()).unwrap();
        assert_eq!(record.tenant_id(), tenant_id());
        assert_eq!(record.asset_id(), asset_a());
        assert_eq!(record.quantity(), TelemetryQuantity::ActivePower);
        assert_eq!(
            record.value(),
            Some(&QuantityValue::Power(ActivePowerMW::new(1.5).unwrap()))
        );
        assert_eq!(record.source_id(), "sim://exit-gate");
        assert_eq!(record.source_sequence(), 1);
        assert_eq!(record.quality(), TelemetryQuality::Good);
        assert_eq!(record.source_quality_code(), None);
        assert_eq!(record.clock_quality(), ClockQuality::Synchronized);
    }

    #[test]
    fn record_construction_rejects_missing_canonical_fields() {
        // Every case below is a whole-record rejection — the no-partial-
        // record rule (P2.11 step 3).

        // Empty source identity.
        let mut parts = complete_record_parts();
        parts.source_id = String::new();
        assert_eq!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::EmptySourceId)
        );

        // A measured record without a value (the partial-record case).
        let mut parts = complete_record_parts();
        parts.value = None;
        assert_eq!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::MissingValue)
        );

        // A value on a MISSING gap marker.
        let mut parts = complete_record_parts();
        parts.quality = TelemetryQuality::Missing;
        parts.source_id = "flexcore:derived:gap-detector".to_string();
        assert_eq!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::ValueOnMissingRecord)
        );

        // MISSING outside the derived namespace.
        let mut parts = complete_record_parts();
        parts.quality = TelemetryQuality::Missing;
        parts.value = None;
        assert_eq!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::MissingRequiresDerivedNamespace)
        );

        // The reserved namespace outside the derived sub-namespace.
        let mut parts = complete_record_parts();
        parts.source_id = "flexcore:something".to_string();
        assert_eq!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::ReservedSourceNamespace)
        );

        // A derived-namespace record claiming a measured quality state.
        let mut parts = complete_record_parts();
        parts.source_id = "flexcore:derived:gap-detector".to_string();
        assert_eq!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::DerivedSourceNamespaceMisused)
        );

        // Quantity/value kind mismatch (SOC record carrying a power value).
        let mut parts = complete_record_parts();
        parts.quantity = TelemetryQuantity::Soc;
        assert_eq!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::ValueQuantityMismatch {
                quantity: TelemetryQuantity::Soc
            })
        );

        // A reactive-power record is only valid with its convention — and
        // a reactive value under a non-reactive quantity is a mismatch.
        let mut parts = complete_record_parts();
        parts.quantity = TelemetryQuantity::ReactivePower;
        assert_eq!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::ValueQuantityMismatch {
                quantity: TelemetryQuantity::ReactivePower
            })
        );

        // An empty text state.
        let mut parts = complete_record_parts();
        parts.quantity = TelemetryQuantity::OperatingMode;
        parts.value = Some(QuantityValue::State("  ".to_string()));
        assert_eq!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::EmptyStateText)
        );

        // ingest_time before event_time (P0.09).
        let mut parts = complete_record_parts();
        parts.ingest_time = ingest(1_699_999_999_999);
        assert!(matches!(
            NormalizedTelemetry::new(parts),
            Err(NormalizedTelemetryError::TimeOrdering(
                TimeModelError::IngestBeforeEvent
            ))
        ));
    }

    #[test]
    fn a_derived_gap_marker_and_a_mapped_source_state_are_accepted() {
        // Platform-derived MISSING marker (P2.04 D-T6): derived namespace,
        // no value, no source quality code needed.
        let marker = NormalizedTelemetry::new(NormalizedTelemetryParts {
            quantity: TelemetryQuantity::ActivePower,
            value: None,
            quality: TelemetryQuality::Missing,
            source_id: "flexcore:derived:gap-detector".to_string(),
            ..complete_record_parts()
        })
        .unwrap();
        assert_eq!(marker.quality(), TelemetryQuality::Missing);
        assert_eq!(marker.value(), None);

        // A mapped richer source state keeps the original code as
        // diagnostic metadata (P0.08 §3; non-silencing).
        let mapped = NormalizedTelemetry::new(NormalizedTelemetryParts {
            quality: TelemetryQuality::Suspect,
            source_quality_code: Some("OUT_OF_RANGE".to_string()),
            value: Some(QuantityValue::Fraction(StateOfCharge::new(0.42).unwrap())),
            quantity: TelemetryQuantity::Soc,
            ..complete_record_parts()
        })
        .unwrap();
        assert_eq!(mapped.quality(), TelemetryQuality::Suspect);
        assert_eq!(mapped.source_quality_code(), Some("OUT_OF_RANGE"));
    }

    #[test]
    fn provider_declarations_reject_bad_source_ids_and_bad_mappings() {
        let mapping = |code: &str| -> Vec<SourceStateMapping> {
            vec![SourceStateMapping {
                source_code: code.to_string(),
                canonical: TelemetryQuality::Good,
            }]
        };

        // Empty provider source ID.
        assert_eq!(
            ProviderDeclaration::new(
                String::new(),
                Duration::from_secs(60),
                QualityMappingDeclaration::new(mapping("GOOD")).unwrap(),
                ClockQuality::Synchronized,
            ),
            Err(ProviderDeclarationError::EmptySourceId)
        );

        // Reserved namespace on a measured provider.
        assert_eq!(
            ProviderDeclaration::new(
                "flexcore:derived:gap-detector".to_string(),
                Duration::from_secs(60),
                QualityMappingDeclaration::new(mapping("GOOD")).unwrap(),
                ClockQuality::Synchronized,
            ),
            Err(ProviderDeclarationError::ReservedSourceNamespace)
        );

        // Empty mapping declaration.
        assert_eq!(
            QualityMappingDeclaration::new(Vec::new()),
            Err(QualityMappingError::EmptyMapping)
        );

        // Empty source code.
        assert_eq!(
            QualityMappingDeclaration::new(mapping("  ")),
            Err(QualityMappingError::EmptySourceCode)
        );

        // Duplicate source code.
        let duplicated = vec![
            SourceStateMapping {
                source_code: "OK".to_string(),
                canonical: TelemetryQuality::Good,
            },
            SourceStateMapping {
                source_code: "OK".to_string(),
                canonical: TelemetryQuality::Suspect,
            },
        ];
        assert_eq!(
            QualityMappingDeclaration::new(duplicated),
            Err(QualityMappingError::DuplicateSourceCode {
                source_code: "OK".to_string()
            })
        );
    }

    // --- Decode-failure contract (minimal fake) -------------------------

    /// A minimal fake provider used only to pin the port contract: valid
    /// frames decode to complete record sets; anything else is a
    /// `DecodeFailed` — never a partial record.
    struct MinimalFakeSource {
        declaration: ProviderDeclaration,
    }

    impl MinimalFakeSource {
        fn new() -> Self {
            Self {
                declaration: ProviderDeclaration::new(
                    "sim://fake".to_string(),
                    Duration::from_secs(60),
                    QualityMappingDeclaration::new(vec![SourceStateMapping {
                        source_code: "GOOD".to_string(),
                        canonical: TelemetryQuality::Good,
                    }])
                    .unwrap(),
                    ClockQuality::Synchronized,
                )
                .unwrap(),
            }
        }
    }

    impl TelemetrySource for MinimalFakeSource {
        fn provider_declaration(&self) -> &ProviderDeclaration {
            &self.declaration
        }
        fn subscribe(
            &self,
            _query: &TelemetryQuery,
        ) -> Result<TelemetrySubscription, TelemetrySourceError> {
            Ok(TelemetrySubscription::from_handle(7))
        }
        fn decode(
            &self,
            subscription: &TelemetrySubscription,
            payload: &[u8],
        ) -> Result<Vec<NormalizedTelemetry>, TelemetrySourceError> {
            if *subscription != TelemetrySubscription::from_handle(7) {
                return Err(TelemetrySourceError::NotSubscribed);
            }
            if payload.is_empty() {
                // The decode-failure rule: an undecodable frame is Err. The
                // error carries a human-safe reason; no record leaves.
                return Err(TelemetrySourceError::DecodeFailed {
                    reason: "frame checksum mismatch".to_string(),
                });
            }
            Ok(vec![NormalizedTelemetry::new(complete_record_parts()).unwrap()])
        }
        fn health(&self) -> TelemetrySourceHealth {
            TelemetrySourceHealth::Healthy
        }
    }

    #[test]
    fn decode_failure_is_err_and_never_a_partial_record() {
        let source = MinimalFakeSource::new();
        let source: &dyn TelemetrySource = &source;

        // The provider declaration is structural: source_id, cadence,
        // quality mapping, clock behaviour are all present.
        assert_eq!(source.source_id(), "sim://fake");
        assert_eq!(source.provider_declaration().cadence(), Duration::from_secs(60));
        assert_eq!(
            source
                .provider_declaration()
                .quality_mapping()
                .canonical_for("GOOD"),
            Some(TelemetryQuality::Good)
        );
        assert_eq!(
            source.provider_declaration().clock_quality(),
            ClockQuality::Synchronized
        );
        assert_eq!(source.health(), TelemetrySourceHealth::Healthy);

        let query = TelemetryQuery::new(
            &tenant_context(),
            vec![asset_a()],
            event(0),
            event(1_000),
            vec![TelemetryQuantity::ActivePower],
        )
        .unwrap();
        let subscription = source.subscribe(&query).unwrap();

        // Failure path: Err with the reason; the Result type offers no way
        // to obtain a partial record alongside it.
        let err = source.decode(&subscription, &[]).unwrap_err();
        assert_eq!(
            err,
            TelemetrySourceError::DecodeFailed {
                reason: "frame checksum mismatch".to_string()
            }
        );
        assert!(err.to_string().contains("checksum mismatch"));

        // Success path: a complete record set, never empty.
        let records = source.decode(&subscription, b"frame").unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].asset_id(), asset_a());

        // An unknown handle is NotSubscribed, not a decode error.
        let unknown = TelemetrySubscription::from_handle(99);
        assert_eq!(
            source.decode(&unknown, b"frame"),
            Err(TelemetrySourceError::NotSubscribed)
        );
    }

    #[test]
    fn the_subscription_handle_is_opaque_and_round_trips() {
        let sub = TelemetrySubscription::from_handle(42);
        assert_eq!(sub.handle(), 42);
    }
}
