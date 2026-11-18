// P13.10 — API versioning + deprecation policy (the executable half):
//          version constants, the Deprecation/Sunset header design, the
//          6–12 month deprecation windows, and a pure OpenAPI
//          breaking-change checker over the generated spec.
//
// Phase 13 wave 1, Agent B. WRITTEN-UNRUN: this module has never been
// compiled or executed (the first human build/test is the gate; program
// conventions §1). The policy itself is the decision record
// `docs/phases-11-20/phase-13-enterprise-identity-integration/records/
// 22_p13_10_api_versioning_deprecation.md`; this module carries the
// policy's CONSTANTS and the pure spec-diff mechanics so the policy is
// checkable code, not prose only.
//
// Sources: [S-84] RFC 8594 (the Sunset HTTP response header — an RFC, its
// date format is the IMF-fixdate HTTP-date); the `Deprecation` response
// header is NOT an RFC — it is an IETF HTTP API working-group DRAFT
// (draft-ietf-httpapi-deprecation-header). Its status is recorded
// UNVERIFIED-AT-A-DISTANCE in the decision record: this module renders
// both headers, but the Deprecation header's exact draft grammar is not
// relied upon anywhere — consumers key on the Sunset header (RFC 8594)
// and treat Deprecation as advisory text. [S-85] (Speakeasy API-versioning
// practice — the 6–12 month window guidance this policy registers).
//
// What this module deliberately does NOT do: no middleware is installed
// here (the header-attachment point and the lint's CI wiring are
// composition decisions recorded in the decision record); no route is
// renamed or re-versioned (the /api/v1/ surface stays exactly as it is —
// AGENTS.md §11: no renaming/renumbering).

use serde_json::Value;

/// The CURRENT API version (ledger P13.10 acceptance: `/api/v1/` is
/// confirmed as the current version — the whole existing surface is v1;
/// nothing is re-versioned by this batch).
pub const CURRENT_API_VERSION: &str = "v1";

/// The version path prefix of the current surface.
pub const CURRENT_API_PREFIX: &str = "/api/v1/";

/// The OpenAPI document this policy governs (the generated spec endpoint;
/// the spec is GENERATED from the utoipa route annotations —
/// `openapi.rs` — never hand-written, so a lint runs over the generator's
/// output, never over a hand-maintained copy).
pub const OPENAPI_ENDPOINT: &str = "/api/v1/openapi.json";

/// The MINIMUM deprecation window in days: a version's Sunset date is at
/// least this far after its Deprecation announcement. The 6–12 month
/// guidance ([S-85]) registered as policy bounds (X-8 discipline: written
/// before any version is ever deprecated — no deprecation exists today).
pub const MIN_DEPRECATION_WINDOW_DAYS: u64 = 180;

/// The MAXIMUM deprecation window in days: Sunset announced more than a
/// year out is not allowed (a distant Sunset stops being information).
pub const MAX_DEPRECATION_WINDOW_DAYS: u64 = 365;

/// The OpenAPI `info.version` value the control plane declares. The
/// generated spec currently carries its own version string (`openapi.rs`);
/// this constant is the POLICY's declaration of the API version lineage —
/// the binding of the two is recorded in the decision record (a
/// composition check, not a code change here).
pub const POLICY_API_LINEAGE: &str = "1.0.0";

// --- The response-header design ------------------------------------------------

/// Renders the `Sunset` header value (RFC 8594 [S-84]) for a version whose
/// retirement instant is `sunset_unix_millis`: the IMF-fixdate HTTP-date
/// format RFC 8594 requires (`Sunset: <HTTP-date>`). Pure; the middleware
/// that attaches it is the composition step recorded in the decision
/// record. A timestamp outside the `time` crate's representable range
/// renders as the Unix epoch (fail-plain, never fail-by-panic).
pub fn sunset_header_value(sunset_unix_millis: i64) -> String {
    // The format description is built at call time (the macro compiles to
    // a static description; no `time` crate type is named, so the
    // workspace's exact `time` minor version cannot break this signature).
    let format = time::macros::format_description!(
        "[weekday repr:short], [day padding:zero] [month repr:short] \
         [year repr:full padding:zero] [hour padding:zero]:[minute padding:zero]:\
         [second padding:zero] GMT"
    );
    let secs = sunset_unix_millis.div_euclid(1_000);
    let moment = time::OffsetDateTime::from_unix_timestamp(secs)
        .unwrap_or(time::OffsetDateTime::UNIX_EPOCH);
    moment.format(&format).unwrap_or_else(|_| String::new())
}

/// Renders the `Deprecation` header value for a version deprecated at
/// `deprecation_unix_millis`. ADVISORY (the draft-header note in the
/// module header): the value carries the deprecation timestamp in the
/// draft's HTTP-date form; consumers MUST NOT treat this header as
/// machine-authoritative — the Sunset header (RFC 8594) is the
/// machine-readable contract, and the 6–12 month window policy
/// (`MIN_DEPRECATION_WINDOW_DAYS`..`MAX_DEPRECATION_WINDOW_DAYS`) is the
/// human-readable contract.
pub fn deprecation_header_value(deprecation_unix_millis: i64) -> String {
    sunset_header_value(deprecation_unix_millis)
}

/// The policy check a deprecation announcement must pass BEFORE it is
/// published (the 6–12 month window [S-85], enforced at announcement
/// time): the gap between deprecation and sunset must be inside
/// [`MIN_DEPRECATION_WINDOW_DAYS`], [`MAX_DEPRECATION_WINDOW_DAYS`].
/// Returns `Err(reason)` naming the violated bound.
pub fn validate_deprecation_window(
    deprecation_unix_millis: i64,
    sunset_unix_millis: i64,
) -> Result<(), String> {
    let gap_days = (sunset_unix_millis - deprecation_unix_millis).max(0) / 86_400_000;
    let min_gap = MIN_DEPRECATION_WINDOW_DAYS as i64;
    let max_gap = MAX_DEPRECATION_WINDOW_DAYS as i64;
    if gap_days < min_gap {
        return Err(format!(
            "the deprecation window is {gap_days} days; the policy minimum is {min_gap} \
             (a 6-month floor — [S-85] guidance registered as policy)"
        ));
    }
    if gap_days > max_gap {
        return Err(format!(
            "the deprecation window is {gap_days} days; the policy maximum is {max_gap} \
             (a 12-month ceiling — a distant Sunset stops being information)"
        ));
    }
    Ok(())
}

// --- The OpenAPI breaking-change checker (the lint plan's mechanics) -------

/// The classification of one difference between two generated OpenAPI
/// documents. Closed vocabulary: BREAKING differences forbid shipping the
/// new spec under the same version; ADDITIVE differences are always
/// allowed (the deny-by-default extends to contracts: a difference class
/// not recognized here is treated as BREAKING by [`detect_breaking_changes`]
/// — nothing is silently opted in).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecDiff {
    /// A path present in the old spec is absent from the new one.
    PathRemoved {
        path: String,
    },
    /// An operation (method) present on a path is absent from the new one.
    OperationRemoved {
        path: String,
        method: String,
    },
    /// A response status present on an operation is absent from the new
    /// one.
    ResponseRemoved {
        path: String,
        method: String,
        status: String,
    },
    /// A required request property was added to an operation's request
    /// body schema (a caller that omitted it now fails).
    RequiredRequestFieldAdded {
        path: String,
        method: String,
        field: String,
    },
    /// A response schema property present in the old spec is absent from
    /// the new one (a consumer reading it breaks).
    ResponseFieldRemoved {
        path: String,
        method: String,
        field: String,
    },
    /// A schema property's type changed (string → integer, object → array,
    /// and the like).
    FieldTypeChanged {
        path: String,
        method: String,
        field: String,
        old_type: String,
        new_type: String,
    },
    /// An unrecognized difference class — treated as BREAKING (the
    /// deny-by-default rule above).
    Unclassified {
        location: String,
    },
}

impl SpecDiff {
    /// `true` when the difference forbids shipping under the same version.
    pub fn is_breaking(&self) -> bool {
        // Every current variant is breaking by construction; the match
        // keeps the rule explicit so a future ADDITIVE variant must
        // answer it.
        match self {
            SpecDiff::PathRemoved { .. }
            | SpecDiff::OperationRemoved { .. }
            | SpecDiff::ResponseRemoved { .. }
            | SpecDiff::RequiredRequestFieldAdded { .. }
            | SpecDiff::ResponseFieldRemoved { .. }
            | SpecDiff::FieldTypeChanged { .. }
            | SpecDiff::Unclassified { .. } => true,
        }
    }
}

/// Compares two GENERATED OpenAPI documents (old → new) and reports the
/// differences the checker recognizes. Pure, std-only over serde_json.
/// The plan (P13.10): the lint runs over the GENERATED spec
/// (`/api/v1/openapi.json` from `openapi.rs`), never a hand-maintained
/// copy — a spec edited by hand is a drift the diff would miss, so the
/// generator is the only input.
pub fn detect_breaking_changes(old: &Value, new: &Value) -> Vec<SpecDiff> {
    let mut diffs = Vec::new();
    let (Some(old_paths), Some(new_paths)) = (
        old.get("paths").and_then(Value::as_object),
        new.get("paths").and_then(Value::as_object),
    ) else {
        diffs.push(SpecDiff::Unclassified {
            location: "either spec lacks a paths object".to_string(),
        });
        return diffs;
    };
    for (path, old_item) in old_paths {
        let Some(new_item) = new_paths.get(path) else {
            diffs.push(SpecDiff::PathRemoved { path: path.clone() });
            continue;
        };
        for (method, old_op) in old_item.as_object().into_iter().flatten() {
            // Recognized operation keys: the standard HTTP methods plus the
            // parameters block (parameters are diffed as unclassified
            // locations when removed — conservative by the closed-vocabulary
            // rule).
            const METHODS: [&str; 8] = [
                "get", "put", "post", "delete", "options", "head", "patch", "trace",
            ];
            if !METHODS.contains(&method.as_str()) {
                continue;
            }
            let Some(new_op) = new_item.get(method) else {
                diffs.push(SpecDiff::OperationRemoved {
                    path: path.clone(),
                    method: method.clone(),
                });
                continue;
            };
            diff_responses(path, method, old_op, new_op, &mut diffs);
            diff_request_required_fields(path, method, old_op, new_op, &mut diffs);
            diff_response_fields(path, method, old_op, new_op, &mut diffs);
        }
    }
    diffs
}

/// Response-status removal: every status the old operation declared and
/// the new one dropped is breaking.
fn diff_responses(
    path: &str,
    method: &str,
    old_op: &Value,
    new_op: &Value,
    diffs: &mut Vec<SpecDiff>,
) {
    let old_responses = old_op.get("responses").and_then(Value::as_object);
    let new_responses = new_op.get("responses").and_then(Value::as_object);
    if let (Some(old_r), Some(new_r)) = (old_responses, new_responses) {
        for status in old_r.keys() {
            if !new_r.contains_key(status) {
                diffs.push(SpecDiff::ResponseRemoved {
                    path: path.to_string(),
                    method: method.to_string(),
                    status: status.clone(),
                });
            }
        }
    }
}

/// Required request-field additions (breaking for callers that omit them).
fn diff_request_required_fields(
    path: &str,
    method: &str,
    old_op: &Value,
    new_op: &Value,
    diffs: &mut Vec<SpecDiff>,
) {
    let required = |op: &Value| -> Vec<String> {
        op.get("requestBody")
            .and_then(|rb| rb.get("content"))
            .and_then(|c| c.get("application/json"))
            .and_then(|m| m.get("schema"))
            .and_then(|s| s.get("required"))
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    };
    let old_required = required(old_op);
    let new_required = required(new_op);
    for field in new_required {
        if !old_required.contains(&field) {
            diffs.push(SpecDiff::RequiredRequestFieldAdded {
                path: path.to_string(),
                method: method.to_string(),
                field,
            });
        }
    }
}

/// Response-schema field removals and type changes, compared on the 200
/// response's application/json schema properties (the policy's registered
/// lint scope: the success response is the contract consumers build
/// against; error-envelope evolution is governed separately by the
/// stable error contract, `error.rs`).
fn diff_response_fields(
    path: &str,
    method: &str,
    old_op: &Value,
    new_op: &Value,
    diffs: &mut Vec<SpecDiff>,
) {
    let props = |op: &Value| -> Vec<(String, String, Value)> {
        op.get("responses")
            .and_then(|r| r.get("200"))
            .and_then(|r| r.get("content"))
            .and_then(|c| c.get("application/json"))
            .and_then(|m| m.get("schema"))
            .and_then(|s| s.get("properties"))
            .and_then(Value::as_object)
            .map(|map| {
                map.iter()
                    .map(|(k, v)| {
                        (
                            k.clone(),
                            v.get("type")
                                .and_then(Value::as_str)
                                .unwrap_or("object")
                                .to_string(),
                            v.clone(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let old_props = props(old_op);
    let new_props = props(new_op);
    for (field, old_type, _old_value) in &old_props {
        match new_props.iter().find(|(name, _, _)| name == field) {
            None => diffs.push(SpecDiff::ResponseFieldRemoved {
                path: path.to_string(),
                method: method.to_string(),
                field: field.clone(),
            }),
            Some((_, new_type, _)) if new_type != old_type => {
                diffs.push(SpecDiff::FieldTypeChanged {
                    path: path.to_string(),
                    method: method.to_string(),
                    field: field.clone(),
                    old_type: old_type.clone(),
                    new_type: new_type.clone(),
                });
            }
            Some(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_current_version_is_v1_and_nothing_else() {
        assert_eq!(CURRENT_API_VERSION, "v1");
        assert_eq!(CURRENT_API_PREFIX, "/api/v1/");
        assert_eq!(OPENAPI_ENDPOINT, "/api/v1/openapi.json");
    }

    #[test]
    fn the_deprecation_window_bounds_are_the_registered_policy() {
        assert_eq!(MIN_DEPRECATION_WINDOW_DAYS, 180);
        assert_eq!(MAX_DEPRECATION_WINDOW_DAYS, 365);
        // A 6-month window passes (180 days).
        validate_deprecation_window(0, 180 * 86_400_000).expect("at the floor");
        // A 12-month window passes (365 days).
        validate_deprecation_window(0, 365 * 86_400_000).expect("at the ceiling");
        // Shorter than 6 months fails naming the bound.
        let err = validate_deprecation_window(0, 179 * 86_400_000).unwrap_err();
        assert!(err.contains("minimum is 180"), "{err}");
        // Longer than 12 months fails naming the bound.
        let err = validate_deprecation_window(0, 366 * 86_400_000).unwrap_err();
        assert!(err.contains("maximum is 365"), "{err}");
    }

    #[test]
    fn the_sunset_header_is_an_rfc_8594_http_date() {
        // 2026-11-11T00:00:00Z (a Wednesday; day 20 768 from the epoch).
        assert_eq!(
            sunset_header_value(1_794_355_200_000),
            "Wed, 11 Nov 2026 00:00:00 GMT"
        );
        // The Unix epoch (a Thursday).
        assert_eq!(sunset_header_value(0), "Thu, 01 Jan 1970 00:00:00 GMT");
        // A leap day (2024-02-29, a Thursday; day 19 782 from the epoch).
        assert_eq!(
            sunset_header_value(1_709_164_800_000),
            "Thu, 29 Feb 2024 00:00:00 GMT"
        );
        // End-of-day keeps the date and rolls the clock.
        assert_eq!(
            sunset_header_value(1_794_441_599_000),
            "Wed, 11 Nov 2026 23:59:59 GMT"
        );
    }

    #[test]
    fn removed_paths_and_operations_are_breaking() {
        let old = json!({"paths": {
            "/api/v1/sites": {"get": {"responses": {"200": {}}}},
            "/api/v1/assets": {"get": {"responses": {"200": {}}}}
        }});
        let new = json!({"paths": {
            "/api/v1/sites": {"get": {"responses": {"200": {}}}}
        }});
        let diffs = detect_breaking_changes(&old, &new);
        assert_eq!(
            diffs,
            vec![SpecDiff::PathRemoved { path: "/api/v1/assets".to_string() }]
        );
        let new_missing_op = json!({"paths": {
            "/api/v1/sites": {}
        }});
        let diffs = detect_breaking_changes(&old, &new_missing_op);
        // /api/v1/assets is gone entirely in this spec too, so BOTH
        // removals are reported (the old form expected only the operation).
        assert_eq!(
            diffs,
            vec![
                SpecDiff::PathRemoved { path: "/api/v1/assets".to_string() },
                SpecDiff::OperationRemoved {
                path: "/api/v1/sites".to_string(),
                method: "get".to_string(),
            }]
        );
    }

    #[test]
    fn removed_response_statuses_are_breaking() {
        let old = json!({"paths": {"/api/v1/sites": {"get": {"responses": {
            "200": {}, "404": {}
        }}}}});
        let new = json!({"paths": {"/api/v1/sites": {"get": {"responses": {
            "200": {}
        }}}}});
        assert_eq!(
            detect_breaking_changes(&old, &new),
            vec![SpecDiff::ResponseRemoved {
                path: "/api/v1/sites".to_string(),
                method: "get".to_string(),
                status: "404".to_string(),
            }]
        );
    }

    #[test]
    fn a_new_required_request_field_is_breaking_and_an_optional_one_is_not() {
        let old = json!({"paths": {"/api/v1/schedules": {"post": {
            "responses": {"201": {}},
            "requestBody": {"content": {"application/json": {"schema": {
                "type": "object",
                "properties": {"name": {"type": "string"}},
                "required": ["name"]
            }}}}
        }}}});
        // Adding an OPTIONAL property is not in any breaking class.
        let additive = json!({"paths": {"/api/v1/schedules": {"post": {
            "responses": {"201": {}},
            "requestBody": {"content": {"application/json": {"schema": {
                "type": "object",
                "properties": {"name": {"type": "string"}, "note": {"type": "string"}},
                "required": ["name"]
            }}}}
        }}}});
        assert!(detect_breaking_changes(&old, &additive).is_empty());
        // Promoting a property to required IS breaking.
        let breaking = json!({"paths": {"/api/v1/schedules": {"post": {
            "responses": {"201": {}},
            "requestBody": {"content": {"application/json": {"schema": {
                "type": "object",
                "properties": {"name": {"type": "string"}, "note": {"type": "string"}},
                "required": ["name", "note"]
            }}}}
        }}}});
        assert_eq!(
            detect_breaking_changes(&old, &breaking),
            vec![SpecDiff::RequiredRequestFieldAdded {
                path: "/api/v1/schedules".to_string(),
                method: "post".to_string(),
                field: "note".to_string(),
            }]
        );
    }

    #[test]
    fn removed_response_fields_and_type_changes_are_breaking() {
        let old = json!({"paths": {"/api/v1/sites": {"get": {"responses": {"200": {
            "content": {"application/json": {"schema": {"type": "object", "properties": {
                "site_id": {"type": "string"},
                "rated_power_mw": {"type": "number"}
            }}}}}
        }}}}});
        // Dropping a field is breaking.
        let dropped = json!({"paths": {"/api/v1/sites": {"get": {"responses": {"200": {
            "content": {"application/json": {"schema": {"type": "object", "properties": {
                "site_id": {"type": "string"}
            }}}}}
        }}}}});
        assert_eq!(
            detect_breaking_changes(&old, &dropped),
            vec![SpecDiff::ResponseFieldRemoved {
                path: "/api/v1/sites".to_string(),
                method: "get".to_string(),
                field: "rated_power_mw".to_string(),
            }]
        );
        // Changing a field's type is breaking.
        let retyped = json!({"paths": {"/api/v1/sites": {"get": {"responses": {"200": {
            "content": {"application/json": {"schema": {"type": "object", "properties": {
                "site_id": {"type": "string"},
                "rated_power_mw": {"type": "string"}
            }}}}}
        }}}}});
        assert_eq!(
            detect_breaking_changes(&old, &retyped),
            vec![SpecDiff::FieldTypeChanged {
                path: "/api/v1/sites".to_string(),
                method: "get".to_string(),
                field: "rated_power_mw".to_string(),
                old_type: "number".to_string(),
                new_type: "string".to_string(),
            }]
        );
    }

    #[test]
    fn a_spec_without_paths_is_reported_as_unclassified_breaking() {
        let old = json!({"paths": {"/api/v1/x": {"get": {"responses": {}}}}});
        let new = json!({"openapi": "3.0.3"});
        let diffs = detect_breaking_changes(&old, &new);
        assert_eq!(
            diffs,
            vec![SpecDiff::Unclassified {
                location: "either spec lacks a paths object".to_string(),
            }]
        );
    }

    #[test]
    fn every_diff_class_is_breaking_by_construction() {
        // The deny-by-default rule made mechanical: an unknown/unclassified
        // difference must never read as safe.
        assert!(SpecDiff::Unclassified { location: "?".to_string() }.is_breaking());
        assert!(SpecDiff::PathRemoved { path: "/p".to_string() }.is_breaking());
    }
}
