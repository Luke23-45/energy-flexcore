// P0.15 — API resource surface: candidate list, no implementation (Phase 0 freeze)
// P1.05 — flexcore-contracts implementation (moved from the Phase 0 seed in
//         flexcore-domain; semantics unchanged).
//
// Implements `ledger/phase-00-product-freeze/15_api_surface.md`.
// Normative source: report §15.1–§15.2 (API styles, candidate REST resources
// and the prohibition), §10.5 (schema governance), §21.4 (SemVer for public
// contracts).
//
// Frozen rules encoded here:
//
// - **Versioned REST base path** `/api/v1`; all frozen resource paths carry
//   that prefix (report §15.1).
// - **16 candidate REST resources** under that base path, transcribed
//   verbatim and in report §15.2 order (see [`REST_RESOURCE_PATHS`]). The
//   list is a candidate list — nothing here is an implementation commitment,
//   and no per-resource method sets, payload schemas, pagination, or error
//   conventions are frozen.
// - **Prohibition: no generic "execute arbitrary command" endpoint**
//   (report §15.2). Every route that mutates dispatch state must be one of
//   the frozen resources (or a documented revision of the list), never a
//   pass-through "execute" path. An arbitrary-execute endpoint would bypass
//   the distinct validation, feasibility, and authorization gates of the
//   frozen command lifecycle (P0.11) and is therefore prohibited, not merely
//   discouraged.
// - **No raw database schemas as public API contracts** (report §10.5). API
//   payloads are contract types (Protobuf cross-process, OpenAPI-described
//   JSON externally), not database rows or table/DDL dumps; internal schema
//   evolution must not leak into this surface.
// - **Public contracts follow SemVer** (report §21.4): REST API, Protobuf
//   schemas, plugin WIT interfaces, and plugin manifests. The 0.x line may
//   break contracts with a documented migration note; v1.0.0 freezes the
//   public API and plugin contracts for the 1.x line.

/// The versioned REST base path (report §15.1: "for example `/api/v1`").
pub const API_BASE_PATH: &str = "/api/v1";

/// The 16 candidate REST resource paths under `/api/v1`, verbatim and in
/// report §15.2 order.
///
/// This is a frozen candidate list. Adding a resource is a documented
/// revision of the list, and no route outside it (or its revisions) may
/// mutate dispatch state — in particular there is no generic
/// "execute arbitrary command" endpoint.
pub const REST_RESOURCE_PATHS: [&str; 16] = [
    "/api/v1/tenants",
    "/api/v1/portfolios",
    "/api/v1/sites",
    "/api/v1/assets",
    "/api/v1/grid-models",
    "/api/v1/telemetry/query",
    "/api/v1/flexibility/envelopes",
    "/api/v1/forecasts",
    "/api/v1/schedules",
    "/api/v1/optimization-runs",
    "/api/v1/feasibility-checks",
    "/api/v1/dispatch-commands",
    "/api/v1/market-commitments",
    "/api/v1/plugins",
    "/api/v1/audit-events",
    "/api/v1/simulation-runs",
];

/// The number of frozen REST resource paths.
pub const REST_RESOURCE_COUNT: usize = 16;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exactly_16_paths() {
        assert_eq!(REST_RESOURCE_PATHS.len(), 16);
        assert_eq!(REST_RESOURCE_COUNT, 16);
    }

    #[test]
    fn every_path_starts_with_api_v1() {
        for path in REST_RESOURCE_PATHS.iter() {
            assert!(
                path.starts_with("/api/v1/"),
                "resource path {path} must carry the versioned /api/v1 base path"
            );
        }
    }

    #[test]
    fn no_two_paths_are_equal() {
        for (i, a) in REST_RESOURCE_PATHS.iter().enumerate() {
            for b in REST_RESOURCE_PATHS.iter().skip(i + 1) {
                assert_ne!(a, b, "resource paths must be unique");
            }
        }
    }

    #[test]
    fn paths_match_frozen_list_verbatim() {
        assert_eq!(
            REST_RESOURCE_PATHS,
            [
                "/api/v1/tenants",
                "/api/v1/portfolios",
                "/api/v1/sites",
                "/api/v1/assets",
                "/api/v1/grid-models",
                "/api/v1/telemetry/query",
                "/api/v1/flexibility/envelopes",
                "/api/v1/forecasts",
                "/api/v1/schedules",
                "/api/v1/optimization-runs",
                "/api/v1/feasibility-checks",
                "/api/v1/dispatch-commands",
                "/api/v1/market-commitments",
                "/api/v1/plugins",
                "/api/v1/audit-events",
                "/api/v1/simulation-runs",
            ]
        );
    }

    #[test]
    fn no_arbitrary_execute_endpoint_is_present() {
        // The §15.2 prohibition: no generic "execute arbitrary command"
        // endpoint. No frozen path may be (or contain) an execute pass-through.
        for path in REST_RESOURCE_PATHS.iter() {
            assert!(
                !path.contains("execute") && !path.ends_with("/run-command"),
                "no arbitrary-execute endpoint may appear in the frozen surface"
            );
        }
    }
}
