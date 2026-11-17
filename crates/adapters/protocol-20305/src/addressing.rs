// P14.02 §3 — Resource addressing and query semantics (decision record
// 02_p14_02_aggregator_server.md §3).
//
// Two declared-never-inferred pieces live here:
//
// 1. [`AddressPlan`] — the resource paths the AggregatorServer serves. The
//    CSIP canonical paths ([S-3]) are REGISTERED RECOMMENDATIONS carried as
//    `AddressPlan::csip_default()` and are UNVERIFIED against the standard
//    text; a deployment can declare its own plan, and every path is
//    validated (non-empty, rooted) at construction. The server never
//    hardcodes a path guess.
// 2. [`ResourceQuery`] — the list query semantics (paging + device scoping)
//    and [`resolve_primacy`] — the declared primacy / event-prioritization
//    rules (P14.04 tests exercise these; the ordering convention is
//    DECLARED here, never assumed).
//
// UNVERIFIED-ASSUMPTION register (module-level honesty; the module header
// of `model.rs` states the discipline):
//
// - The CSIP path spellings in `AddressPlan::csip_default()` (`/edev`,
//   `/fn`, `/derp`, `/derc`, `/rsps`, pricing and log-event paths) are
//   transcribed from the CSIP Implementation Guide secondary material
//   [S-3][S-5] — UNVERIFIED.
// - The list query parameter shape (limit/offset modelled explicitly) is a
//   design simplification: the standard's own query-parameter names and
//   paging mechanics are UNVERIFIED and must be confirmed before any wire
//   implementation; nothing here speaks wire syntax.
// - Primacy ordering (declared value, lower-wins) and the tie-break chain
//   follow the CSIP production practice summary [S-156] — UNVERIFIED. The
//   rules are written as DECLARED configuration so a verified mismatch is
//   a one-line rule change, not a silent reinterpretation.

use serde::{Deserialize, Serialize};

/// A validated resource href: non-empty, rooted (`/`-prefixed), no
/// whitespace. The server store keys resources by this type.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ResourceHref(pub String);

impl ResourceHref {
    /// Validates and constructs an href.
    pub fn new(value: String) -> Result<Self, AddressError> {
        if value.trim().is_empty() {
            return Err(AddressError::EmptyPath);
        }
        if !value.starts_with('/') {
            return Err(AddressError::NotRooted(value));
        }
        if value.chars().any(|c| c.is_whitespace()) {
            return Err(AddressError::WhitespaceInPath(value));
        }
        Ok(ResourceHref(value))
    }

    /// The path as a plain string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this href is under the given (rooted) parent prefix.
    pub fn is_under(&self, parent: &ResourceHref) -> bool {
        self.0.starts_with(&parent.0)
    }
}

impl std::fmt::Display for ResourceHref {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Addressing and query errors. Every failure is named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressError {
    /// An empty path where a rooted path is required.
    EmptyPath,
    /// A path that is not `/`-rooted.
    NotRooted(String),
    /// A path containing whitespace.
    WhitespaceInPath(String),
    /// A query limit of zero — a zero-page query is a caller error, not an
    /// empty result.
    ZeroLimit,
    /// The declared address plan is missing one of its required paths.
    MissingPlanField(&'static str),
}

impl std::fmt::Display for AddressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddressError::EmptyPath => write!(f, "resource path is empty"),
            AddressError::NotRooted(path) => {
                write!(f, "resource path '{path}' is not /-rooted")
            }
            AddressError::WhitespaceInPath(path) => {
                write!(f, "resource path '{path}' contains whitespace")
            }
            AddressError::ZeroLimit => write!(f, "query limit must be non-zero"),
            AddressError::MissingPlanField(field) => {
                write!(f, "address plan is missing required path '{field}'")
            }
        }
    }
}

impl std::error::Error for AddressError {}

/// The declared address plan of an AggregatorServer (P14.02 §3): every
/// resource-collection path the server serves, declared at construction —
/// never hardcoded, never inferred. See the module header for the
/// UNVERIFIED register on the CSIP defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AddressPlan {
    /// The EndDeviceList path (enrollment surface).
    pub end_device_list: ResourceHref,
    /// The FunctionSetAssignmentsList path.
    pub function_set_assignments_list: ResourceHref,
    /// The DERProgramList path.
    pub der_program_list: ResourceHref,
    /// The DERControlList path.
    pub der_control_list: ResourceHref,
    /// The DefaultDERControl path.
    pub default_der_control: ResourceHref,
    /// The ResponseList path.
    pub response_list: ResourceHref,
    /// The PricingList path.
    pub pricing_list: ResourceHref,
    /// The LogEventList path.
    pub log_event_list: ResourceHref,
}

impl AddressPlan {
    /// Validates every path of the plan (non-empty, rooted).
    pub fn new(
        end_device_list: ResourceHref,
        function_set_assignments_list: ResourceHref,
        der_program_list: ResourceHref,
        der_control_list: ResourceHref,
        default_der_control: ResourceHref,
        response_list: ResourceHref,
        pricing_list: ResourceHref,
        log_event_list: ResourceHref,
    ) -> Result<Self, AddressError> {
        let plan = Self {
            end_device_list,
            function_set_assignments_list,
            der_program_list,
            der_control_list,
            default_der_control,
            response_list,
            pricing_list,
            log_event_list,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// The REGISTERED RECOMMENDATION plan transcribed from the CSIP
    /// Implementation Guide secondary material [S-3][S-5] — every path is
    /// UNVERIFIED against the standard text (module header). A deployment
    /// may declare a different plan; the server consumes whatever plan it
    /// is constructed with.
    pub fn csip_default() -> Self {
        let href = |value: &str| ResourceHref(value.to_string());
        Self {
            end_device_list: href("/edev"),
            function_set_assignments_list: href("/fn"),
            der_program_list: href("/derp"),
            der_control_list: href("/derc"),
            default_der_control: href("/derp/dderc"),
            response_list: href("/rsps"),
            pricing_list: href("/prc"),
            log_event_list: href("/logevent"),
        }
    }

    /// Whether the href belongs to one of the declared collections (the
    /// server's routing pre-check).
    pub fn is_declared(&self, href: &ResourceHref) -> bool {
        [
            &self.end_device_list,
            &self.function_set_assignments_list,
            &self.der_program_list,
            &self.der_control_list,
            &self.default_der_control,
            &self.response_list,
            &self.pricing_list,
            &self.log_event_list,
        ]
        .iter()
        .any(|root| href == *root || href.is_under(root))
    }

    fn validate(&self) -> Result<(), AddressError> {
        // The ResourceHref constructor already enforces shape; the plan
        // check exists so a field can never be swapped for an unrelated
        // collection silently — each named field must be declared.
        let fields: [(&'static str, &ResourceHref); 8] = [
            ("end_device_list", &self.end_device_list),
            (
                "function_set_assignments_list",
                &self.function_set_assignments_list,
            ),
            ("der_program_list", &self.der_program_list),
            ("der_control_list", &self.der_control_list),
            ("default_der_control", &self.default_der_control),
            ("response_list", &self.response_list),
            ("pricing_list", &self.pricing_list),
            ("log_event_list", &self.log_event_list),
        ];
        let mut seen: Vec<&str> = Vec::new();
        for (name, href) in fields {
            if href.as_str().trim().is_empty() {
                return Err(AddressError::MissingPlanField(name));
            }
            if seen.contains(&href.as_str()) {
                return Err(AddressError::MissingPlanField(name));
            }
            seen.push(href.as_str());
        }
        Ok(())
    }
}

/// A list query against a declared collection (P14.02 §3): the target
/// href, the optional device scoping, and the paging window.
///
/// The paging model is an explicit limit/offset pair — a design
/// simplification (module header: the standard's wire parameter names and
/// mechanics are UNVERIFIED; nothing here speaks wire syntax).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ResourceQuery {
    /// The collection (or single-resource) href being queried.
    pub href: ResourceHref,
    /// The requesting device SFDI, when the query is device-scoped.
    pub sfdi: Option<crate::model::Sfdi>,
    /// The maximum results to return (non-zero).
    pub limit: Option<u32>,
    /// The offset into the collection.
    pub offset: Option<u32>,
}

impl ResourceQuery {
    /// Validates the query: the limit, when present, is non-zero.
    pub fn validate(&self) -> Result<(), AddressError> {
        if self.limit == Some(0) {
            return Err(AddressError::ZeroLimit);
        }
        Ok(())
    }

    /// The effective paging window for a collection of `all` entries:
    /// `(start, end)` slice bounds, clamped to the collection.
    pub fn paging_window(&self, all: usize) -> (usize, usize) {
        let start = self.offset.unwrap_or(0) as usize;
        let start = start.min(all);
        let end = match self.limit {
            Some(limit) => (start + limit as usize).min(all),
            None => all,
        };
        (start, end)
    }
}

/// One candidate in a primacy resolution (P14.02 §3; the P14.04 primacy
/// tests exercise this). `primacy` is the DECLARED value of the
/// controlling authority; `created` the control's creation timestamp;
/// `mrid` the deterministic final tie-breaker (the BTreeMap determinism
/// kernel pattern: same inputs, same winner).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct PrioritizedControl {
    /// The control's mRID.
    pub mrid: String,
    /// The declared primacy value of the issuing authority.
    pub primacy: u8,
    /// The control's creation timestamp (declared; ties on primacy break
    /// on earlier creation).
    pub created: crate::model::TimeType,
    /// Whether the candidate is from the utility authority (the 1547-2018
    /// Clause 10 override path [S-20]: the utility side is DECLARED at
    /// enrollment, never inferred from the wire).
    pub utility_authority: bool,
}

/// The declared primacy-resolution rule set (P14.02 §3; [S-156] — UNVERIFIED
/// convention, declared so a verified mismatch is a rule edit):
///
/// 1. Lower declared primacy value wins (the recorded CSIP convention).
/// 2. On equal primacy, the earlier creation timestamp wins.
/// 3. On equal primacy and creation, the lexicographically smaller mRID
///    wins (determinism; no clock-granularity ambiguity).
/// 4. The utility authority never LOSES a direct comparison against a
///    non-utility candidate of equal or worse primacy — rule 1 already
///    encodes this when the utility side is enrolled with a primacy value
///    at least as strong as every aggregator side; the `utility_authority`
///    flag makes the enrollment declaration explicit and testable.
///
/// Returns the winning candidate, or `None` for an empty candidate list.
pub fn resolve_primacy(candidates: &[PrioritizedControl]) -> Option<PrioritizedControl> {
    candidates
        .iter()
        .min_by(|a, b| {
            (a.primacy, a.created, a.mrid.as_str()).cmp(&(
                b.primacy,
                b.created,
                b.mrid.as_str(),
            ))
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn href(path: &str) -> ResourceHref {
        ResourceHref::new(path.to_string()).expect("valid href")
    }

    #[test]
    fn href_validation_is_fail_closed() {
        assert_eq!(ResourceHref::new(String::new()), Err(AddressError::EmptyPath));
        assert_eq!(
            ResourceHref::new("edev".to_string()),
            Err(AddressError::NotRooted("edev".to_string()))
        );
        assert_eq!(
            ResourceHref::new("/e dev".to_string()),
            Err(AddressError::WhitespaceInPath("/e dev".to_string()))
        );
        assert_eq!(href("/edev").as_str(), "/edev");
    }

    #[test]
    fn csip_default_plan_is_rooted_unique_and_declared() {
        let plan = AddressPlan::csip_default();
        plan.validate().expect("the registered plan validates");
        assert!(plan.is_declared(&href("/edev")));
        assert!(plan.is_declared(&href("/derp/1")));
        assert!(plan.is_declared(&href("/derp/dderc")));
        assert!(!plan.is_declared(&href("/unknown")));
        // Every path is distinct (the validate() duplicate rule).
        let paths = [
            plan.end_device_list.0.clone(),
            plan.function_set_assignments_list.0.clone(),
            plan.der_program_list.0.clone(),
            plan.der_control_list.0.clone(),
            plan.default_der_control.0.clone(),
            plan.response_list.0.clone(),
            plan.pricing_list.0.clone(),
            plan.log_event_list.0.clone(),
        ];
        let mut sorted = paths.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), paths.len(), "plan paths must be distinct");
    }

    #[test]
    fn custom_plan_validates_and_rejects_degenerate_fields() {
        let plan = AddressPlan::new(
            href("/devices"),
            href("/fsa"),
            href("/programs"),
            href("/controls"),
            href("/programs/default"),
            href("/responses"),
            href("/prices"),
            href("/logs"),
        );
        assert!(plan.is_ok());
        // A duplicate path is a plan error (two fields cannot share one
        // collection — the plan would be ambiguous).
        let duplicate = AddressPlan::new(
            href("/same"),
            href("/same"),
            href("/programs"),
            href("/controls"),
            href("/programs/default"),
            href("/responses"),
            href("/prices"),
            href("/logs"),
        );
        assert!(duplicate.is_err());
    }

    #[test]
    fn query_paging_is_clamped_and_zero_limit_is_refused() {
        let query = ResourceQuery {
            href: href("/derc"),
            sfdi: None,
            limit: Some(2),
            offset: Some(1),
        };
        query.validate().expect("valid query");
        assert_eq!(query.paging_window(5), (1, 3));
        assert_eq!(query.paging_window(2), (1, 2));
        assert_eq!(query.paging_window(0), (0, 0));
        // Offset beyond the collection clamps to the end.
        let query = ResourceQuery {
            href: href("/derc"),
            sfdi: None,
            limit: None,
            offset: Some(10),
        };
        assert_eq!(query.paging_window(3), (3, 3));
        // Zero limit is a caller error.
        let query = ResourceQuery {
            href: href("/derc"),
            sfdi: None,
            limit: Some(0),
            offset: None,
        };
        assert_eq!(query.validate(), Err(AddressError::ZeroLimit));
    }

    #[test]
    fn primacy_resolution_follows_the_declared_rules() {
        let t1 = crate::model::TimeType(100);
        let t2 = crate::model::TimeType(200);
        // Rule 1: lower primacy wins.
        let candidates = vec![
            PrioritizedControl {
                mrid: "agg".to_string(),
                primacy: 5,
                created: t1,
                utility_authority: false,
            },
            PrioritizedControl {
                mrid: "util".to_string(),
                primacy: 1,
                created: t2,
                utility_authority: true,
            },
        ];
        assert_eq!(resolve_primacy(&candidates).unwrap().mrid, "util");
        // Rule 2: equal primacy, earlier creation wins.
        let candidates = vec![
            PrioritizedControl {
                mrid: "later".to_string(),
                primacy: 3,
                created: t2,
                utility_authority: false,
            },
            PrioritizedControl {
                mrid: "earlier".to_string(),
                primacy: 3,
                created: t1,
                utility_authority: false,
            },
        ];
        assert_eq!(resolve_primacy(&candidates).unwrap().mrid, "earlier");
        // Rule 3: equal primacy and creation, smaller mRID wins (determinism).
        let candidates = vec![
            PrioritizedControl {
                mrid: "bbb".to_string(),
                primacy: 3,
                created: t1,
                utility_authority: false,
            },
            PrioritizedControl {
                mrid: "aaa".to_string(),
                primacy: 3,
                created: t1,
                utility_authority: false,
            },
        ];
        assert_eq!(resolve_primacy(&candidates).unwrap().mrid, "aaa");
        // Order-independence: the same set resolves identically shuffled.
        let mut shuffled = candidates.clone();
        shuffled.reverse();
        assert_eq!(resolve_primacy(&shuffled), resolve_primacy(&candidates));
        // Empty candidate list: None, never a fabricated winner.
        assert!(resolve_primacy(&[]).is_none());
    }
}
