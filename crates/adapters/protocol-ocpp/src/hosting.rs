// P17.02 — The CSMS hosting boundary: the two hosting shapes as declared
// data, with the edge-agent-hosted depot shape as the P17-E-2 DEFAULT
// implemented per conventions §4 (agents never close the E-decision; the
// default is implemented only because P17.02 explicitly says so).
//
// Decision record:
// `docs/phases-11-20/phase-17-ev-fleet-charging/records/02_p17_02_csms_hosting_boundary.md`
// (registers P17-E-2 OPEN with the recorded DEFAULT and what closes it).
//
// The two shapes, both DESIGNED here:
//
// - **Edge-agent-hosted (the P17-E-2 DEFAULT for depot sites).** The CSMS
//   endpoint runs inside the edge agent at the depot. WHY (the recorded
//   rationale): offline tolerance matters at a depot — backhaul loss must
//   not sever the charge-point↔CSMS link — and the edge agent's existing
//   store-and-forward buffer (P6.09) and mTLS uplink are REUSED, not
//   rebuilt. The telemetry and command-relay paths run edge-local; the
//   platform uplink carries records and relayed lifecycle output.
// - **Control-plane-hosted.** The CSMS endpoint runs in the control
//   plane. WHERE it fits: sites with reliable backhaul and no edge agent
//   deployment; the charge points connect directly to the platform. The
//   recorded cost: a backhaul outage severs the CSMS link (and the
//   domain's link-loss rule — NOT asset unavailability — governs the
//   recorded state).
//
// What this module IS: the declared SHAPE data the composition root
// consumes. What it is NOT: the transport, the process placement, or any
// deployment artifact — the WebSocket seam (crate::csms::CsmsTransport)
// is UNRUN in both shapes, and no server has ever run in either.

use serde::{Deserialize, Serialize};

/// The CSMS hosting shape (P17-E-2; both shapes designed, the DEFAULT
/// implemented per conventions §4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CsmsHostingShape {
    /// The edge agent hosts the CSMS endpoint (the P17-E-2 DEFAULT for
    /// depot sites). The store-and-forward flag is STRUCTURAL: the shape
    /// is refused without buffer reuse (a depot CSMS without offline
    /// tolerance would contradict the recorded rationale).
    EdgeAgentHosted {
        /// The store-and-forward buffer reuse (P6.09). STRUCTURAL: true.
        store_and_forward: bool,
    },
    /// The control plane hosts the CSMS endpoint (sites with reliable
    /// backhaul and no edge agent). Requires a RECORDED REASON (the
    /// recorded cost is named — an unexplained shape is a construction
    /// error).
    ControlPlaneHosted {
        /// The recorded reason the control-plane shape was chosen.
        reason: String,
    },
}

impl CsmsHostingShape {
    /// The P17-E-2 DEFAULT for depot sites (implemented per conventions
    /// §4; the E-decision itself stays OPEN).
    pub fn for_depot_site() -> Self {
        CsmsHostingShape::EdgeAgentHosted {
            store_and_forward: true,
        }
    }

    /// The recorded DEFAULT statement (travels with every shape choice).
    pub const DEFAULT_STATEMENT: &'static str =
        "P17-E-2 DEFAULT (OPEN): edge-agent-hosted for depot sites — offline \
         tolerance matters at a depot and the edge agent's store-and-forward \
         buffer (P17.02 rationale) and mTLS uplink are reused. The E-decision \
         is closed by the human owner naming the first depot site and its \
         connectivity profile.";

    /// Whether the shape tolerates a backhaul outage (the honest
    /// capability answer per shape: edge-agent-hosted yes, control-
    /// plane-hosted no — the cost is recorded, not hidden).
    pub const fn tolerates_backhaul_outage(&self) -> bool {
        matches!(self, CsmsHostingShape::EdgeAgentHosted { .. })
    }
}

/// The placement record a composition root constructs (the shape, the
/// site, and the adapter surfaces it binds). Construction validates the
/// shape's own rules: the edge-agent shape requires store-and-forward
/// reuse; the control-plane shape requires a recorded reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsmsPlacement {
    /// The site label (deployment-declared; non-empty).
    pub site: String,
    /// The hosting shape.
    pub shape: CsmsHostingShape,
    /// The recorded notes (review pointers, boundary references).
    pub notes: Vec<String>,
}

impl CsmsPlacement {
    /// Constructs the placement; fails closed on an empty site label, an
    /// edge-agent shape without buffer reuse, or a control-plane shape
    /// without a recorded reason.
    pub fn new(site: String, shape: CsmsHostingShape) -> Result<Self, HostingError> {
        if site.trim().is_empty() {
            return Err(HostingError::EmptyField("site"));
        }
        match &shape {
            CsmsHostingShape::EdgeAgentHosted {
                store_and_forward,
            } => {
                if !*store_and_forward {
                    return Err(HostingError::EdgeShapeRequiresStoreAndForward);
                }
            }
            CsmsHostingShape::ControlPlaneHosted { reason } => {
                if reason.trim().is_empty() {
                    return Err(HostingError::ControlPlaneShapeRequiresReason);
                }
            }
        }
        Ok(Self {
            site,
            shape,
            notes: Vec::new(),
        })
    }

    /// The depot DEFAULT placement (the P17-E-2 DEFAULT shape for a named
    /// depot site).
    pub fn depot_default(site: String) -> Result<Self, HostingError> {
        Self::new(site, CsmsHostingShape::for_depot_site())
    }

    /// The shape's honest capability summary (for the composition
    /// surface).
    pub fn capability_summary(&self) -> String {
        match &self.shape {
            CsmsHostingShape::EdgeAgentHosted { .. } => format!(
                "site '{}': CSMS edge-agent-hosted (P17-E-2 DEFAULT, OPEN); backhaul-outage \
                 tolerant; store-and-forward buffer reused (P6.09)",
                self.site
            ),
            CsmsHostingShape::ControlPlaneHosted { reason } => format!(
                "site '{}': CSMS control-plane-hosted; backhaul outage severs the CSMS link \
                 (the link-loss rule, not asset unavailability, governs the recorded state); \
                 recorded reason: {reason}",
                self.site
            ),
        }
    }
}

/// Hosting errors (named).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostingError {
    /// The site label is empty.
    EmptyField(&'static str),
    /// The edge-agent shape was constructed without the store-and-forward
    /// buffer — the shape contradicts its own recorded rationale and is
    /// refused.
    EdgeShapeRequiresStoreAndForward,
    /// The control-plane shape was constructed without a recorded reason.
    ControlPlaneShapeRequiresReason,
}

impl std::fmt::Display for HostingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostingError::EmptyField(field) => write!(f, "field '{field}' is empty"),
            HostingError::EdgeShapeRequiresStoreAndForward => {
                write!(f, "the edge-agent-hosted shape requires the store-and-forward buffer reuse (P6.09); the shape's recorded rationale is offline tolerance")
            }
            HostingError::ControlPlaneShapeRequiresReason => {
                write!(f, "the control-plane-hosted shape requires a recorded reason (the backhaul-outage cost is named, not hidden)")
            }
        }
    }
}

impl std::error::Error for HostingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn depot_default_is_edge_agent_hosted_with_buffer_reuse() {
        let placement = CsmsPlacement::depot_default("depot-north".to_string())
            .expect("valid placement");
        assert_eq!(placement.shape, CsmsHostingShape::for_depot_site());
        assert!(placement.shape.tolerates_backhaul_outage());
        let summary = placement.capability_summary();
        assert!(summary.contains("P17-E-2 DEFAULT, OPEN"));
        assert!(summary.contains("depot-north"));
        assert!(CsmsHostingShape::DEFAULT_STATEMENT.contains("edge-agent-hosted for depot sites"));
    }

    #[test]
    fn shapes_enforce_their_own_recorded_rules() {
        // Edge shape without buffer reuse is refused.
        assert_eq!(
            CsmsPlacement::new(
                "depot-north".to_string(),
                CsmsHostingShape::EdgeAgentHosted {
                    store_and_forward: false
                },
            ),
            Err(HostingError::EdgeShapeRequiresStoreAndForward)
        );
        // Control-plane shape without a reason is refused.
        assert_eq!(
            CsmsPlacement::new(
                "site-a".to_string(),
                CsmsHostingShape::ControlPlaneHosted {
                    reason: "  ".to_string()
                },
            ),
            Err(HostingError::ControlPlaneShapeRequiresReason)
        );
        // A control-plane shape with a reason constructs and honestly
        // reports the backhaul cost.
        let placement = CsmsPlacement::new(
            "site-a".to_string(),
            CsmsHostingShape::ControlPlaneHosted {
                reason: "reliable backhaul, no edge agent deployed".to_string(),
            },
        )
        .expect("valid");
        assert!(!placement.shape.tolerates_backhaul_outage());
        assert!(placement
            .capability_summary()
            .contains("backhaul outage severs the CSMS link"));
        // An empty site label is refused.
        assert_eq!(
            CsmsPlacement::depot_default("  ".to_string()),
            Err(HostingError::EmptyField("site"))
        );
    }
}
