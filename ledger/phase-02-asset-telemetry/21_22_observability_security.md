# P2.21 / P2.22 — Telemetry observability and the Phase 2 security test slice

| Field | Value |
|---|---|
| Subtasks | P2.21 (telemetry observability), P2.22 (Phase 2 security test slice) |
| Report trace | §21.3, §17.4 (stale-data gates), §19.2 (test layer 7), §16.2; P1.20, P1.23 |
| Source | Ledger P2.21/P2.22; the Phase 1 no-secrets rule (`ledger/phase-01-rust-foundation/23_24_test_coverage_note.md`) |
| Status | Code + tests delivered in this slice; executable runs gated per file (see "How to run"). No cargo command was executed while writing this slice (coordinator instruction) — the ledger checkboxes stay unticked until the build/test run confirms them. |

## How to run

- The pure test files run with the normal `cargo test` flow:
  `apps/control-plane/tests/security_phase2.rs` (Test backend, no
  infrastructure) and the non-DB tests of
  `apps/worker/tests/security_phase2.rs`.
- The database-gated tests (the cross-tenant ingestion boundary, the P2.21
  pipeline-metric cases in `apps/worker/tests/ingestion.rs`, and the
  DB-gated case in `apps/worker/tests/security_phase2.rs`) run only with
  `FLEXCORE_TEST_DATABASE_URL` set; otherwise every test skips cleanly with
  a printed reason (the recorded P2.06 harness convention). NATS is not
  needed: the pipeline publishes through the OUTBOX port only.
- The `/metrics` scrape acceptance is asserted as a test
  (`the_metrics_scrape_exposes_all_six_telemetry_metric_families`); the
  live scrape in the Compose stack remains the operator's verification
  method per the ledger.

## P2.21 — metric inventory and the recorded cardinality decisions

All families render on the control plane's `/metrics` (the P1.20 registry,
extended — not replaced; `apps/control-plane/src/metrics.rs`), always, with
zero values included, so a scrape exposes the full inventory on an idle
system. The worker pipeline records the same-named families on its
[`IngestionMetrics`] handle (`apps/worker/src/ingestion.rs`); one naming
space, no second vocabulary.

| # | Metric family | Type | Labels | Cardinality decision (DECIDED ONCE) |
|---|---|---|---|---|
| 1 | `flexcore_telemetry_ingestion_delay_milliseconds` | summary (`_sum`/`_count`) | none | Per-BATCH summary over STORED records of (`ingest_time − event_time`, ms; negative deltas clamp to 0 — clock skew is a separate signal, P2.08/P2.14). A per-asset time series per metric family would be unbounded cardinality; the per-asset breakdown travels in the per-record log lines (see Tracing), never as labels. |
| 2 | `flexcore_telemetry_stale_assets` / `flexcore_telemetry_asset_stale_total` | gauge / counter | `asset_id` (family 2b only) | A per-asset series exists ONLY while the asset's latest evaluation is STALE — the series is removed when the asset reports fresh again (bounded by the currently-stale set). The count reuses the P2.08 staleness rule (`flexcore_domain::quality_rules::staleness`) evaluated in the ingestion path; it never re-implements the age comparison and never rewrites the stored quality state (staleness is view-time, P2.08). |
| 3 | `flexcore_telemetry_queue_depth` | gauge | none | The ingestion bounded channel's current length (`FLEXCORE_INGEST_CHANNEL_CAPACITY`, default 10 000), sampled by the pipeline loop (and at drain). No labels. |
| 4 | `flexcore_telemetry_events_total` | counter | `event` (closed: `TelemetryReceived`, `TelemetryQualityChanged`, `AssetAvailabilityChanged`) | The frozen §15.3 event types are the closed label set. Recorded finding: P1.20 shipped request/error families only — no outbox-pending metric existed in code — so this slice introduces ONE family as the single home for the telemetry outbox-event counts (extended, not duplicated; a generic outbox-pending gauge stays with the publisher's future observability work). |
| 5 | `flexcore_telemetry_quarantine_total` / `flexcore_telemetry_quarantine_by_reason_total` | counter / counter | none / `reason_class` (closed, five classes) | Total + by reason class. The free-text quarantine reason is NEVER a label (attacker/adapter-controlled text would be unbounded); the classification maps the rejection stage and rule onto the closed classes: `source_rejected`, `decode_failed`, `conflicting_duplicate`, `out_of_range_event_time`, `store_rejected`. |
| 6 | `flexcore_telemetry_quality_states_total` | counter | `quality` (closed: the six P0.08 states) | Per-STATE totals are the default (recorded decision); per-asset quality counts go to logs. All six states — GOOD, SUSPECT, BAD, MISSING, ESTIMATED, STALE — render with zero included, so `STALE` can never silently go missing (the ledger's named risk). Counted over STORED records (duplicates are not new values). |

Worker-side rendering: `IngestionMetrics::render_prometheus` emits the same
family names plus the worker-only `flexcore_telemetry_stale_observations_total`
(a bounded total; the per-asset staleness series lives in the control-plane
registry). Recorded limitation: the worker's probe server (P1.19) carries
`/health` and `/ready` only — it is not a second API server, and adding a
worker `/metrics` surface is a composition change owned by the worker's
hosting owner, not this slice. The ledger's `/metrics` acceptance is met by
the control-plane registry, where the scrape lives.

## P2.21 — tracing (P1.20 correlation rule)

- Every frame opens an ingestion span carrying `tenant_id` and `source_id`
  (`telemetry_ingest_frame`).
- Stage 7 emits one per-record log line per stored record carrying ALL six
  correlation fields: `tenant_id`, `site_id`, `asset_id`, `source_id`,
  `event_time`, `quality` (plus the quantity). This is the per-asset
  breakdown the metric families deliberately do not carry.
- The staleness signal emits a dedicated warn line with the same fields plus
  the configured `freshness_limit_ms`.

## P2.21 — the staleness signal (step 3)

A metric + log only. The freshness limit is DEPLOYMENT CONFIGURATION
(`FLEXCORE_STALE_FRESHNESS_LIMIT_MS`, worker `main.rs`; unset = no
staleness evaluation in the pipeline) — never a hard-coded constant (P0.16
rule; P2.08: per-use-case limits). Alerting thresholds are a deployment
concern and are not part of this repository's code.

## P2.21 — the no-secrets re-test

`apps/worker/tests/security_phase2.rs`
(`telemetry_log_lines_never_contain_credentials`) renders the ingestion
pipeline's representative log records (the exact field sets of the stage-1
rejection, quarantine, batch-committed, per-record correlation, and
staleness call sites) through a capturing tracing subscriber, with
adversarial credential values in the source auth context and the raw
rejected payload. Assertions: the correlation fields (source IDs, tenant/
asset IDs) ARE rendered — source IDs are not secrets — and no password,
bearer token, API key, connection string, or raw payload content appears in
any rendered line.

## P2.22 — coverage mapping: test file → acceptance criterion

| Test file / case | P2.22 acceptance criterion | Notes |
|---|---|---|
| `apps/control-plane/tests/security_phase2.rs` `cross_tenant_registry_crud_is_404_with_no_data_leak` | Tenant isolation per new route: tenant delete/portfolio/site/asset delete, asset patch, deactivate — all 404 | Cross-tenant = 404 rule; the body carries no referenced id, no data field, and none of the rejected write's content |
| same file `cross_tenant_telemetry_state_and_views_are_404_with_no_data_leak` | Telemetry query, current state (`GET /api/v1/assets/{id}/state`), historical/asset view, site/portfolio views — 404; quarantine list tenant-scoped | B's quarantine list is 200-empty, never A's payload; error bodies assert-free of A's telemetry values |
| same file `an_asset_cannot_be_registered_under_another_tenants_site` | Registry security: foreign-tenant parent reference rejected | 404, no asset created under A's site |
| same file `a_portfolio_cannot_be_moved_across_tenants` | Registry security: no cross-tenant move | (a) no re-parent/rename route exists (405); (b) the only update route cannot change the site/tenant binding; (c) the delete-and-recreate path is closed (B's delete is 404) |
| same file `telemetry_reads_return_only_the_callers_own_tenant_data` | Telemetry query cannot cross tenants | Same query: A sees its record, B is 404 |
| same file `deny_by_default_holds_across_the_phase2_surface` | Deny-by-default; no role bypass | 401 unauthenticated on every Phase 2 route; TenantAdmin denied the PlatformAdmin-only `tenant.delete`; Viewer denied writes and `quarantine.read` even on its own tenant; least-privilege own-tenant telemetry read still granted |
| `apps/worker/tests/security_phase2.rs` `source_authentication_matches_the_recorded_phase2_stub_status` | Source auth per the RECORDED stub status (P2.10 step 6) | The test pins the recorded stub, not a stronger auth; real cryptographic source authentication is a later phase |
| same file `a_foreign_or_unavailable_source_identity_is_rejected` | Unauthenticated source rejected; another tenant's identity rejected | Trusted-source-id configuration is the stub's tenant-scoping knob |
| same file `ingestion_for_another_tenants_asset_is_rejected_by_the_tenant_scoped_store` (DB-gated) | Telemetry cannot be ingested for another tenant's asset | The real store boundary: `TenantMismatch` rejects the whole batch, nothing stored in either tenant, nothing quarantined; the own-tenant batch stores cleanly after |
| `apps/worker/tests/ingestion.rs` (P2.21 cases appended) | Metric inventory + staleness + quarantine-class behaviour on the real pipeline | Delay/quality over stored records; staleness fires only with a configured limit and never re-labels the stored state; quarantine classes stay separated |

## Not-yet-covered list (P2.22 step 5), with owning phases

| Item | Why not in Phase 2 | Owning phase |
|---|---|---|
| Protocol-adapter authentication (OpenEMS and later adapters: mTLS, per-adapter credentials) | Phase 2's only provider is the simulator; real adapters arrive with the first site integration | Phase 4 (first site integration, report release ladder) |
| Edge enrollment / edge identity verification | The site view honestly reports `simulator`/`not_connected`; no edge agent exists yet | Phase 4 (edge identity is created with the first site integration) |
| Cryptographic source authentication (signing, key rotation, replay protection at the source boundary) | The RECORDED P2.10 stub is provider declaration + optional trusted-source-id only | Later phase per the recorded stub status (post-P2.10; named when the adapter line starts) |
| OIDC/OAuth2 production identity integration (real token verification at the API middleware) | P1.17 recorded gap: the dev resolver is the only shipped integration | Later phase (owns `VerifiedClaims` attestation) |
| Command forgery protections (signed commands, sequence numbers, expirations) | No commands exist in Phase 2; the command lifecycle lands in Phase 3 | Phase 3/4 (lifecycle then dispatch kernel) |
| Plugin privilege escalation / sandbox escape | No plugin host exists (WASM host is the P0.5 SDK line) | Phase 7 |
| Rate limiting / anti-abuse on the telemetry query surface | The P2.12 range/page limits bound a single query; multi-request abuse controls are deployment configuration | Deployment configuration / later phases |
| Multi-tenant RLS under every future service role | Phase 1/2 define the recorded service roles; new roles extend the P1.13 test matrix | Per new role, from Phase 3 on |
| Secrets scanning in CI (continuous leak detection) | Covered per-write by the no-secrets tests here and P1.20/P1.21; continuous scanning is a CI concern | P2.27 (CI extension) and later |

## Scope and control status

These metrics and tests cover the design baseline only. FlexCore is a
supervisory orchestration layer; the entire 0.x line is simulation-only,
and nothing here constitutes a certification claim, a safety case, or
permission for live grid control. The security tests assert authorization
behaviour only.
