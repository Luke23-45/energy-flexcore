// P3.08 — Inverter P-Q envelope (pure rules, no I/O).
//
// Implements ledger P3.08 ("Inverter P-Q envelope"), report §11.3 and §11.1.
// Decision record:
// `ledger/phase-03-flexibility-scheduling/05_08_bess_pq_models.md`.
//
// THE ENVELOPE, EXACTLY AS SPECIFIED (report §11.3):
//
//   P² + Q² ≤ S_max²
//   P_min(m, V) ≤ P ≤ P_max(m, V)
//   Q_min(P, V, m) ≤ Q ≤ Q_max(P, V, m)
//
// - The apparent-power constraint is documented as **necessary but not
//   complete** (§11.3): it is the basic limit, not a complete universal
//   inverter model. The per-mode/per-voltage tables below carry the
//   device-specific bounds; neither alone is the whole model.
// - Units are canonical (§10.2): MW, MVAr, MVA. P > 0 means EXPORT at the
//   point of connection (frozen rule, §10.2) — so charging is NEGATIVE P and
//   discharging is POSITIVE P. **Charging and discharging are both
//   represented** by their own per-mode limit tables (configuration).
// - **NO power-factor inequality that assumes P ≥ 0.** A bidirectional asset
//   spends most of its life at P < 0 (charging); any bound of the form
//   |Q| ≤ P·tan(φ) or |Q| ≤ √(S_max² − P²) applied only for P ≥ 0 would
//   wrongly reject half the operating range (§11.3). The envelope uses the
//   symmetric circle constraint plus declared per-mode tables only. The test
//   suite contains a negative-P case that FAILS if such an inequality is
//   ever introduced (ledger P3.08 step 3).
// - **Voltage dependence is configuration**: per-mode limit rows may declare
//   the voltage they apply to. When the operating voltage is MISSING or not
//   among the documented rows, the envelope defaults to the MOST RESTRICTIVE
//   documented value across all rows of the mode — the narrowing is recorded
//   on the evaluation output ([`VoltageTreatment::MostRestrictiveDocumented`]),
//   never silent (ledger P3.08 risk note).
// - **Unknown narrows (frozen P3.02 rule)**: a mode declared with UNKNOWN
//   provenance credits NO operating points (mode credit requires known
//   provenance, §11.1). A (mode, P) point with no declared Q rows collapses
//   the Q dimension to [0, 0] MVAr — the recorded collapse-to-zero fallback,
//   never "unconstrained".
// - **Reactive sign is declared, never assumed** (§10.2): the envelope
//   carries the declared [`ReactiveSignConvention`] of the governing grid
//   model / device profile as a required constructor argument — there is no
//   default. The declared convention is metadata for interpreting Q; the
//   numeric bounds themselves are the declared per-mode tables.
//
// IEEE 1547-2018 Clause 5 [R18] SCOPE STATEMENT (ledger P3.08 step 4):
//
//   IEEE 1547-2018 defines DER interconnection and interoperability
//   performance requirements. Its Clause 5 includes reactive-power and
//   voltage/power-control capabilities; the exact capability and settings
//   depend on the applicable category, performance requirements, utility
//   profile, and equipment. It is **not** a universal specification for
//   internal BMS cell-temperature protection, and equipment
//   safety/protection and conformance testing under IEEE 1547.1 remain
//   separate responsibilities. Implementing this envelope claims NO
//   conformance with IEEE 1547-2018 or any other standard.
//
// Control status: pure domain rules for the simulation-only 0.x line. No I/O,
// no solver coupling, no device-write path. FlexCore is a supervisory
// orchestration layer: no inverter inner-loop control, no protection-grade
// functionality, no sub-cycle modelling (AGENTS.md rules 2–3).

use flexcore_contracts::capability::Provenance;
use flexcore_contracts::units::ReactiveSignConvention;

/// Evaluation tolerance in each value's own unit (MW, MVAr, MVA):
/// comparisons are boundary-inclusive within this epsilon — a point exactly
/// on the apparent-power circle or exactly at a declared limit is feasible.
pub const PQ_EVALUATION_TOLERANCE: f64 = 1e-9;

// --- Errors -----------------------------------------------------------------

/// Errors returned by P-Q envelope construction. Each variant names the
/// offending field/value — never a bare "invalid input" (P3.04 style).
#[derive(Debug, Clone, PartialEq)]
pub enum PqEnvelopeError {
    /// A numeric value was NaN or infinite; envelope parameters must be
    /// finite.
    NonFiniteValue {
        /// The offending field.
        field: &'static str,
        /// The offending value.
        value: f64,
    },
    /// S_max was zero or negative; apparent power must be strictly positive.
    NonPositiveSMax {
        /// The offending value.
        value: f64,
    },
    /// No operating mode was declared; an envelope with no modes credits
    /// nothing and is a configuration error, not an empty pass-through.
    NoModesDeclared,
    /// A mode name was empty.
    EmptyModeName,
    /// The same mode name was declared twice.
    DuplicateMode {
        /// The duplicated mode name.
        mode: String,
    },
    /// A P-limit row was inverted (P_min > P_max).
    InvertedPLimits {
        /// The offending mode.
        mode: String,
        /// The declared P_min.
        p_min_mw: f64,
        /// The declared P_max.
        p_max_mw: f64,
    },
    /// A Q-limit row was inverted (Q_min > Q_max).
    InvertedQLimits {
        /// The offending mode.
        mode: String,
        /// The declared Q_min.
        q_min_mvar: f64,
        /// The declared Q_max.
        q_max_mvar: f64,
    },
    /// A Q row's P-band was inverted (band lower > upper).
    InvertedPBand {
        /// The offending mode.
        mode: String,
        /// The declared band lower bound.
        p_band_min_mw: f64,
        /// The declared band upper bound.
        p_band_max_mw: f64,
    },
}

impl std::fmt::Display for PqEnvelopeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PqEnvelopeError::NonFiniteValue { field, value } => {
                write!(f, "envelope parameter '{field}' must be finite (found {value})")
            }
            PqEnvelopeError::NonPositiveSMax { value } => {
                write!(f, "S_max must be strictly positive MVA (found {value})")
            }
            PqEnvelopeError::NoModesDeclared => {
                write!(f, "the envelope must declare at least one operating mode")
            }
            PqEnvelopeError::EmptyModeName => {
                write!(f, "operating mode names must be non-empty")
            }
            PqEnvelopeError::DuplicateMode { mode } => {
                write!(f, "operating mode '{mode}' is declared more than once")
            }
            PqEnvelopeError::InvertedPLimits { mode, p_min_mw, p_max_mw } => write!(
                f,
                "mode '{mode}': P limits must satisfy P_min <= P_max (found {p_min_mw} > {p_max_mw} MW)"
            ),
            PqEnvelopeError::InvertedQLimits { mode, q_min_mvar, q_max_mvar } => write!(
                f,
                "mode '{mode}': Q limits must satisfy Q_min <= Q_max (found {q_min_mvar} > {q_max_mvar} MVAr)"
            ),
            PqEnvelopeError::InvertedPBand { mode, p_band_min_mw, p_band_max_mw } => write!(
                f,
                "mode '{mode}': Q-row P-band must satisfy band_min <= band_max (found {p_band_min_mw} > {p_band_max_mw} MW)"
            ),
        }
    }
}

impl std::error::Error for PqEnvelopeError {}

// --- Configuration types -----------------------------------------------------

/// One P-limit row: `P_min(m, V) ≤ P ≤ P_max(m, V)` for one mode at one
/// documented voltage. `voltage_kv: None` declares a voltage-independent row
/// (it applies at every voltage).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PVoltageRow {
    /// The documented voltage in kV, or `None` for a voltage-independent row.
    pub voltage_kv: Option<f64>,
    /// Inclusive lower active-power bound in MW (negative = import/charge
    /// under the frozen export-positive rule, §10.2).
    pub p_min_mw: f64,
    /// Inclusive upper active-power bound in MW (positive = export/discharge).
    pub p_max_mw: f64,
}

/// One Q-limit row: `Q_min(P, V, m) ≤ Q ≤ Q_max(P, V, m)` — the declared Q
/// window applies to active-power points inside the row's P-band, at the
/// row's documented voltage. `voltage_kv: None` declares a
/// voltage-independent row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PBandQRow {
    /// Inclusive lower bound of the P-band this row covers, in MW.
    pub p_band_min_mw: f64,
    /// Inclusive upper bound of the P-band this row covers, in MW.
    pub p_band_max_mw: f64,
    /// The documented voltage in kV, or `None` for a voltage-independent row.
    pub voltage_kv: Option<f64>,
    /// Inclusive lower reactive-power bound in MVAr (in the DECLARED
    /// reactive sign convention).
    pub q_min_mvar: f64,
    /// Inclusive upper reactive-power bound in MVAr (in the DECLARED
    /// reactive sign convention).
    pub q_max_mvar: f64,
}

/// The per-mode limit table of one operating mode (configuration, §11.3).
/// Both charge and discharge modes are represented by their own tables.
#[derive(Debug, Clone, PartialEq)]
pub struct PqModeLimits {
    /// The mode name (for example "CHARGE", "DISCHARGE"); unique per
    /// envelope.
    pub mode: String,
    /// Provenance of this mode's limit tables. UNKNOWN provenance credits NO
    /// operating points (mode credit requires known provenance, §11.1 / P3.02).
    pub provenance: Provenance,
    /// P-limit rows (`P_min(m,V)`, `P_max(m,V)`). An empty table narrows P
    /// to [0, 0] MW (the recorded collapse-to-zero fallback, P3.02).
    pub p_rows: Vec<PVoltageRow>,
    /// Q-limit rows (`Q_min(P,V,m)`, `Q_max(P,V,m)`). Points with no
    /// covering row narrow Q to [0, 0] MVAr (the recorded fallback).
    pub q_rows: Vec<PBandQRow>,
}

impl PqModeLimits {
    /// A mode whose tables carry configured provenance.
    pub fn configured(mode: &str, p_rows: Vec<PVoltageRow>, q_rows: Vec<PBandQRow>) -> Self {
        Self {
            mode: mode.to_string(),
            provenance: Provenance::Configured,
            p_rows,
            q_rows,
        }
    }
}

/// The full P-Q envelope configuration (report §11.3).
#[derive(Debug, Clone, PartialEq)]
pub struct InverterPqConfig {
    /// Apparent-power limit in MVA (strictly positive).
    pub s_max_mva: f64,
    /// The DECLARED reactive sign convention of the governing grid model /
    /// device profile (§10.2) — required, never assumed, never inferred.
    pub reactive_sign: ReactiveSignConvention,
    /// Per-mode limit tables; charge and discharge both represented.
    pub modes: Vec<PqModeLimits>,
}

// --- Output types -------------------------------------------------------------

/// A named, structured P-Q evaluation violation. Every variant names the
/// offending value and the limit.
#[derive(Debug, Clone, PartialEq)]
pub enum PqViolation {
    /// The operating mode has no declared limit table. An undeclared mode
    /// credits nothing (unknown never widens, P3.02).
    ModeNotDeclared {
        /// The undeclared mode name.
        mode: String,
    },
    /// The mode's limit tables carry UNKNOWN provenance; mode credit
    /// requires known provenance (§11.1), so the point is not credited.
    ModeNotCredited {
        /// The mode name.
        mode: String,
        /// The recorded reason (audit-safe).
        reason: &'static str,
    },
    /// The mode's P rows do not intersect to a non-empty window (declared
    /// rows conflict); never repaired by widening.
    PWindowInfeasible {
        /// The mode name.
        mode: String,
    },
    /// P above the effective upper limit for the (mode, voltage).
    PAboveMax {
        /// The declared P in MW.
        p_mw: f64,
        /// The effective limit in MW.
        limit_mw: f64,
    },
    /// P below the effective lower limit for the (mode, voltage). A
    /// negative-P point inside its declared window never triggers this —
    /// there is no power-factor assumption (§11.3).
    PBelowMin {
        /// The declared P in MW.
        p_mw: f64,
        /// The effective limit in MW.
        limit_mw: f64,
    },
    /// The mode's covering Q rows do not intersect to a non-empty window.
    QWindowInfeasible {
        /// The mode name.
        mode: String,
    },
    /// Q above the effective upper limit for the (P, V, mode).
    QAboveMax {
        /// The declared Q in MVAr.
        q_mvar: f64,
        /// The effective limit in MVAr.
        limit_mvar: f64,
    },
    /// Q below the effective lower limit for the (P, V, mode).
    QBelowMin {
        /// The declared Q in MVAr.
        q_mvar: f64,
        /// The effective limit in MVAr.
        limit_mvar: f64,
    },
    /// P² + Q² exceeded S_max² (§11.3 basic constraint — necessary but not
    /// complete).
    ApparentPowerExceeded {
        /// The apparent power of the point in MVA.
        s_mva: f64,
        /// The declared S_max in MVA.
        s_max_mva: f64,
    },
}

impl std::fmt::Display for PqViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PqViolation::ModeNotDeclared { mode } => {
                write!(f, "operating mode '{mode}' has no declared limit table (unknown narrows: no credit)")
            }
            PqViolation::ModeNotCredited { mode, reason } => {
                write!(f, "operating mode '{mode}' is not credited: {reason}")
            }
            PqViolation::PWindowInfeasible { mode } => {
                write!(f, "mode '{mode}': declared P rows conflict (empty intersection; never widened)")
            }
            PqViolation::PAboveMax { p_mw, limit_mw } => {
                write!(f, "P = {p_mw} MW exceeds the effective P_max {limit_mw} MW")
            }
            PqViolation::PBelowMin { p_mw, limit_mw } => {
                write!(f, "P = {p_mw} MW is below the effective P_min {limit_mw} MW")
            }
            PqViolation::QWindowInfeasible { mode } => {
                write!(f, "mode '{mode}': declared Q rows conflict at this operating point (empty intersection; never widened)")
            }
            PqViolation::QAboveMax { q_mvar, limit_mvar } => {
                write!(f, "Q = {q_mvar} MVAr exceeds the effective Q_max {limit_mvar} MVAr")
            }
            PqViolation::QBelowMin { q_mvar, limit_mvar } => {
                write!(f, "Q = {q_mvar} MVAr is below the effective Q_min {limit_mvar} MVAr")
            }
            PqViolation::ApparentPowerExceeded { s_mva, s_max_mva } => {
                write!(f, "apparent power {s_mva} MVA exceeds S_max {s_max_mva} MVA (P² + Q² ≤ S_max², report §11.3)")
            }
        }
    }
}

/// How the voltage dimension was resolved for an evaluation (recorded, never
/// silent — ledger P3.08 risk note).
#[derive(Debug, Clone, PartialEq)]
pub enum VoltageTreatment {
    /// Rows documented for exactly this voltage were used.
    DocumentedForVoltage {
        /// The matched voltage in kV.
        voltage_kv: f64,
    },
    /// The voltage was missing or not among the documented rows: the MOST
    /// RESTRICTIVE documented value across all rows of the mode was used
    /// (the recorded default; the narrowing is conservative by construction).
    MostRestrictiveDocumented {
        /// The recorded reason (audit-safe).
        reason: &'static str,
    },
    /// All rows of the mode are voltage-independent (`voltage_kv: None`).
    VoltageIndependentRows,
}

/// The effective limits an evaluation was checked against.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectivePqLimits {
    /// Effective inclusive P lower bound in MW.
    pub p_min_mw: f64,
    /// Effective inclusive P upper bound in MW.
    pub p_max_mw: f64,
    /// Effective inclusive Q lower bound in MVAr.
    pub q_min_mvar: f64,
    /// Effective inclusive Q upper bound in MVAr.
    pub q_max_mvar: f64,
    /// The declared apparent-power limit in MVA.
    pub s_max_mva: f64,
}

/// The provenance record on a P-Q evaluation (propagated, not consumed —
/// the P3.02 discipline).
#[derive(Debug, Clone, PartialEq)]
pub struct PqEvaluationProvenance {
    /// The evaluated mode name.
    pub mode: String,
    /// The mode table's declared provenance (`UNKNOWN` for an undeclared or
    /// uncredited mode).
    pub mode_provenance: Provenance,
    /// The declared reactive sign convention (§10.2).
    pub reactive_sign: ReactiveSignConvention,
    /// How the voltage dimension was resolved.
    pub voltage_treatment: VoltageTreatment,
    /// Dimensions recorded unknown for this evaluation (P3.02: visible,
    /// never silently unconstrained).
    pub unknown_dimensions: Vec<PqUnknownDimension>,
}

/// One dimension recorded as unknown on a P-Q evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct PqUnknownDimension {
    /// The dimension name (frozen vocabulary string).
    pub dimension: &'static str,
    /// The recorded explanation (audit-safe).
    pub note: String,
}

/// One evaluated operating point.
#[derive(Debug, Clone, PartialEq)]
pub struct PqPoint {
    /// The operating mode (must match a declared mode table).
    pub mode: String,
    /// Active power in MW (P > 0 = export; charging is negative).
    pub p_mw: f64,
    /// Reactive power in MVAr (in the declared reactive sign convention).
    pub q_mvar: f64,
    /// Operating voltage in kV, when known. Missing voltage defaults the
    /// envelope to the most restrictive documented value (recorded).
    pub voltage_kv: Option<f64>,
}

/// The result of checking one operating point against the §11.3 envelope.
#[derive(Debug, Clone, PartialEq)]
pub struct PqEvaluation {
    /// The apparent power of the point in MVA: √(P² + Q²).
    pub apparent_power_mva: f64,
    /// The effective limits the point was checked against.
    pub effective_limits: EffectivePqLimits,
    /// All named violations; empty = the point lies inside the envelope
    /// within [`PQ_EVALUATION_TOLERANCE`].
    pub violations: Vec<PqViolation>,
    /// The provenance record.
    pub provenance: PqEvaluationProvenance,
}

// --- The envelope -------------------------------------------------------------

/// The inverter P-Q envelope (report §11.3): the apparent-power circle plus
/// configurable per-mode, per-voltage P and Q limit tables. Pure
/// feasibility/limits layer — no control, no conformance claim.
#[derive(Debug, Clone, PartialEq)]
pub struct InverterPqEnvelope {
    s_max_mva: f64,
    reactive_sign: ReactiveSignConvention,
    modes: Vec<PqModeLimits>,
}

/// The apparent power of an (P, Q) point in MVA: √(P² + Q²).
pub fn apparent_power_mva(p_mw: f64, q_mvar: f64) -> f64 {
    (p_mw * p_mw + q_mvar * q_mvar).sqrt()
}

fn require_finite(field: &'static str, value: f64) -> Result<(), PqEnvelopeError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(PqEnvelopeError::NonFiniteValue { field, value })
    }
}

impl InverterPqEnvelope {
    /// Constructs and validates the envelope: S_max strictly positive and
    /// finite; at least one mode; unique non-empty mode names; every row
    /// finite and internally ordered. Rows may be empty per mode — that
    /// dimension then collapses to zero (the recorded P3.02 fallback), it is
    /// never treated as unconstrained.
    pub fn new(config: InverterPqConfig) -> Result<Self, PqEnvelopeError> {
        require_finite("s_max_mva", config.s_max_mva)?;
        if config.s_max_mva <= 0.0 {
            return Err(PqEnvelopeError::NonPositiveSMax { value: config.s_max_mva });
        }
        if config.modes.is_empty() {
            return Err(PqEnvelopeError::NoModesDeclared);
        }
        let mut seen: Vec<&str> = Vec::new();
        for mode in &config.modes {
            if mode.mode.trim().is_empty() {
                return Err(PqEnvelopeError::EmptyModeName);
            }
            if seen.contains(&mode.mode.as_str()) {
                return Err(PqEnvelopeError::DuplicateMode { mode: mode.mode.clone() });
            }
            seen.push(&mode.mode);
            for row in &mode.p_rows {
                if let Some(voltage) = row.voltage_kv {
                    require_finite("p_row.voltage_kv", voltage)?;
                }
                require_finite("p_row.p_min_mw", row.p_min_mw)?;
                require_finite("p_row.p_max_mw", row.p_max_mw)?;
                if row.p_min_mw > row.p_max_mw {
                    return Err(PqEnvelopeError::InvertedPLimits {
                        mode: mode.mode.clone(),
                        p_min_mw: row.p_min_mw,
                        p_max_mw: row.p_max_mw,
                    });
                }
            }
            for row in &mode.q_rows {
                if let Some(voltage) = row.voltage_kv {
                    require_finite("q_row.voltage_kv", voltage)?;
                }
                require_finite("q_row.p_band_min_mw", row.p_band_min_mw)?;
                require_finite("q_row.p_band_max_mw", row.p_band_max_mw)?;
                require_finite("q_row.q_min_mvar", row.q_min_mvar)?;
                require_finite("q_row.q_max_mvar", row.q_max_mvar)?;
                if row.p_band_min_mw > row.p_band_max_mw {
                    return Err(PqEnvelopeError::InvertedPBand {
                        mode: mode.mode.clone(),
                        p_band_min_mw: row.p_band_min_mw,
                        p_band_max_mw: row.p_band_max_mw,
                    });
                }
                if row.q_min_mvar > row.q_max_mvar {
                    return Err(PqEnvelopeError::InvertedQLimits {
                        mode: mode.mode.clone(),
                        q_min_mvar: row.q_min_mvar,
                        q_max_mvar: row.q_max_mvar,
                    });
                }
            }
        }
        Ok(Self {
            s_max_mva: config.s_max_mva,
            reactive_sign: config.reactive_sign,
            modes: config.modes,
        })
    }

    /// The declared apparent-power limit in MVA.
    pub fn s_max_mva(&self) -> f64 {
        self.s_max_mva
    }

    /// The declared reactive sign convention (§10.2).
    pub fn reactive_sign(&self) -> ReactiveSignConvention {
        self.reactive_sign
    }

    /// The declared mode tables.
    pub fn modes(&self) -> &[PqModeLimits] {
        &self.modes
    }

    /// Checks one operating point against the envelope. All applicable
    /// limits are checked and every violation is reported (the point is
    /// never silently moved onto the envelope).
    pub fn evaluate(&self, point: &PqPoint) -> Result<PqEvaluation, PqEnvelopeError> {
        require_finite("p_mw", point.p_mw)?;
        require_finite("q_mvar", point.q_mvar)?;
        if let Some(v) = point.voltage_kv {
            require_finite("voltage_kv", v)?;
        }

        let table = match self.modes.iter().find(|m| m.mode == point.mode) {
            Some(table) => table,
            None => {
                return Ok(self.undeclared_mode_evaluation(point));
            }
        };
        if table.provenance == Provenance::Unknown {
            return Ok(PqEvaluation {
                apparent_power_mva: apparent_power_mva(point.p_mw, point.q_mvar),
                effective_limits: EffectivePqLimits {
                    p_min_mw: 0.0,
                    p_max_mw: 0.0,
                    q_min_mvar: 0.0,
                    q_max_mvar: 0.0,
                    s_max_mva: self.s_max_mva,
                },
                violations: vec![PqViolation::ModeNotCredited {
                    mode: table.mode.clone(),
                    reason: "mode limit tables carry UNKNOWN provenance; mode credit \
requires known provenance (report §11.1, P3.02 rule)",
                }],
                provenance: PqEvaluationProvenance {
                    mode: table.mode.clone(),
                    mode_provenance: Provenance::Unknown,
                    reactive_sign: self.reactive_sign,
                    voltage_treatment: VoltageTreatment::MostRestrictiveDocumented {
                        reason: "mode not credited; voltage treatment not applicable",
                    },
                    unknown_dimensions: vec![PqUnknownDimension {
                        dimension: "DEVICE_MODE",
                        note: "the mode table carries UNKNOWN provenance; no operating \
points are credited (u is declared, never assumed, §11.1)"
                            .to_string(),
                    }],
                },
            });
        }

        // --- P limits: P_min(m, V) ≤ P ≤ P_max(m, V) ------------------------
        let p_candidates = voltage_rows(
            &table
                .p_rows
                .iter()
                .map(|row| (row.voltage_kv, (row.p_min_mw, row.p_max_mw)))
                .collect::<Vec<_>>(),
            point.voltage_kv,
        );
        let (voltage_treatment, p_rows_used) = match p_candidates {
            VoltageRows::Documented(rows) => {
                let voltage = point.voltage_kv.unwrap_or(f64::NAN);
                (
                    VoltageTreatment::DocumentedForVoltage { voltage_kv: voltage },
                    rows,
                )
            }
            VoltageRows::AllRows(rows) => {
                let has_voltage_dependent_rows =
                    table.p_rows.iter().any(|row| row.voltage_kv.is_some());
                let treatment = if has_voltage_dependent_rows {
                    VoltageTreatment::MostRestrictiveDocumented {
                        reason: "operating voltage missing or undocumented; the most \
restrictive documented value across all rows of the mode was used (recorded \
default, ledger P3.08)",
                    }
                } else {
                    VoltageTreatment::VoltageIndependentRows
                };
                (treatment, rows)
            }
        };
        let mut unknown_dimensions: Vec<PqUnknownDimension> = Vec::new();
        let (p_min, p_max) = if p_rows_used.is_empty() {
            unknown_dimensions.push(PqUnknownDimension {
                dimension: "ACTIVE_POWER",
                note: "no P-limit rows declared for this mode; P collapses to [0, 0] MW \
(the recorded P3.02 collapse-to-zero fallback, never unconstrained)"
                    .to_string(),
            });
            (0.0, 0.0)
        } else {
            let lower = p_rows_used.iter().map(|&(min, _)| min).fold(f64::NEG_INFINITY, f64::max);
            let upper = p_rows_used.iter().map(|&(_, max)| max).fold(f64::INFINITY, f64::min);
            (lower, upper)
        };
        let p_window_infeasible = p_min > p_max + PQ_EVALUATION_TOLERANCE;

        let mut violations = Vec::new();
        if p_window_infeasible {
            violations.push(PqViolation::PWindowInfeasible { mode: table.mode.clone() });
        } else {
            if point.p_mw > p_max + PQ_EVALUATION_TOLERANCE {
                violations.push(PqViolation::PAboveMax { p_mw: point.p_mw, limit_mw: p_max });
            }
            if point.p_mw < p_min - PQ_EVALUATION_TOLERANCE {
                violations.push(PqViolation::PBelowMin { p_mw: point.p_mw, limit_mw: p_min });
            }
        }

        // --- Q limits: Q_min(P, V, m) ≤ Q ≤ Q_max(P, V, m) ------------------
        let mut q_candidates: Vec<&PBandQRow> = table
            .q_rows
            .iter()
            .filter(|row| {
                point.p_mw >= row.p_band_min_mw - PQ_EVALUATION_TOLERANCE
                    && point.p_mw <= row.p_band_max_mw + PQ_EVALUATION_TOLERANCE
            })
            .collect();
        // Voltage narrowing: prefer rows documented for the exact voltage;
        // with missing/undocumented voltage use ALL covering rows (most
        // restrictive intersection, recorded).
        if let Some(voltage) = point.voltage_kv {
            let exact: Vec<&PBandQRow> = q_candidates
                .iter()
                .copied()
                .filter(|row| row.voltage_kv == Some(voltage))
                .collect();
            if !exact.is_empty() {
                q_candidates = exact;
            }
        }
        let (q_min, q_max) = if q_candidates.is_empty() {
            unknown_dimensions.push(PqUnknownDimension {
                dimension: "REACTIVE_POWER",
                note: "no declared Q-limit row covers this operating point; Q collapses \
to [0, 0] MVAr (the recorded P3.02 collapse-to-zero fallback, never \
unconstrained)"
                    .to_string(),
            });
            (0.0, 0.0)
        } else {
            let lower = q_candidates
                .iter()
                .map(|row| row.q_min_mvar)
                .fold(f64::NEG_INFINITY, f64::max);
            let upper = q_candidates
                .iter()
                .map(|row| row.q_max_mvar)
                .fold(f64::INFINITY, f64::min);
            (lower, upper)
        };
        let q_window_infeasible = q_min > q_max + PQ_EVALUATION_TOLERANCE;
        if q_window_infeasible {
            violations.push(PqViolation::QWindowInfeasible { mode: table.mode.clone() });
        } else {
            if point.q_mvar > q_max + PQ_EVALUATION_TOLERANCE {
                violations.push(PqViolation::QAboveMax { q_mvar: point.q_mvar, limit_mvar: q_max });
            }
            if point.q_mvar < q_min - PQ_EVALUATION_TOLERANCE {
                violations.push(PqViolation::QBelowMin { q_mvar: point.q_mvar, limit_mvar: q_min });
            }
        }

        // --- Apparent power: P² + Q² ≤ S_max² (necessary but not complete) --
        let s = apparent_power_mva(point.p_mw, point.q_mvar);
        if s > self.s_max_mva + PQ_EVALUATION_TOLERANCE {
            violations.push(PqViolation::ApparentPowerExceeded {
                s_mva: s,
                s_max_mva: self.s_max_mva,
            });
        }

        Ok(PqEvaluation {
            apparent_power_mva: s,
            effective_limits: EffectivePqLimits {
                p_min_mw: p_min,
                p_max_mw: p_max,
                q_min_mvar: q_min,
                q_max_mvar: q_max,
                s_max_mva: self.s_max_mva,
            },
            violations,
            provenance: PqEvaluationProvenance {
                mode: table.mode.clone(),
                mode_provenance: table.provenance,
                reactive_sign: self.reactive_sign,
                voltage_treatment,
                unknown_dimensions,
            },
        })
    }

    // --- internals ------------------------------------------------------

    /// The evaluation of a point whose mode has no declared table.
    fn undeclared_mode_evaluation(&self, point: &PqPoint) -> PqEvaluation {
        PqEvaluation {
            apparent_power_mva: apparent_power_mva(point.p_mw, point.q_mvar),
            effective_limits: EffectivePqLimits {
                p_min_mw: 0.0,
                p_max_mw: 0.0,
                q_min_mvar: 0.0,
                q_max_mvar: 0.0,
                s_max_mva: self.s_max_mva,
            },
            violations: vec![PqViolation::ModeNotDeclared { mode: point.mode.clone() }],
            provenance: PqEvaluationProvenance {
                mode: point.mode.clone(),
                mode_provenance: Provenance::Unknown,
                reactive_sign: self.reactive_sign,
                voltage_treatment: VoltageTreatment::MostRestrictiveDocumented {
                    reason: "mode has no declared table; voltage treatment not applicable",
                },
                unknown_dimensions: vec![PqUnknownDimension {
                    dimension: "DEVICE_MODE",
                    note: "no limit table is declared for this mode; no operating points \
are credited (unknown narrows: no credit, P3.02)"
                        .to_string(),
                }],
            },
        }
    }
}

/// The outcome of voltage-based row selection (internal).
enum VoltageRows {
    /// Rows documented for the exact operating voltage.
    Documented(Vec<(f64, f64)>),
    /// All rows (missing/undocumented voltage → most restrictive
    /// intersection).
    AllRows(Vec<(f64, f64)>),
}

/// Selects the voltage-applicable (lower, upper) pairs from a row set: rows
/// documented for the exact operating voltage when it is known and
/// documented; otherwise ALL rows — their intersection is the most
/// restrictive documented reading, the recorded missing-voltage default
/// (ledger P3.08 risk note).
fn voltage_rows(rows: &[(Option<f64>, (f64, f64))], voltage: Option<f64>) -> VoltageRows {
    if let Some(v) = voltage {
        let exact: Vec<(f64, f64)> = rows
            .iter()
            .filter(|(documented, _)| *documented == Some(v))
            .map(|(_, bounds)| *bounds)
            .collect();
        if !exact.is_empty() {
            return VoltageRows::Documented(exact);
        }
    }
    VoltageRows::AllRows(rows.iter().map(|(_, bounds)| *bounds).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn discharge_row(voltage: Option<f64>, p_min: f64, p_max: f64) -> PVoltageRow {
        PVoltageRow { voltage_kv: voltage, p_min_mw: p_min, p_max_mw: p_max }
    }

    fn q_row(
        p_band: (f64, f64),
        voltage: Option<f64>,
        q_min: f64,
        q_max: f64,
    ) -> PBandQRow {
        PBandQRow {
            p_band_min_mw: p_band.0,
            p_band_max_mw: p_band.1,
            voltage_kv: voltage,
            q_min_mvar: q_min,
            q_max_mvar: q_max,
        }
    }

    fn base_config() -> InverterPqConfig {
        InverterPqConfig {
            s_max_mva: 3.0,
            reactive_sign: ReactiveSignConvention::InjectionPositive,
            modes: vec![
                PqModeLimits::configured(
                    "DISCHARGE",
                    vec![discharge_row(None, 0.0, 3.0)],
                    vec![q_row((0.0, 3.0), None, -1.5, 1.5)],
                ),
                PqModeLimits::configured(
                    "CHARGE",
                    vec![discharge_row(None, -2.0, 0.0)],
                    vec![q_row((-2.0, 0.0), None, -1.5, 1.5)],
                ),
            ],
        }
    }

    fn point(mode: &str, p: f64, q: f64) -> PqPoint {
        PqPoint { mode: mode.to_string(), p_mw: p, q_mvar: q, voltage_kv: None }
    }

    // --- construction ---------------------------------------------------

    #[test]
    fn base_envelope_constructs_and_exposes_declared_metadata() {
        let envelope = InverterPqEnvelope::new(base_config()).unwrap();
        assert_eq!(envelope.s_max_mva(), 3.0);
        assert_eq!(envelope.reactive_sign(), ReactiveSignConvention::InjectionPositive);
        assert_eq!(envelope.modes().len(), 2);
        // Both conventions are declarable — there is no default (§10.2).
        let mut config = base_config();
        config.reactive_sign = ReactiveSignConvention::ConsumptionPositive;
        assert_eq!(
            InverterPqEnvelope::new(config).unwrap().reactive_sign(),
            ReactiveSignConvention::ConsumptionPositive
        );
    }

    #[test]
    fn construction_rejects_structurally_invalid_configuration() {
        let mut config = base_config();
        config.s_max_mva = 0.0;
        assert_eq!(InverterPqEnvelope::new(config), Err(PqEnvelopeError::NonPositiveSMax { value: 0.0 }));

        let mut config = base_config();
        config.modes = vec![];
        assert_eq!(InverterPqEnvelope::new(config), Err(PqEnvelopeError::NoModesDeclared));

        let mut config = base_config();
        config.modes.push(PqModeLimits::configured("CHARGE", vec![], vec![]));
        assert!(matches!(
            InverterPqEnvelope::new(config),
            Err(PqEnvelopeError::DuplicateMode { .. })
        ));

        let mut config = base_config();
        config.modes[0].p_rows = vec![discharge_row(None, 4.0, 3.0)];
        assert!(matches!(
            InverterPqEnvelope::new(config),
            Err(PqEnvelopeError::InvertedPLimits { .. })
        ));

        let mut config = base_config();
        config.modes[1].q_rows = vec![q_row((-2.0, 0.0), None, 1.0, -1.0)];
        assert!(matches!(
            InverterPqEnvelope::new(config),
            Err(PqEnvelopeError::InvertedQLimits { .. })
        ));

        let mut config = base_config();
        config.modes[0].q_rows = vec![q_row((1.0, 0.0), None, -1.0, 1.0)];
        assert!(matches!(
            InverterPqEnvelope::new(config),
            Err(PqEnvelopeError::InvertedPBand { .. })
        ));

        let mut config = base_config();
        config.s_max_mva = f64::NAN;
        assert!(matches!(
            InverterPqEnvelope::new(config),
            Err(PqEnvelopeError::NonFiniteValue { field: "s_max_mva", .. })
        ));
    }

    // --- apparent power ---------------------------------------------------

    #[test]
    fn apparent_power_boundary_is_inclusive() {
        let envelope = InverterPqEnvelope::new(base_config()).unwrap();
        // Exactly on the circle: p² + q² = S_max² with q = 1.5 (within the
        // declared Q window) and p = √(9 − 2.25) — feasible.
        let p = (9.0 - 1.5 * 1.5).sqrt();
        let evaluation = envelope.evaluate(&point("DISCHARGE", p, 1.5)).unwrap();
        assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
        assert!((evaluation.apparent_power_mva - 3.0).abs() <= PQ_EVALUATION_TOLERANCE);
        // Off the circle (p² + q² = 11.84 > S_max² = 9) → named violation.
        let evaluation = envelope.evaluate(&point("DISCHARGE", 2.8, 2.0)).unwrap();
        assert!(matches!(
            evaluation.violations.last(),
            Some(PqViolation::ApparentPowerExceeded { .. })
        ));
    }

    // --- bidirectional representation -------------------------------------

    #[test]
    fn charge_and_discharge_modes_are_both_represented() {
        let envelope = InverterPqEnvelope::new(base_config()).unwrap();
        let discharge = envelope.evaluate(&point("DISCHARGE", 2.0, 0.5)).unwrap();
        assert!(discharge.violations.is_empty(), "{:?}", discharge.violations);
        let charge = envelope.evaluate(&point("CHARGE", -2.0, 0.5)).unwrap();
        assert!(charge.violations.is_empty(), "{:?}", charge.violations);
    }

    #[test]
    fn negative_p_point_is_accepted_no_power_factor_assumption_exists() {
        // This test FAILS if a power-factor inequality assuming P >= 0 is
        // ever introduced (§11.3; ledger P3.08 step 3): a PF-assumption would
        // reject this negative-P (charging) point that satisfies P² + Q² ≤
        // S_max² and its declared per-mode window.
        let envelope = InverterPqEnvelope::new(base_config()).unwrap();
        let evaluation = envelope.evaluate(&point("CHARGE", -2.0, 1.0)).unwrap();
        assert!(
            evaluation.violations.is_empty(),
            "a negative-P point inside the envelope must be accepted; violations: {:?}",
            evaluation.violations
        );
        assert_eq!(evaluation.effective_limits.p_min_mw, -2.0);
    }

    // --- per-mode tables --------------------------------------------------

    #[test]
    fn p_window_violations_are_named_per_mode() {
        let envelope = InverterPqEnvelope::new(base_config()).unwrap();
        let evaluation = envelope.evaluate(&point("DISCHARGE", 3.5, 0.0)).unwrap();
        assert!(matches!(
            evaluation.violations.first(),
            Some(PqViolation::PAboveMax { p_mw, limit_mw } if *p_mw == 3.5 && *limit_mw == 3.0)
        ));
        let evaluation = envelope.evaluate(&point("CHARGE", -2.5, 0.0)).unwrap();
        assert!(matches!(
            evaluation.violations.first(),
            Some(PqViolation::PBelowMin { p_mw, limit_mw } if *p_mw == -2.5 && *limit_mw == -2.0)
        ));
    }

    #[test]
    fn q_limits_resolve_by_p_band() {
        let mut config = base_config();
        // Two P-bands with different Q windows for the DISCHARGE mode.
        config.modes[0].q_rows = vec![
            q_row((0.0, 1.0), None, -1.5, 1.5),
            q_row((1.0, 3.0), None, -0.5, 0.5),
        ];
        let envelope = InverterPqEnvelope::new(config).unwrap();
        // P in the first band: wide Q window.
        let evaluation = envelope.evaluate(&point("DISCHARGE", 0.5, 1.4)).unwrap();
        assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
        // P in the second band: narrow Q window; 1.0 MVAr exceeds it.
        let evaluation = envelope.evaluate(&point("DISCHARGE", 2.0, 1.0)).unwrap();
        assert!(matches!(
            evaluation.violations.iter().find(|v| matches!(v, PqViolation::QAboveMax { .. })),
            Some(PqViolation::QAboveMax { limit_mvar, .. } if *limit_mvar == 0.5)
        ));
    }

    #[test]
    fn uncovered_p_point_collapses_q_to_zero() {
        let mut config = base_config();
        config.modes[0].q_rows = vec![q_row((0.0, 1.0), None, -1.5, 1.5)];
        let envelope = InverterPqEnvelope::new(config).unwrap();
        // P = 2.0 is outside every Q-row band → Q collapses to [0, 0].
        let evaluation = envelope.evaluate(&point("DISCHARGE", 2.0, 0.1)).unwrap();
        assert_eq!(evaluation.effective_limits.q_min_mvar, 0.0);
        assert_eq!(evaluation.effective_limits.q_max_mvar, 0.0);
        assert!(matches!(
            evaluation.violations.iter().find(|v| matches!(v, PqViolation::QAboveMax { .. })),
            Some(PqViolation::QAboveMax { limit_mvar, .. } if *limit_mvar == 0.0)
        ));
        assert!(evaluation
            .provenance
            .unknown_dimensions
            .iter()
            .any(|d| d.dimension == "REACTIVE_POWER"));
    }

    // --- voltage handling -------------------------------------------------

    #[test]
    fn missing_voltage_defaults_to_the_most_restrictive_documented_value() {
        let mut config = base_config();
        config.modes[0].p_rows = vec![
            discharge_row(Some(11.0), 0.0, 3.0),   // wide at 11 kV
            discharge_row(Some(0.4), 0.0, 1.0),    // restrictive at 0.4 kV
        ];
        let envelope = InverterPqEnvelope::new(config).unwrap();
        // Missing voltage → intersection across documented voltages →
        // [0, 1] MW; P = 2 MW exceeds the most restrictive limit.
        let evaluation =
            envelope.evaluate(&PqPoint { mode: "DISCHARGE".into(), p_mw: 2.0, q_mvar: 0.0, voltage_kv: None })
                .unwrap();
        assert_eq!(evaluation.effective_limits.p_max_mw, 1.0);
        assert!(matches!(
            evaluation.violations.first(),
            Some(PqViolation::PAboveMax { limit_mw, .. } if *limit_mw == 1.0)
        ));
        assert!(matches!(
            evaluation.provenance.voltage_treatment,
            VoltageTreatment::MostRestrictiveDocumented { .. }
        ));
        // Documented voltage → that row's window.
        let evaluation =
            envelope.evaluate(&PqPoint { mode: "DISCHARGE".into(), p_mw: 2.0, q_mvar: 0.0, voltage_kv: Some(11.0) })
                .unwrap();
        assert!(evaluation.violations.is_empty(), "{:?}", evaluation.violations);
        assert_eq!(evaluation.effective_limits.p_max_mw, 3.0);
    }

    #[test]
    fn undocumented_voltage_also_defaults_to_the_most_restrictive_value() {
        let mut config = base_config();
        config.modes[0].p_rows = vec![
            discharge_row(Some(11.0), 0.0, 3.0),
            discharge_row(Some(0.4), 0.0, 1.0),
        ];
        let envelope = InverterPqEnvelope::new(config).unwrap();
        let evaluation =
            envelope.evaluate(&PqPoint { mode: "DISCHARGE".into(), p_mw: 2.0, q_mvar: 0.0, voltage_kv: Some(6.6) })
                .unwrap();
        assert_eq!(evaluation.effective_limits.p_max_mw, 1.0);
        assert!(matches!(
            evaluation.provenance.voltage_treatment,
            VoltageTreatment::MostRestrictiveDocumented { .. }
        ));
    }

    // --- unknown narrows ---------------------------------------------------

    #[test]
    fn undeclared_mode_credits_nothing() {
        let envelope = InverterPqEnvelope::new(base_config()).unwrap();
        let evaluation = envelope.evaluate(&point("GRID_SUPPORT", 0.0, 0.0)).unwrap();
        assert!(matches!(
            evaluation.violations.first(),
            Some(PqViolation::ModeNotDeclared { mode } if mode == "GRID_SUPPORT")
        ));
        assert_eq!(evaluation.provenance.mode_provenance, Provenance::Unknown);
    }

    #[test]
    fn unknown_provenance_mode_credits_nothing() {
        let mut config = base_config();
        config.modes.push(PqModeLimits {
            mode: "WATT_VAR".to_string(),
            provenance: Provenance::Unknown,
            p_rows: vec![discharge_row(None, 0.0, 3.0)],
            q_rows: vec![q_row((0.0, 3.0), None, -1.5, 1.5)],
        });
        let envelope = InverterPqEnvelope::new(config).unwrap();
        // Even the origin is not credited: mode credit requires known
        // provenance (§11.1).
        let evaluation = envelope.evaluate(&point("WATT_VAR", 0.0, 0.0)).unwrap();
        assert!(matches!(
            evaluation.violations.first(),
            Some(PqViolation::ModeNotCredited { mode, .. } if mode == "WATT_VAR")
        ));
    }
}
