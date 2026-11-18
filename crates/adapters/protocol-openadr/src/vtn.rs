// P14.05 — VEN management and the delivery planner (polling + push).
//
// The VTN registry holds the reviewed VEN inventory (enrolment records —
// acceptance is an operator decision, never inferred from protocol
// traffic), and the delivery planner answers "how does an event reach a
// given VEN" for the two DECLARED delivery modes. The planner returns a
// DELIVERY PLAN (a record), never a delivery: nothing here opens a
// connection, sends a message, or touches a network — the designed serving
// seam (`crate::endpoints`) is where a serving layer would execute a plan.

use std::collections::BTreeMap;

use crate::model::Ven;
use crate::scope::DeliveryMode;

/// Errors returned by VEN management and the delivery planner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VtnRegistryError {
    /// The VEN is not registered. There is no implicit fleet target and no
    /// auto-enrolment: an unknown VEN is a named refusal.
    VenNotRegistered { ven_id: String },
    /// A registration already exists for the VEN id; re-registration goes
    /// through the explicit update path, not a silent overwrite.
    VenAlreadyRegistered { ven_id: String },
    /// The VEN record is invalid (model-level validation failed).
    InvalidVen(crate::model::ModelError),
    /// The VEN is registered but NOT accepted for event delivery — the
    /// operator enrolment decision is still pending or was declined.
    VenNotAccepted { ven_id: String },
    /// The requested delivery mode cannot serve this VEN (e.g. push
    /// requested but the VEN registered no push endpoint).
    DeliveryModeUnavailable { ven_id: String, mode: String },
}

impl std::fmt::Display for VtnRegistryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VtnRegistryError::VenNotRegistered { ven_id } => {
                write!(f, "VEN '{ven_id}' is not registered; there is no auto-enrolment")
            }
            VtnRegistryError::VenAlreadyRegistered { ven_id } => {
                write!(f, "VEN '{ven_id}' is already registered; use the explicit update path")
            }
            VtnRegistryError::InvalidVen(err) => write!(f, "invalid VEN record: {err}"),
            VtnRegistryError::VenNotAccepted { ven_id } => {
                write!(f, "VEN '{ven_id}' is registered but not accepted for event delivery (operator enrolment decision pending or declined)")
            }
            VtnRegistryError::DeliveryModeUnavailable { ven_id, mode } => {
                write!(f, "delivery mode '{mode}' cannot serve VEN '{ven_id}'")
            }
        }
    }
}

impl std::error::Error for VtnRegistryError {}

impl From<crate::model::ModelError> for VtnRegistryError {
    fn from(err: crate::model::ModelError) -> Self {
        VtnRegistryError::InvalidVen(err)
    }
}

/// The VTN's VEN registry: the reviewed enrolment inventory.
///
/// `BTreeMap` keyed by VEN id (house rule: deterministic ordering, the
/// kernel BTreeMap pattern carried forward). Construction requires the
/// adapter id so the registry is attributable; there is no `Default`.
#[derive(Debug, Clone)]
pub struct VtnRegistry {
    /// The adapter id this registry belongs to.
    pub adapter_id: String,
    vens: BTreeMap<String, Ven>,
}

impl VtnRegistry {
    /// Constructs an empty registry for the given adapter id.
    pub fn new(adapter_id: impl Into<String>) -> Self {
        Self {
            adapter_id: adapter_id.into(),
            vens: BTreeMap::new(),
        }
    }

    /// Registers a VEN (the enrolment record; `accepted` starts as the
    /// caller records it — the operator surface owns that flag).
    pub fn register(&mut self, ven: Ven) -> Result<(), VtnRegistryError> {
        if self.vens.contains_key(&ven.ven_id) {
            return Err(VtnRegistryError::VenAlreadyRegistered {
                ven_id: ven.ven_id.clone(),
            });
        }
        self.vens.insert(ven.ven_id.clone(), ven);
        Ok(())
    }

    /// Replaces an existing VEN record; refuses when the VEN is unknown
    /// (updates are not registrations in disguise).
    pub fn update(&mut self, ven: Ven) -> Result<(), VtnRegistryError> {
        if !self.vens.contains_key(&ven.ven_id) {
            return Err(VtnRegistryError::VenNotRegistered {
                ven_id: ven.ven_id.clone(),
            });
        }
        self.vens.insert(ven.ven_id.clone(), ven);
        Ok(())
    }

    /// The registered VEN record, if present.
    pub fn ven(&self, ven_id: &str) -> Option<&Ven> {
        self.vens.get(ven_id)
    }

    /// All registered VEN records, in deterministic (VEN id) order.
    pub fn all(&self) -> impl Iterator<Item = &Ven> {
        self.vens.values()
    }

    /// The number of registered VENs.
    pub fn len(&self) -> usize {
        self.vens.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.vens.is_empty()
    }

    /// The VENs an event targets, resolved and validated: every target
    /// must be registered AND accepted. Refuses with the first named gap.
    pub fn resolve_targets(
        &self,
        target_ven_ids: &[String],
    ) -> Result<Vec<&Ven>, VtnRegistryError> {
        let mut resolved = Vec::new();
        for ven_id in target_ven_ids {
            let ven = self
                .vens
                .get(ven_id)
                .ok_or_else(|| VtnRegistryError::VenNotRegistered { ven_id: ven_id.clone() })?;
            if !ven.accepted {
                return Err(VtnRegistryError::VenNotAccepted { ven_id: ven_id.clone() });
            }
            resolved.push(ven);
        }
        Ok(resolved)
    }
}

/// A per-VEN delivery plan: WHICH mode serves the VEN and over which
/// surface. A record — execution belongs to the designed serving seam.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryPlan {
    /// The VEN the plan serves.
    pub ven_id: String,
    /// The declared mode that will serve the VEN.
    pub mode: DeliveryMode,
    /// The surface the mode uses: for polling, the designed poll route;
    /// for push, the VEN's registered push endpoint.
    pub surface: String,
}

impl DeliveryPlan {
    /// Canonical description for logs and records.
    pub fn describe(&self) -> String {
        match self.mode {
            DeliveryMode::Polling => {
                format!("poll: VEN {} pulls via {}", self.ven_id, self.surface)
            }
            DeliveryMode::Push => {
                format!("push: VTN notifies VEN {} at {}", self.ven_id, self.surface)
            }
        }
    }
}

/// Errors returned by the delivery planner (distinct from registry errors
/// so a plan refusal is never confused with an inventory problem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryPlanError {
    /// The VEN is unknown or not accepted (the planner resolves targets
    /// first; this names the underlying registry refusal).
    Registry(VtnRegistryError),
    /// The VEN is accepted but the requested mode cannot serve it.
    ModeUnavailable { ven_id: String, mode: String },
}

impl std::fmt::Display for DeliveryPlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeliveryPlanError::Registry(err) => write!(f, "{err}"),
            DeliveryPlanError::ModeUnavailable { ven_id, mode } => write!(
                f,
                "mode '{mode}' cannot serve VEN '{ven_id}' (e.g. push without a registered push endpoint)"
            ),
        }
    }
}

impl std::error::Error for DeliveryPlanError {}

impl From<VtnRegistryError> for DeliveryPlanError {
    fn from(err: VtnRegistryError) -> Self {
        DeliveryPlanError::Registry(err)
    }
}

/// The delivery planner over a [`VtnRegistry`]. Holds the declared
/// delivery-mode preference order (deterministic; the FIRST available
/// declared mode serves the VEN). Constructed only through
/// [`DeliveryPlanner::new`] with at least one declared mode.
#[derive(Debug, Clone)]
pub struct DeliveryPlanner {
    /// The adapter id this planner belongs to.
    pub adapter_id: String,
    /// The declared preference order (subset of the scope's declared
    /// modes; non-empty by construction).
    pub declared_mode_order: Vec<DeliveryMode>,
}

impl DeliveryPlanner {
    /// Constructs a planner with the given deterministic preference order.
    /// An empty order is a construction error — a planner that prefers
    /// nothing plans nothing.
    pub fn new(
        adapter_id: impl Into<String>,
        declared_mode_order: Vec<DeliveryMode>,
    ) -> Result<Self, DeliveryPlanError> {
        if declared_mode_order.is_empty() {
            // Named refusal, not a default: the mode order is a reviewed
            // deployment declaration.
            return Err(DeliveryPlanError::ModeUnavailable {
                ven_id: "<planner>".to_string(),
                mode: "<no declared delivery modes>".to_string(),
            });
        }
        Ok(Self {
            adapter_id: adapter_id.into(),
            declared_mode_order,
        })
    }

    /// Plans delivery for one VEN in the requested modes: the first
    /// requested-and-declared mode the VEN can actually use wins.
    pub fn plan_for(
        &self,
        registry: &VtnRegistry,
        ven_id: &str,
        requested_modes: &[DeliveryMode],
    ) -> Result<DeliveryPlan, DeliveryPlanError> {
        let ven = registry
            .ven(ven_id)
            .ok_or_else(|| VtnRegistryError::VenNotRegistered {
                ven_id: ven_id.to_string(),
            })?;
        if !ven.accepted {
            return Err(VtnRegistryError::VenNotAccepted {
                ven_id: ven_id.to_string(),
            }
            .into());
        }
        for &mode in requested_modes {
            // Only modes the PLANNER declared can be planned at all.
            if !self.declared_mode_order.contains(&mode) {
                continue;
            }
            match mode {
                DeliveryMode::Polling => {
                    return Ok(DeliveryPlan {
                        ven_id: ven.ven_id.clone(),
                        mode,
                        surface: format!(
                            "poll route at the agreed {} ms frequency",
                            ven.poll_frequency_millis
                        ),
                    });
                }
                DeliveryMode::Push => {
                    if let Some(endpoint) = &ven.push_endpoint {
                        return Ok(DeliveryPlan {
                            ven_id: ven.ven_id.clone(),
                            mode,
                            surface: endpoint.clone(),
                        });
                    }
                    // Push requested but no registered endpoint: try the
                    // next requested mode; if none fits, the named refusal
                    // below fires.
                }
            }
        }
        Err(DeliveryPlanError::ModeUnavailable {
            ven_id: ven.ven_id.clone(),
            mode: requested_modes
                .iter()
                .map(|m| m.name())
                .collect::<Vec<_>>()
                .join("|"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ModelError;

    fn accepted_polling_ven(id: &str) -> Ven {
        Ven::new(id.to_string(), format!("VEN {id}"), None, 60_000, true).expect("valid ven")
    }

    #[test]
    fn registration_update_and_deterministic_ordering() {
        let mut registry = VtnRegistry::new("openadr-vtn");
        registry
            .register(accepted_polling_ven("ven-b"))
            .expect("register b");
        registry.register(accepted_polling_ven("ven-a")).expect("register a");
        let ids: Vec<&str> = registry.all().map(|v| v.ven_id.as_str()).collect();
        assert_eq!(ids, vec!["ven-a", "ven-b"], "BTreeMap ordering is deterministic");
        // Duplicate registration refused.
        assert_eq!(
            registry.register(accepted_polling_ven("ven-a")),
            Err(VtnRegistryError::VenAlreadyRegistered {
                ven_id: "ven-a".to_string()
            })
        );
        // Update of an unknown VEN refused.
        assert_eq!(
            registry.update(accepted_polling_ven("ven-z")),
            Err(VtnRegistryError::VenNotRegistered {
                ven_id: "ven-z".to_string()
            })
        );
    }

    #[test]
    fn invalid_ven_records_are_refused_at_registration() {
        let mut registry = VtnRegistry::new("openadr-vtn");
        // Ven::new refuses the empty id at construction, before the
        // registry ever sees it: the refusal point is the constructor.
        assert!(matches!(
            Ven::new(String::new(), "x".to_string(), None, 60_000, true),
            Err(ModelError::EmptyField("ven_id"))
        ));
    }

    #[test]
    fn target_resolution_refuses_unknown_and_unaccepted_vens() {
        let mut registry = VtnRegistry::new("openadr-vtn");
        registry.register(accepted_polling_ven("ven-a")).expect("register");
        registry
            .register(Ven::new(
                "ven-pending".to_string(),
                "pending".to_string(),
                None,
                60_000,
                false,
            )
            .expect("valid ven"))
            .expect("register pending");
        assert_eq!(
            registry.resolve_targets(&["ven-z".to_string()]),
            Err(VtnRegistryError::VenNotRegistered {
                ven_id: "ven-z".to_string()
            })
        );
        assert_eq!(
            registry.resolve_targets(&["ven-pending".to_string()]),
            Err(VtnRegistryError::VenNotAccepted {
                ven_id: "ven-pending".to_string()
            })
        );
        registry
            .resolve_targets(&["ven-a".to_string()])
            .expect("accepted VEN resolves");
    }

    #[test]
    fn planner_prefers_first_declared_mode_and_refuses_unservable() {
        let mut registry = VtnRegistry::new("openadr-vtn");
        registry.register(accepted_polling_ven("ven-a")).expect("register");
        registry
            .register(Ven::new(
                "ven-push".to_string(),
                "push VEN".to_string(),
                Some("https://ven.example/push".to_string()),
                60_000,
                true,
            )
            .expect("valid ven"))
            .expect("register push ven");
        let planner =
            DeliveryPlanner::new("openadr-vtn", vec![DeliveryMode::Push, DeliveryMode::Polling])
                .expect("valid planner");
        // Push VEN: push wins (declared first).
        let plan = planner
            .plan_for(&registry, "ven-push", &[DeliveryMode::Push, DeliveryMode::Polling])
            .expect("push plan");
        assert_eq!(plan.mode, DeliveryMode::Push);
        assert!(plan.describe().contains("https://ven.example/push"));
        // Polling-only VEN: push unavailable, polling serves.
        let plan = planner
            .plan_for(&registry, "ven-a", &[DeliveryMode::Push, DeliveryMode::Polling])
            .expect("poll plan");
        assert_eq!(plan.mode, DeliveryMode::Polling);
        // Polling-only VEN with push requested only: named refusal.
        assert_eq!(
            planner.plan_for(&registry, "ven-a", &[DeliveryMode::Push]),
            Err(DeliveryPlanError::ModeUnavailable {
                ven_id: "ven-a".to_string(),
                mode: "push".to_string()
            })
        );
    }

    #[test]
    fn planner_without_declared_modes_is_a_construction_error() {
        assert!(DeliveryPlanner::new("openadr-vtn", Vec::new()).is_err());
    }
}
