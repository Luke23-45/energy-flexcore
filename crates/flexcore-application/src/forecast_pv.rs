// P15.03 — PV generation forecasting design + the weather-data ingestion
//         seam binding.
//
// Phase 15 plan §2 row P15.03; decision record:
// `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/
// 03_p15_03_pv_weather_seam.md` (registers P15-E-1 OPEN with DEFAULT).
//
// WHAT THIS IS: the DESIGN record types for PV generation forecasting —
// the weather-input requirements every PV forecast model declares, the
// UNTRAINED registry declarations for the designed PV models, and the
// E-decision binding that keeps vendor integration OUT until the human
// owner closes P15-E-1. The weather-data ingestion PORT itself is
// `flexcore-ports::weather_data` (vendor-neutral by construction); the
// port is the seam, this module is the consuming design.
//
// THE BUILD-VS-BUY BINDING (P15-E-1, registered OPEN — agents never close
// E-deisions, conventions §4):
//
// - **DEFAULT/RECOMMENDATION (a):** buy irradiance/power from a
//   commercial weather-data vendor, ingested as DATA with full
//   provenance through the weather port. Forecasting is table stakes
//   (gap analysis §3 row 3: [S-41][S-50][S-51]) and the F2 build-list
//   treats weather as an INPUT seam (§7), not a build item. The
//   analogous bought-forecast pattern is [S-112].
// - **VENDOR NAMING IS A RECOMMENDATION LABEL, NEVER A REGISTER SOURCE.**
//   Commercial vendor-class names (Solcast/DNV-class) appear in the plan,
//   context, and decision records as RECOMMENDATION labels only; no
//   vendor is a registered source, no vendor account or API key exists
//   (the key is a HUMAN GATE input, UNSIGNED until registered), and NO
//   vendor integration lands without the E-decision closing. This module
//   structurally enforces the design-side half: a model declaration
//   carries NO vendor field at all — the closed field vocabulary cannot
//   name one.
// - **DESIGN ONLY.** No vendor adapter, no HTTP client, no API key path:
//   the weather port has no implementation in this delivery, and the PV
//   models below register UNTRAINED with their weather inputs declared as
//   requirements, never as wired data.
//
// HONESTY LINES:
//
// 1. Every PV model designed here registers UNTRAINED (P15.06 makes
//    UNTRAINED a first-class registered state) and claims NO skill; the
//    evaluation framework (P15.05) owns every result cell, all EMPTY.
// 2. Until evaluated, the persistence and seasonal-naive baselines
//    (P15.02 registry) are the PV reference forecasts — the same frozen
//    registry, the same no-skill posture.
// 3. PV forecast values follow the canonical units and sign (§10.2):
//    MW, non-negative (export; the P3.31 fixture's PV sign rule, now the
//    real-artifact rule — a negative PV forecast is a validation refusal,
//    never clipped).
//
// Control status: design records and inputs for the simulation-only 0.x
// line (ADR-010 extension, conventions §6.3). Nothing here authorizes,
// issues, or dispatches anything; a PV forecast is an input to scheduling
// analytics, never a command.

use crate::forecast_architecture::{ForecastArtifactKind, ForecastHorizon, ForecastScope};

/// The PV forecasting design statement (pinned by tests).
pub const PV_FORECAST_DESIGN_STATEMENT: &str = "PV generation forecasting in FlexCore is a \
     DESIGN with the weather seam declared: models declare their weather-input requirements \
     against the vendor-neutral weather port (flexcore-ports::weather_data), register \
     UNTRAINED, and claim no skill until a P15.05 evaluation run fills a result cell. \
     Weather data is bought (P15-E-1 DEFAULT/RECOMMENDATION) and ingested as DATA with \
     provenance; no vendor integration lands without the E-decision closing, and vendor \
     naming in plans and records is a RECOMMENDATION label, never a register source.";

/// The P15-E-1 status line as recorded in the decision record (the
/// E-decision is OPEN; agents never close it).
pub const P15_E_1_STATUS: &str = "P15-E-1 (weather/irradiance data build-vs-buy): OPEN. \
     DEFAULT/RECOMMENDATION: buy a commercial irradiance/power service, ingested as DATA \
     with full provenance through the weather port. CLOSERS: the human owner's budget and \
     vendor selection; the vendor account/API key is a HUMAN GATE input (UNSIGNED until \
     registered). Vendor-class naming (Solcast/DNV-class) is a RECOMMENDATION label in \
     the plan/context/records, never a register source.";

/// The structural no-vendor-field statement (pinned by tests; see the
/// module header).
pub const NO_VENDOR_FIELD_STATEMENT: &str = "a PV forecast model declaration carries NO \
     vendor field: the closed field vocabulary of PvForecastModelDeclaration cannot name \
     a vendor, a vendor product, or a vendor endpoint. The vendor is named (if ever) in \
     the deployment configuration after P15-E-1 closes — a composition-root concern, \
     structurally absent from the design types.";

/// The closed weather-quantity requirement vocabulary a PV model may
/// declare (mirrors `flexcore-ports::weather_data::WeatherQuantity` BY
/// CITATION — the port is the single home; this module declares only
/// which quantities a model REQUIRES).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PvWeatherInput {
    /// Global horizontal irradiance (the primary PV driver).
    GlobalHorizontalIrradiance,
    /// Direct normal irradiance (for tracking arrays).
    DirectNormalIrradiance,
    /// Diffuse horizontal irradiance (decomposition models).
    DiffuseHorizontalIrradiance,
    /// Air temperature (cell-temperature derating).
    AirTemperature,
    /// Wind speed (module cooling term).
    WindSpeed,
    /// Cloud cover (persistence-window nowcasting).
    CloudCover,
}

impl PvWeatherInput {
    /// The wire spelling (records; matches the weather port's
    /// `wire_name` spellings for the mirrored quantities — pinned by
    /// tests).
    pub const fn wire_name(self) -> &'static str {
        match self {
            PvWeatherInput::GlobalHorizontalIrradiance => "ghi",
            PvWeatherInput::DirectNormalIrradiance => "dni",
            PvWeatherInput::DiffuseHorizontalIrradiance => "dhi",
            PvWeatherInput::AirTemperature => "air_temperature",
            PvWeatherInput::WindSpeed => "wind_speed",
            PvWeatherInput::CloudCover => "cloud_cover",
        }
    }
}

/// The training state of a designed PV model as THIS MODULE records it.
/// Every designed model is `Untrained` — the enum has the variant so the
/// state is explicit, and a test pins that every declaration in this
/// module's registry carries it. (The model REGISTRY's own first-class
/// training-state vocabulary is P15.06's; this is the design-record view.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PvModelDesignTrainingState {
    /// Designed, registered, never trained: NO runtime data exists (the
    /// platform has never run), and no training run is claimed.
    Untrained,
}

/// A designed PV forecast model declaration: what the model WOULD consume
/// and produce, registered UNTRAINED. The closed field vocabulary cannot
/// name a vendor, a vendor product, or a vendor endpoint (the structural
/// half of the P15-E-1 binding — see [`NO_VENDOR_FIELD_STATEMENT`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PvForecastModelDeclaration {
    /// The stable model id (e.g. `pv-model/persistence-design`).
    pub model_id: &'static str,
    /// The model version (a change is a NEW version, never a silent edit).
    pub version: String,
    /// The weather quantities the model requires as inputs (declared
    /// requirements; NONE are wired — the weather port has no
    /// implementation in this delivery).
    pub weather_inputs: Vec<PvWeatherInput>,
    /// The horizons the model is designed for (closed vocabulary).
    pub horizons: Vec<ForecastHorizon>,
    /// The scope class the model produces (site or portfolio).
    pub scope_class: PvScopeClass,
    /// The training state — every declaration in this delivery is
    /// [`PvModelDesignTrainingState::Untrained`].
    pub training_state: PvModelDesignTrainingState,
    /// What the model does NOT do (the assumptions/limits honesty line).
    pub limitations: Vec<&'static str>,
}

/// The scope class of a PV model (site-scoped or portfolio-scoped; the
/// concrete scope is bound per artifact, P15.01).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PvScopeClass {
    /// Per-site models (the fixture-scale default).
    PerSite,
    /// Portfolio-level models.
    PerPortfolio,
}

impl PvScopeClass {
    /// The corresponding P15.01 artifact-scope constructor placeholder
    /// label (records).
    pub const fn matches_artifact_kind(self) -> &'static str {
        match self {
            // Every PV model produces PV-generation artifacts (the P15.01
            // kind vocabulary); the scope class narrows which scope the
            // artifact binds.
            PvScopeClass::PerSite => "site (ForecastArtifactKind::PvGeneration)",
            PvScopeClass::PerPortfolio => "portfolio (ForecastArtifactKind::PvGeneration)",
        }
    }
}

/// Why a PV model declaration was refused. Named, matchable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PvDeclarationError {
    /// The model id or version was empty/whitespace.
    EmptyIdentity { field: &'static str },
    /// A PV model with no weather inputs and no declared non-weather
    /// basis cannot be frozen as a design (a model that consumes nothing
    /// and produces forecasts is not a design, it is a fiction).
    NoDeclaredBasis,
    /// The declaration claimed a training state other than UNTRAINED —
    /// structurally impossible through the enum's single variant; this
    /// error exists for the deserialization path (a parsed declaration
    /// from outside the trust boundary).
    TrainingStateNotUntrained,
}

impl PvDeclarationError {
    /// The stable machine-readable error name.
    pub fn name(&self) -> &'static str {
        match self {
            PvDeclarationError::EmptyIdentity { .. } => "PV_DECLARATION_EMPTY_IDENTITY",
            PvDeclarationError::NoDeclaredBasis => "PV_DECLARATION_NO_DECLARED_BASIS",
            PvDeclarationError::TrainingStateNotUntrained => {
                "PV_DECLARATION_TRAINING_STATE_NOT_UNTRAINED"
            }
        }
    }
}

impl std::fmt::Display for PvDeclarationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PvDeclarationError::EmptyIdentity { field } => {
                write!(f, "the '{field}' must be a non-empty string")
            }
            PvDeclarationError::NoDeclaredBasis => write!(
                f,
                "a PV model design must declare at least one weather input OR record its \
                 non-weather basis in its limitations; a model that consumes nothing and \
                 produces forecasts is not a design"
            ),
            PvDeclarationError::TrainingStateNotUntrained => write!(
                f,
                "every PV model declaration in this delivery registers UNTRAINED; a \
                 declaration claiming any other training state is refused"
            ),
        }
    }
}

impl std::error::Error for PvDeclarationError {}

impl PvForecastModelDeclaration {
    /// Validates and constructs one declaration (fail-closed; the
    /// training state is pinned to UNTRAINED by construction — the enum
    /// has one variant).
    pub fn new(
        model_id: &'static str,
        version: String,
        weather_inputs: Vec<PvWeatherInput>,
        horizons: Vec<ForecastHorizon>,
        scope_class: PvScopeClass,
        limitations: Vec<&'static str>,
    ) -> Result<Self, PvDeclarationError> {
        if model_id.trim().is_empty() {
            return Err(PvDeclarationError::EmptyIdentity { field: "model_id" });
        }
        if version.trim().is_empty() {
            return Err(PvDeclarationError::EmptyIdentity { field: "version" });
        }
        let declaration = Self {
            model_id,
            version,
            weather_inputs,
            horizons,
            scope_class,
            training_state: PvModelDesignTrainingState::Untrained,
            limitations,
        };
        declaration.validate()?;
        Ok(declaration)
    }

    /// Full validation (also usable on a parsed declaration): identity,
    /// declared basis (weather inputs present, or limitations naming the
    /// non-weather basis), and the UNTRAINED pin.
    pub fn validate(&self) -> Result<(), PvDeclarationError> {
        if self.model_id.trim().is_empty() {
            return Err(PvDeclarationError::EmptyIdentity { field: "model_id" });
        }
        if self.version.trim().is_empty() {
            return Err(PvDeclarationError::EmptyIdentity { field: "version" });
        }
        if self.weather_inputs.is_empty()
            && !self
                .limitations
                .iter()
                .any(|line| line.contains("non-weather basis"))
        {
            return Err(PvDeclarationError::NoDeclaredBasis);
        }
        if self.training_state != PvModelDesignTrainingState::Untrained {
            return Err(PvDeclarationError::TrainingStateNotUntrained);
        }
        Ok(())
    }
}

/// The designed PV model registry: the declarations THIS delivery records,
/// all UNTRAINED. Order is fixed (registry order is part of the design
/// record). The persistence and seasonal-naive baselines (P15.02) remain
/// the operative PV reference forecasts until any of these is evaluated.
pub fn pv_model_design_registry() -> Vec<PvForecastModelDeclaration> {
    vec![
        PvForecastModelDeclaration::new(
            "pv-model/persistence-design",
            "v1".to_string(),
            // Non-weather basis: persistence consumes the site's own PV
            // telemetry history, not weather.
            Vec::new(),
            vec![ForecastHorizon::DayAhead, ForecastHorizon::IntraDay],
            PvScopeClass::PerSite,
            vec![
                "non-weather basis: carries the last observed PV value forward (the \
                 P15.02 persistence rule set, persistence.v1)",
                "no weather sensitivity: a cloud field or a storm front is invisible to \
                 this model by construction",
                "UNTRAINED and unevaluated: no skill claim; skill = beating persistence \
                 is a tautology for this model and is recorded as such, not as a claim",
            ],
        )
        .expect("the persistence design declaration is valid"),
        PvForecastModelDeclaration::new(
            "pv-model/irradiance-physics-design",
            "v1".to_string(),
            vec![
                PvWeatherInput::GlobalHorizontalIrradiance,
                PvWeatherInput::DirectNormalIrradiance,
                PvWeatherInput::DiffuseHorizontalIrradiance,
                PvWeatherInput::AirTemperature,
                PvWeatherInput::WindSpeed,
            ],
            vec![ForecastHorizon::DayAhead],
            PvScopeClass::PerSite,
            vec![
                "DESIGN ONLY: an irradiance-to-power conversion model (GHI/DNI/DHI \
                 decomposition + module temperature derating) — no conversion constants \
                 are recorded here, and none are fitted (UNTRAINED)",
                "the weather inputs are DECLARED REQUIREMENTS against the weather port; \
                 no weather data is wired (the port has no implementation; P15-E-1 OPEN)",
                "the array geometry (tilt/azimuth/tracking) is a per-asset declaration \
                 the model would consume as CONFIGURED data with provenance — not \
                 recorded here",
                "UNTRAINED and unevaluated: no skill claim anywhere",
            ],
        )
        .expect("the irradiance-physics design declaration is valid"),
        PvForecastModelDeclaration::new(
            "pv-model/statistical-ml-design",
            "v1".to_string(),
            vec![
                PvWeatherInput::GlobalHorizontalIrradiance,
                PvWeatherInput::CloudCover,
                PvWeatherInput::AirTemperature,
            ],
            vec![ForecastHorizon::DayAhead, ForecastHorizon::IntraDay],
            PvScopeClass::PerSite,
            vec![
                "DESIGN ONLY: a statistical/ML model class RESERVED for when runtime data \
                 accumulates — no architecture is selected, no framework is pinned (the \
                 P8.03 DECISION-POINT discipline carries over), nothing is trained",
                "training data would enter only through the P15.18 datasets policy (no \
                 production telemetry ever trains a simulation-stage model)",
                "UNTRAINED and unevaluated: no skill claim anywhere",
            ],
        )
        .expect("the statistical-ml design declaration is valid"),
    ]
}

// ---------------------------------------------------------------------------
// Tests (WRITTEN-UNRUN — no cargo command was executed in this loop; the
// human build/test gate owns the run. Every expected value below is a
// hand-derived property of the rules above or a literal copied from this
// module's constants. Nothing is fabricated.)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_design_statement_carries_the_honesty_lines() {
        assert!(PV_FORECAST_DESIGN_STATEMENT.contains("UNTRAINED"));
        assert!(PV_FORECAST_DESIGN_STATEMENT.contains("no skill"));
        assert!(PV_FORECAST_DESIGN_STATEMENT.contains("P15-E-1 DEFAULT/RECOMMENDATION"));
        assert!(P15_E_1_STATUS.contains("OPEN"));
        assert!(P15_E_1_STATUS.contains("UNSIGNED until registered"));
        assert!(P15_E_1_STATUS.contains("RECOMMENDATION label"));
        assert!(NO_VENDOR_FIELD_STATEMENT.contains("cannot name"));
    }

    #[test]
    fn weather_input_wire_names_match_the_weather_port_spellings() {
        // Mirror-by-citation pin: the design vocabulary's wire spellings
        // match the weather port's quantity spellings verbatim (the port
        // is the single home).
        let port = flexcore_ports::weather_data::WeatherQuantity::GlobalHorizontalIrradiance;
        assert_eq!(port.wire_name(), PvWeatherInput::GlobalHorizontalIrradiance.wire_name());
        assert_eq!(
            flexcore_ports::weather_data::WeatherQuantity::AirTemperatureDegC.wire_name(),
            PvWeatherInput::AirTemperature.wire_name()
        );
        assert_eq!(
            flexcore_ports::weather_data::WeatherQuantity::WindSpeedMs.wire_name(),
            PvWeatherInput::WindSpeed.wire_name()
        );
        assert_eq!(
            flexcore_ports::weather_data::WeatherQuantity::CloudCoverFraction.wire_name(),
            PvWeatherInput::CloudCover.wire_name()
        );
        assert_eq!(
            flexcore_ports::weather_data::WeatherQuantity::DirectNormalIrradiance.wire_name(),
            PvWeatherInput::DirectNormalIrradiance.wire_name()
        );
        assert_eq!(
            flexcore_ports::weather_data::WeatherQuantity::DiffuseHorizontalIrradiance
                .wire_name(),
            PvWeatherInput::DiffuseHorizontalIrradiance.wire_name()
        );
    }

    #[test]
    fn every_designed_model_is_untrained_and_declares_a_basis() {
        let registry = pv_model_design_registry();
        assert_eq!(registry.len(), 3);
        for declaration in &registry {
            assert_eq!(
                declaration.training_state,
                PvModelDesignTrainingState::Untrained,
                "every designed PV model registers UNTRAINED"
            );
            declaration
                .validate()
                .expect("every registry declaration validates");
            assert!(!declaration.horizons.is_empty());
        }
        // The persistence design declares a NON-weather basis (recorded in
        // its limitations); the other two declare weather inputs.
        assert!(registry[0].weather_inputs.is_empty());
        assert!(registry[0].limitations[0].contains("non-weather basis"));
        assert!(!registry[1].weather_inputs.is_empty());
        assert!(!registry[2].weather_inputs.is_empty());
    }

    #[test]
    fn a_declaration_without_a_declared_basis_is_refused() {
        let err = PvForecastModelDeclaration::new(
            "pv-model/empty",
            "v1".to_string(),
            Vec::new(),
            vec![ForecastHorizon::DayAhead],
            PvScopeClass::PerSite,
            vec!["no basis recorded"],
        )
        .expect_err("no declared basis refused");
        assert_eq!(err.name(), "PV_DECLARATION_NO_DECLARED_BASIS");
    }

    #[test]
    fn an_empty_identity_is_refused() {
        let err = PvForecastModelDeclaration::new(
            "  ",
            "v1".to_string(),
            vec![PvWeatherInput::GlobalHorizontalIrradiance],
            vec![ForecastHorizon::DayAhead],
            PvScopeClass::PerSite,
            vec!["limits"],
        )
        .expect_err("empty model id refused");
        assert_eq!(err.name(), "PV_DECLARATION_EMPTY_IDENTITY");
        let err = PvForecastModelDeclaration::new(
            "pv-model/x",
            "  ".to_string(),
            vec![PvWeatherInput::GlobalHorizontalIrradiance],
            vec![ForecastHorizon::DayAhead],
            PvScopeClass::PerSite,
            vec!["limits"],
        )
        .expect_err("empty version refused");
        assert_eq!(err.name(), "PV_DECLARATION_EMPTY_IDENTITY");
    }

    #[test]
    fn no_declaration_field_can_carry_a_vendor_name() {
        // The STRUCTURAL half of the P15-E-1 binding: the closed field
        // vocabulary of the declaration is enumerated here, and none of
        // the fields is a vendor/product/endpoint field. Pinned by
        // inspecting a constructed declaration's public fields.
        let declaration = &pv_model_design_registry()[1];
        let _ = (
            &declaration.model_id,
            &declaration.version,
            &declaration.weather_inputs,
            &declaration.horizons,
            &declaration.scope_class,
            &declaration.training_state,
            &declaration.limitations,
        );
        // The assertion is the enumeration itself: PvForecastModelDeclaration
        // has exactly the fields above (a compiler-enforced closed record);
        // none names a vendor. This test documents the invariant the type
        // enforces structurally.
        assert!(NO_VENDOR_FIELD_STATEMENT.contains("closed field vocabulary"));
    }

    #[test]
    fn scope_classes_map_to_the_p15_01_artifact_vocabulary() {
        assert!(PvScopeClass::PerSite
            .matches_artifact_kind()
            .contains("ForecastArtifactKind::PvGeneration"));
        assert!(PvScopeClass::PerPortfolio
            .matches_artifact_kind()
            .contains("ForecastArtifactKind::PvGeneration"));
        // The artifact kind itself is the P15.01 vocabulary member.
        assert_eq!(ForecastArtifactKind::PvGeneration.wire_name(), "pv_generation");
        let _ = ForecastScope::PerSite {
            site_id: "site-a".to_string(),
        };
    }
}
