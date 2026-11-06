# CHECKLIST — Phase 2: Asset and Telemetry Foundation (Stage 2 exit gate)

Exit gate (§23 Stage 2): *synthetic BESS/PV/load/EVSE portfolio telemetry is
ingested, normalized, queried, and replayed deterministically.*

Each item needs **evidence** (a named artefact, command output, or test run),
not assertion. Tick an item only when the evidence exists and is linked.

## Entry and registry

- [ ] Phase 1 exit gate passed (or passed with conditions); handover received; P1.26 workflow re-runs green; implemented-API-subset gap recorded — Evidence: Phase 2 entry note (P2.01)
- [ ] Registry domain model: entity graph with cardinalities; asset-type enum per §2.2 (backup generation excluded); edge identity reserved; market region a label; deletion rule decided; capability profile a reference only — Evidence: design doc (P2.02)
- [ ] Registry implementation: use-case tests pass; `AssetRegistered` / `AssetCapabilityUpdated` / `AssetAvailabilityChanged` via outbox; duplicate/missing-site/cross-tenant rejected; no infra imports in domain/application — Evidence: `cargo test` (P2.03)

## Telemetry model and storage

- [ ] Canonical telemetry model: all fields defined; per-asset quantities with canonical units; P > 0 = export; Q per declared convention; event vs ingest time; source identity + sequence mandatory — Evidence: design doc (P2.04)
- [ ] Telemetry storage design: partitioning method + interval + rationale; PK includes partition key; indexes cover current-state + history queries; pre-creation + out-of-range path specified; retention is configuration; TimescaleDB non-use recorded — Evidence: design doc (P2.05)
- [ ] Migrations + repositories: up/down clean; partition-routing test; `EXPLAIN` shows pruning; RLS test; duplicate rejected by unique constraint — Evidence: integration test run (P2.06)

## Ingestion and quality

- [ ] Ingestion pipeline design: all seven stages defined; no-silent-skip rule; quarantine design; backpressure limit + at-limit behaviour; events via outbox — Evidence: design doc (P2.07)
- [ ] Quality + stale: unit tests per rule; SUSPECT-vs-BAD boundary recorded + tested; STALE boundary test; MISSING markers are derived records; ESTIMATED only with an explicit rule; `TelemetryQualityChanged` emitted — Evidence: unit tests (P2.08)
- [ ] Duplicates/out-of-order: exact duplicate → one row; conflicting duplicate → quarantine; late older record stored without regressing state; redelivery → one row — Evidence: integration tests (P2.09)
- [ ] Ingestion service: happy-path, mixed-validity, duplicate, out-of-order, backpressure tests pass; quarantine metric increments; port-only dependency; source-auth status recorded honestly — Evidence: integration tests (P2.10)
- [ ] `TelemetrySource` port: subscribe/decode/source_id/health; tenant-scoped queries; decode failure specified; provider must declare cadence/quality mapping/clock behaviour; provider set is exactly one (simulator) — Evidence: port review (P2.11)

## API, simulator, events, views

- [ ] API extensions: Stage 2 routes exist, none outside the P0.15 list; time-range + page-size + six-state quality filter enforced; cross-tenant denied; OpenAPI regenerated — Evidence: API tests (P2.12)
- [ ] Scenario model: schema defined; determinism scope stated (`ingest_time` excluded); four asset profiles with assumptions; scenario versioned under `scenarios/synthetic-portfolio/` — Evidence: scenario artefact (P2.13)
- [ ] Simulator source: determinism test (identical records across runs); SOC in [0,1] + P within limits over 24 h; sequence monotonic; port-only access — Evidence: `cargo test` (P2.14)
- [ ] Fault injection: all seven fault types have end-to-end tests with expected outcomes; faults are scenario config; no wall-clock-dependent tests — Evidence: fault tests (P2.15)
- [ ] Event publication: granularity decision recorded; all three events tested with expected payloads; outbox-only publication; redelivery → no duplicate events — Evidence: outbox assertions (P2.16)
- [ ] Registry views: portfolio/site/asset views with specified fields; edge connectivity honest; `simulated: true` + quality on all simulator values; pagination + cross-tenant tests — Evidence: API tests (P2.17)
- [ ] Current-state view: latest-by-event_time test; STALE boundary; MISSING (not zero); quality + age + `simulated` in response; index use in `EXPLAIN` — Evidence: tests + `EXPLAIN` (P2.18)
- [ ] Historical view: range/quality/aggregation/pagination tests; aggregation matches hand-computed fixture; no pagination duplicates/gaps; `EXPLAIN` pruning; oversized range → 422 — Evidence: tests + `EXPLAIN` (P2.19)
- [ ] Retention/partition maintenance: pre-creation test; retention detaches only out-of-window partitions; current-partition drop refused; audit record per action — Evidence: integration tests (P2.20)
- [ ] Observability: `/metrics` exposes ingestion delay, staleness, queue depth, outbox, quarantine, quality counts; spans carry correlation + quality; no-secrets test passes — Evidence: `/metrics` scrape (P2.21)

## Tests

- [ ] Phase 2 security suite: tenant isolation per new route; cross-tenant registry/telemetry rejected; unauthenticated/wrong-tenant source rejected; not-yet-covered list with owning phases — Evidence: `cargo test` security suite (P2.22)
- [ ] Phase 2 fault-injection suite: all fault cases pass (ingestion, storage, clock, volume); no ambiguous partial batch; clock-skew flagged; fault condition asserted first per test — Evidence: fault-test log (P2.23)

## Replay, exit gate, CI

- [ ] Deterministic replay: exit-gate scenario replays with zero mismatches; changed-seed replay fails with a diff; `ingest_time` excluded from comparison; report has version/seed/code/counts/verdict — Evidence: replay report (P2.24)
- [ ] Versioned corrections / as-of replay: correction creates a new version, original intact; as-of query returns pre-correction value at T1 and corrected value at T2; current-state view reads latest version only; as-of replay reproduces a past decision's inputs — Evidence: integration + as-of tests (P2.31)
- [ ] Exit-gate portfolio: ≥2 sites, each with BESS + PV + load + EVSE + site meter, registered via API; full 24-hour horizon ingested; records canonical; faults accounted for — Evidence: coverage counts (P2.25)
- [ ] Stage 2 exit-gate demonstration: ingest + normalize + query (current + historical) + deterministic replay on a clean stack; fault variant passes; run record lists exact versions + observed (not target) performance numbers — Evidence: demonstration record (P2.26)
- [ ] CI extensions: migration tests, scenario regression (replay verdict in log), telemetry contract suite all run and pass; Phase 1 gates still green — Evidence: CI run history (P2.27)

## Handover and gate

- [ ] Handover pack for Phase 3: every P2.01–P2.27 deliverable listed with location + status; capability-profile-reference status stated; open questions with owning gates — Evidence: handover index (P2.28)
- [ ] Exit-gate review held; every checklist item evidenced or excepted; demonstration + replay shown live or from a fresh recorded run; outcome recorded — Evidence: review record (P2.29)
- [ ] Anti-scope audit filed: no flexibility/scheduling/feasibility/dispatch-path/console/edge/plugin/Python-worker/market artefacts; no field telemetry; foundations and report untouched — Evidence: `git status` + file listing (P2.30)

## Gate outcome

- Outcome: ☐ Pass ☐ Pass with conditions ☐ Fail
- Conditions / blocking items: ________________________________________________
- Reviewers (names + groups): ________________________________________________
- Date: ______________________
