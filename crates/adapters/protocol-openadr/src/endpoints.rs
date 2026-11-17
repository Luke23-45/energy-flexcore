// P14.05 — The OpenADR 3.0 REST/JSON endpoint design (the designed seam).
//
// The VTN's HTTP surface is designed here AS DATA: every route is a
// [`RouteSpec`] record naming the method, the path pattern, the
// request/response payload types, and the adapter-side handler contract.
// NO HTTP framework is imported, no socket is opened, no server loop
// exists — endpoint serving is a designed seam that a later wave (or the
// operator's own deployment shell) wires onto [`VtnEndpointDesign`].
//
// This is the honest deliverable class the phase plan states for P14.05:
// "endpoint serving is a designed seam (no HTTP framework dependency)".
// The routes below follow the secondary-source reading of the OpenADR 3.0
// REST/JSON surface [S-10]; EVERY path literal is UNVERIFIED until checked
// against the specification text (implementation-time human gate, plan §7
// note 6). A path that differs at that gate is a recorded edit here — the
// route data is the single place the serving layer would read, so the
// design record stays the single source.

use serde::{Deserialize, Serialize};

/// The HTTP verb of a designed route (closed vocabulary; the design does
/// not invent verbs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
}

impl HttpMethod {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Delete => "DELETE",
        }
    }
}

/// The payload model types a route exchanges. Closed vocabulary: a route
/// may only exchange types this enum names, so the serving layer can never
/// invent an ad-hoc payload shape. Every variant maps to a type in
/// [`crate::model`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PayloadKind {
    /// [`crate::model::OpenAdrEvent`] — event publication / poll response.
    Event,
    /// [`crate::model::Ven`] — registration / VEN management payloads.
    Ven,
    /// [`crate::model::ReportSpec`] — report request payloads.
    ReportSpec,
    /// A JSON error record (the platform's named-error shape, serialized).
    Error,
    /// An empty acknowledgement (e.g. a report-submission receipt).
    Ack,
}

impl PayloadKind {
    /// Canonical name for records and error messages.
    pub const fn name(self) -> &'static str {
        match self {
            PayloadKind::Event => "event",
            PayloadKind::Ven => "ven",
            PayloadKind::ReportSpec => "report_spec",
            PayloadKind::Error => "error",
            PayloadKind::Ack => "ack",
        }
    }
}

/// One designed route of the VTN surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteSpec {
    /// The HTTP verb.
    pub method: HttpMethod,
    /// The path pattern. UNVERIFIED against the OpenADR 3.0 specification
    /// text [S-10]; the `{ven_id}`-style tokens are this design's path
    /// parameter syntax (UNVERIFIED whether 3.0 REST uses the same token
    /// shape).
    pub path_pattern: String,
    /// Which common resource class the route serves (ties the route to the
    /// reviewed scope declaration
    /// (`flexcore_protocol_ports::scope::ResourceClass`) — a route outside
    /// the declared classes is a design error).
    pub interaction: flexcore_protocol_ports::scope::ResourceClass,
    /// The declared delivery mode the route serves.
    pub delivery_mode: Option<crate::scope::DeliveryMode>,
    /// The request payload kind.
    pub request: PayloadKind,
    /// The response payload kind.
    pub response: PayloadKind,
    /// The route's purpose, in one reviewed sentence.
    pub purpose: String,
}

/// Errors returned by the endpoint design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointDesignError {
    /// A route field is empty or malformed.
    InvalidRoute(&'static str),
    /// Two designed routes share (method, path pattern).
    DuplicateRoute { method: String, path: String },
    /// A route's delivery mode is declared for an interaction that does not
    /// carry one (e.g. a delivery mode on VEN management).
    DeliveryModeNotAllowed { path: String },
}

impl std::fmt::Display for EndpointDesignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EndpointDesignError::InvalidRoute(field) => {
                write!(f, "route spec field '{field}' is empty or malformed")
            }
            EndpointDesignError::DuplicateRoute { method, path } => {
                write!(f, "duplicate designed route {method} {path}")
            }
            EndpointDesignError::DeliveryModeNotAllowed { path } => {
                write!(f, "route '{path}' declares a delivery mode for an interaction that does not carry one")
            }
        }
    }
}

impl std::error::Error for EndpointDesignError {}

/// The complete designed VTN endpoint surface. Constructed only through
/// [`VtnEndpointDesign::new`], which validates every route and rejects
/// duplicates — there is no `Default` and no way to add routes after
/// construction (the surface is a reviewed design artifact, not a mutable
/// registry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtnEndpointDesign {
    /// The adapter id the design belongs to.
    pub adapter_id: String,
    /// The designed routes, in design order.
    pub routes: Vec<RouteSpec>,
    /// The serving-seam statement (recorded verbatim on the design record).
    pub serving_seam_statement: String,
}

impl VtnEndpointDesign {
    /// Constructs and validates an endpoint design from the given routes.
    pub fn new(
        adapter_id: String,
        routes: Vec<RouteSpec>,
        serving_seam_statement: String,
    ) -> Result<Self, EndpointDesignError> {
        if adapter_id.trim().is_empty() {
            return Err(EndpointDesignError::InvalidRoute("adapter_id"));
        }
        if serving_seam_statement.trim().is_empty() {
            return Err(EndpointDesignError::InvalidRoute("serving_seam_statement"));
        }
        let mut seen: Vec<(HttpMethod, &str)> = Vec::new();
        for route in &routes {
            if route.path_pattern.trim().is_empty() || !route.path_pattern.starts_with('/') {
                return Err(EndpointDesignError::InvalidRoute("path_pattern"));
            }
            if route.purpose.trim().is_empty() {
                return Err(EndpointDesignError::InvalidRoute("purpose"));
            }
            // A delivery mode only makes sense on event/report delivery
            // routes; registration and VEN-management routes carry none.
            let carries_mode = matches!(
                route.interaction,
                flexcore_protocol_ports::scope::ResourceClass::Control
                    | flexcore_protocol_ports::scope::ResourceClass::Telemetry
            );
            if route.delivery_mode.is_some() && !carries_mode {
                return Err(EndpointDesignError::DeliveryModeNotAllowed {
                    path: route.path_pattern.clone(),
                });
            }
            if seen
                .iter()
                .any(|&(m, p)| m == route.method && p == route.path_pattern)
            {
                return Err(EndpointDesignError::DuplicateRoute {
                    method: route.method.name().to_string(),
                    path: route.path_pattern.clone(),
                });
            }
            seen.push((route.method, route.path_pattern.as_str()));
        }
        Ok(Self {
            adapter_id,
            routes,
            serving_seam_statement,
        })
    }

    /// The canonical designed VTN surface for this crate (record 21 §3).
    ///
    /// UNVERIFIED path literals: every path below follows the
    /// secondary-source reading of OpenADR 3.0 REST/JSON [S-10] and MUST be
    /// checked against the specification text at implementation time. The
    /// checks are: (1) the registration route name and verb; (2) the poll
    /// route shape and whether polling is GET or POST; (3) the push /
    /// notification route; (4) the report request/submission routes; (5)
    /// whether VEN management is protocol-specified or FlexCore-specific
    /// (it is recorded as FlexCore-specific here).
    pub fn canonical_vtn_design() -> Result<Self, EndpointDesignError> {
        use crate::scope::DeliveryMode;
        use flexcore_protocol_ports::scope::ResourceClass;
        let routes = vec![
            // Party registration: a VEN enrols and agrees a poll frequency.
            RouteSpec {
                method: HttpMethod::Post,
                // UNVERIFIED: 3.0 registration path.
                path_pattern: "/registration".to_string(),
                interaction: ResourceClass::Discovery,
                delivery_mode: None,
                request: PayloadKind::Ven,
                response: PayloadKind::Ven,
                purpose: "VEN party registration: enrolment record created, poll frequency agreed; acceptance stays a human-reviewed operator decision".to_string(),
            },
            // Event polling: the VEN pulls events at the agreed frequency.
            RouteSpec {
                method: HttpMethod::Get,
                // UNVERIFIED: 3.0 poll path shape.
                path_pattern: "/poll/{ven_id}".to_string(),
                interaction: ResourceClass::Control,
                delivery_mode: Some(DeliveryMode::Polling),
                request: PayloadKind::Ack,
                response: PayloadKind::Event,
                purpose: "VEN-initiated event poll: returns the VEN's undelivered events (polling delivery mode)".to_string(),
            },
            // Push delivery: the VTN notifies a VEN-registered endpoint.
            RouteSpec {
                method: HttpMethod::Post,
                // UNVERIFIED: 3.0 push/notification path; the VEN's
                // endpoint is the VEN record's push_endpoint.
                path_pattern: "/push/{ven_id}".to_string(),
                interaction: ResourceClass::Control,
                delivery_mode: Some(DeliveryMode::Push),
                request: PayloadKind::Event,
                response: PayloadKind::Ack,
                purpose: "VTN-initiated push notification of a new or modified event to a VEN that registered a push endpoint (push delivery mode)".to_string(),
            },
            // Report requests: the VTN asks a VEN for telemetry reports.
            RouteSpec {
                method: HttpMethod::Post,
                // UNVERIFIED: 3.0 report request path.
                path_pattern: "/report/{ven_id}".to_string(),
                interaction: ResourceClass::Telemetry,
                delivery_mode: Some(DeliveryMode::Polling),
                request: PayloadKind::ReportSpec,
                response: PayloadKind::Ack,
                purpose: "report-spec request: register a telemetry report requirement for one VEN (report payloads are telemetry records, never commands)".to_string(),
            },
            // FlexCore-specific VEN management surface (NOT
            // protocol-specified; recorded as such in the scope).
            RouteSpec {
                method: HttpMethod::Get,
                path_pattern: "/vens".to_string(),
                interaction: ResourceClass::Discovery,
                delivery_mode: None,
                request: PayloadKind::Ack,
                response: PayloadKind::Ven,
                purpose: "operator VEN inventory (FlexCore-specific surface, not an OpenADR-specified interaction)".to_string(),
            },
        ];
        Self::new(
            "openadr-vtn".to_string(),
            routes,
            "endpoint serving is a designed seam: this design names method, path, and payload \
             kinds as data; no HTTP framework is imported and no socket is opened in this crate"
                .to_string(),
        )
    }

    /// The routes serving one scope interaction.
    pub fn routes_for(
        &self,
        interaction: flexcore_protocol_ports::scope::ResourceClass,
    ) -> Vec<&RouteSpec> {
        self.routes.iter().filter(|r| r.interaction == interaction).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scope::DeliveryMode;
    use flexcore_protocol_ports::scope::ResourceClass;

    #[test]
    fn canonical_design_constructs_and_routes_resolve() {
        let design = VtnEndpointDesign::canonical_vtn_design().expect("valid design");
        assert_eq!(design.routes.len(), 5);
        assert_eq!(
            design.routes_for(ResourceClass::Control).len(),
            2,
            "poll + push are the two event-delivery routes"
        );
        assert_eq!(design.routes_for(ResourceClass::Discovery).len(), 2);
        // The two event-delivery routes carry the two declared delivery modes.
        let modes: Vec<_> = design
            .routes_for(ResourceClass::Control)
            .iter()
            .filter_map(|r| r.delivery_mode)
            .collect();
        assert!(modes.contains(&DeliveryMode::Polling));
        assert!(modes.contains(&DeliveryMode::Push));
    }

    #[test]
    fn duplicate_routes_are_refused() {
        let route = RouteSpec {
            method: HttpMethod::Get,
            path_pattern: "/poll/{ven_id}".to_string(),
            interaction: ResourceClass::Control,
            delivery_mode: Some(DeliveryMode::Polling),
            request: PayloadKind::Ack,
            response: PayloadKind::Event,
            purpose: "poll".to_string(),
        };
        assert_eq!(
            VtnEndpointDesign::new(
                "openadr-vtn".to_string(),
                vec![route.clone(), route],
                "serving is a designed seam".to_string(),
            ),
            Err(EndpointDesignError::DuplicateRoute {
                method: "GET".to_string(),
                path: "/poll/{ven_id}".to_string()
            })
        );
    }

    #[test]
    fn delivery_mode_on_a_non_delivery_interaction_is_refused() {
        let route = RouteSpec {
            method: HttpMethod::Post,
            path_pattern: "/registration".to_string(),
            interaction: ResourceClass::Discovery,
            delivery_mode: Some(DeliveryMode::Push),
            request: PayloadKind::Ven,
            response: PayloadKind::Ven,
            purpose: "registration".to_string(),
        };
        assert_eq!(
            VtnEndpointDesign::new(
                "openadr-vtn".to_string(),
                vec![route],
                "serving is a designed seam".to_string(),
            ),
            Err(EndpointDesignError::DeliveryModeNotAllowed {
                path: "/registration".to_string()
            })
        );
    }

    #[test]
    fn path_patterns_must_start_with_a_slash() {
        let route = RouteSpec {
            method: HttpMethod::Get,
            path_pattern: "poll".to_string(),
            interaction: ResourceClass::Control,
            delivery_mode: Some(DeliveryMode::Polling),
            request: PayloadKind::Ack,
            response: PayloadKind::Event,
            purpose: "poll".to_string(),
        };
        assert_eq!(
            VtnEndpointDesign::new(
                "openadr-vtn".to_string(),
                vec![route],
                "serving is a designed seam".to_string(),
            ),
            Err(EndpointDesignError::InvalidRoute("path_pattern"))
        );
    }
}
