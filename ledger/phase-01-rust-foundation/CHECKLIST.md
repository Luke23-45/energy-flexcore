# CHECKLIST — Phase 1: Modular Rust Foundation (Stage 1 exit gate)

Exit gate (§23 Stage 1): *one end-to-end non-control workflow works through
API, application use case, database, and event publication.*

Each item needs **evidence** (a named artefact, command output, or test run),
not assertion. Tick an item only when the evidence exists and is linked.

## Entry and toolchain

- [ ] Phase 0 exit gate passed (or passed with conditions, conditions listed); handover pack received and inventoried; D1–D3 status recorded — Evidence: Phase 1 entry note (P1.01)
- [ ] Toolchain pinned: `rust-toolchain.toml` exact version; `rustc --version` on a clean checkout prints it; edition 2024 set — Evidence: `rustc --version` output (P1.02)
- [ ] Workspace created: root `Cargo.toml` + `Cargo.lock`; `cargo metadata --no-deps` lists exactly the Phase 1 member crates; license field does not assert Apache-2.0 — Evidence: `cargo metadata` output (P1.03)
- [ ] Dependency rule enforced: policy document + CI script; negative test (forbidden edge fails) recorded; script passes on the real tree — Evidence: CI run (P1.04)

## Crates

- [ ] `flexcore-contracts`: unit tests pass; no infra deps in `cargo tree`; SOC rejects outside [0,1]; quality enum has exactly six variants; MW/MWh do not unify — Evidence: `cargo test -p flexcore-contracts` (P1.05)
- [ ] `flexcore-domain`: unit + property tests pass; every §14.1 transition tested; ≥5 non-edges rejected; `DispatchCommand` construction enforces §14.3 invariants; no infra deps — Evidence: `cargo test -p flexcore-domain` (P1.06)
- [ ] `flexcore-ports`: traits only; no infra import; no mention of PostgreSQL/NATS/Axum/vendor types; application tests run against fakes — Evidence: `cargo tree` + grep (P1.07)
- [ ] `flexcore-application`: tests pass against fake ports with no DB/broker; outbox written atomically with state; cross-tenant rejected; no SQLx/NATS/Axum import — Evidence: `cargo test -p flexcore-application` (P1.08)
- [ ] `flexcore-events`: all 26 events as types (count test); round-trip serialization per event; idempotency key mandatory; no NATS/PostgreSQL import — Evidence: `cargo test -p flexcore-events` (P1.09)
- [ ] `flexcore-storage` + `flexcore-audit`: tests pass; audit port has no update/delete; no duplicated repository trait — Evidence: `cargo test`, API inspection (P1.10)
- [ ] `flexcore-authz`: deny-by-default tests pass (unknown action/scope/missing context); decisions carry reasons; tenant context only from claims constructor — Evidence: `cargo test -p flexcore-authz` (P1.11)

## Persistence and messaging

- [ ] Schema design: `tenant_id` on every tenant-owned table; RLS policy per table; quality enum exactly six values; outbox design (unique idempotency key + delivery state); migration discipline written; no telemetry table — Evidence: schema design doc (P1.12)
- [ ] PostgreSQL adapter: integration tests pass on real PostgreSQL 18; migrations up/down clean; RLS test (tenant A cannot read tenant B via runtime role); outbox atomicity test; SOC check-constraint test — Evidence: integration test run (P1.13)
- [ ] JetStream topology: all 26 events mapped to subjects; retention/replication/ack/replay/DLQ per stream; at-least-once + no-global-ordering stated — Evidence: topology design doc (P1.14)
- [ ] NATS adapter + outbox publisher: integration tests pass on real NATS 2.15.x; broker-restart test (zero lost, zero duplicate side effects); delivered-only-after-ack test; `Nats-Msg-Id` = idempotency key — Evidence: integration test run (P1.15)
- [ ] Idempotent consumers + inbox: duplicate-delivery → one side effect; crash-mid-process → one side effect; poison message → DLQ after `max_deliver`; ack-after-commit ordering — Evidence: integration test run (P1.16)

## API, worker, observability

- [ ] Axum control plane: API tests pass on the Compose stack; OpenAPI generated from code; 401 unauthenticated; cross-tenant denied; no internal error text in responses; no route outside the P0.15 list; no arbitrary-command endpoint — Evidence: `cargo test -p flexcore-control-plane` (P1.17)
- [ ] Buf governance: `buf.yaml`/`buf.gen.yaml` present; `buf lint` passes; `buf breaking` in CI with negative test; protos-now-or-later decision recorded — Evidence: Buf CI run (P1.18)
- [ ] Worker: starts, publishes, consumes on the Compose stack; graceful shutdown test; `/health` vs `/ready` differ under stopped broker — Evidence: Compose test run (P1.19)
- [ ] Tracing/metrics: JSON log line carries trace ID + tenant ID + actor; `/metrics` exposes request/outbox/consumer-lag metrics; no-secrets test passes; OTel SDK versions pinned — Evidence: log inspection + `/metrics` scrape (P1.20)
- [ ] Config/secrets: missing-config fails fast with a named error; no default credentials; secret scan passes + negative test recorded; dev credentials marked dev-only — Evidence: test + scan output (P1.21)
- [ ] Probes/shutdown (control plane): `/ready` fails when Postgres/NATS down; `/health` stays up; SIGTERM drains in-flight requests — Evidence: probe tests (P1.22)

## Tests

- [ ] Security suite: tenant-isolation per route/method; auth-bypass (4 token cases); replay → one side effect; not-yet-covered list with owning phases — Evidence: `cargo test` security suite (P1.23)
- [ ] Fault-injection slice: all six fault cases pass (dropped, reordered, duplicated, broker outage, DB outage, clock skew); broker-outage → zero lost; DB-outage → zero partial writes — Evidence: fault-test log (P1.24)

## Deployment and CI

- [ ] Compose stack: `docker compose up` on a clean checkout → all services healthy; migrations automatic (or recorded explicit step); smoke script passes; image tags pinned (no `latest`) — Evidence: clean-checkout run + smoke output (P1.25)
- [ ] Stage 1 exit-gate demonstration: workflow completes end-to-end on a clean stack with zero manual SQL/NATS steps; entities in Postgres, events in JetStream/inbox; cross-tenant denied; NATS-outage variant passes; run record lists exact versions — Evidence: demonstration record (P1.26)
- [ ] CI: all applicable §22.2 gates run and pass; not-yet-applicable list with owning phases; deliberately failing change fails CI; SBOM generated per build — Evidence: CI run history (P1.27)
- [ ] License inventory: every direct + transitive dependency listed with license; non-permissive/unclear flagged; NATS license note present; delivered to D4 owners with D4 still OPEN — Evidence: SBOM + inventory (P1.28)

## Handover and gate

- [ ] Crate-boundary audit: dependency check passes per crate; zero empty crates; no duplicated trait/type without justification; violations fixed or recorded — Evidence: audit report (P1.30)
- [ ] Handover pack for Phase 2: every P1.01–P1.28 deliverable listed with location + status; implemented-API-subset gap stated; open questions with owning gates — Evidence: handover index (P1.29)
- [ ] Exit-gate review held; every checklist item evidenced or excepted; end-to-end workflow demonstrated live or from a fresh recorded run; outcome recorded — Evidence: review record (P1.31)
- [ ] Anti-scope audit filed: no telemetry/registry-UI/scheduling/feasibility/dispatch-path/edge/plugin/console/Python-worker artefacts; foundations and report untouched — Evidence: `git status` + file listing (P1.32)

## Gate outcome

- Outcome: ☐ Pass ☐ Pass with conditions ☐ Fail
- Conditions / blocking items: ________________________________________________
- Reviewers (names + groups): ________________________________________________
- Date: ______________________
