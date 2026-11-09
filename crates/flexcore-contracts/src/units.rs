// P0.07 — Canonical units and sign conventions (Phase 0 freeze)
// P1.05 — flexcore-contracts implementation (moved from the Phase 0 seed in
//         flexcore-domain and upgraded with serde and display-only helpers).
//
// Implements `ledger/phase-00-product-freeze/07_units_sign_conventions.md`.
// Normative source: report §10.2 (units and sign conventions), §11.2 (BESS
// net power); foundations specification §6.2 governs on conflict.
//
// Frozen rules encoded here:
// - Canonical units are MW, MVAr, MVA, MWh (plus kV/A/Hz conventions and SOC
//   as a fraction). No kW, kWh, or other scaled unit appears in any contract;
//   scaled units are display-only and are converted at the adapter.
// - Active power: **P > 0 means export/injection into the grid** at the point
//   of connection; P < 0 means import/consumption from the grid.
// - Reactive power: sign is **not** a universal constant. Each grid model and
//   each device profile declares whether Q > 0 means injection (generator
//   convention) or consumption (load convention); adapters record the mapping
//   they apply; platform contracts carry the declared convention explicitly.
//   Never inferred from sign alone (foundations §6.2). This module offers no
//   default convention: [`interpret_reactive_power`] requires the caller to
//   supply one.
// - SOC is a **fraction in [0,1]** — the single canonical representation at
//   the API boundary. Percentages exist only as display or vendor-source
//   values and are converted at the adapter; a percent value is never
//   persisted as canonical SOC. `StateOfCharge::from_percent` is a separate,
//   explicitly named boundary constructor — never the default path.
// - BESS net power: `P_net = P_dis − P_ch` with `P_dis ≥ 0` and `P_ch ≥ 0`.
//   `P_net > 0` is export, `P_net < 0` is import (sign rule above).
//   Simultaneous charge-and-discharge prohibition is an optimization-
//   formulation concern (report §11.2; implementation is Phase 3).
// - Voltage: kV at the nominal base in the network model; per-unit in the
//   solver representation with the base declared explicitly. Current: A or
//   per-unit on an explicit base, with phase identification.
//
// Serialization note: each unit newtype serializes as its canonical f64 under
// an explicit unit field name (for example `{"mw": 3.5}`), so the canonical
// unit travels with the value; deserialization re-runs the constructor
// validation (finite, range).

use serde::{Deserialize, Serialize};

/// Errors returned by the P0.07 unit types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitsError {
    /// The value is NaN or infinite; canonical quantities must be finite.
    NotFinite,
    /// The value violates the declared range of the type (for example SOC
    /// outside [0,1] or a percent outside [0,100]).
    OutOfRange,
    /// A non-negative power was required but a negative value was supplied
    /// (BESS charge/discharge magnitudes).
    NegativePower,
    /// A base quantity for a per-unit representation must be strictly
    /// positive.
    NonPositiveBase,
}

impl std::fmt::Display for UnitsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnitsError::NotFinite => write!(f, "value must be finite (no NaN or infinity)"),
            UnitsError::OutOfRange => write!(f, "value is outside the declared range"),
            UnitsError::NegativePower => write!(f, "value must be non-negative"),
            UnitsError::NonPositiveBase => write!(f, "per-unit base must be strictly positive"),
        }
    }
}

impl std::error::Error for UnitsError {}

fn require_finite(value: f64) -> Result<(), UnitsError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(UnitsError::NotFinite)
    }
}

// --- Serialization DTOs -----------------------------------------------------
//
// Each DTO carries the canonical f64 under an explicit unit field name, so a
// serialized value is never a bare, unit-less number. Deserialization routes
// back through the validating constructor.

#[derive(Serialize, Deserialize)]
struct ActivePowerMwDto {
    mw: f64,
}

#[derive(Serialize, Deserialize)]
struct ReactivePowerMvarDto {
    mvar: f64,
}

#[derive(Serialize, Deserialize)]
struct ApparentPowerMvaDto {
    mva: f64,
}

#[derive(Serialize, Deserialize)]
struct EnergyMwhDto {
    mwh: f64,
}

#[derive(Serialize, Deserialize)]
struct SocFractionDto {
    fraction: f64,
}

// --- Active power (MW) ------------------------------------------------------

/// Active power P in **MW** (canonical unit, report §10.2).
///
/// Sign rule (frozen, report §10.2 / foundations §6.2): stated at the
/// asset/site point of connection — **P > 0 means export/injection into the
/// grid**; P < 0 means import/consumption from the grid.
///
/// Deliberately a distinct type from [`EnergyMWh`], [`ReactivePowerMVAr`],
/// [`ApparentPowerMVA`], and [`StateOfCharge`]: no `From`/`Into` conversion
/// exists between any two of them, so unit confusion is a compile error.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ActivePowerMwDto", into = "ActivePowerMwDto")]
pub struct ActivePowerMW(f64);

impl ActivePowerMW {
    /// Constructs an active-power value, rejecting NaN and infinities.
    pub fn new(value: f64) -> Result<Self, UnitsError> {
        require_finite(value)?;
        Ok(Self(value))
    }

    /// The canonical MW value.
    pub fn value(self) -> f64 {
        self.0
    }

    /// Display-only conversion to **kW** (report §10.2 §5: scaled units are
    /// display-only, never persisted, and never emitted in a canonical API
    /// contract). The name carries `display` deliberately — never use this
    /// value for storage or canonical exchange.
    pub fn as_kw_display(self) -> f64 {
        self.0 * 1_000.0
    }
}

impl TryFrom<f64> for ActivePowerMW {
    type Error = UnitsError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<ActivePowerMwDto> for ActivePowerMW {
    type Error = UnitsError;

    fn try_from(dto: ActivePowerMwDto) -> Result<Self, Self::Error> {
        Self::new(dto.mw)
    }
}

impl From<ActivePowerMW> for ActivePowerMwDto {
    fn from(v: ActivePowerMW) -> Self {
        Self { mw: v.0 }
    }
}

impl std::fmt::Display for ActivePowerMW {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} MW", self.0)
    }
}

// --- Reactive power (MVAr) --------------------------------------------------

/// Reactive power Q in **MVAr** (canonical unit, report §10.2).
///
/// The numeric value alone has **no universal meaning**: the direction it
/// denotes is fixed only by the [`ReactiveSignConvention`] declared by the
/// governing grid model and device profile (see
/// [`interpret_reactive_power`]). Never inferred from sign alone.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ReactivePowerMvarDto", into = "ReactivePowerMvarDto")]
pub struct ReactivePowerMVAr(f64);

impl ReactivePowerMVAr {
    /// Constructs a reactive-power value, rejecting NaN and infinities.
    pub fn new(value: f64) -> Result<Self, UnitsError> {
        require_finite(value)?;
        Ok(Self(value))
    }

    /// The canonical MVAr value.
    pub fn value(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for ReactivePowerMVAr {
    type Error = UnitsError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<ReactivePowerMvarDto> for ReactivePowerMVAr {
    type Error = UnitsError;

    fn try_from(dto: ReactivePowerMvarDto) -> Result<Self, Self::Error> {
        Self::new(dto.mvar)
    }
}

impl From<ReactivePowerMVAr> for ReactivePowerMvarDto {
    fn from(v: ReactivePowerMVAr) -> Self {
        Self { mvar: v.0 }
    }
}

impl std::fmt::Display for ReactivePowerMVAr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} MVAr", self.0)
    }
}

// --- Apparent power (MVA) ---------------------------------------------------

/// Apparent power S in **MVA** (canonical unit, report §10.2).
///
/// Used in P–Q envelope bounds (report §11.3).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "ApparentPowerMvaDto", into = "ApparentPowerMvaDto")]
pub struct ApparentPowerMVA(f64);

impl ApparentPowerMVA {
    /// Constructs an apparent-power value, rejecting NaN and infinities.
    pub fn new(value: f64) -> Result<Self, UnitsError> {
        require_finite(value)?;
        Ok(Self(value))
    }

    /// The canonical MVA value.
    pub fn value(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for ApparentPowerMVA {
    type Error = UnitsError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<ApparentPowerMvaDto> for ApparentPowerMVA {
    type Error = UnitsError;

    fn try_from(dto: ApparentPowerMvaDto) -> Result<Self, Self::Error> {
        Self::new(dto.mva)
    }
}

impl From<ApparentPowerMVA> for ApparentPowerMvaDto {
    fn from(v: ApparentPowerMVA) -> Self {
        Self { mva: v.0 }
    }
}

impl std::fmt::Display for ApparentPowerMVA {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} MVA", self.0)
    }
}

// --- Energy (MWh) -----------------------------------------------------------

/// Energy E in **MWh** (canonical unit, report §10.2).
///
/// SOC is not energy; state of charge is carried separately as
/// [`StateOfCharge`]. Deliberately a distinct type from [`ActivePowerMW`] —
/// MW and MWh do not unify, and no `From`/`Into` conversion exists between
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "EnergyMwhDto", into = "EnergyMwhDto")]
pub struct EnergyMWh(f64);

impl EnergyMWh {
    /// Constructs an energy value, rejecting NaN and infinities.
    pub fn new(value: f64) -> Result<Self, UnitsError> {
        require_finite(value)?;
        Ok(Self(value))
    }

    /// The canonical MWh value.
    pub fn value(self) -> f64 {
        self.0
    }

    /// Display-only conversion to **kWh** (report §10.2 §5: scaled units are
    /// display-only, never persisted, and never emitted in a canonical API
    /// contract). The name carries `display` deliberately — never use this
    /// value for storage or canonical exchange.
    pub fn as_kwh_display(self) -> f64 {
        self.0 * 1_000.0
    }
}

impl TryFrom<f64> for EnergyMWh {
    type Error = UnitsError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl TryFrom<EnergyMwhDto> for EnergyMWh {
    type Error = UnitsError;

    fn try_from(dto: EnergyMwhDto) -> Result<Self, Self::Error> {
        Self::new(dto.mwh)
    }
}

impl From<EnergyMWh> for EnergyMwhDto {
    fn from(v: EnergyMWh) -> Self {
        Self { mwh: v.0 }
    }
}

impl std::fmt::Display for EnergyMWh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} MWh", self.0)
    }
}

// --- Reactive sign convention ----------------------------------------------

/// The reactive-power sign convention declared by a grid model or device
/// profile (report §10.2; foundations §6.2).
///
/// Each grid model and each device profile declares exactly one of these;
/// each adapter records the mapping it applies; platform contracts carry the
/// declared convention explicitly rather than assuming one. There is **no
/// default** — no `Default` impl and no constructor without the convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReactiveSignConvention {
    /// **Q > 0 means injection into the grid** (generator convention).
    InjectionPositive,
    /// **Q > 0 means consumption from the grid** (load convention).
    ConsumptionPositive,
}

/// The direction a reactive-power value denotes, once interpreted under a
/// declared [`ReactiveSignConvention`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReactivePowerDirection {
    /// Injection into the grid at the point of connection.
    Injection,
    /// Consumption from the grid at the point of connection.
    Consumption,
    /// The value is exactly zero; no direction applies.
    Zero,
}

/// Interprets a [`ReactivePowerMVAr`] value under the [`ReactiveSignConvention`]
/// declared by the governing grid model and device profile.
///
/// This is the only supported way to turn a reactive-power sign into a
/// direction; reading the sign directly is exactly the "inferred from sign
/// alone" mistake foundations §6.2 prohibits. The convention is a required
/// argument — there is no default and no overload without it.
pub fn interpret_reactive_power(
    convention: ReactiveSignConvention,
    q: ReactivePowerMVAr,
) -> ReactivePowerDirection {
    if q.0 == 0.0 {
        return ReactivePowerDirection::Zero;
    }
    match convention {
        ReactiveSignConvention::InjectionPositive => {
            if q.0 > 0.0 {
                ReactivePowerDirection::Injection
            } else {
                ReactivePowerDirection::Consumption
            }
        }
        ReactiveSignConvention::ConsumptionPositive => {
            if q.0 > 0.0 {
                ReactivePowerDirection::Consumption
            } else {
                ReactivePowerDirection::Injection
            }
        }
    }
}

// --- State of charge --------------------------------------------------------

/// State of charge: a **fraction in [0,1]** (report §10.2; foundations §6.2).
///
/// The fraction is the single canonical representation at the API boundary.
/// Percentages exist only as display or vendor-source values and are
/// converted at the adapter; a percent value is never persisted as canonical
/// and never appears in an API contract as canonical SOC. Accordingly, the
/// fraction constructor is the primary path and the percent path is a
/// separate, explicitly named boundary constructor
/// ([`StateOfCharge::from_percent`]).
///
/// The inner value is private so the [0,1] invariant holds for every
/// instance that was built through [`StateOfCharge::new`], `TryFrom<f64>`, or
/// `from_percent`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "SocFractionDto", into = "SocFractionDto")]
pub struct StateOfCharge(f64);

impl StateOfCharge {
    /// Lower bound of the canonical SOC fraction range.
    pub const MIN: f64 = 0.0;
    /// Upper bound of the canonical SOC fraction range.
    pub const MAX: f64 = 1.0;

    /// Constructs a state of charge from a **fraction**, rejecting NaN,
    /// infinities, and any value outside [0,1]. This is the canonical
    /// construction path.
    pub fn new(fraction: f64) -> Result<Self, UnitsError> {
        require_finite(fraction)?;
        if !(StateOfCharge::MIN..=StateOfCharge::MAX).contains(&fraction) {
            return Err(UnitsError::OutOfRange);
        }
        Ok(Self(fraction))
    }

    /// Boundary constructor from a **percent** value (display or vendor-source
    /// input, report §10.2 §4). Explicitly named `from_percent` so the
    /// conversion is visible at every call site; the converted fraction is
    /// the only thing this type stores — a percent value is never persisted
    /// as canonical SOC. Rejects NaN, infinities, and any percent outside
    /// [0,100].
    pub fn from_percent(percent: f64) -> Result<Self, UnitsError> {
        require_finite(percent)?;
        if !(0.0..=100.0).contains(&percent) {
            return Err(UnitsError::OutOfRange);
        }
        Ok(Self(percent / 100.0))
    }

    /// The canonical fraction in [0,1]. There is no percent accessor —
    /// percent is a display/vendor concern handled at the adapter.
    pub fn value(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for StateOfCharge {
    type Error = UnitsError;

    fn try_from(fraction: f64) -> Result<Self, Self::Error> {
        Self::new(fraction)
    }
}

impl TryFrom<SocFractionDto> for StateOfCharge {
    type Error = UnitsError;

    fn try_from(dto: SocFractionDto) -> Result<Self, Self::Error> {
        Self::new(dto.fraction)
    }
}

impl From<StateOfCharge> for SocFractionDto {
    fn from(v: StateOfCharge) -> Self {
        Self { fraction: v.0 }
    }
}

impl std::fmt::Display for StateOfCharge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} SOC-fraction", self.0)
    }
}

// --- BESS net power ---------------------------------------------------------

/// Computes the BESS net active-power export at the point of connection:
/// `P_net = P_dis − P_ch` with `P_dis ≥ 0` and `P_ch ≥ 0` (report §11.2).
///
/// Both inputs are non-negative magnitudes; a negative `discharge` or
/// `charge` value is rejected. The result follows the frozen active-power
/// sign rule: `P_net > 0` is export, `P_net < 0` is import.
///
/// The prohibition of simultaneous charge and discharge is an
/// optimization-formulation concern (report §11.2) and is not enforced here.
pub fn bess_net_power(
    discharge: ActivePowerMW,
    charge: ActivePowerMW,
) -> Result<ActivePowerMW, UnitsError> {
    require_finite(discharge.0)?;
    require_finite(charge.0)?;
    if discharge.0 < 0.0 {
        return Err(UnitsError::NegativePower);
    }
    if charge.0 < 0.0 {
        return Err(UnitsError::NegativePower);
    }
    ActivePowerMW::new(discharge.0 - charge.0)
}

// --- Per-unit base ----------------------------------------------------------

/// The explicitly declared base for a per-unit representation (report §10.2;
/// foundations §6.2).
///
/// Voltage is kV at the nominal base in the network model and per-unit in the
/// solver representation; current is A or per-unit on an explicit base. In
/// both cases the base is **declared explicitly** and never assumed, so any
/// per-unit value must be traceable to one of these.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "PerUnitBaseDto", into = "PerUnitBaseDto")]
pub struct PerUnitBase {
    /// Base voltage in kV (nominal base of the network model).
    pub base_kv: f64,
    /// Base apparent power in MVA.
    pub base_mva: f64,
}

impl PerUnitBase {
    /// Constructs an explicit base, rejecting NaN, infinities, and
    /// non-positive bases.
    pub fn new(base_kv: f64, base_mva: f64) -> Result<Self, UnitsError> {
        require_finite(base_kv)?;
        require_finite(base_mva)?;
        if base_kv <= 0.0 || base_mva <= 0.0 {
            return Err(UnitsError::NonPositiveBase);
        }
        Ok(Self { base_kv, base_mva })
    }
}

impl TryFrom<PerUnitBaseDto> for PerUnitBase {
    type Error = UnitsError;

    fn try_from(dto: PerUnitBaseDto) -> Result<Self, Self::Error> {
        Self::new(dto.base_kv, dto.base_mva)
    }
}

impl From<PerUnitBase> for PerUnitBaseDto {
    fn from(v: PerUnitBase) -> Self {
        Self {
            base_kv: v.base_kv,
            base_mva: v.base_mva,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct PerUnitBaseDto {
    base_kv: f64,
    base_mva: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soc_accepts_zero_and_one() {
        assert_eq!(StateOfCharge::new(0.0).unwrap().value(), 0.0);
        assert_eq!(StateOfCharge::new(1.0).unwrap().value(), 1.0);
        assert_eq!(StateOfCharge::new(0.5).unwrap().value(), 0.5);
    }

    #[test]
    fn soc_rejects_the_specified_bounds_and_non_finite() {
        // P1.05 acceptance: SOC construction rejects -0.01 and 1.01.
        assert_eq!(StateOfCharge::new(-0.01), Err(UnitsError::OutOfRange));
        assert_eq!(StateOfCharge::new(1.01), Err(UnitsError::OutOfRange));
        assert_eq!(StateOfCharge::new(-0.1), Err(UnitsError::OutOfRange));
        assert_eq!(StateOfCharge::new(1.1), Err(UnitsError::OutOfRange));
        assert_eq!(StateOfCharge::new(-1.0), Err(UnitsError::OutOfRange));
        assert_eq!(StateOfCharge::new(2.0), Err(UnitsError::OutOfRange));
        assert_eq!(StateOfCharge::new(f64::NAN), Err(UnitsError::NotFinite));
        assert_eq!(StateOfCharge::new(f64::INFINITY), Err(UnitsError::NotFinite));
    }

    #[test]
    fn soc_try_from_matches_new() {
        assert_eq!(StateOfCharge::try_from(0.25).unwrap().value(), 0.25);
        assert_eq!(StateOfCharge::try_from(1.5), Err(UnitsError::OutOfRange));
    }

    #[test]
    fn soc_from_percent_is_a_separate_explicit_constructor() {
        // from_percent(50) is exactly fraction 0.5 (P1.05 acceptance).
        assert_eq!(StateOfCharge::from_percent(50.0).unwrap().value(), 0.5);
        assert_eq!(StateOfCharge::from_percent(0.0).unwrap().value(), 0.0);
        assert_eq!(StateOfCharge::from_percent(100.0).unwrap().value(), 1.0);
        // from_percent(101) is rejected (P1.05 acceptance).
        assert_eq!(StateOfCharge::from_percent(101.0), Err(UnitsError::OutOfRange));
        assert_eq!(StateOfCharge::from_percent(-0.5), Err(UnitsError::OutOfRange));
        assert_eq!(StateOfCharge::from_percent(f64::NAN), Err(UnitsError::NotFinite));
    }

    #[test]
    fn power_newtypes_reject_non_finite() {
        assert_eq!(ActivePowerMW::new(f64::NAN), Err(UnitsError::NotFinite));
        assert_eq!(ReactivePowerMVAr::new(f64::INFINITY), Err(UnitsError::NotFinite));
        assert_eq!(ApparentPowerMVA::new(f64::NAN), Err(UnitsError::NotFinite));
        assert_eq!(EnergyMWh::new(f64::NEG_INFINITY), Err(UnitsError::NotFinite));
    }

    #[test]
    fn power_newtypes_accept_finite_values() {
        assert_eq!(ActivePowerMW::new(-3.5).unwrap().value(), -3.5);
        assert_eq!(ReactivePowerMVAr::new(2.0).unwrap().value(), 2.0);
        assert_eq!(ApparentPowerMVA::new(4.0).unwrap().value(), 4.0);
        assert_eq!(EnergyMWh::new(10.0).unwrap().value(), 10.0);
    }

    #[test]
    fn mw_and_mwh_do_not_unify() {
        // P1.05 acceptance (API-surface form of the compile-distinctness
        // requirement): ActivePowerMW and EnergyMWh are distinct types with
        // no From/Into conversion between them, so a value of one can never
        // be passed where the other is expected — the types themselves are
        // the proof. The runtime check below pins that they never collapse
        // into one representation.
        let power = ActivePowerMW::new(5.0).unwrap();
        let energy = EnergyMWh::new(5.0).unwrap();
        assert_ne!(
            std::any::type_name::<ActivePowerMW>(),
            std::any::type_name::<EnergyMWh>(),
            "MW and MWh must remain distinct types"
        );
        assert_eq!(power.value(), 5.0);
        assert_eq!(energy.value(), 5.0);
    }

    #[test]
    fn display_conversions_are_named_and_scaled() {
        let power = ActivePowerMW::new(2.5).unwrap();
        assert_eq!(power.as_kw_display(), 2_500.0);
        let energy = EnergyMWh::new(0.75).unwrap();
        assert_eq!(energy.as_kwh_display(), 750.0);
    }

    #[test]
    fn bess_net_power_computes_discharge_minus_charge() {
        let net =
            bess_net_power(ActivePowerMW::new(5.0).unwrap(), ActivePowerMW::new(2.0).unwrap())
                .unwrap();
        assert_eq!(net.value(), 3.0);
        assert!(net.value() > 0.0, "P_net > 0 must mean export");
    }

    #[test]
    fn bess_net_power_import_when_charge_exceeds_discharge() {
        let net =
            bess_net_power(ActivePowerMW::new(1.0).unwrap(), ActivePowerMW::new(4.0).unwrap())
                .unwrap();
        assert_eq!(net.value(), -3.0);
        assert!(net.value() < 0.0, "P_net < 0 must mean import");
    }

    #[test]
    fn bess_net_power_zero_when_discharge_equals_charge() {
        let net =
            bess_net_power(ActivePowerMW::new(3.0).unwrap(), ActivePowerMW::new(3.0).unwrap())
                .unwrap();
        assert_eq!(net.value(), 0.0);
    }

    #[test]
    fn bess_net_power_accepts_zero_magnitudes() {
        let net =
            bess_net_power(ActivePowerMW::new(0.0).unwrap(), ActivePowerMW::new(0.0).unwrap())
                .unwrap();
        assert_eq!(net.value(), 0.0);
    }

    #[test]
    fn bess_net_power_rejects_negative_magnitudes() {
        assert_eq!(
            bess_net_power(ActivePowerMW::new(-1.0).unwrap(), ActivePowerMW::new(2.0).unwrap()),
            Err(UnitsError::NegativePower)
        );
        assert_eq!(
            bess_net_power(ActivePowerMW::new(2.0).unwrap(), ActivePowerMW::new(-1.0).unwrap()),
            Err(UnitsError::NegativePower)
        );
    }

    #[test]
    fn bess_net_power_rejects_non_finite() {
        // Constructed via the tuple form to reach the guard with a
        // non-finite value (tuple construction is possible inside the
        // declaring module; the function must still defend itself).
        assert_eq!(
            bess_net_power(ActivePowerMW(f64::NAN), ActivePowerMW::new(1.0).unwrap()),
            Err(UnitsError::NotFinite)
        );
        assert_eq!(
            bess_net_power(ActivePowerMW::new(1.0).unwrap(), ActivePowerMW(f64::INFINITY)),
            Err(UnitsError::NotFinite)
        );
    }

    #[test]
    fn reactive_sign_injection_positive_convention() {
        let conv = ReactiveSignConvention::InjectionPositive;
        assert_eq!(
            interpret_reactive_power(conv, ReactivePowerMVAr::new(5.0).unwrap()),
            ReactivePowerDirection::Injection
        );
        assert_eq!(
            interpret_reactive_power(conv, ReactivePowerMVAr::new(-5.0).unwrap()),
            ReactivePowerDirection::Consumption
        );
        assert_eq!(
            interpret_reactive_power(conv, ReactivePowerMVAr::new(0.0).unwrap()),
            ReactivePowerDirection::Zero
        );
    }

    #[test]
    fn reactive_sign_consumption_positive_convention() {
        let conv = ReactiveSignConvention::ConsumptionPositive;
        assert_eq!(
            interpret_reactive_power(conv, ReactivePowerMVAr::new(5.0).unwrap()),
            ReactivePowerDirection::Consumption
        );
        assert_eq!(
            interpret_reactive_power(conv, ReactivePowerMVAr::new(-5.0).unwrap()),
            ReactivePowerDirection::Injection
        );
        assert_eq!(
            interpret_reactive_power(conv, ReactivePowerMVAr::new(0.0).unwrap()),
            ReactivePowerDirection::Zero
        );
    }

    #[test]
    fn reactive_sign_same_value_opposite_conventions_differ() {
        // The same numeric sign maps to opposite directions under the two
        // declared conventions: Q sign is never a universal constant. The
        // convention is a required argument of interpret_reactive_power —
        // the API offers no default convention and no convention-free path.
        let q = ReactivePowerMVAr::new(3.0).unwrap();
        assert_eq!(
            interpret_reactive_power(ReactiveSignConvention::InjectionPositive, q),
            ReactivePowerDirection::Injection
        );
        assert_eq!(
            interpret_reactive_power(ReactiveSignConvention::ConsumptionPositive, q),
            ReactivePowerDirection::Consumption
        );
    }

    #[test]
    fn per_unit_base_accepts_positive_bases() {
        let base = PerUnitBase::new(20.0, 50.0).unwrap();
        assert_eq!(base.base_kv, 20.0);
        assert_eq!(base.base_mva, 50.0);
    }

    #[test]
    fn per_unit_base_rejects_non_positive_and_non_finite() {
        assert_eq!(PerUnitBase::new(0.0, 50.0), Err(UnitsError::NonPositiveBase));
        assert_eq!(PerUnitBase::new(20.0, -1.0), Err(UnitsError::NonPositiveBase));
        assert_eq!(PerUnitBase::new(f64::NAN, 50.0), Err(UnitsError::NotFinite));
        assert_eq!(PerUnitBase::new(20.0, f64::INFINITY), Err(UnitsError::NotFinite));
    }

    // --- serde round trips ----------------------------------------------

    #[test]
    fn unit_newtypes_serialize_with_explicit_field_names() {
        let power = ActivePowerMW::new(3.25).unwrap();
        let json = serde_json::to_value(&power).unwrap();
        assert_eq!(json, serde_json::json!({ "mw": 3.25 }));

        let q = ReactivePowerMVAr::new(-1.5).unwrap();
        let json = serde_json::to_value(&q).unwrap();
        assert_eq!(json, serde_json::json!({ "mvar": -1.5 }));

        let s = ApparentPowerMVA::new(4.0).unwrap();
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json, serde_json::json!({ "mva": 4.0 }));

        let e = EnergyMWh::new(12.5).unwrap();
        let json = serde_json::to_value(&e).unwrap();
        assert_eq!(json, serde_json::json!({ "mwh": 12.5 }));

        let soc = StateOfCharge::new(0.5).unwrap();
        let json = serde_json::to_value(&soc).unwrap();
        assert_eq!(json, serde_json::json!({ "fraction": 0.5 }));
    }

    #[test]
    fn unit_newtypes_round_trip_through_serde() {
        let power = ActivePowerMW::new(3.25).unwrap();
        assert_eq!(
            serde_json::from_value::<ActivePowerMW>(serde_json::to_value(&power).unwrap()).unwrap(),
            power
        );
        let soc = StateOfCharge::new(0.75).unwrap();
        assert_eq!(
            serde_json::from_value::<StateOfCharge>(serde_json::to_value(&soc).unwrap()).unwrap(),
            soc
        );
    }

    #[test]
    fn unit_newtypes_serde_rejects_invalid_values() {
        // Deserialization re-runs the constructor validation: a non-finite
        // payload (JSON has no NaN, so exercise the range rule) and an
        // out-of-range SOC never deserialize.
        let err = serde_json::from_value::<StateOfCharge>(serde_json::json!({ "fraction": 1.01 }))
            .unwrap_err();
        assert!(err.to_string().contains("outside the declared range"));
        let err = serde_json::from_value::<StateOfCharge>(serde_json::json!({ "fraction": -0.01 }))
            .unwrap_err();
        assert!(err.to_string().contains("outside the declared range"));
    }

    #[test]
    fn reactive_sign_convention_serde_round_trip() {
        for conv in [
            ReactiveSignConvention::InjectionPositive,
            ReactiveSignConvention::ConsumptionPositive,
        ] {
            let json = serde_json::to_value(&conv).unwrap();
            assert_eq!(json, serde_json::to_value(&json).unwrap());
            let back: ReactiveSignConvention = serde_json::from_value(json).unwrap();
            assert_eq!(back, conv);
        }
    }
}
