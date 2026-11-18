// P14.15 — The DOE consumption interface (decision record
// `docs/phases-11-20/phase-14-protocol-device-connectivity/records/
// 63_p14_15_doe_consumption_interface.md`).
//
// Sources: [S-158] AEMO Project EDGE — Determining the Impact of Update
// Frequency on Operating Envelope Efficacy, Workstream 3 (the update-
// cadence finding); [S-159] SA Power Networks Flexible Exports (the lapse
// behavior — an expired/stale dynamic limit reverts to the STATIC export
// limit); [S-160] Amber Electric flexible-exports note (the customer-side
// breach/curtailment framing). **Every DOE semantic below is UNVERIFIED
// against these primary sources** — the gap analysis cites them; their
// texts were not consulted here. The implementation-time human gate
// confirms: the EDGE Workstream 3 update-frequency semantics, the exact
// SAPN lapse conditions and static-limit fallback, and the CSIP-AUS
// envelope framing. A confirmed mismatch is a recorded edit, never a
// silent reinterpretation (plan §7 note 6).
//
// WHAT THIS MODULE IS (and is not):
//
// - A **consumption** interface: DOE envelopes are ingested as DATA and
//   turned into optimizer constraint data with issue timestamps, expiry,
//   and declared update cadence. The optimizer consumes the bounds — the
//   module produces no schedule, no dispatch decision, no setpoint
//   (phase-14 context §3 constraint 3: "DOE envelopes enter the optimizer
//   as constraint data only").
// - A **record** surface: breach and curtailment telemetry records the
//   P11 observability taxonomy CONSUMES (the taxonomy is defined once in
//   Phase 11 and never redefined — gap analysis §9 dependency rule). These
//   records are operator-attention data, never protection signals.
// - NOT an issuance interface: this module never creates, modifies, or
//   approves a DOE — envelopes arrive from the DNSP/operator surface as
//   declared data with provenance. Nothing here talks to a device.
//
// SIGN RULE (P0.07 §2, frozen): P > 0 = EXPORT at the point of
// connection. An import limit bounds |P| while consuming (P < 0); an
// export limit bounds P while exporting (P > 0). The bounds pair produced
// for the optimizer is exactly [−import_limit, +export_limit] in MW.
//
// LAPSE BEHAVIOR (the SAPN rule, UNVERIFIED transcription [S-159]): when
// a DOE is expired, stale beyond its declared cadence budget, or not yet
// in effect, the connection point reverts to its STATIC limit — the
// declared fixed import/export limits of the connection agreement. The
// fallback is mandatory declared data on every evaluation (a DOE without
// a static fallback never evaluates — the consumer refuses it rather
// than guessing an unconstrained default).
//
// SIMULATOR HANDOFF (recorded, not exercised here): the DOE profile shape
// for `flexcore-simulator` lives in this module — wave-3 Agent A
// (P14.17) extends `crates/flexcore-simulator/src/profiles.rs` with a DOE
// behaviour profile consuming
// [`DynamicOperatingEnvelope`], [`StaticConnectionLimit`], and
// [`effective_envelope_bounds_mw`] from THIS module (the simulator never
// redefines the semantics; it only samples them per tick). This module
// contains no simulator code and flexcore-simulator is untouched by
// P14.15 (the wave-3 ownership boundary, plan §3).

use flexcore_contracts::capability::Provenance;
use flexcore_contracts::time_model::EventTime;
use flexcore_contracts::units::ActivePowerMW;

use crate::ids::SiteId;

// --- Connection points and scopes -------------------------------------------

/// The connection point a DOE applies to: a validated, non-empty
/// identifier of the network connection point (the DNSP's own spelling is
/// recorded data — this module never parses network names).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ConnectionPointId(String);

impl ConnectionPointId {
    /// Constructs a validated connection-point id (non-empty, bounded
    /// length; no whitespace-only values).
    pub fn new(value: String) -> Result<Self, DoeError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(DoeError::EmptyField { field: "connection_point_id" });
        }
        if trimmed.len() > 256 {
            return Err(DoeError::FieldTooLong { field: "connection_point_id" });
        }
        Ok(Self(trimmed.to_string()))
    }

    /// The declared id.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ConnectionPointId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What one DOE applies to: a single connection point or a declared,
/// non-empty group of them (plan §2 P14.15: "per-connection-point /
/// per-group"). A group with duplicate members is refused — the group is
/// a reviewed declaration, not a bag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvelopeScope {
    /// One connection point.
    Single(ConnectionPointId),
    /// A named group of connection points (non-empty, no duplicates).
    Group {
        /// The group's declared name (non-empty).
        name: String,
        /// The member connection points (non-empty, no duplicates).
        members: Vec<ConnectionPointId>,
    },
}

impl EnvelopeScope {
    /// Every connection point this scope covers (sorted, deduplicated —
    /// the deterministic view consumers iterate).
    pub fn connection_points(&self) -> Vec<&ConnectionPointId> {
        match self {
            EnvelopeScope::Single(id) => vec![id],
            EnvelopeScope::Group { members, .. } => {
                let mut out: Vec<&ConnectionPointId> = members.iter().collect();
                out.sort();
                out.dedup();
                out
            }
        }
    }

    /// Whether the scope covers one connection point.
    pub fn covers(&self, point: &ConnectionPointId) -> bool {
        match self {
            EnvelopeScope::Single(id) => id == point,
            EnvelopeScope::Group { members, .. } => members.contains(point),
        }
    }
}

// --- Limits -------------------------------------------------------------------

/// The declared import/export limits of one envelope or static fallback,
/// in canonical MW (the AGENTS.md §4 units rule). Both values must be
/// finite and non-negative; an import limit of zero forbids import, an
/// export limit of zero forbids export — zeros are meaningful bounds, not
/// defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImportExportLimits {
    /// The maximum import magnitude in MW (consumption, P < 0). Non-negative.
    pub import_limit_mw: f64,
    /// The maximum export in MW (P > 0). Non-negative.
    pub export_limit_mw: f64,
}

impl ImportExportLimits {
    /// Constructs and validates a limits pair.
    pub fn new(import_limit_mw: f64, export_limit_mw: f64) -> Result<Self, DoeError> {
        if !import_limit_mw.is_finite() || !export_limit_mw.is_finite() {
            return Err(DoeError::NonFiniteValue { field: "import/export limits" });
        }
        if import_limit_mw < 0.0 || export_limit_mw < 0.0 {
            return Err(DoeError::NegativeLimit {
                field: if import_limit_mw < 0.0 { "import_limit_mw" } else { "export_limit_mw" },
                value: if import_limit_mw < 0.0 { import_limit_mw } else { export_limit_mw },
            });
        }
        Ok(Self { import_limit_mw, export_limit_mw })
    }
}

/// The STATIC connection limit: the fixed import/export limits of the
/// connection agreement the DOE falls back to on lapse (the SAPN rule,
/// UNVERIFIED transcription [S-159]). Declared data with a citation — a
/// static limit without a source document is not a reviewed limit.
#[derive(Debug, Clone, PartialEq)]
pub struct StaticConnectionLimit {
    /// The connection point (or site) the static limit belongs to. For a
    /// GROUP scope the static limit must be provided per member by the
    /// consumer — this type carries one point's static limit only (the
    /// honest granularity; a group-level static fallback is the
    /// consumer's declared composition, never an assumption here).
    pub connection_point: ConnectionPointId,
    /// The static limits.
    pub limits: ImportExportLimits,
    /// The source citation (the connection agreement / tariff reference).
    pub citation: String,
    /// The provenance of the static limit (a connection agreement is
    /// `Configured`; an unknown value is `Unknown` and the P3.02 narrowing
    /// rule applies wherever it feeds an envelope).
    pub provenance: Provenance,
}

impl StaticConnectionLimit {
    /// Constructs and validates a static connection limit.
    pub fn new(
        connection_point: ConnectionPointId,
        limits: ImportExportLimits,
        citation: String,
        provenance: Provenance,
    ) -> Result<Self, DoeError> {
        if citation.trim().is_empty() {
            return Err(DoeError::EmptyField { field: "citation" });
        }
        Ok(Self { connection_point, limits, citation, provenance })
    }
}

// --- The envelope --------------------------------------------------------------

/// The declared update cadence of one DOE (the EDGE Workstream 3 subject
/// [S-158], UNVERIFIED semantics): how often the operator updates the
/// envelope, and how many missed updates the consumer tolerates before
/// the envelope is STALE and lapses to the static limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateCadence {
    /// The expected update interval in seconds (strictly positive).
    pub expected_interval_seconds: i64,
    /// The staleness budget in update intervals (at least 1: the envelope
    /// must tolerate its own interval, or it is a push-only stream with no
    /// cadence — which this type does not model; declare the real cadence).
    pub max_intervals_before_stale: u32,
}

impl UpdateCadence {
    /// The freshness limit in seconds: the update is expected within
    /// `expected_interval_seconds * max_intervals_before_stale`.
    pub fn freshness_limit_seconds(&self) -> i64 {
        self.expected_interval_seconds
            .saturating_mul(i64::from(self.max_intervals_before_stale))
    }
}

/// A dynamic operating envelope as INGESTED DATA: per-connection-point or
/// per-group import/export limits with issue timestamps, expiry, and a
/// declared update cadence ([S-158][S-159]; every semantic UNVERIFIED
/// against the primary sources).
///
/// The envelope is declared data with provenance; this module never
/// issues, extends, or approves one. `latest_update_unix_ms` is the
/// timestamp of the most recent update RECEIVED for this envelope (>= the
/// issue time — enforced at construction; the freshness evaluation
/// compares it against the declared cadence).
#[derive(Debug, Clone, PartialEq)]
pub struct DynamicOperatingEnvelope {
    /// The operator's envelope id (non-empty, bounded; recorded data).
    pub envelope_id: String,
    /// What the envelope applies to.
    pub scope: EnvelopeScope,
    /// The dynamic import/export limits.
    pub limits: ImportExportLimits,
    /// The issue timestamp (UTC, Unix ms — the contracts time-model rule).
    pub issued_at_unix_ms: i64,
    /// The expiry timestamp (UTC, Unix ms; strictly after the issue time).
    pub expires_at_unix_ms: i64,
    /// The declared update cadence.
    pub cadence: UpdateCadence,
    /// The timestamp of the most recent update received (>= issued_at).
    pub latest_update_unix_ms: i64,
    /// The issuing authority's declared name (non-empty; recorded data).
    pub issuer: String,
    /// The provenance of the envelope (an operator issuance is
    /// `Configured`; an unknown-origin envelope is `Unknown` and narrows,
    /// never widens — the P3.02 rule).
    pub provenance: Provenance,
    /// The human-safe provenance note (never raw wire text; the P1.08
    /// rule).
    pub provenance_note: String,
}

impl DynamicOperatingEnvelope {
    /// Constructs and validates an envelope. Every timestamp is checked:
    /// expiry strictly after issue; the latest update at or after issue
    /// and not before it; identifiers and notes non-empty.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        envelope_id: String,
        scope: EnvelopeScope,
        limits: ImportExportLimits,
        issued_at_unix_ms: i64,
        expires_at_unix_ms: i64,
        cadence: UpdateCadence,
        latest_update_unix_ms: i64,
        issuer: String,
        provenance: Provenance,
        provenance_note: String,
    ) -> Result<Self, DoeError> {
        if envelope_id.trim().is_empty() {
            return Err(DoeError::EmptyField { field: "envelope_id" });
        }
        if envelope_id.len() > 256 {
            return Err(DoeError::FieldTooLong { field: "envelope_id" });
        }
        if issuer.trim().is_empty() {
            return Err(DoeError::EmptyField { field: "issuer" });
        }
        if provenance_note.trim().is_empty() {
            return Err(DoeError::EmptyField { field: "provenance_note" });
        }
        match &scope {
            EnvelopeScope::Single(_) => {}
            EnvelopeScope::Group { name, members } => {
                if name.trim().is_empty() {
                    return Err(DoeError::EmptyField { field: "group_name" });
                }
                if members.is_empty() {
                    return Err(DoeError::EmptyGroup { group: name.clone() });
                }
                let mut seen: Vec<&ConnectionPointId> = Vec::new();
                for member in members {
                    if seen.contains(&member) {
                        return Err(DoeError::DuplicateGroupMember {
                            group: name.clone(),
                        });
                    }
                    seen.push(member);
                }
            }
        }
        if expires_at_unix_ms <= issued_at_unix_ms {
            return Err(DoeError::InvertedValidity {
                issued_at_unix_ms,
                expires_at_unix_ms,
            });
        }
        if cadence.expected_interval_seconds <= 0 {
            return Err(DoeError::NonPositiveCadence {
                value: cadence.expected_interval_seconds,
            });
        }
        if cadence.max_intervals_before_stale == 0 {
            return Err(DoeError::ZeroStalenessBudget);
        }
        if latest_update_unix_ms < issued_at_unix_ms {
            return Err(DoeError::UpdateBeforeIssue {
                issued_at_unix_ms,
                latest_update_unix_ms,
            });
        }
        Ok(Self {
            envelope_id,
            scope,
            limits,
            issued_at_unix_ms,
            expires_at_unix_ms,
            cadence,
            latest_update_unix_ms,
            issuer,
            provenance,
            provenance_note,
        })
    }
}

// --- Evaluation: the consumption interface --------------------------------------

/// Why a DOE is not in effect at an evaluation instant (the lapse reasons;
/// the SAPN fallback applies in every case — UNVERIFIED transcription
/// [S-159]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LapseReason {
    /// The evaluation instant is before the envelope's issue time.
    NotYetInEffect,
    /// The evaluation instant is at or after the envelope's expiry.
    Expired,
    /// The envelope is within its validity window but no update arrived
    /// within the declared cadence budget (the EDGE Workstream 3 subject,
    /// UNVERIFIED semantics [S-158]).
    Stale,
}

impl LapseReason {
    /// Canonical name (the stored spelling).
    pub const fn name(self) -> &'static str {
        match self {
            LapseReason::NotYetInEffect => "not_yet_in_effect",
            LapseReason::Expired => "expired",
            LapseReason::Stale => "stale",
        }
    }
}

/// The result of evaluating one envelope at one instant: either the
/// dynamic limits are in effect, or the envelope has lapsed and the
/// STATIC limit governs (the SAPN rule — the fallback is mandatory data
/// on every evaluation).
#[derive(Debug, Clone, PartialEq)]
pub enum EffectiveEnvelope {
    /// The dynamic envelope is in effect.
    Dynamic {
        /// The in-effect dynamic limits.
        limits: ImportExportLimits,
    },
    /// The envelope has lapsed; the static limit governs until a fresh
    /// update (or the static limit itself is the whole truth when no DOE
    /// is declared).
    Lapsed {
        /// Why the envelope lapsed.
        reason: LapseReason,
        /// The static fallback limits (mandatory declared data).
        fallback: ImportExportLimits,
    },
}

/// Evaluates one envelope at one instant against the mandatory static
/// fallback for its scope's granularity. The evaluation is a pure
/// function of (envelope, static fallback, now) — no clock is read (the
/// injected-Clock rule; the caller supplies the evaluation instant).
///
/// Precedence of the lapse reasons (deterministic): NOT-YET-IN-EFFECT
/// before EXPIRED before STALE — an envelope outside its window never
/// reports staleness.
pub fn evaluate_envelope(
    envelope: &DynamicOperatingEnvelope,
    fallback: &StaticConnectionLimit,
    now: &EventTime,
) -> Result<EffectiveEnvelope, DoeError> {
    let now_ms = now.unix_millis();
    let now_ms = i64::try_from(now_ms)
        .map_err(|_| DoeError::TimestampOutOfRange { value: now_ms })?;
    if fallback.limits.import_limit_mw.is_nan() || fallback.limits.export_limit_mw.is_nan() {
        return Err(DoeError::NonFiniteValue { field: "static fallback limits" });
    }
    if now_ms < envelope.issued_at_unix_ms {
        return Ok(EffectiveEnvelope::Lapsed {
            reason: LapseReason::NotYetInEffect,
            fallback: fallback.limits,
        });
    }
    if now_ms >= envelope.expires_at_unix_ms {
        return Ok(EffectiveEnvelope::Lapsed {
            reason: LapseReason::Expired,
            fallback: fallback.limits,
        });
    }
    let age_ms = now_ms
        .saturating_sub(envelope.latest_update_unix_ms)
        .max(0);
    let freshness_limit_ms = envelope
        .cadence
        .freshness_limit_seconds()
        .saturating_mul(1000);
    if age_ms > freshness_limit_ms {
        return Ok(EffectiveEnvelope::Lapsed {
            reason: LapseReason::Stale,
            fallback: fallback.limits,
        });
    }
    Ok(EffectiveEnvelope::Dynamic { limits: envelope.limits })
}

/// The optimizer constraint bounds in canonical MW for one effective
/// envelope: `Ok((p_min_mw, p_max_mw))` where `p_min = -import_limit`
/// and `p_max = +export_limit` (the frozen P > 0 = export rule). The
/// optimizer consumes these as CONSTRAINT DATA ONLY (phase-14 context
/// §3 constraint 3) — this module produces no schedule and no setpoint.
///
/// This is ALSO the seam the wave-3 simulator profile consumes per tick
/// (the recorded P14.15 → P14.17 handoff; see the module header): the
/// simulator samples `evaluate_envelope` at each tick and clips its
/// synthetic output with [`effective_envelope_bounds_mw`].
pub fn effective_envelope_bounds_mw(
    effective: &EffectiveEnvelope,
) -> (f64, f64) {
    let limits = match effective {
        EffectiveEnvelope::Dynamic { limits } => *limits,
        EffectiveEnvelope::Lapsed { fallback, .. } => *fallback,
    };
    (-limits.import_limit_mw, limits.export_limit_mw)
}

// --- Breach / curtailment telemetry ----------------------------------------------

/// The direction of a breach relative to the point of connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreachDirection {
    /// Import (consumption) exceeded the import limit (P below −import).
    Import,
    /// Export exceeded the export limit (P above +export).
    Export,
}

impl BreachDirection {
    /// Canonical name (the stored spelling).
    pub const fn name(self) -> &'static str {
        match self {
            BreachDirection::Import => "import",
            BreachDirection::Export => "export",
        }
    }
}

/// A breach telemetry record: an observed active power outside the
/// effective envelope bounds at a connection point. A RECORD, produced by
/// the platform's monitoring path from telemetry — never a protection
/// signal and never a dispatch decision. The P11 observability taxonomy
/// consumes these (trace_id correlation and the alert-rule shape are
/// defined once in Phase 11 and never redefined — gap analysis §9).
#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeBreachRecord {
    /// The envelope whose bounds were breached (empty = the static limit
    /// was breached — the honest representation of a no-DOE breach).
    pub envelope_id: Option<String>,
    /// The connection point.
    pub connection_point: ConnectionPointId,
    /// The breached direction.
    pub direction: BreachDirection,
    /// The observed active power (canonical MW, P > 0 = export; the sign
    /// is carried, never reinterpreted).
    pub observed_mw: ActivePowerMW,
    /// The bound that was exceeded, in MW (the effective bound magnitude:
    /// the import or export limit in force).
    pub limit_mw: f64,
    /// The observation timestamp (UTC, Unix ms — declared by the caller;
    /// the contracts time model validates event times at ingest).
    pub observed_at_unix_ms: i64,
    /// The site (portfolio context) the connection point belongs to, when
    /// known (declared, never inferred).
    pub site: Option<SiteId>,
}

impl EnvelopeBreachRecord {
    /// Constructs and validates a breach record. The limit must be finite
    /// and non-negative, and must actually be exceeded by the observed
    /// power in the declared direction — a breach record that does not
    /// breach is a data-quality defect, refused at construction.
    pub fn new(
        envelope_id: Option<String>,
        connection_point: ConnectionPointId,
        direction: BreachDirection,
        observed_mw: ActivePowerMW,
        limit_mw: f64,
        observed_at_unix_ms: i64,
        site: Option<SiteId>,
    ) -> Result<Self, DoeError> {
        if !limit_mw.is_finite() || limit_mw < 0.0 {
            return Err(DoeError::NegativeLimit { field: "limit_mw", value: limit_mw });
        }
        let p = observed_mw.value();
        let breached = match direction {
            BreachDirection::Export => p > limit_mw,
            BreachDirection::Import => p < -limit_mw,
        };
        if !breached {
            return Err(DoeError::NotABreach {
                direction: direction.name(),
                observed_mw: p,
                limit_mw,
            });
        }
        if let Some(id) = &envelope_id {
            if id.trim().is_empty() {
                return Err(DoeError::EmptyField { field: "envelope_id" });
            }
        }
        Ok(Self {
            envelope_id,
            connection_point,
            direction,
            observed_mw,
            limit_mw,
            observed_at_unix_ms,
            site,
        })
    }
}

/// A curtailment telemetry record: the declared reduction of export (or
/// import capability) exercised to stay inside the effective envelope —
/// recorded data for the operator console and the P11 taxonomy, never a
/// command (the curtailment itself is a lifecycle command's outcome;
/// this record only observes it).
#[derive(Debug, Clone, PartialEq)]
pub struct CurtailmentRecord {
    /// The envelope that motivated the curtailment (when one was in
    /// force).
    pub envelope_id: Option<String>,
    /// The connection point.
    pub connection_point: ConnectionPointId,
    /// The curtailed magnitude in MW (strictly positive — a zero
    /// curtailment is not a curtailment event).
    pub curtailed_mw: f64,
    /// The direction that was curtailed.
    pub direction: BreachDirection,
    /// The record timestamp (UTC, Unix ms; declared by the caller).
    pub recorded_at_unix_ms: i64,
    /// The site context, when known (declared, never inferred).
    pub site: Option<SiteId>,
}

impl CurtailmentRecord {
    /// Constructs and validates a curtailment record.
    pub fn new(
        envelope_id: Option<String>,
        connection_point: ConnectionPointId,
        curtailed_mw: f64,
        direction: BreachDirection,
        recorded_at_unix_ms: i64,
        site: Option<SiteId>,
    ) -> Result<Self, DoeError> {
        if !curtailed_mw.is_finite() || curtailed_mw <= 0.0 {
            return Err(DoeError::NonPositiveCurtailment { value: curtailed_mw });
        }
        if let Some(id) = &envelope_id {
            if id.trim().is_empty() {
                return Err(DoeError::EmptyField { field: "envelope_id" });
            }
        }
        Ok(Self {
            envelope_id,
            connection_point,
            curtailed_mw,
            direction,
            recorded_at_unix_ms,
            site,
        })
    }
}

// --- Errors ------------------------------------------------------------------------

/// Errors the DOE consumption interface reports. Every variant names the
/// offending field/value (the P3.04 style).
#[derive(Debug, Clone, PartialEq)]
pub enum DoeError {
    /// A numeric value was NaN or infinite.
    NonFiniteValue {
        /// The offending field.
        field: &'static str,
    },
    /// A limit or magnitude was negative where only non-negative is
    /// declared.
    NegativeLimit {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// A required text field was empty (or whitespace-only).
    EmptyField {
        /// The offending field.
        field: &'static str,
    },
    /// A text field exceeded its declared bound.
    FieldTooLong {
        /// The offending field.
        field: &'static str,
    },
    /// A group scope declared no members.
    EmptyGroup {
        /// The offending group name.
        group: String,
    },
    /// A group scope declared the same connection point twice.
    DuplicateGroupMember {
        /// The offending group name.
        group: String,
    },
    /// The envelope's expiry was not strictly after its issue time.
    InvertedValidity {
        /// The declared issue time (Unix ms).
        issued_at_unix_ms: i64,
        /// The declared expiry (Unix ms).
        expires_at_unix_ms: i64,
    },
    /// The declared update interval was not strictly positive.
    NonPositiveCadence {
        /// The offending value (seconds).
        value: i64,
    },
    /// The declared staleness budget was zero.
    ZeroStalenessBudget,
    /// The received-update timestamp precedes the issue timestamp.
    UpdateBeforeIssue {
        /// The declared issue time (Unix ms).
        issued_at_unix_ms: i64,
        /// The declared update time (Unix ms).
        latest_update_unix_ms: i64,
    },
    /// An `EventTime` fell outside the i64 Unix-ms range this module
    /// evaluates in (the module's declared arithmetic domain).
    TimestampOutOfRange {
        /// The offending value (i128 Unix ms).
        value: i128,
    },
    /// A breach record was constructed for a value inside the bounds.
    NotABreach {
        /// The declared direction.
        direction: &'static str,
        /// The observed power (MW).
        observed_mw: f64,
        /// The declared limit (MW).
        limit_mw: f64,
    },
    /// A curtailment magnitude was not strictly positive.
    NonPositiveCurtailment {
        /// The offending value (MW).
        value: f64,
    },
}

impl std::fmt::Display for DoeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DoeError::NonFiniteValue { field } => {
                write!(f, "DOE field '{field}' must be finite")
            }
            DoeError::NegativeLimit { field, value } => {
                write!(f, "DOE field '{field}' must be non-negative (found {value})")
            }
            DoeError::EmptyField { field } => {
                write!(f, "DOE field '{field}' must be non-empty")
            }
            DoeError::FieldTooLong { field } => {
                write!(f, "DOE field '{field}' exceeds its declared length bound")
            }
            DoeError::EmptyGroup { group } => {
                write!(f, "DOE group '{group}' declares no members")
            }
            DoeError::DuplicateGroupMember { group } => {
                write!(f, "DOE group '{group}' declares a connection point twice")
            }
            DoeError::InvertedValidity { issued_at_unix_ms, expires_at_unix_ms } => {
                write!(
                    f,
                    "DOE expiry ({expires_at_unix_ms}) must be strictly after issue ({issued_at_unix_ms})"
                )
            }
            DoeError::NonPositiveCadence { value } => {
                write!(f, "DOE update cadence must be strictly positive seconds (found {value})")
            }
            DoeError::ZeroStalenessBudget => {
                write!(f, "DOE staleness budget must be at least one update interval")
            }
            DoeError::UpdateBeforeIssue { issued_at_unix_ms, latest_update_unix_ms } => {
                write!(
                    f,
                    "DOE update ({latest_update_unix_ms}) must not precede issue ({issued_at_unix_ms})"
                )
            }
            DoeError::TimestampOutOfRange { value } => {
                write!(f, "DOE evaluation instant {value} is outside the module's Unix-ms domain")
            }
            DoeError::NotABreach { direction, observed_mw, limit_mw } => {
                write!(
                    f,
                    "{direction} breach record does not breach: observed {observed_mw} MW against limit {limit_mw} MW"
                )
            }
            DoeError::NonPositiveCurtailment { value } => {
                write!(f, "curtailment magnitude must be strictly positive MW (found {value})")
            }
        }
    }
}

impl std::error::Error for DoeError {}

#[cfg(test)]
mod tests {
    use super::*;

    const POINT_A: &str = "cp-nmi-0001";
    const POINT_B: &str = "cp-nmi-0002";

    fn point_a() -> ConnectionPointId {
        ConnectionPointId::new(POINT_A.to_string()).expect("valid id")
    }

    fn point_b() -> ConnectionPointId {
        ConnectionPointId::new(POINT_B.to_string()).expect("valid id")
    }

    fn static_limit() -> StaticConnectionLimit {
        StaticConnectionLimit::new(
            point_a(),
            ImportExportLimits::new(5.0, 3.0).expect("valid limits"),
            "connection agreement (declared data)".to_string(),
            Provenance::Configured,
        )
        .expect("valid static limit")
    }

    fn envelope(cadence_seconds: i64, issued_ms: i64, expires_ms: i64) -> DynamicOperatingEnvelope {
        DynamicOperatingEnvelope::new(
            "doe-1".to_string(),
            EnvelopeScope::Single(point_a()),
            ImportExportLimits::new(2.0, 1.0).expect("valid limits"),
            issued_ms,
            expires_ms,
            UpdateCadence {
                expected_interval_seconds: cadence_seconds,
                max_intervals_before_stale: 2,
            },
            issued_ms,
            "dnsp-operator (declared)".to_string(),
            Provenance::Configured,
            "operator-issued envelope (UNVERIFIED semantics pending the primary-source gate)"
                .to_string(),
        )
        .expect("valid envelope")
    }

    fn event(ms: i64) -> EventTime {
        EventTime::from_unix_millis(ms).expect("valid event time")
    }

    // -- lapse tests --------------------------------------------------------

    #[test]
    fn expired_envelope_lapses_to_the_static_limit() {
        // The SAPN rule [S-159]: past expiry, the dynamic limits no longer
        // govern — the connection point reverts to the static limit.
        let envelope = envelope(60, 1_000, 2_000);
        let fallback = static_limit();
        let effective = evaluate_envelope(&envelope, &fallback, &event(2_000))
            .expect("evaluation");
        assert_eq!(
            effective,
            EffectiveEnvelope::Lapsed {
                reason: LapseReason::Expired,
                fallback: ImportExportLimits::new(5.0, 3.0).expect("valid"),
            }
        );
        // And the bounds are the STATIC bounds, not the dynamic ones.
        let (p_min, p_max) = effective_envelope_bounds_mw(&effective);
        assert_eq!(p_min, -5.0);
        assert_eq!(p_max, 3.0);
    }

    #[test]
    fn not_yet_issued_envelope_lapses_to_the_static_limit() {
        let envelope = envelope(60, 5_000, 6_000);
        let fallback = static_limit();
        let effective = evaluate_envelope(&envelope, &fallback, &event(4_999))
            .expect("evaluation");
        assert_eq!(
            effective,
            EffectiveEnvelope::Lapsed {
                reason: LapseReason::NotYetInEffect,
                fallback: ImportExportLimits::new(5.0, 3.0).expect("valid"),
            }
        );
    }

    #[test]
    fn stale_envelope_within_its_window_lapses_to_the_static_limit() {
        // The EDGE Workstream 3 subject [S-158]: cadence 60 s, budget 2
        // intervals → freshness limit 120 s. An update received at issue;
        // at 1 ms past the 120 s budget (issue + 120.001 s) the envelope
        // is stale INSIDE its window and lapses to the static limit.
        let envelope = envelope(60, 1_000, 600_000);
        let fallback = static_limit();
        // Inside the budget: dynamic limits govern.
        let effective = evaluate_envelope(&envelope, &fallback, &event(120_000))
            .expect("evaluation");
        assert_eq!(
            effective,
            EffectiveEnvelope::Dynamic {
                limits: ImportExportLimits::new(2.0, 1.0).expect("valid"),
            }
        );
        // One millisecond past the budget: stale, static governs.
        let effective = evaluate_envelope(&envelope, &fallback, &event(121_001))
            .expect("evaluation");
        assert_eq!(
            effective,
            EffectiveEnvelope::Lapsed {
                reason: LapseReason::Stale,
                fallback: ImportExportLimits::new(5.0, 3.0).expect("valid"),
            }
        );
    }

    #[test]
    fn not_yet_in_effect_outranks_expired_outranks_stale() {
        // An envelope evaluated before issue NEVER reports stale, even
        // though its update age would be huge — the reason precedence is
        // deterministic.
        let envelope = envelope(1, 10_000, 20_000);
        let fallback = static_limit();
        let effective = evaluate_envelope(&envelope, &fallback, &event(500))
            .expect("evaluation");
        assert_eq!(
            effective,
            EffectiveEnvelope::Lapsed {
                reason: LapseReason::NotYetInEffect,
                fallback: fallback.limits,
            }
        );
    }

    // -- bounds / sign tests -------------------------------------------------

    #[test]
    fn dynamic_bounds_follow_the_export_sign_rule() {
        // P > 0 = export: bounds are [-import, +export].
        let envelope = envelope(60, 1_000, 2_000);
        let fallback = static_limit();
        let effective = evaluate_envelope(&envelope, &fallback, &event(1_000))
            .expect("evaluation");
        let (p_min, p_max) = effective_envelope_bounds_mw(&effective);
        assert_eq!(p_min, -2.0, "import limit bounds consumption (negative P)");
        assert_eq!(p_max, 1.0, "export limit bounds export (positive P)");
    }

    #[test]
    fn zero_limits_are_meaningful_bounds() {
        // Import=0 forbids import; export=0 forbids export. Zeros are not
        // defaults and not "unconstrained" — the P3.02 spirit applied to
        // DOE bounds.
        let limits = ImportExportLimits::new(0.0, 0.0).expect("valid limits");
        assert_eq!(limits.import_limit_mw, 0.0);
        assert_eq!(limits.export_limit_mw, 0.0);
    }

    // -- construction-gate tests ----------------------------------------------

    #[test]
    fn envelope_construction_refuses_degenerate_declarations() {
        let good_limits = ImportExportLimits::new(2.0, 1.0).expect("valid");
        let cadence = UpdateCadence { expected_interval_seconds: 60, max_intervals_before_stale: 2 };
        // Inverted validity.
        assert!(matches!(
            DynamicOperatingEnvelope::new(
                "doe-bad".to_string(),
                EnvelopeScope::Single(point_a()),
                good_limits,
                2_000,
                2_000,
                cadence,
                2_000,
                "issuer".to_string(),
                Provenance::Configured,
                "note".to_string(),
            ),
            Err(DoeError::InvertedValidity { .. })
        ));
        // Update before issue.
        assert!(matches!(
            DynamicOperatingEnvelope::new(
                "doe-bad".to_string(),
                EnvelopeScope::Single(point_a()),
                good_limits,
                2_000,
                9_000,
                cadence,
                1_000,
                "issuer".to_string(),
                Provenance::Configured,
                "note".to_string(),
            ),
            Err(DoeError::UpdateBeforeIssue { .. })
        ));
        // Zero cadence and zero staleness budget.
        assert!(matches!(
            DynamicOperatingEnvelope::new(
                "doe-bad".to_string(),
                EnvelopeScope::Single(point_a()),
                good_limits,
                1_000,
                9_000,
                UpdateCadence { expected_interval_seconds: 0, max_intervals_before_stale: 2 },
                1_000,
                "issuer".to_string(),
                Provenance::Configured,
                "note".to_string(),
            ),
            Err(DoeError::NonPositiveCadence { .. })
        ));
        assert!(matches!(
            DynamicOperatingEnvelope::new(
                "doe-bad".to_string(),
                EnvelopeScope::Single(point_a()),
                good_limits,
                1_000,
                9_000,
                UpdateCadence { expected_interval_seconds: 60, max_intervals_before_stale: 0 },
                1_000,
                "issuer".to_string(),
                Provenance::Configured,
                "note".to_string(),
            ),
            Err(DoeError::ZeroStalenessBudget)
        ));
        // Empty group and duplicate members.
        assert!(matches!(
            DynamicOperatingEnvelope::new(
                "doe-bad".to_string(),
                EnvelopeScope::Group { name: "g".to_string(), members: Vec::new() },
                good_limits,
                1_000,
                9_000,
                cadence,
                1_000,
                "issuer".to_string(),
                Provenance::Configured,
                "note".to_string(),
            ),
            Err(DoeError::EmptyGroup { .. })
        ));
        assert!(matches!(
            DynamicOperatingEnvelope::new(
                "doe-bad".to_string(),
                EnvelopeScope::Group {
                    name: "g".to_string(),
                    members: vec![point_a(), point_a()],
                },
                good_limits,
                1_000,
                9_000,
                cadence,
                1_000,
                "issuer".to_string(),
                Provenance::Configured,
                "note".to_string(),
            ),
            Err(DoeError::DuplicateGroupMember { .. })
        ));
        // Negative limits.
        assert!(matches!(
            ImportExportLimits::new(-1.0, 1.0),
            Err(DoeError::NegativeLimit { .. })
        ));
    }

    #[test]
    fn group_scope_covers_and_lists_deterministically() {
        let scope = EnvelopeScope::Group {
            name: "feeder-7".to_string(),
            members: vec![point_b(), point_a()],
        };
        assert!(scope.covers(&point_a()));
        assert!(scope.covers(&point_b()));
        let listed = scope.connection_points();
        // Sorted, deterministic listing.
        assert_eq!(listed[0].as_str(), POINT_A);
        assert_eq!(listed[1].as_str(), POINT_B);
        assert_eq!(listed.len(), 2);
        assert!(!scope.covers(&ConnectionPointId::new("cp-other".to_string()).expect("valid")));
    }

    // -- breach / curtailment telemetry tests -----------------------------------

    #[test]
    fn breach_records_require_an_actual_breach() {
        // Export breach: observed +4 MW against a 3 MW export limit.
        let record = EnvelopeBreachRecord::new(
            Some("doe-1".to_string()),
            point_a(),
            BreachDirection::Export,
            ActivePowerMW::new(4.0).expect("valid power"),
            3.0,
            10_000,
            None,
        )
        .expect("valid breach");
        assert_eq!(record.direction.name(), "export");
        // Import breach: observed -6 MW against a 5 MW import limit.
        let record = EnvelopeBreachRecord::new(
            None,
            point_a(),
            BreachDirection::Import,
            ActivePowerMW::new(-6.0).expect("valid power"),
            5.0,
            10_000,
            None,
        )
        .expect("valid breach (static-limit breach: no envelope id)");
        assert!(record.envelope_id.is_none());
        // Inside the bounds: a named refusal, never a spurious record.
        assert!(matches!(
            EnvelopeBreachRecord::new(
                Some("doe-1".to_string()),
                point_a(),
                BreachDirection::Export,
                ActivePowerMW::new(2.0).expect("valid power"),
                3.0,
                10_000,
                None,
            ),
            Err(DoeError::NotABreach { .. })
        ));
    }

    #[test]
    fn curtailment_records_require_positive_magnitude() {
        let record = CurtailmentRecord::new(
            Some("doe-1".to_string()),
            point_a(),
            0.4,
            BreachDirection::Export,
            10_000,
            None,
        )
        .expect("valid curtailment");
        assert_eq!(record.curtailed_mw, 0.4);
        assert!(matches!(
            CurtailmentRecord::new(
                Some("doe-1".to_string()),
                point_a(),
                0.0,
                BreachDirection::Export,
                10_000,
                None,
            ),
            Err(DoeError::NonPositiveCurtailment { .. })
        ));
        assert!(matches!(
            CurtailmentRecord::new(
                Some("doe-1".to_string()),
                point_a(),
                f64::NAN,
                BreachDirection::Import,
                10_000,
                None,
            ),
            Err(DoeError::NonPositiveCurtailment { .. })
        ));
    }
}
