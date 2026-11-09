// P3.02 — Capability provenance and unknown-state handling (pure rules, no I/O).
//
// Implements ledger P3.02 ("Capability provenance and unknown-state handling",
// report §11.1, §10.4, §17.5 invariant 9). Decision record:
// `ledger/phase-03-flexibility-scheduling/01_04_capability_contract.md`.
//
// THE RULE, DECIDED ONCE (this module is its single implementation):
//
//   **An unknown-provenance capability NARROWS the feasible set to a
//   documented conservative default per asset class. Unknown NEVER widens.**
//
// - Effective bound = declared bound ∩ conservative default for
//   (asset class, constraint kind). If the intersection is empty the
//   dimension is INFEASIBLE (empty) — it is never repaired by widening back
//   to the declared bound.
// - `MEASURED`, `CONFIGURED`, and `INFERRED` provenance use the declared
//   bounds as declared. Recorded decision: configured values come from a
//   named configuration source (registry, device profile, nameplate) and
//   inferred values from a declared derivation rule (§11.1 requires adapters
//   to state which constraints are which, and all three are visible states
//   downstream); only UNKNOWN narrows.
// - The conservative defaults per asset class are the table in
//   [`conservative_default`]; the recorded fallback for a (class, kind) pair
//   without an explicit table entry is **collapse-to-zero** in the
//   constraint's declared unit — the most conservative reading, never a
//   silent "unconstrained". P3.05–P3.11 state richer per-asset defaults on
//   top of this table; over-restriction is acceptable in simulation and is
//   recorded as a limitation, not fixed by widening (ledger risk note).
// - Discrete declarations narrow too: an unknown numeric value set is
//   filtered to the values inside the conservative default interval, and an
//   unknown mode set credits **no modes** (empty) — mode credit requires a
//   known provenance.
//
// Provenance is PROPAGATED, not consumed: envelope outputs carry
// [`EnvelopeProvenance`] (the P3.12 envelope consumes it) and schedule
// records carry [`ScheduleProvenanceEntry`], so a downstream reader can see
// which limits were measured, configured, inferred, or unknown (ledger P3.02
// step 3). A BAD/MISSING quality flag is never silently treated as GOOD
// (§10.4); likewise an UNKNOWN capability is never silently treated as
// unconstrained (§17.5 invariant 9).
//
// The provenance ENUM itself is part of the versioned wire schema and lives
// in `flexcore_contracts::capability::Provenance` (P3.01); this module owns
// only the narrowing RULE and the propagation types.
//
// Control status: pure domain rules for the simulation-only 0.x line. No I/O,
// no solver coupling, no field-write path.

use flexcore_contracts::capability::{
    CAPABILITY_SCHEMA_VERSION, ConstraintKind, ConstraintUnit, Provenance,
};

use crate::ids::AssetId;

/// Errors returned by the P3.02 provenance rule constructors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceRuleError {
    /// A declared bound was NaN or infinite; declared capability bounds must
    /// be finite.
    NonFiniteBound,
    /// A declared interval was inverted (lower > upper).
    InvertedBound,
    /// A schedule provenance entry declared a capability schema version this
    /// domain does not support (only the frozen P3.01 version is accepted).
    SchemaVersionMismatch {
        /// The version found on the record.
        found: u32,
        /// The supported version.
        supported: u32,
    },
}

impl std::fmt::Display for ProvenanceRuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProvenanceRuleError::NonFiniteBound => {
                write!(f, "declared capability bounds must be finite")
            }
            ProvenanceRuleError::InvertedBound => {
                write!(f, "declared capability interval must satisfy lower <= upper")
            }
            ProvenanceRuleError::SchemaVersionMismatch { found, supported } => write!(
                f,
                "capability schema version {found} is not supported (supported version: {supported})"
            ),
        }
    }
}

impl std::error::Error for ProvenanceRuleError {}

// --- Asset classes ----------------------------------------------------------

/// The four asset classes modelled in Phase 3 (report §11.2–§11.5), behind
/// the generic §11.1 contract. The conservative-default table is keyed per
/// class so later phases can diverge the defaults without changing the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetClass {
    /// Battery energy storage system (report §11.2).
    Bess,
    /// Inverter P-Q envelope (report §11.3).
    InverterPq,
    /// Flexible load (report §11.4).
    FlexibleLoad,
    /// EV fleet (report §11.5).
    EvFleet,
}

impl AssetClass {
    /// All four Phase 3 asset classes.
    pub const ALL: [AssetClass; 4] = [
        AssetClass::Bess,
        AssetClass::InverterPq,
        AssetClass::FlexibleLoad,
        AssetClass::EvFleet,
    ];

    /// The canonical name from the frozen vocabulary.
    pub const fn name(self) -> &'static str {
        match self {
            AssetClass::Bess => "BESS",
            AssetClass::InverterPq => "INVERTER_PQ",
            AssetClass::FlexibleLoad => "FLEXIBLE_LOAD",
            AssetClass::EvFleet => "EV_FLEET",
        }
    }
}

// --- Bound intervals and narrowed sets --------------------------------------

/// A declared closed interval bound (`None` side = unbounded on that side).
/// `lower ≤ upper` holds by construction whenever both sides are present, so
/// a `BoundInterval` always contains at least one value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundInterval {
    /// Inclusive lower bound, if declared.
    pub lower: Option<f64>,
    /// Inclusive upper bound, if declared.
    pub upper: Option<f64>,
}

impl BoundInterval {
    /// Constructs and validates a declared bound (finite, ordered).
    pub fn new(lower: Option<f64>, upper: Option<f64>) -> Result<Self, ProvenanceRuleError> {
        if let Some(l) = lower {
            if !l.is_finite() {
                return Err(ProvenanceRuleError::NonFiniteBound);
            }
        }
        if let Some(u) = upper {
            if !u.is_finite() {
                return Err(ProvenanceRuleError::NonFiniteBound);
            }
        }
        if let (Some(l), Some(u)) = (lower, upper) {
            if l > u {
                return Err(ProvenanceRuleError::InvertedBound);
            }
        }
        Ok(Self { lower, upper })
    }

    /// The intersection of two closed intervals; `None` if empty.
    pub fn intersect(&self, other: &BoundInterval) -> Option<BoundInterval> {
        let lower = match (self.lower, other.lower) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        let upper = match (self.upper, other.upper) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };
        if let (Some(l), Some(u)) = (lower, upper) {
            if l > u {
                return None;
            }
        }
        Some(BoundInterval { lower, upper })
    }
}

/// The feasible set of one constrained dimension after the P3.02 rule is
/// applied.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NarrowedSet {
    /// A closed interval of allowed values (possibly a single point; `None`
    /// side = unbounded on that side).
    Interval {
        /// Inclusive lower bound, if any.
        lower: Option<f64>,
        /// Inclusive upper bound, if any.
        upper: Option<f64>,
    },
    /// The narrowing made the dimension infeasible: no value satisfies both
    /// the declared bounds and the conservative default. Never repaired by
    /// widening.
    Empty,
}

/// Subset predicate: every value allowed by `narrowed` is also allowed by
/// `declared`. This is the machine-checked form of "unknown never widens"
/// (relative to the declared bounds).
pub fn narrowing_never_widens(declared: &BoundInterval, narrowed: &NarrowedSet) -> bool {
    match *narrowed {
        NarrowedSet::Empty => true,
        NarrowedSet::Interval { lower, upper } => {
            let lower_ok = match (lower, declared.lower) {
                (Some(n), Some(d)) => n >= d,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => true,
            };
            let upper_ok = match (upper, declared.upper) {
                (Some(n), Some(d)) => n <= d,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => true,
            };
            let ordered = match (lower, upper) {
                (Some(l), Some(u)) => l <= u,
                _ => true,
            };
            lower_ok && upper_ok && ordered
        }
    }
}

/// Strict-narrowing predicate: `narrowed` is a subset of `declared` and not
/// equal to it. Used by the P3.02 acceptance test (all-unknown limits give a
/// strictly smaller feasible set than measured limits).
pub fn strictly_narrower(declared: &BoundInterval, narrowed: &NarrowedSet) -> bool {
    if !narrowing_never_widens(declared, narrowed) {
        return false;
    }
    match *narrowed {
        NarrowedSet::Empty => true,
        NarrowedSet::Interval { lower, upper } => {
            let lower_strict = match (lower, declared.lower) {
                (Some(n), Some(d)) => n > d,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => false,
            };
            let upper_strict = match (upper, declared.upper) {
                (Some(n), Some(d)) => n < d,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                (None, None) => false,
            };
            lower_strict || upper_strict
        }
    }
}

// --- The conservative-default table -----------------------------------------

/// How an unknown-provenance bound is narrowed for one (asset class, kind).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnknownNarrowing {
    /// Intersect the declared bound with this closed interval.
    ClampTo {
        /// Inclusive default lower bound, if any.
        lower: Option<f64>,
        /// Inclusive default upper bound, if any.
        upper: Option<f64>,
    },
}

/// The recorded conservative-default table (P3.02 step 2; the full table and
/// its rationale are in the decision record).
///
/// Explicit entries, per asset class (identical values initially, recorded
/// per class so P3.05–P3.11 can diverge them without changing the rule):
///
/// | Class | Kind | Unknown default | Reading |
/// |---|---|---|---|
/// | all four | `RAMP_RATE` | [0, 0] MW/min | no ramp flexibility is credited; power must hold constant between intervals until ramp provenance improves |
/// | all four | `TEMPERATURE_HEALTH` | [0, 0] | no temperature/health-dependent capability credit (a declared MW derate or availability factor narrows to zero credit) |
///
/// The recorded FALLBACK for any (class, kind) pair without an explicit
/// entry is collapse-to-zero: `ClampTo { lower: Some(0.0), upper: Some(0.0) }`
/// in the constraint's declared unit. An unknown capability is never silently
/// treated as unconstrained (§17.5 invariant 9).
pub fn conservative_default(class: AssetClass, kind: ConstraintKind) -> UnknownNarrowing {
    let _ = class; // defaults are per-class rows; identical values today, divergence-ready
    match kind {
        ConstraintKind::RampRate | ConstraintKind::TemperatureHealth => {
            UnknownNarrowing::ClampTo { lower: Some(0.0), upper: Some(0.0) }
        }
        // Recorded fallback: collapse-to-zero (never "unconstrained").
        _ => UnknownNarrowing::ClampTo { lower: Some(0.0), upper: Some(0.0) },
    }
}

fn default_interval(class: AssetClass, kind: ConstraintKind) -> BoundInterval {
    match conservative_default(class, kind) {
        UnknownNarrowing::ClampTo { lower, upper } => {
            BoundInterval { lower, upper }
        }
    }
}

// --- The rule ---------------------------------------------------------------

/// Applies the P3.02 rule to one interval-bounded constraint dimension.
///
/// - measured / configured / inferred: the declared bounds, as declared;
/// - unknown: the declared bounds ∩ the conservative default for
///   (class, kind); empty intersection → [`NarrowedSet::Empty`].
pub fn narrow_interval(
    class: AssetClass,
    kind: ConstraintKind,
    provenance: Provenance,
    declared: BoundInterval,
) -> NarrowedSet {
    match provenance {
        Provenance::Measured | Provenance::Configured | Provenance::Inferred => {
            NarrowedSet::Interval { lower: declared.lower, upper: declared.upper }
        }
        Provenance::Unknown => match declared.intersect(&default_interval(class, kind)) {
            Some(intersection) => NarrowedSet::Interval {
                lower: intersection.lower,
                upper: intersection.upper,
            },
            None => NarrowedSet::Empty,
        },
    }
}

/// Applies the P3.02 rule to a declared numeric value set: unknown provenance
/// keeps only the values inside the conservative default interval (order
/// preserved, possibly empty); known provenance keeps all declared values.
pub fn narrow_value_set(
    class: AssetClass,
    kind: ConstraintKind,
    provenance: Provenance,
    values: &[f64],
) -> Vec<f64> {
    match provenance {
        Provenance::Measured | Provenance::Configured | Provenance::Inferred => values.to_vec(),
        Provenance::Unknown => {
            let default = default_interval(class, kind);
            values
                .iter()
                .copied()
                .filter(|v| {
                    let lower_ok = match default.lower {
                        Some(l) => *v >= l,
                        None => true,
                    };
                    let upper_ok = match default.upper {
                        Some(u) => *v <= u,
                        None => true,
                    };
                    lower_ok && upper_ok
                })
                .collect()
        }
    }
}

/// Applies the P3.02 rule to a declared mode set (`u`, §11.1): unknown
/// provenance credits **no modes** (empty) — mode credit requires a known
/// provenance; known provenance keeps all declared modes.
pub fn narrow_allowed_modes(provenance: Provenance, declared: &[String]) -> Vec<String> {
    match provenance {
        Provenance::Measured | Provenance::Configured | Provenance::Inferred => declared.to_vec(),
        Provenance::Unknown => Vec::new(),
    }
}

// --- Provenance propagation types -------------------------------------------

/// One constrained dimension with its provenance and the effective bound
/// after the P3.02 rule. This is the unit of provenance visibility in
/// envelope outputs (P3.12) and schedule records (P3.21/P3.22).
#[derive(Debug, Clone, PartialEq)]
pub struct ProvenancedLimit {
    /// The asset class the narrowing was applied for.
    pub asset_class: AssetClass,
    /// The constraint kind.
    pub kind: ConstraintKind,
    /// The canonical unit of the bounds.
    pub unit: ConstraintUnit,
    /// The declared provenance.
    pub provenance: Provenance,
    /// The declared bound.
    pub declared: BoundInterval,
    /// The effective bound after the P3.02 rule.
    pub effective: NarrowedSet,
}

impl ProvenancedLimit {
    /// Constructs a provenance-carrying limit and applies the P3.02 rule.
    pub fn new(
        asset_class: AssetClass,
        kind: ConstraintKind,
        unit: ConstraintUnit,
        provenance: Provenance,
        declared: BoundInterval,
    ) -> Self {
        let effective = narrow_interval(asset_class, kind, provenance, declared);
        Self {
            asset_class,
            kind,
            unit,
            provenance,
            declared,
            effective,
        }
    }
}

/// Per-asset provenance counts over a set of [`ProvenancedLimit`] values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProvenanceSummary {
    /// Number of measured limits.
    pub measured: usize,
    /// Number of configured limits.
    pub configured: usize,
    /// Number of inferred limits.
    pub inferred: usize,
    /// Number of unknown limits (each of these was narrowed; none was
    /// silently treated as unconstrained).
    pub unknown: usize,
}

impl ProvenanceSummary {
    /// Total number of limits counted.
    pub fn total(&self) -> usize {
        self.measured + self.configured + self.inferred + self.unknown
    }

    /// True when every limit carries a known provenance (no UNKNOWN rows).
    pub fn is_fully_known(&self) -> bool {
        self.unknown == 0
    }
}

/// The provenance record embedded in a flexibility-envelope output (ledger
/// P3.02 step 3; the P3.12 envelope consumes it): which limits were inferred
/// or unknown, and what the P3.02 rule did to each of them.
#[derive(Debug, Clone, PartialEq)]
pub struct EnvelopeProvenance {
    /// The asset class the envelope was computed for.
    pub asset_class: AssetClass,
    /// One row per constrained dimension.
    pub limits: Vec<ProvenancedLimit>,
    /// The provenance counts over `limits`.
    pub summary: ProvenanceSummary,
}

impl EnvelopeProvenance {
    /// Constructs the envelope provenance record and computes the summary.
    pub fn new(asset_class: AssetClass, limits: Vec<ProvenancedLimit>) -> Self {
        let mut summary = ProvenanceSummary::default();
        for limit in &limits {
            match limit.provenance {
                Provenance::Measured => summary.measured += 1,
                Provenance::Configured => summary.configured += 1,
                Provenance::Inferred => summary.inferred += 1,
                Provenance::Unknown => summary.unknown += 1,
            }
        }
        Self {
            asset_class,
            limits,
            summary,
        }
    }
}

/// The provenance record embedded in a schedule record (ledger P3.02 step 3):
/// per asset, the capability schema version the limits came from and the
/// envelope provenance, so a downstream reader can see which limits were
/// inferred or unknown when the schedule was produced.
#[derive(Debug, Clone, PartialEq)]
pub struct ScheduleProvenanceEntry {
    /// The asset the entry describes.
    pub asset_id: AssetId,
    /// The capability schema version of the model the limits came from.
    pub capability_schema_version: u32,
    /// The envelope provenance for the asset.
    pub envelope: EnvelopeProvenance,
}

impl ScheduleProvenanceEntry {
    /// Constructs a schedule provenance entry, rejecting a capability schema
    /// version this domain does not support (P3.01 versioning, §10.5).
    pub fn new(
        asset_id: AssetId,
        capability_schema_version: u32,
        envelope: EnvelopeProvenance,
    ) -> Result<Self, ProvenanceRuleError> {
        if capability_schema_version != CAPABILITY_SCHEMA_VERSION {
            return Err(ProvenanceRuleError::SchemaVersionMismatch {
                found: capability_schema_version,
                supported: CAPABILITY_SCHEMA_VERSION,
            });
        }
        Ok(Self {
            asset_id,
            capability_schema_version,
            envelope,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flexcore_contracts::ids::FlexUuid;

    fn interval(lower: f64, upper: f64) -> BoundInterval {
        BoundInterval::new(Some(lower), Some(upper)).unwrap()
    }

    // --- table and fallback ---------------------------------------------

    #[test]
    fn conservative_defaults_are_recorded_per_asset_class() {
        // The two explicitly recorded rows: ramp and temperature/health for
        // every Phase 3 asset class collapse to zero credit.
        for class in AssetClass::ALL {
            for kind in [ConstraintKind::RampRate, ConstraintKind::TemperatureHealth] {
                assert_eq!(
                    conservative_default(class, kind),
                    UnknownNarrowing::ClampTo { lower: Some(0.0), upper: Some(0.0) },
                    "recorded default for {}/{}/{}",
                    class.name(),
                    kind.name(),
                    "unknown"
                );
            }
        }
    }

    #[test]
    fn fallback_for_unlisted_pairs_is_collapse_to_zero() {
        // Absent an explicit entry, the recorded fallback is collapse-to-zero
        // — never "unconstrained" (§17.5 invariant 9).
        for class in AssetClass::ALL {
            assert_eq!(
                conservative_default(class, ConstraintKind::ActivePower),
                UnknownNarrowing::ClampTo { lower: Some(0.0), upper: Some(0.0) }
            );
        }
    }

    // --- the rule -------------------------------------------------------

    #[test]
    fn measured_configured_and_inferred_use_declared_bounds_as_declared() {
        for provenance in [Provenance::Measured, Provenance::Configured, Provenance::Inferred] {
            for class in AssetClass::ALL {
                let narrowed = narrow_interval(
                    class,
                    ConstraintKind::ActivePower,
                    provenance,
                    interval(-2.0, 3.0),
                );
                assert_eq!(
                    narrowed,
                    NarrowedSet::Interval { lower: Some(-2.0), upper: Some(3.0) },
                    "{} must not narrow a {}/{} dimension",
                    provenance.name(),
                    class.name(),
                    "ACTIVE_POWER"
                );
            }
        }
    }

    #[test]
    fn unknown_narrows_ramp_to_zero_for_every_asset_class() {
        // Measured ramp [0, 5] MW/min stays [0, 5]; the same declaration with
        // unknown provenance narrows to [0, 0] — strictly smaller (P3.02
        // acceptance, per asset class).
        for class in AssetClass::ALL {
            let measured = narrow_interval(
                class,
                ConstraintKind::RampRate,
                Provenance::Measured,
                interval(0.0, 5.0),
            );
            assert_eq!(
                measured,
                NarrowedSet::Interval { lower: Some(0.0), upper: Some(5.0) }
            );
            let unknown = narrow_interval(
                class,
                ConstraintKind::RampRate,
                Provenance::Unknown,
                interval(0.0, 5.0),
            );
            assert_eq!(unknown, NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) });
            assert!(
                strictly_narrower(&interval(0.0, 5.0), &unknown),
                "unknown ramp must be strictly smaller than measured for {}",
                class.name()
            );
        }
    }

    #[test]
    fn unknown_narrows_temperature_health_to_zero_credit_for_every_asset_class() {
        // A measured temperature/health availability factor [0, 0.8] stays as
        // declared; unknown narrows to zero credit (P3.02 acceptance).
        for class in AssetClass::ALL {
            let measured = narrow_interval(
                class,
                ConstraintKind::TemperatureHealth,
                Provenance::Measured,
                interval(0.0, 0.8),
            );
            assert_eq!(
                measured,
                NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.8) }
            );
            let unknown = narrow_interval(
                class,
                ConstraintKind::TemperatureHealth,
                Provenance::Unknown,
                interval(0.0, 0.8),
            );
            assert_eq!(unknown, NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) });
            assert!(strictly_narrower(&interval(0.0, 0.8), &unknown));
        }
    }

    #[test]
    fn empty_intersection_is_infeasible_never_widened_back() {
        // A declared ramp *minimum* of 2 MW/min does not contain the default
        // [0, 0]: the intersection is empty and the dimension is infeasible —
        // never repaired by widening back to the declared bound.
        let narrowed = narrow_interval(
            AssetClass::Bess,
            ConstraintKind::RampRate,
            Provenance::Unknown,
            interval(2.0, 5.0),
        );
        assert_eq!(narrowed, NarrowedSet::Empty);
        assert!(narrowing_never_widens(&interval(2.0, 5.0), &narrowed));
        assert!(strictly_narrower(&interval(2.0, 5.0), &narrowed));
    }

    #[test]
    fn value_sets_filter_against_the_default_for_unknown_provenance() {
        let values = vec![0.0, 1.0, 2.0, 3.0];
        assert_eq!(
            narrow_value_set(
                AssetClass::Bess,
                ConstraintKind::DeviceMode,
                Provenance::Measured,
                &values
            ),
            values
        );
        // Unknown: only values inside the recorded default [0, 0] survive.
        assert_eq!(
            narrow_value_set(
                AssetClass::Bess,
                ConstraintKind::DeviceMode,
                Provenance::Unknown,
                &values
            ),
            vec![0.0]
        );
    }

    #[test]
    fn mode_sets_credit_no_modes_for_unknown_provenance() {
        let modes = vec!["CHARGE".to_string(), "DISCHARGE".to_string()];
        assert_eq!(
            narrow_allowed_modes(Provenance::Configured, &modes),
            modes,
            "known provenance keeps all declared modes"
        );
        assert!(
            narrow_allowed_modes(Provenance::Unknown, &modes).is_empty(),
            "mode credit requires a known provenance (u is declared, never assumed, §11.1)"
        );
    }

    // --- never-widens property loop over the fixture matrix ---------------

    #[test]
    fn property_unknown_never_widens_for_any_declared_bound_or_class() {
        // Fixture matrix of declared bounds (including bounds that exclude 0
        // and one-sided bounds) over every class, kind, and provenance: the
        // narrowed set is always a subset of the declared bound.
        let declared_fixtures = [
            interval(0.0, 5.0),
            interval(-2.0, 3.0),
            interval(2.0, 5.0),
            interval(-4.0, -1.0),
            BoundInterval::new(Some(0.0), None).unwrap(),
            BoundInterval::new(None, Some(1.5)).unwrap(),
            interval(0.0, 0.0),
        ];
        for class in AssetClass::ALL {
            for kind in ConstraintKind::ALL {
                for provenance in Provenance::ALL {
                    for declared in &declared_fixtures {
                        let narrowed = narrow_interval(class, kind, provenance, *declared);
                        assert!(
                            narrowing_never_widens(declared, &narrowed),
                            "{}/{}/{}: narrowed set must never exceed the declared bound",
                            class.name(),
                            kind.name(),
                            provenance.name()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn property_unknown_is_always_within_the_recorded_default() {
        // For unknown provenance the narrowed set must additionally be a
        // subset of the conservative default for the (class, kind) pair.
        let default = interval(0.0, 0.0);
        let declared_fixtures = [interval(0.0, 5.0), interval(-2.0, 3.0), interval(2.0, 5.0)];
        for class in AssetClass::ALL {
            for kind in ConstraintKind::ALL {
                for declared in &declared_fixtures {
                    let narrowed = narrow_interval(class, kind, Provenance::Unknown, *declared);
                    assert!(
                        narrowing_never_widens(&default, &narrowed),
                        "{}/{}: unknown must narrow to within the recorded default",
                        class.name(),
                        kind.name()
                    );
                }
            }
        }
    }

    // --- bound interval helpers -----------------------------------------

    #[test]
    fn bound_interval_rejects_non_finite_and_inverted_bounds() {
        assert_eq!(
            BoundInterval::new(Some(f64::NAN), Some(1.0)),
            Err(ProvenanceRuleError::NonFiniteBound)
        );
        assert_eq!(
            BoundInterval::new(Some(0.0), Some(f64::INFINITY)),
            Err(ProvenanceRuleError::NonFiniteBound)
        );
        assert_eq!(
            BoundInterval::new(Some(2.0), Some(1.0)),
            Err(ProvenanceRuleError::InvertedBound)
        );
        // Equal bounds are a legal single point.
        assert!(BoundInterval::new(Some(1.0), Some(1.0)).is_ok());
        // Unbounded sides are legal.
        assert!(BoundInterval::new(None, None).is_ok());
    }

    #[test]
    fn intersect_computes_closed_interval_intersections() {
        let a = interval(0.0, 5.0);
        let b = interval(3.0, 8.0);
        assert_eq!(a.intersect(&b), Some(interval(3.0, 5.0)));
        // Disjoint: empty.
        assert_eq!(interval(0.0, 1.0).intersect(&interval(2.0, 3.0)), None);
        // With an unbounded side.
        assert_eq!(
            interval(0.0, 5.0).intersect(&BoundInterval::new(None, Some(2.0)).unwrap()),
            Some(interval(0.0, 2.0))
        );
    }

    #[test]
    fn strictly_narrower_distinguishes_subset_from_equal() {
        let declared = interval(0.0, 5.0);
        assert!(strictly_narrower(&declared, &NarrowedSet::Interval { lower: Some(0.0), upper: Some(3.0) }));
        assert!(strictly_narrower(&declared, &NarrowedSet::Empty));
        assert!(!strictly_narrower(
            &declared,
            &NarrowedSet::Interval { lower: Some(0.0), upper: Some(5.0) }
        ));
        // Not a subset at all → not strictly narrower.
        assert!(!strictly_narrower(
            &declared,
            &NarrowedSet::Interval { lower: Some(-1.0), upper: Some(5.0) }
        ));
    }

    // --- propagation ----------------------------------------------------

    #[test]
    fn envelope_provenance_carries_per_limit_visibility_and_summary() {
        let limits = vec![
            ProvenancedLimit::new(
                AssetClass::Bess,
                ConstraintKind::ActivePower,
                ConstraintUnit::Megawatt,
                Provenance::Measured,
                interval(-2.0, 3.0),
            ),
            ProvenancedLimit::new(
                AssetClass::Bess,
                ConstraintKind::RampRate,
                ConstraintUnit::MegawattPerMinute,
                Provenance::Unknown,
                interval(0.0, 5.0),
            ),
            ProvenancedLimit::new(
                AssetClass::Bess,
                ConstraintKind::TemperatureHealth,
                ConstraintUnit::Dimensionless,
                Provenance::Inferred,
                interval(0.0, 0.8),
            ),
        ];
        let envelope = EnvelopeProvenance::new(AssetClass::Bess, limits);
        assert_eq!(envelope.summary.total(), 3);
        assert_eq!(envelope.summary.measured, 1);
        assert_eq!(envelope.summary.inferred, 1);
        assert_eq!(envelope.summary.unknown, 1);
        assert!(!envelope.summary.is_fully_known());
        // The unknown row is visible with its narrowed effective bound — the
        // envelope reader can see exactly which limit was unknown and what
        // the rule did to it.
        let unknown_row = envelope
            .limits
            .iter()
            .find(|l| l.provenance == Provenance::Unknown)
            .unwrap();
        assert_eq!(unknown_row.effective, NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) });
    }

    #[test]
    fn schedule_provenance_entry_records_the_schema_version() {
        let envelope = EnvelopeProvenance::new(
            AssetClass::InverterPq,
            vec![ProvenancedLimit::new(
                AssetClass::InverterPq,
                ConstraintKind::ReactivePower,
                ConstraintUnit::Megavar,
                Provenance::Configured,
                interval(-1.0, 1.0),
            )],
        );
        let entry = ScheduleProvenanceEntry::new(
            AssetId::new(FlexUuid::parse("0192a5e0-8c3d-7e2f-9a1b-3c4d5e6f7a8b").unwrap()),
            CAPABILITY_SCHEMA_VERSION,
            envelope,
        )
        .unwrap();
        assert_eq!(entry.capability_schema_version, CAPABILITY_SCHEMA_VERSION);
        assert_eq!(entry.envelope.asset_class, AssetClass::InverterPq);

        // A foreign schema version never enters a schedule record.
        assert_eq!(
            ScheduleProvenanceEntry::new(
                AssetId::new(FlexUuid::NIL),
                CAPABILITY_SCHEMA_VERSION + 1,
                EnvelopeProvenance::new(AssetClass::Bess, vec![]),
            )
            .unwrap_err(),
            ProvenanceRuleError::SchemaVersionMismatch {
                found: CAPABILITY_SCHEMA_VERSION + 1,
                supported: CAPABILITY_SCHEMA_VERSION,
            }
        );
    }
}
