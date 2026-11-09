// P3.16 — Market/PCC tracking: the §12.2 tracking term at the DECLARED
//         point of common coupling (pure rules, no I/O, no solver).
//
// Implements ledger P3.16 ("Market/PCC tracking"), report §12.2 and §10.2.
// Decision record:
// `ledger/phase-03-flexibility-scheduling/14_16_objective_pcc.md`.
//
// THE TRACKING TERM, DECIDED ONCE (report §12.2):
//
//   min_x  Σ_t  λ_track · (P_PCC,t − P_commitment,t)²  + …
//
// - **Tracking happens at the DECLARED PCC, never at the sum of arbitrary
//   bus injections** (§12.2). The PCC identity ([`PccRef`]) is a REQUIRED,
//   VALIDATED field on every tracking point: non-empty, whitespace-free,
//   constructible straight from the Phase 2 site registry
//   ([`PccRef::from_site`] over `Site::point_of_common_coupling`). A
//   tracking point without a validated PCC identity cannot be constructed.
// - **Sign convention** (§10.2, frozen): `P > 0` means export at the PCC;
//   `P < 0` means import. The mismatch is `P_PCC − P_commitment` (positive
//   = over-delivery relative to the commitment); the square makes the cost
//   sign-symmetric, the signed mismatch is recorded per interval for
//   diagnostics.
// - **Modelled losses**: each delivery point declares a loss model
//   ([`LossModel`]). The recorded v0.2.0 model is a net-export fraction:
//   `P_PCC,t = (1 − loss_fraction) · Σ P_asset,i,t`, where the sum runs
//   over the asset injections ATTRIBUTED to that delivery point. This is a
//   declared linearisation of the synthetic baseline, NOT a network power
//   flow (the grid model belongs to Phase 4, §12.4–§12.5) — the
//   import-direction simplification is recorded in [`LOSS_MODEL_NOTE`].
// - **Multiple delivery points stay SEPARATE** (§12.2): every configured
//   point produces its own tracking term with its own λ, loss model,
//   per-interval series, and total ([`PccTrackingEvaluation::points`]).
//   The evaluation's `total` is an explicitly labelled sum of separate
//   terms — the per-point records are the primary output and are never
//   merged into one number. Duplicate PCC identities in one config are
//   rejected (two terms for one identity would be a merge mistake).
// - **Commitments are SYNTHETIC test commitments** ([`SyntheticCommitment`],
//   [`COMMITMENTS_SYNTHETIC_LABEL`]): the type carries the label and the
//   version, and the ONLY access to the values is through the labelled
//   accessor ([`SyntheticCommitment::labeled_values`]) — the label cannot
//   be dropped. D1 is OPEN: no market, no jurisdiction, no market
//   qualification is claimed anywhere in this module (§0.3, §12.2).
//
// Relation to the objective (P3.14): the §12.2 tracking term is a
// separately recorded term composed into the Tier 1 formulation (P3.17).
// It is NOT one of the §12.1 five terms and NOT the foundations §7.5
// "commitment deviation" term (which is excluded from the v0.2.0
// objective — see the `scheduling_objective` module header and the
// decision doc).
//
// Control status: pure rules for the simulation-only 0.x line. No I/O, no
// persistence, no device-write path; a tracking evaluation is a CANDIDATE
// metric, not a delivery or settlement record.

use flexcore_domain::entities::Site;

// --- Recorded constants ------------------------------------------------------

/// The synthetic-commitment provenance label. Commitments in v0.2.0 are
/// SYNTHETIC test commitments (D1 OPEN).
pub const COMMITMENTS_SYNTHETIC_LABEL: &str = "SYNTHETIC";

/// The recorded statement carried on the commitment type and in the
/// schedule record: synthetic test data, no market qualification.
pub const SYNTHETIC_COMMITMENT_NOTE: &str =
    "SYNTHETIC test commitments for the simulation-only v0.2.0 line (D1 \
OPEN: no jurisdiction/market is decided). Not market data, not a market \
product, and no market qualification is claimed.";

/// The tracking-term statement carried in the schedule record (report
/// §12.2).
pub const TRACKING_TERM_STATEMENT: &str =
    "min \u{3a3}_t \u{3bb}_track \u{b7} (P_PCC,t \u{2212} P_commitment,t)\u{b2} \
(+) \u{2026} evaluated at the DECLARED point of common coupling, never the \
sum of arbitrary bus injections (report \u{a7}12.2).";

/// The recorded sign convention of the tracking term (report §10.2).
pub const TRACKING_SIGN_CONVENTION: &str =
    "P > 0 means export at the PCC; P < 0 means import. Mismatch = \
P_PCC - P_commitment; positive = over-delivery relative to the \
commitment. The cost uses the square (sign-symmetric); the signed \
mismatch is recorded per interval for diagnostics.";

/// The recorded loss-model note (ASSUMPTION; see the module header).
pub const LOSS_MODEL_NOTE: &str =
    "ASSUMPTION (recorded simplification): the modelled loss is a declared \
net-export fraction per delivery point, P_PCC = (1 - loss_fraction) x \
\u{3a3} P_asset attributed to that point. For import intervals (net P < 0) \
the fraction reduces the import magnitude, which understates losses on \
import; the network-level loss treatment belongs to the Phase 4 grid \
model (\u{a7}12.4-\u{a7}12.5), which this module does not implement.";

/// The unit of the tracking weight λ (recorded in the schedule record).
pub const LAMBDA_TRACK_UNIT: &str = "currency-units/(MW^2*h)";

// --- Errors ------------------------------------------------------------------

/// Errors returned by PCC-tracking configuration and evaluation. Each
/// variant names the offending field/value (P3.04 error-catalogue style).
#[derive(Debug, Clone, PartialEq)]
pub enum PccTrackingError {
    /// The PCC identity was empty — PCC identity is a REQUIRED field.
    EmptyPccIdentity,
    /// The PCC identity had leading/trailing whitespace; identities are
    /// matched verbatim against the site registry.
    PccIdentityWhitespace,
    /// The commitment version was empty — an unversioned commitment is an
    /// untracked input and is rejected.
    EmptyCommitmentVersion,
    /// The commitment version contained whitespace.
    CommitmentVersionWhitespace,
    /// The λ_track weight was NaN, infinite, or negative (a penalty weight
    /// ≥ 0).
    InvalidLambdaTrack {
        /// The offending value.
        value: f64,
    },
    /// The loss fraction was outside [0, 1).
    InvalidLossFraction {
        /// The offending value.
        value: f64,
    },
    /// The tracking-point list was empty; at least one delivery point is
    /// required.
    EmptyTrackingPoints,
    /// The same PCC identity appeared twice in one configuration or input
    /// list; per-point terms must stay separate, and two entries for one
    /// identity would be a merge mistake.
    DuplicatePcc {
        /// The duplicated identity.
        pcc: String,
    },
    /// A configured point had no commitment series.
    MissingCommitment {
        /// The point's identity.
        pcc: String,
    },
    /// A configured point had no attributed asset schedule.
    MissingSchedule {
        /// The point's identity.
        pcc: String,
    },
    /// The interval length must be positive.
    NonPositiveIntervalSeconds,
    /// A series length did not match the evaluation's interval count.
    SeriesLengthMismatch {
        /// Which point/series.
        pcc: String,
        /// The expected length.
        expected: usize,
        /// The actual length.
        found: usize,
    },
    /// A series value was NaN or infinite.
    NonFiniteSeriesValue {
        /// Which point/series.
        pcc: String,
        /// Which series.
        series: &'static str,
        /// The interval index.
        interval: u32,
        /// The offending value.
        value: f64,
    },
}

impl std::fmt::Display for PccTrackingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PccTrackingError::EmptyPccIdentity => {
                write!(f, "the PCC identity is a required, validated field and must be non-empty")
            }
            PccTrackingError::PccIdentityWhitespace => write!(
                f,
                "the PCC identity must not have leading/trailing whitespace (identities are \
                 matched verbatim against the site registry)"
            ),
            PccTrackingError::EmptyCommitmentVersion => {
                write!(f, "the commitment version must be non-empty (no untracked commitments)")
            }
            PccTrackingError::CommitmentVersionWhitespace => {
                write!(f, "the commitment version must be whitespace-free")
            }
            PccTrackingError::InvalidLambdaTrack { value } => {
                write!(f, "lambda_track must be finite and >= 0 (found {value})")
            }
            PccTrackingError::InvalidLossFraction { value } => {
                write!(f, "the loss fraction must be in [0, 1) (found {value})")
            }
            PccTrackingError::EmptyTrackingPoints => {
                write!(f, "at least one tracking point (declared PCC) is required")
            }
            PccTrackingError::DuplicatePcc { pcc } => write!(
                f,
                "the PCC identity '{pcc}' appears more than once; multiple delivery points \
                 stay separate, and two terms for one identity would be a merge mistake"
            ),
            PccTrackingError::MissingCommitment { pcc } => {
                write!(f, "no synthetic commitment was supplied for the declared PCC '{pcc}'")
            }
            PccTrackingError::MissingSchedule { pcc } => write!(
                f,
                "no attributed asset schedule was supplied for the declared PCC '{pcc}'"
            ),
            PccTrackingError::NonPositiveIntervalSeconds => {
                write!(f, "the interval length must be a positive number of seconds")
            }
            PccTrackingError::SeriesLengthMismatch { pcc, expected, found } => write!(
                f,
                "the series for PCC '{pcc}' has {found} entries but the evaluation covers \
                 {expected} intervals"
            ),
            PccTrackingError::NonFiniteSeriesValue { pcc, series, interval, value } => write!(
                f,
                "series '{series}' for PCC '{pcc}' at interval {interval} must be finite \
                 (found {value})"
            ),
        }
    }
}

impl std::error::Error for PccTrackingError {}

// --- PCC identity ------------------------------------------------------------

/// The declared PCC identity: a REQUIRED, VALIDATED reference into the
/// Phase 2 site registry (`Site::point_of_common_coupling`, P0.10 §1 row 3).
/// Tracking is always evaluated AT this declared point — never at the sum
/// of arbitrary bus injections (report §12.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PccRef(String);

impl PccRef {
    /// Validates and wraps a PCC identity: non-empty, no leading/trailing
    /// whitespace (identities are matched verbatim against the registry).
    pub fn new(identity: &str) -> Result<Self, PccTrackingError> {
        if identity.is_empty() {
            return Err(PccTrackingError::EmptyPccIdentity);
        }
        if identity.trim() != identity {
            return Err(PccTrackingError::PccIdentityWhitespace);
        }
        Ok(Self(identity.to_string()))
    }

    /// Takes the declared PCC identity from the Phase 2 site registry
    /// entity (the site already validated it non-empty at construction).
    pub fn from_site(site: &Site) -> Self {
        Self(site.point_of_common_coupling().to_string())
    }

    /// The identity string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for PccRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// --- Synthetic commitments ---------------------------------------------------

/// The labelled view of a synthetic commitment's values: the ONLY access
/// path to the commitment values — the provenance label, the version, and
/// the synthetic-commitment note travel WITH the values on every read
/// (mirrors the P3.31 four-place labelling rule; this is the type-level
/// place, with the schedule record and the evaluation diagnostics as the
/// downstream places).
#[derive(Debug, Clone, PartialEq)]
pub struct LabeledCommitment<'a> {
    /// The provenance label ([`COMMITMENTS_SYNTHETIC_LABEL`]) — always
    /// `SYNTHETIC`.
    pub provenance: &'static str,
    /// The commitment version.
    pub version: &'a str,
    /// The per-interval commitment values in MW (`P > 0` export, §10.2).
    pub values_mw: &'a [f64],
    /// The recorded synthetic-commitment statement
    /// ([`SYNTHETIC_COMMITMENT_NOTE`]).
    pub note: &'static str,
}

/// A synthetic test commitment at one delivery point (D1 OPEN): versioned,
/// labelled SYNTHETIC by construction, with the values reachable only
/// through the labelled accessor. No market qualification is claimed —
/// these are test inputs for the §12.2 tracking term on the
/// simulation-only 0.x line.
#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticCommitment {
    version: String,
    values_mw: Vec<f64>,
}

impl SyntheticCommitment {
    /// Constructs and validates a synthetic commitment: non-empty,
    /// whitespace-free version; non-empty series; finite values (any sign —
    /// import commitments are negative under the §10.2 convention).
    pub fn new(version: &str, values_mw: Vec<f64>) -> Result<Self, PccTrackingError> {
        if version.is_empty() {
            return Err(PccTrackingError::EmptyCommitmentVersion);
        }
        if version.chars().any(|c| c.is_whitespace()) {
            return Err(PccTrackingError::CommitmentVersionWhitespace);
        }
        for (i, v) in values_mw.iter().enumerate() {
            if !v.is_finite() {
                return Err(PccTrackingError::NonFiniteSeriesValue {
                    pcc: "(commitment under construction)".to_string(),
                    series: "commitment",
                    interval: i as u32,
                    value: *v,
                });
            }
        }
        Ok(Self {
            version: version.to_string(),
            values_mw,
        })
    }

    /// The commitment version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The labelled values view — the only access path to the commitment
    /// values (the label cannot be dropped).
    pub fn labeled_values(&self) -> LabeledCommitment<'_> {
        LabeledCommitment {
            provenance: COMMITMENTS_SYNTHETIC_LABEL,
            version: &self.version,
            values_mw: &self.values_mw,
            note: SYNTHETIC_COMMITMENT_NOTE,
        }
    }

    /// The number of intervals covered.
    pub fn len(&self) -> usize {
        self.values_mw.len()
    }

    /// Whether the commitment covers no intervals.
    pub fn is_empty(&self) -> bool {
        self.values_mw.is_empty()
    }
}

// --- Configuration -----------------------------------------------------------

/// The declared loss model of one delivery point (see [`LOSS_MODEL_NOTE`]
/// for the recorded simplification).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LossModel {
    /// No modelled loss: `P_PCC = Σ P_asset` attributed to the point.
    None,
    /// A declared net-export fraction: `P_PCC = (1 − fraction) · Σ P_asset`,
    /// `fraction ∈ [0, 1)`.
    NetExportFraction {
        /// The fraction of net export lost between the attributed asset
        /// injections and the PCC.
        fraction: f64,
    },
}

impl LossModel {
    /// The effective loss fraction (0.0 for [`LossModel::None`]).
    pub const fn fraction(&self) -> f64 {
        match self {
            LossModel::None => 0.0,
            LossModel::NetExportFraction { fraction } => *fraction,
        }
    }

    /// Validates a loss model: a fraction must be finite and in [0, 1).
    pub fn validate(&self) -> Result<(), PccTrackingError> {
        let fraction = self.fraction();
        if !fraction.is_finite() || !(0.0..1.0).contains(&fraction) {
            return Err(PccTrackingError::InvalidLossFraction { value: fraction });
        }
        Ok(())
    }
}

/// One tracking point: the declared PCC (required, validated), the tracking
/// weight λ, and the declared loss model. Multiple points = separate terms.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackingPoint {
    pcc: PccRef,
    lambda_track: f64,
    loss: LossModel,
}

impl TrackingPoint {
    /// Constructs and validates a tracking point: λ finite and ≥ 0, loss
    /// model valid.
    pub fn new(pcc: PccRef, lambda_track: f64, loss: LossModel) -> Result<Self, PccTrackingError> {
        if !lambda_track.is_finite() || lambda_track < 0.0 {
            return Err(PccTrackingError::InvalidLambdaTrack { value: lambda_track });
        }
        loss.validate()?;
        Ok(Self { pcc, lambda_track, loss })
    }

    /// The declared PCC identity.
    pub fn pcc(&self) -> &PccRef {
        &self.pcc
    }

    /// The tracking weight λ in currency-units/(MW²·h).
    pub const fn lambda_track(&self) -> f64 {
        self.lambda_track
    }

    /// The declared loss model.
    pub const fn loss(&self) -> &LossModel {
        &self.loss
    }
}

/// The PCC-tracking configuration: one entry per delivery point, each with
/// its own λ and loss model. Duplicate identities are rejected — the
/// per-point terms stay separate by construction (report §12.2).
#[derive(Debug, Clone, PartialEq)]
pub struct PccTrackingConfig {
    points: Vec<TrackingPoint>,
}

impl PccTrackingConfig {
    /// Constructs and validates the configuration: non-empty, unique PCC
    /// identities.
    pub fn new(points: Vec<TrackingPoint>) -> Result<Self, PccTrackingError> {
        if points.is_empty() {
            return Err(PccTrackingError::EmptyTrackingPoints);
        }
        for i in 0..points.len() {
            for earlier in &points[..i] {
                if earlier.pcc == points[i].pcc {
                    return Err(PccTrackingError::DuplicatePcc {
                        pcc: points[i].pcc.as_str().to_string(),
                    });
                }
            }
        }
        Ok(Self { points })
    }

    /// The tracking points (separate terms, never merged).
    pub fn points(&self) -> &[TrackingPoint] {
        &self.points
    }

    /// The declared PCC identities, in configuration order.
    pub fn pcc_identities(&self) -> Vec<String> {
        self.points.iter().map(|p| p.pcc.as_str().to_string()).collect()
    }
}

// --- Evaluation inputs -------------------------------------------------------

/// The asset-side schedule attributed to ONE delivery point: the per-interval
/// sum of the attributed assets' net active power in MW at their points of
/// connection, in the canonical sign (`P > 0` export, §10.2). The
/// attribution of assets to delivery points is the caller's declared
/// configuration; this module never re-derives it from bus topology (there
/// is no grid model in v0.2.0 — that is Phase 4).
#[derive(Debug, Clone, PartialEq)]
pub struct PointSchedule {
    /// The delivery point the schedule is attributed to.
    pub pcc: PccRef,
    /// Per-interval attributed net export in MW (`P > 0` export).
    pub asset_net_export_mw: Vec<f64>,
}

/// The PCC-tracking evaluation inputs: the interval length, one synthetic
/// commitment per configured point, one attributed schedule per configured
/// point.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackingInputs {
    /// The scheduling interval length in seconds (must be > 0).
    pub interval_seconds: u32,
    /// One synthetic commitment per configured point (identity-matched).
    pub commitments: Vec<(PccRef, SyntheticCommitment)>,
    /// One attributed asset schedule per configured point (identity-matched).
    pub schedules: Vec<PointSchedule>,
}

// --- Evaluation shapes -------------------------------------------------------

/// One interval's tracking record at one point: the PCC position, the
/// commitment, the signed mismatch, and the interval's tracking cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IntervalTracking {
    /// The interval index.
    pub interval: u32,
    /// The net active power at the PCC in MW (after the declared loss
    /// model; `P > 0` export).
    pub p_pcc_mw: f64,
    /// The synthetic commitment in MW (`P > 0` export).
    pub commitment_mw: f64,
    /// The signed mismatch `P_PCC − P_commitment` in MW (positive =
    /// over-delivery).
    pub mismatch_mw: f64,
    /// `λ · mismatch² · Δt` in currency-units.
    pub cost: f64,
}

/// The tracking record of ONE delivery point — a separate term, reported
/// independently (report §12.2: multiple delivery points are never merged
/// into one number).
#[derive(Debug, Clone, PartialEq)]
pub struct PointTracking {
    /// The declared PCC identity.
    pub pcc: String,
    /// The point's tracking weight λ in currency-units/(MW²·h).
    pub lambda_track: f64,
    /// The point's effective loss fraction (0.0 for [`LossModel::None`]).
    pub loss_fraction: f64,
    /// The per-interval tracking records.
    pub per_interval: Vec<IntervalTracking>,
    /// The point's tracking total: Σ_t λ · mismatch² · Δt in currency-units.
    pub total_cost: f64,
}

/// The result of a PCC-tracking evaluation: one separate record per
/// delivery point, plus the explicitly labelled sum of the separate terms.
#[derive(Debug, Clone, PartialEq)]
pub struct PccTrackingEvaluation {
    /// The per-point tracking records (separate terms — the primary
    /// output).
    pub points: Vec<PointTracking>,
    /// The sum of the separate per-point totals. Explicitly labelled as a
    /// sum: the per-point records are authoritative and never merged away.
    pub total: f64,
    /// The synthetic-commitment label the evaluation was computed under
    /// ([`COMMITMENTS_SYNTHETIC_LABEL`]).
    pub synthetic_label: &'static str,
}

// --- Evaluation --------------------------------------------------------------

/// Evaluates the §12.2 tracking term at every configured delivery point.
/// For each point: `P_PCC,t = (1 − loss) · Σ P_asset,t` (the attributed
/// assets), `mismatch,t = P_PCC,t − P_commitment,t`, cost `= λ ·
/// mismatch² · Δt`. All series must cover the same interval count.
pub fn evaluate_pcc_tracking(
    config: &PccTrackingConfig,
    inputs: &TrackingInputs,
) -> Result<PccTrackingEvaluation, PccTrackingError> {
    if inputs.interval_seconds == 0 {
        return Err(PccTrackingError::NonPositiveIntervalSeconds);
    }
    let dt_h = inputs.interval_seconds as f64 / 3600.0;

    for i in 0..inputs.commitments.len() {
        for (earlier_pcc, _) in &inputs.commitments[..i] {
            if *earlier_pcc == inputs.commitments[i].0 {
                return Err(PccTrackingError::DuplicatePcc {
                    pcc: inputs.commitments[i].0.as_str().to_string(),
                });
            }
        }
    }
    for i in 0..inputs.schedules.len() {
        for earlier in &inputs.schedules[..i] {
            if earlier.pcc == inputs.schedules[i].pcc {
                return Err(PccTrackingError::DuplicatePcc {
                    pcc: inputs.schedules[i].pcc.as_str().to_string(),
                });
            }
        }
    }

    // The evaluation's interval count is anchored on the FIRST configured
    // point's schedule; a missing schedule for it is reported below as
    // MissingSchedule with the point's identity, and every other series is
    // length-checked against this anchor.
    let interval_count = config
        .points()
        .first()
        .and_then(|point| {
            inputs
                .schedules
                .iter()
                .find(|s| s.pcc == point.pcc)
                .map(|s| s.asset_net_export_mw.len())
        })
        .unwrap_or(0);

    let mut points = Vec::with_capacity(config.points().len());
    let mut total = 0.0;
    for point in config.points() {
        let (_, commitment) = inputs
            .commitments
            .iter()
            .find(|(pcc, _)| pcc == point.pcc())
            .ok_or_else(|| PccTrackingError::MissingCommitment {
                pcc: point.pcc().as_str().to_string(),
            })?;
        let schedule = inputs
            .schedules
            .iter()
            .find(|s| s.pcc == point.pcc())
            .ok_or_else(|| PccTrackingError::MissingSchedule {
                pcc: point.pcc().as_str().to_string(),
            })?;
        if schedule.asset_net_export_mw.len() != interval_count {
            return Err(PccTrackingError::SeriesLengthMismatch {
                pcc: point.pcc().as_str().to_string(),
                expected: interval_count,
                found: schedule.asset_net_export_mw.len(),
            });
        }
        if commitment.len() != interval_count {
            return Err(PccTrackingError::SeriesLengthMismatch {
                pcc: point.pcc().as_str().to_string(),
                expected: interval_count,
                found: commitment.len(),
            });
        }
        let labeled = commitment.labeled_values();
        let loss_fraction = point.loss().fraction();
        let mut per_interval = Vec::with_capacity(interval_count);
        let mut point_total = 0.0;
        for t in 0..interval_count {
            let injected = schedule.asset_net_export_mw[t];
            if !injected.is_finite() {
                return Err(PccTrackingError::NonFiniteSeriesValue {
                    pcc: point.pcc().as_str().to_string(),
                    series: "asset_net_export_mw",
                    interval: t as u32,
                    value: injected,
                });
            }
            let commitment_mw = labeled.values_mw[t];
            let p_pcc = (1.0 - loss_fraction) * injected;
            let mismatch = p_pcc - commitment_mw;
            let cost = point.lambda_track() * mismatch * mismatch * dt_h;
            per_interval.push(IntervalTracking {
                interval: t as u32,
                p_pcc_mw: p_pcc,
                commitment_mw,
                mismatch_mw: mismatch,
                cost,
            });
            point_total += cost;
        }
        points.push(PointTracking {
            pcc: point.pcc().as_str().to_string(),
            lambda_track: point.lambda_track(),
            loss_fraction,
            per_interval,
            total_cost: point_total,
        });
        total += point_total;
    }

    Ok(PccTrackingEvaluation {
        points,
        total,
        synthetic_label: COMMITMENTS_SYNTHETIC_LABEL,
    })
}

/// The ordered record fields the P3.21 schedule record receives for the
/// §12.2 tracking term: the term statement, the sign convention, the loss
/// note, and — per delivery point, kept separate — the identity, λ and its
/// unit, the loss fraction, and the commitment's version + SYNTHETIC label
/// + note. Requires one commitment per configured point.
pub fn tracking_record_fields(
    config: &PccTrackingConfig,
    commitments: &[(PccRef, SyntheticCommitment)],
) -> Result<Vec<(String, String)>, PccTrackingError> {
    let mut fields = vec![
        ("tracking.term".to_string(), TRACKING_TERM_STATEMENT.to_string()),
        ("tracking.sign_convention".to_string(), TRACKING_SIGN_CONVENTION.to_string()),
        ("tracking.loss_model_note".to_string(), LOSS_MODEL_NOTE.to_string()),
        ("tracking.commitment_provenance".to_string(), COMMITMENTS_SYNTHETIC_LABEL.to_string()),
        (
            "tracking.commitments_are".to_string(),
            "SYNTHETIC test commitments; no market qualification is claimed (D1 OPEN)".to_string(),
        ),
    ];
    for (index, point) in config.points().iter().enumerate() {
        let (_, commitment) = commitments
            .iter()
            .find(|(pcc, _)| pcc == point.pcc())
            .ok_or_else(|| PccTrackingError::MissingCommitment {
                pcc: point.pcc().as_str().to_string(),
            })?;
        let prefix = format!("tracking.point[{index}]");
        fields.push((format!("{prefix}.pcc"), point.pcc().as_str().to_string()));
        fields.push((format!("{prefix}.lambda_track"), format!("{}", point.lambda_track())));
        fields.push((format!("{prefix}.lambda_track_unit"), LAMBDA_TRACK_UNIT.to_string()));
        fields.push((
            format!("{prefix}.loss_fraction"),
            format!("{}", point.loss().fraction()),
        ));
        fields.push((
            format!("{prefix}.commitment_version"),
            commitment.version().to_string(),
        ));
        fields.push((
            format!("{prefix}.commitment_provenance"),
            commitment.labeled_values().provenance.to_string(),
        ));
        fields.push((
            format!("{prefix}.commitment_note"),
            SYNTHETIC_COMMITMENT_NOTE.to_string(),
        ));
    }
    fields.push((
        "tracking.multiple_points".to_string(),
        "one separate tracking term per delivery point; never merged into one number".to_string(),
    ));
    Ok(fields)
}
