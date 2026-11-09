// P3.09 — Volt-var / volt-watt capability scoping (pure rules, no I/O).
//
// Implements ledger P3.09 ("Volt-var / volt-watt capability scoping"), report
// §11.3 (inverter P-Q envelope) and §25.2 (D3 network class). Decision record:
// `ledger/phase-03-flexibility-scheduling/09_11_load_ev_models.md`.
//
// THE SCOPING RULE, DECIDED ONCE:
//
//   **A volt-var or volt-watt response is credited ONLY where a curve is
//   configured with a traceable source (device datasheet or utility profile).
//   Where curves are absent — or where the only "source" is a labelled
//   ASSUMPTION — the scheduler must NOT assume a volt-var response: the
//   capability is recorded UNKNOWN per the frozen P3.02 provenance rule and
//   the dimension narrows conservatively (unknown never widens).**
//
// - The per-profile scoping record ([`ProfileVoltVarScope`]) carries, for
//   each inverter-bearing behaviour profile of the synthetic portfolio,
//   whether a volt-var and a volt-watt curve is configured and its source.
//   The survey of scenario-v2.json is in
//   [`synthetic_portfolio_scopes`]: the scenario declares NO curve data for
//   any profile (verified against the scenario file — an absence of
//   configuration, not an assumption), so every profile scopes to
//   NOT-CONFIGURED with UNKNOWN provenance.
// - **Effect on the envelope (ledger P3.09 step 3)**: an unconfigured profile
//   credits NO volt-var/volt-watt operating points. The narrowing is applied
//   with the frozen P3.02 rule ([`crate::capability_provenance::narrow_interval`]
//   for [`AssetClass::InverterPq`]) — for a declared Q window the effective
//   volt-var credit collapses to [0, 0] MVAr (the recorded collapse-to-zero
//   fallback), never "unconstrained", and no invented curve enters any
//   envelope input. The P-Q envelope of P3.08 carries the numeric tables;
//   this module decides whether a volt-var/volt-watt table may be assumed at
//   all. It must be consulted before any curve is added to an envelope.
// - A curve whose declared source is [`CurveSource::Assumption`] carries
//   UNKNOWN provenance and likewise credits nothing: an assumption is not
//   datasheet or utility data. It becomes creditable only when re-pointed to
//   a traceable source. The assumption label travels on the scope record so
//   the provenance discipline stays visible (§11.1: adapters state which
//   limits are measured, configured, inferred, or unknown).
// - **IEEE 1547-2018 [R18] scope is unchanged from P3.08**: Clause 5 covers
//   reactive-power/voltage-power capability categories; the exact capability
//   and settings depend on the applicable category, performance
//   requirements, utility profile, and equipment. Scoping a profile to
//   "no curve configured" claims nothing about what any real device does or
//   does not implement, and nothing here claims conformance with IEEE 1547
//   or any other standard.
//
// Control status: pure domain rules for the simulation-only 0.x line. No I/O,
// no solver coupling, no device-write path. FlexCore performs no inverter
// inner-loop control and no protection-grade functionality (AGENTS.md
// rules 2–3).

use flexcore_contracts::capability::{ConstraintKind, Provenance};

use crate::capability_provenance::{narrow_interval, AssetClass, BoundInterval, NarrowedSet};

/// The recorded reason carried on every NOT-CONFIGURED scope (ledger P3.09
/// step 2 — the scheduler must not assume a volt-var response).
pub const VOLT_VAR_UNCONFIGURED_REASON: &str =
    "no volt-var/volt-watt curve is configured for this profile in \
scenario-v2.json; the scheduler must NOT assume a volt-var response; the \
capability is recorded UNKNOWN per the frozen P3.02 provenance rule and \
narrows conservatively (unknown never widens)";

/// The recorded reason carried when a curve's only source is a labelled
/// ASSUMPTION (an assumption is not datasheet or utility data).
pub const VOLT_VAR_ASSUMPTION_REASON: &str =
    "the curve source is a labelled ASSUMPTION, not traceable device-datasheet \
or utility-profile data; an assumption carries UNKNOWN provenance and credits \
no volt-var/volt-watt operating points (P3.02: unknown never widens) until it \
is re-pointed to a traceable source";

// --- Scoping types -----------------------------------------------------------

/// The declared source of a configured volt-var/volt-watt curve. Every value
/// is a named provenance category — there is no unnamed default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CurveSource {
    /// The curve comes from the device datasheet (nameplate documentation).
    DeviceDatasheet,
    /// The curve comes from the applicable utility profile (interconnection
    /// / operating profile).
    UtilityProfile,
    /// The curve exists only as a labelled assumption. An assumption carries
    /// UNKNOWN provenance and credits nothing (see
    /// [`VOLT_VAR_ASSUMPTION_REASON`]).
    Assumption,
}

impl CurveSource {
    /// The canonical label of the source (audit-safe vocabulary).
    pub const fn label(self) -> &'static str {
        match self {
            CurveSource::DeviceDatasheet => "DEVICE_DATASHEET",
            CurveSource::UtilityProfile => "UTILITY_PROFILE",
            CurveSource::Assumption => "ASSUMPTION",
        }
    }

    /// The provenance the source maps to under the P3.02 rule: datasheet and
    /// utility curves are CONFIGURED (named configuration sources);
    /// assumptions are UNKNOWN (they narrow, never widen).
    pub const fn provenance(self) -> Provenance {
        match self {
            CurveSource::DeviceDatasheet | CurveSource::UtilityProfile => Provenance::Configured,
            CurveSource::Assumption => Provenance::Unknown,
        }
    }
}

/// Whether one curve family (volt-var or volt-watt) is configured for a
/// profile, and from where.
#[derive(Debug, Clone, PartialEq)]
pub enum CurveStatus {
    /// A curve is configured from the declared source. A non-empty note
    /// records where it came from.
    Configured {
        /// The declared source of the curve.
        source: CurveSource,
        /// The recorded note (audit-safe).
        note: String,
    },
    /// No curve is configured. The recorded reason states the absence —
    /// an unconfigured capability is never silently treated as
    /// unconstrained (P3.02).
    NotConfigured {
        /// The recorded reason (audit-safe).
        reason: String,
    },
}

/// The per-profile volt-var/volt-watt scoping record (ledger P3.09
/// deliverable: one row of the scoping table, in typed form).
#[derive(Debug, Clone, PartialEq)]
pub struct ProfileVoltVarScope {
    /// The behaviour-profile ID (as declared in scenario-v2.json).
    pub profile_id: String,
    /// The volt-var curve scoping.
    pub volt_var: CurveStatus,
    /// The volt-watt curve scoping.
    pub volt_watt: CurveStatus,
}

impl ProfileVoltVarScope {
    /// The recorded default scope for a profile with no curve data: both
    /// curve families NOT-CONFIGURED with the recorded reason. There is no
    /// constructor that manufactures a configured scope from nothing — a
    /// configured scope must be declared explicitly by the caller.
    pub fn unconfigured(profile_id: &str) -> Self {
        Self {
            profile_id: profile_id.to_string(),
            volt_var: CurveStatus::NotConfigured {
                reason: VOLT_VAR_UNCONFIGURED_REASON.to_string(),
            },
            volt_watt: CurveStatus::NotConfigured {
                reason: VOLT_VAR_UNCONFIGURED_REASON.to_string(),
            },
        }
    }

    /// The volt-var curve source, when one is configured from a traceable
    /// source (datasheet or utility profile). `None` for an unconfigured
    /// profile AND for an assumption-only curve — no volt-var contribution
    /// may be assumed in either case.
    pub fn volt_var_contribution(&self) -> Option<CurveSource> {
        match &self.volt_var {
            CurveStatus::Configured { source: s @ (CurveSource::DeviceDatasheet | CurveSource::UtilityProfile), .. } => {
                Some(*s)
            }
            _ => None,
        }
    }

    /// The volt-watt curve source, when one is configured from a traceable
    /// source (same rule as [`ProfileVoltVarScope::volt_var_contribution`]).
    pub fn volt_watt_contribution(&self) -> Option<CurveSource> {
        match &self.volt_watt {
            CurveStatus::Configured { source: s @ (CurveSource::DeviceDatasheet | CurveSource::UtilityProfile), .. } => {
                Some(*s)
            }
            _ => None,
        }
    }

    /// The provenance the volt-var dimension carries into the envelope:
    /// CONFIGURED for a traceable source, UNKNOWN otherwise (unconfigured or
    /// assumption-only — unknown never widens, P3.02).
    pub fn volt_var_provenance(&self) -> Provenance {
        match &self.volt_var {
            CurveStatus::Configured { source, .. } => source.provenance(),
            CurveStatus::NotConfigured { .. } => Provenance::Unknown,
        }
    }

    /// The provenance the volt-watt dimension carries into the envelope.
    pub fn volt_watt_provenance(&self) -> Provenance {
        match &self.volt_watt {
            CurveStatus::Configured { source, .. } => source.provenance(),
            CurveStatus::NotConfigured { .. } => Provenance::Unknown,
        }
    }
}

// --- Envelope effect -----------------------------------------------------------

/// The effect of a profile's scoping on its flexibility envelope (ledger
/// P3.09 step 3), recorded — never silent.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvelopeEffect {
    /// A curve is configured from a traceable source: the declared curve may
    /// be credited per its declared (CONFIGURED) provenance inside the P3.08
    /// envelope tables. The envelope still credits only the declared tables —
    /// never an interpolated or invented curve.
    CreditedPerDeclaredProvenance {
        /// The traceable source of the curve.
        source: CurveSource,
    },
    /// No volt-var/volt-watt credit: the curves are absent or assumption-
    /// only, the capability is recorded UNKNOWN, and the dimension narrows
    /// conservatively via the frozen P3.02 rule. No invented curve appears
    /// in any envelope input.
    NoVoltVarCredit {
        /// The recorded reason (audit-safe).
        reason: &'static str,
    },
}

/// The envelope effect of the profile's volt-var scoping.
pub fn volt_var_envelope_effect(scope: &ProfileVoltVarScope) -> EnvelopeEffect {
    match scope.volt_var_contribution() {
        Some(source) => EnvelopeEffect::CreditedPerDeclaredProvenance { source },
        None => match &scope.volt_var {
            CurveStatus::Configured { .. } => {
                EnvelopeEffect::NoVoltVarCredit { reason: VOLT_VAR_ASSUMPTION_REASON }
            }
            CurveStatus::NotConfigured { .. } => {
                EnvelopeEffect::NoVoltVarCredit { reason: VOLT_VAR_UNCONFIGURED_REASON }
            }
        },
    }
}

/// The envelope effect of the profile's volt-watt scoping.
pub fn volt_watt_envelope_effect(scope: &ProfileVoltVarScope) -> EnvelopeEffect {
    match scope.volt_watt_contribution() {
        Some(source) => EnvelopeEffect::CreditedPerDeclaredProvenance { source },
        None => match &scope.volt_watt {
            CurveStatus::Configured { .. } => {
                EnvelopeEffect::NoVoltVarCredit { reason: VOLT_VAR_ASSUMPTION_REASON }
            }
            CurveStatus::NotConfigured { .. } => {
                EnvelopeEffect::NoVoltVarCredit { reason: VOLT_VAR_UNCONFIGURED_REASON }
            }
        },
    }
}

// --- The narrowing (frozen P3.02 rule, reused, not reimplemented) ------------

/// Applies the frozen P3.02 rule to the volt-var dimension of a declared
/// reactive-power window: a traceable source keeps the declared window as
/// declared; an unconfigured or assumption-only scope narrows via UNKNOWN
/// provenance — the declared window ∩ the conservative default for
/// `INVERTER_PQ`/`REACTIVE_POWER` (collapse-to-zero; empty intersection →
/// infeasible, never widened back).
pub fn narrow_volt_var_reactive_credit(
    scope: &ProfileVoltVarScope,
    declared_q_window: BoundInterval,
) -> NarrowedSet {
    narrow_interval(
        AssetClass::InverterPq,
        ConstraintKind::ReactivePower,
        scope.volt_var_provenance(),
        declared_q_window,
    )
}

/// Applies the frozen P3.02 rule to the volt-watt dimension of a declared
/// active-power window (same rule as
/// [`narrow_volt_var_reactive_credit`], `ACTIVE_POWER` kind).
pub fn narrow_volt_watt_active_credit(
    scope: &ProfileVoltVarScope,
    declared_p_window: BoundInterval,
) -> NarrowedSet {
    narrow_interval(
        AssetClass::InverterPq,
        ConstraintKind::ActivePower,
        scope.volt_watt_provenance(),
        declared_p_window,
    )
}

// --- The synthetic-portfolio survey -------------------------------------------

/// The per-profile scoping survey of the synthetic portfolio
/// (scenario-v2.json): every behaviour profile declares NO volt-var or
/// volt-watt curve data (verified against the scenario file — the scenario
/// format has no curve fields at all), so every profile scopes to
/// NOT-CONFIGURED with UNKNOWN provenance. The typed survey is the
/// code-side mirror of the scoping table in the decision record
/// `ledger/phase-03-flexibility-scheduling/09_11_load_ev_models.md`.
///
/// Only the two inverter-bearing profile families (`bess_daily_cycle`,
/// `pv_clear_sky`) carry an inverter P-Q envelope at all; the load, EVSE,
/// and site-meter profiles are recorded for completeness with the same
/// not-configured status (they have no declared reactive capability either,
/// so no volt-var scope exists to assume there either).
pub fn synthetic_portfolio_scopes() -> Vec<ProfileVoltVarScope> {
    [
        "bess_daily_cycle",
        "pv_clear_sky",
        "load_baseline_reducible",
        "evse_connection_windows",
        "site_meter_aggregate",
    ]
    .iter()
    .map(|id| ProfileVoltVarScope::unconfigured(id))
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q_window() -> BoundInterval {
        BoundInterval::new(Some(-1.5), Some(1.5)).unwrap()
    }

    // --- unconfigured profiles credit nothing -----------------------------

    #[test]
    fn unconfigured_profile_yields_no_volt_var_contribution() {
        // Ledger P3.09 acceptance: an unconfigured profile yields NO
        // volt-var contribution — no invented curve appears in any envelope
        // input.
        let scope = ProfileVoltVarScope::unconfigured("pv_clear_sky");
        assert_eq!(scope.volt_var_contribution(), None);
        assert_eq!(scope.volt_watt_contribution(), None);
        assert_eq!(scope.volt_var_provenance(), Provenance::Unknown);
        assert_eq!(scope.volt_watt_provenance(), Provenance::Unknown);
        assert!(matches!(
            volt_var_envelope_effect(&scope),
            EnvelopeEffect::NoVoltVarCredit { .. }
        ));
        assert!(matches!(
            volt_watt_envelope_effect(&scope),
            EnvelopeEffect::NoVoltVarCredit { .. }
        ));
    }

    #[test]
    fn unconfigured_profile_narrows_a_declared_q_window_to_zero_credit() {
        // The envelope effect: even against a declared Q window of
        // [-1.5, 1.5] MVAr, the unknown-provenance volt-var dimension
        // collapses to [0, 0] MVAr — the recorded P3.02 collapse-to-zero
        // fallback, never "unconstrained".
        let scope = ProfileVoltVarScope::unconfigured("bess_daily_cycle");
        let narrowed = narrow_volt_var_reactive_credit(&scope, q_window());
        assert_eq!(
            narrowed,
            NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) }
        );
        // And the volt-watt side likewise against a declared P window.
        let p_window = BoundInterval::new(Some(0.0), Some(3.0)).unwrap();
        let narrowed = narrow_volt_watt_active_credit(&scope, p_window);
        assert_eq!(
            narrowed,
            NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) }
        );
    }

    // --- traceable sources keep the declared window ------------------------

    #[test]
    fn traceable_sources_credit_the_declared_window() {
        for source in [CurveSource::DeviceDatasheet, CurveSource::UtilityProfile] {
            let scope = ProfileVoltVarScope {
                profile_id: "future-profile".to_string(),
                volt_var: CurveStatus::Configured {
                    source,
                    note: "declared curve, traceable source".to_string(),
                },
                volt_watt: CurveStatus::NotConfigured {
                    reason: VOLT_VAR_UNCONFIGURED_REASON.to_string(),
                },
            };
            assert_eq!(scope.volt_var_contribution(), Some(source));
            assert_eq!(scope.volt_var_provenance(), Provenance::Configured);
            assert!(matches!(
                volt_var_envelope_effect(&scope),
                EnvelopeEffect::CreditedPerDeclaredProvenance { .. }
            ));
            let narrowed = narrow_volt_var_reactive_credit(&scope, q_window());
            assert_eq!(
                narrowed,
                NarrowedSet::Interval { lower: Some(-1.5), upper: Some(1.5) }
            );
            // The unconfigured volt-watt side still credits nothing.
            assert_eq!(scope.volt_watt_contribution(), None);
            assert_eq!(
                narrow_volt_watt_active_credit(
                    &scope,
                    BoundInterval::new(Some(0.0), Some(3.0)).unwrap()
                ),
                NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) }
            );
        }
    }

    #[test]
    fn assumption_sourced_curve_credits_nothing() {
        // A labelled ASSUMPTION is not datasheet or utility data: UNKNOWN
        // provenance, no contribution, zero credit.
        let scope = ProfileVoltVarScope {
            profile_id: "assumed-profile".to_string(),
            volt_var: CurveStatus::Configured {
                source: CurveSource::Assumption,
                note: "ASSUMPTION: shape taken from a textbook volt-var curve"
                    .to_string(),
            },
            volt_watt: CurveStatus::Configured {
                source: CurveSource::Assumption,
                note: "ASSUMPTION: no volt-watt curve assumed".to_string(),
            },
        };
        assert_eq!(scope.volt_var_contribution(), None);
        assert_eq!(scope.volt_watt_contribution(), None);
        assert_eq!(scope.volt_var_provenance(), Provenance::Unknown);
        assert!(matches!(
            volt_var_envelope_effect(&scope),
            EnvelopeEffect::NoVoltVarCredit { reason: VOLT_VAR_ASSUMPTION_REASON }
        ));
        let narrowed = narrow_volt_var_reactive_credit(&scope, q_window());
        assert_eq!(
            narrowed,
            NarrowedSet::Interval { lower: Some(0.0), upper: Some(0.0) }
        );
    }

    // --- the synthetic-portfolio survey -------------------------------------

    #[test]
    fn synthetic_portfolio_scopes_match_scenario_v2_json() {
        // Every behaviour profile of scenario-v2.json, all NOT-CONFIGURED
        // (the scenario declares no curve fields for any profile).
        let scopes = synthetic_portfolio_scopes();
        let ids: Vec<&str> = scopes
            .iter()
            .map(|s| s.profile_id.as_str())
            .collect();
        assert_eq!(
            ids,
            vec![
                "bess_daily_cycle",
                "pv_clear_sky",
                "load_baseline_reducible",
                "evse_connection_windows",
                "site_meter_aggregate",
            ]
        );
        for scope in &scopes {
            assert_eq!(scope.volt_var_contribution(), None, "{}", scope.profile_id);
            assert_eq!(scope.volt_watt_contribution(), None, "{}", scope.profile_id);
            assert_eq!(scope.volt_var_provenance(), Provenance::Unknown);
            assert!(matches!(
                volt_var_envelope_effect(scope),
                EnvelopeEffect::NoVoltVarCredit { .. }
            ));
        }
    }

    // --- source labels -------------------------------------------------------

    #[test]
    fn curve_source_labels_and_provenance_mapping() {
        assert_eq!(CurveSource::DeviceDatasheet.label(), "DEVICE_DATASHEET");
        assert_eq!(CurveSource::UtilityProfile.label(), "UTILITY_PROFILE");
        assert_eq!(CurveSource::Assumption.label(), "ASSUMPTION");
        assert_eq!(CurveSource::DeviceDatasheet.provenance(), Provenance::Configured);
        assert_eq!(CurveSource::UtilityProfile.provenance(), Provenance::Configured);
        assert_eq!(CurveSource::Assumption.provenance(), Provenance::Unknown);
    }
}
