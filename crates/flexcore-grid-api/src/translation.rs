// P4.02 — The canonical-units translation layer (report §10.2; ledger
// P4.02 step 2: "the adapter must translate FlexCore's canonical units/sign
// into the provider's model and translate results back — the translation
// layer is owned, tested code, not an assumption").
//
// Owned mapping, both directions, every rule frozen in
// flexcore-contracts (`units.rs`) and the report §10.2:
//
// | Quantity        | FlexCore canonical                       | Worker model |
// |---|---|---|
// | Active power    | MW; `P > 0` = EXPORT at the PCC (frozen) | injection into the network at the bus is positive (`p_injection_mw`) |
// | Reactive power  | MVAr; sign per the DECLARED `ReactiveSignConvention` — never inferred | injection into the network at the bus is positive (`q_injection_mvar`) |
// | Voltage         | kV at the nominal base AND per-unit in the solver representation | per-unit of the bus nominal voltage (`vm_pu`) |
// | Current         | A (or per-unit on an explicit base)      | kA |
// | Loading         | fraction of the element's declared rating | percent |
//
// Round-trip guarantees (tested in `tests/translation_roundtrip.rs` with
// hand-computed fixtures, both directions):
// - `candidate_to_backend` then `injection_from_backend` reproduces the
//   canonical candidate exactly (float-identical values), under BOTH
//   declared reactive conventions.
// - `voltage_kv_from_pu(voltage_pu_from_kv(kv, nominal), nominal)` returns
//   the original kV within float tolerance; same for the current and
//   loading conversions.
//
// The reactive-power rule is deliberately asymmetric: the canonical value
// is only interpretable under the convention DECLARED by the governing
// grid model and device profile, so every conversion that touches Q takes
// the convention as an explicit argument. There is no default and no
// conversion that reads the sign alone (foundations §6.2; the contracts
// crate offers no such path either — `interpret_reactive_power` requires
// the convention).

use crate::backend::{BackendBranchResult, BackendBusResult, BackendInjection, BackendTransformerResult};
use crate::port::{
    BranchLoading, BranchSpec, BusSpec, BusVoltage, CandidateInjection, TransformerLoading,
    TransformerSpec,
};
use serde::{Deserialize, Serialize};
use flexcore_contracts::units::{
    ActivePowerMW, PerUnitBase, ReactivePowerMVAr, ReactiveSignConvention, UnitsError,
};

/// Translation failures. These are contract-level input/unit errors —
/// never power-flow outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TranslationError {
    /// A numeric value is NaN or infinite.
    NonFinite { field: &'static str },
    /// A value that must be strictly positive is not.
    NonPositive { field: &'static str },
    /// A magnitude that must be non-negative is negative (voltage
    /// magnitudes, currents).
    NegativeMagnitude { field: &'static str },
    /// A canonical contract type rejected the value (the contracts error
    /// is recorded verbatim).
    Units(UnitsError),
    /// The declared reactive sign convention is required and was not
    /// supplied — the caller cannot leave Q's sign undeclared.
    MissingReactiveConvention,
    /// A result row's identifier does not match the model element it was
    /// mapped to (contract ordering broken upstream).
    IdentifierMismatch {
        element: &'static str,
        expected: String,
        found: String,
    },
}

impl std::fmt::Display for TranslationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslationError::NonFinite { field } => write!(f, "value '{field}' must be finite"),
            TranslationError::NonPositive { field } => {
                write!(f, "value '{field}' must be strictly positive")
            }
            TranslationError::NegativeMagnitude { field } => {
                write!(f, "magnitude '{field}' must be non-negative")
            }
            TranslationError::Units(error) => write!(f, "canonical unit violation: {error}"),
            TranslationError::MissingReactiveConvention => {
                write!(
                    f, "the reactive sign convention is required; Q is never interpreted \
                        without the declared convention (§10.2)"
                )
            }
            TranslationError::IdentifierMismatch {
                element,
                expected,
                found,
            } => write!(
                f,
                "result row for {element} '{found}' does not match the model element '{expected}'"
            ),
        }
    }
}

impl std::error::Error for TranslationError {}

fn require_finite(field: &'static str, value: f64) -> Result<(), TranslationError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(TranslationError::NonFinite { field })
    }
}

/// The translation profile of one evaluation: the declared reactive sign
/// convention of the governing grid model and device profile, and the
/// explicit per-unit base of the solver representation (report §10.2:
/// voltage kV at the nominal base / per-unit in the solver representation;
/// current A or per-unit on an explicit base — `PerUnitBase` is the
/// contracts-owned explicit base).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TranslationProfile {
    /// The DECLARED reactive sign convention (no default — §10.2).
    pub reactive_convention: ReactiveSignConvention,
    /// The explicit per-unit base (base kV and base MVA).
    pub base: PerUnitBase,
}

impl TranslationProfile {
    /// Constructs a profile; `PerUnitBase`'s own constructor has validated
    /// the base, so nothing further is required here.
    pub fn new(
        reactive_convention: ReactiveSignConvention,
        base: PerUnitBase,
    ) -> Self {
        Self {
            reactive_convention,
            base,
        }
    }

    /// The base current in A for a voltage level in kV:
    /// `I_base = S_base / (sqrt(3) * V)` (hand-checked in tests:
    /// 100 MVA at 20 kV gives 2886.751... A).
    pub fn base_current_a(&self, voltage_kv: f64) -> Result<f64, TranslationError> {
        require_finite("voltage_kv", voltage_kv)?;
        if voltage_kv <= 0.0 {
            return Err(TranslationError::NonPositive { field: "voltage_kv" });
        }
        let base_va = self.base.base_mva * 1.0e6;
        let base_v = voltage_kv * 1.0e3;
        Ok(base_va / (f64::sqrt(3.0) * base_v))
    }
}

// ---------------------------------------------------------------------------
// Canonical -> worker model (injections)
// ---------------------------------------------------------------------------

/// Maps one canonical candidate injection onto the worker's injection
/// element (canonical units → worker model, direction 1).
///
/// Active power: canonical `P > 0` (export) maps to a positive injection
/// into the network; `P < 0` (import) maps to a negative injection. The
/// value crosses unchanged in MW — the SIGN MEANING is what the mapping
/// fixes (VERIFY-BUILD item 5 in `backend.rs` documents the worker-side
/// element materialisation).
///
/// Reactive power: the canonical MVAr value is interpreted under the
/// candidate's DECLARED convention — injection-positive passes the sign
/// through; consumption-positive negates it. This is the only place the
/// convention is applied, and it is applied identically in the inverse
/// mapping, which is what makes the round trip exact.
pub fn candidate_to_backend(candidate: &CandidateInjection) -> Result<BackendInjection, TranslationError> {
    let p_mw = candidate.active_power.value();
    require_finite("active_power_mw", p_mw)?;
    let q_mvar = candidate.reactive_power.value();
    require_finite("reactive_power_mvar", q_mvar)?;

    let q_injection_mvar = match candidate.reactive_sign_convention {
        ReactiveSignConvention::InjectionPositive => q_mvar,
        ReactiveSignConvention::ConsumptionPositive => -q_mvar,
    };

    Ok(BackendInjection {
        injection_id: candidate.injection_id.clone(),
        bus_id: candidate.bus_id.clone(),
        p_injection_mw: p_mw,
        q_injection_mvar,
        interval_start: candidate.interval_start,
    })
}

/// Maps one worker injection back onto the canonical representation
/// (worker model → canonical units, direction 2 — the exact inverse of
/// [`candidate_to_backend`] for the same declared convention).
pub fn injection_from_backend(
    injection: &BackendInjection,
    reactive_convention: ReactiveSignConvention,
) -> Result<CandidateInjection, TranslationError> {
    require_finite("p_injection_mw", injection.p_injection_mw)?;
    require_finite("q_injection_mvar", injection.q_injection_mvar)?;

    let q_mvar = match reactive_convention {
        ReactiveSignConvention::InjectionPositive => injection.q_injection_mvar,
        ReactiveSignConvention::ConsumptionPositive => -injection.q_injection_mvar,
    };

    Ok(CandidateInjection {
        injection_id: injection.injection_id.clone(),
        bus_id: injection.bus_id.clone(),
        active_power: ActivePowerMW::new(injection.p_injection_mw).map_err(TranslationError::Units)?,
        reactive_power: ReactivePowerMVAr::new(q_mvar).map_err(TranslationError::Units)?,
        reactive_sign_convention: reactive_convention,
        interval_start: injection.interval_start,
    })
}

// ---------------------------------------------------------------------------
// Worker results -> canonical representations
// ---------------------------------------------------------------------------

/// Converts one worker bus result into the canonical bus voltage with BOTH
/// representations (kV at the nominal base and per-unit of the bus nominal
/// voltage).
pub fn bus_voltage_from_backend(
    bus: &BusSpec,
    result: &BackendBusResult,
) -> Result<BusVoltage, TranslationError> {
    if bus.bus_id != result.bus_id {
        return Err(TranslationError::IdentifierMismatch {
            element: "bus",
            expected: bus.bus_id.clone(),
            found: result.bus_id.clone(),
        });
    }
    let vm_kv = voltage_kv_from_pu(result.vm_pu, bus.nominal_kv)?;
    Ok(BusVoltage {
        bus_id: bus.bus_id.clone(),
        vm_pu: result.vm_pu,
        vm_kv,
    })
}

/// Per-unit → kV at the nominal base: `V = vm_pu * V_nominal`.
pub fn voltage_kv_from_pu(vm_pu: f64, nominal_kv: f64) -> Result<f64, TranslationError> {
    require_finite("vm_pu", vm_pu)?;
    require_finite("nominal_kv", nominal_kv)?;
    if vm_pu < 0.0 {
        return Err(TranslationError::NegativeMagnitude { field: "vm_pu" });
    }
    if nominal_kv <= 0.0 {
        return Err(TranslationError::NonPositive { field: "nominal_kv" });
    }
    Ok(vm_pu * nominal_kv)
}

/// kV at the nominal base → per-unit: `vm_pu = V / V_nominal` (the inverse
/// of [`voltage_kv_from_pu`]).
pub fn voltage_pu_from_kv(vm_kv: f64, nominal_kv: f64) -> Result<f64, TranslationError> {
    require_finite("vm_kv", vm_kv)?;
    require_finite("nominal_kv", nominal_kv)?;
    if vm_kv < 0.0 {
        return Err(TranslationError::NegativeMagnitude { field: "vm_kv" });
    }
    if nominal_kv <= 0.0 {
        return Err(TranslationError::NonPositive { field: "nominal_kv" });
    }
    Ok(vm_kv / nominal_kv)
}

/// kA → A (the worker reports currents in kA; canonical is A).
pub fn current_a_from_ka(i_ka: f64) -> Result<f64, TranslationError> {
    require_finite("i_ka", i_ka)?;
    Ok(i_ka * 1.0e3)
}

/// A → kA (the inverse of [`current_a_from_ka`]).
pub fn current_ka_from_a(i_a: f64) -> Result<f64, TranslationError> {
    require_finite("i_a", i_a)?;
    Ok(i_a / 1.0e3)
}

/// Current in A → per-unit on an EXPLICIT base (report §10.2: current is A
/// or per-unit on an explicit base — the base is a required argument,
/// never assumed).
pub fn current_per_unit(i_a: f64, base_current_a: f64) -> Result<f64, TranslationError> {
    require_finite("i_a", i_a)?;
    require_finite("base_current_a", base_current_a)?;
    if i_a < 0.0 {
        return Err(TranslationError::NegativeMagnitude { field: "i_a" });
    }
    if base_current_a <= 0.0 {
        return Err(TranslationError::NonPositive { field: "base_current_a" });
    }
    Ok(i_a / base_current_a)
}

/// Percent → fraction of the declared rating (canonical loading is a
/// fraction; percent is display-only, §10.2).
pub fn loading_fraction_from_percent(loading_percent: f64) -> Result<f64, TranslationError> {
    require_finite("loading_percent", loading_percent)?;
    Ok(loading_percent / 100.0)
}

/// Fraction → percent (the inverse of [`loading_fraction_from_percent`]).
pub fn loading_percent_from_fraction(loading_fraction: f64) -> Result<f64, TranslationError> {
    require_finite("loading_fraction", loading_fraction)?;
    Ok(loading_fraction * 100.0)
}

/// Converts one worker branch result into the canonical branch loading:
/// currents in A, loading as a fraction of the branch's declared rated
/// current (the explicit rating base from the model spec).
pub fn branch_loading_from_backend(
    branch: &BranchSpec,
    result: &BackendBranchResult,
) -> Result<BranchLoading, TranslationError> {
    if branch.branch_id != result.branch_id {
        return Err(TranslationError::IdentifierMismatch {
            element: "branch",
            expected: branch.branch_id.clone(),
            found: result.branch_id.clone(),
        });
    }
    let current_from_a = current_a_from_ka(result.i_from_ka)?;
    let current_to_a = current_a_from_ka(result.i_to_ka)?;
    if current_from_a < 0.0 {
        return Err(TranslationError::NegativeMagnitude { field: "current_from_a" });
    }
    if current_to_a < 0.0 {
        return Err(TranslationError::NegativeMagnitude { field: "current_to_a" });
    }
    require_finite("rated_current_a", branch.rated_current_a)?;
    if branch.rated_current_a <= 0.0 {
        return Err(TranslationError::NonPositive { field: "rated_current_a" });
    }
    let loading_fraction = loading_fraction_from_percent(result.loading_percent)?;
    if loading_fraction < 0.0 {
        return Err(TranslationError::NegativeMagnitude { field: "loading_fraction" });
    }
    Ok(BranchLoading {
        branch_id: branch.branch_id.clone(),
        current_from_a,
        current_to_a,
        loading_fraction,
    })
}

/// Converts one worker transformer result into the canonical transformer
/// loading: loading as a fraction of the transformer's declared rated MVA
/// (the explicit rating base from the model spec), side currents in A.
pub fn transformer_loading_from_backend(
    transformer: &TransformerSpec,
    result: &BackendTransformerResult,
) -> Result<TransformerLoading, TranslationError> {
    if transformer.transformer_id != result.transformer_id {
        return Err(TranslationError::IdentifierMismatch {
            element: "transformer",
            expected: transformer.transformer_id.clone(),
            found: result.transformer_id.clone(),
        });
    }
    let hv_current_a = current_a_from_ka(result.i_hv_ka)?;
    let lv_current_a = current_a_from_ka(result.i_lv_ka)?;
    if hv_current_a < 0.0 {
        return Err(TranslationError::NegativeMagnitude { field: "hv_current_a" });
    }
    if lv_current_a < 0.0 {
        return Err(TranslationError::NegativeMagnitude { field: "lv_current_a" });
    }
    require_finite("rated_mva", transformer.rated_mva)?;
    if transformer.rated_mva <= 0.0 {
        return Err(TranslationError::NonPositive { field: "rated_mva" });
    }
    let loading_fraction = loading_fraction_from_percent(result.loading_percent)?;
    if loading_fraction < 0.0 {
        return Err(TranslationError::NegativeMagnitude { field: "loading_fraction" });
    }
    Ok(TransformerLoading {
        transformer_id: transformer.transformer_id.clone(),
        loading_fraction,
        hv_current_a,
        lv_current_a,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_current_is_hand_computed_for_100mva_at_20kv() {
        // I_base = 100e6 / (sqrt(3) * 20e3) = 2886.7513459... A.
        let profile = TranslationProfile::new(
            ReactiveSignConvention::InjectionPositive,
            PerUnitBase::new(20.0, 100.0).expect("valid base"),
        );
        let base = profile.base_current_a(20.0).expect("computes");
        let expected = 100.0e6 / (f64::sqrt(3.0) * 20.0e3);
        assert!((base - expected).abs() < 1.0e-9);
        assert!((base - 2886.751_3).abs() < 1.0e-3);
    }

    #[test]
    fn per_unit_base_is_validated_by_the_contracts_crate() {
        assert!(PerUnitBase::new(0.0, 100.0).is_err());
        assert!(PerUnitBase::new(20.0, -1.0).is_err());
        assert!(PerUnitBase::new(f64::NAN, 100.0).is_err());
    }
}
