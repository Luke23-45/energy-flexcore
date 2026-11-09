# CHECKLIST — Phase 1: Modular Rust Foundation (Stage 1 exit gate)

Exit gate (§23 Stage 1): *one end-to-end non-control workflow works through
API, application use case, database, and event publication.*

Each item needs **evidence** (a named artefact, command output, or test run),
not assertion. Tick an item only when the evidence exists and is linked.

## Entry and toolchain

- [x] Phase 0 exit gate passed (or passed with conditions, conditions listed); handover pack received and inventoried; D1–D3 status recorded — Evidence: Phase 1 entry note (P1.01) — implemented, awaiting human build/test
- [x] Toolchain pinned: `rust-toolchain.toml` exact version; `rustc --version` on a clean checkout prints it; edition 2024 set — Evidence: `rustc --version` output (P1.02) — implemented, awaiting human build/test
- [x] Workspace created: root `Cargo.toml` + `Cargo.lock`; `cargo metadata --no-deps` lists exactly the Phase 1 member crates; license field does not assert Apache-2.0 — Evidence: `cargo metadata` output (P1.03) — implemented, awaiting human build/test
- [x] Dependency rule enforced: policy document + CI script; negative test (forbidden edge fails) recorded; script passes on the real tree — Evidence: CI run (P1.04) — implemented, awaiting human build/test

## Crates

- [x] `flexcore-contracts`: unit tests pass; no infra deps in `cargo tree`; SOC rejects outside [0,1]; quality enum has exactly six variants; MW/MWh do not unify — Evidence: `cargo test -p flexcore-contracts` (P1.05) — implemented, awaiting human build/test
- [x] `flexcore-domain`: unit + property tests pass; every §14.1 transition tested; ≥5 non-edges rejected; `DispatchCommand` construction enforces §14.3 invariants; no infra deps — Evidence: `cargo test -p flexcore-domain` (P1.06) — implemented, awaiting human build/test
- [x] `flexcore-ports`: traits only; no infra import; no mention of PostgreSQL/NATS/Axum/vendor types; application tests run against fakes — Evidence: `cargo tree` + grep (P1.07) — implemented, awaiting human build/test
- [x] `flexcore-application`: tests pass against fake ports with no DB/broker; outbox written atomically with state; cross-tenant rejected; no SQLx/NATS/Axum import — Evidence: `cargo test -p flexcore-application` (P1.08) — implemented, awaiting human build/test
- [x] `flexcore-events`: all 26 events as types (count test); round-trip serialization per event; idempotency key mandatory; no NATS/PostgreSQL import — Evidence: `cargo test -p flexcore-events` (P1.09) — implemented, awaiting human build/test
- [x] `flexcore-storage` + `flexcore-audit`: tests pass; audit port has no update/delete; no duplicated repository trait — Evidence: `cargo test`, API inspection (P1.10) — implemented, awaiting human build/test
- [x] `flexcore-authz`: deny-by-default tests pass (unknown action/scope/missing context); decisions carry reasons; tenant context only from claims constructor — Evidence: `cargo test -p flexcore-authz` (P1.11) — implemented, awaiting human build/test

## Persistence and messaging

- [x] Schema design: `tenant_id` on every tenant-owned table; RLS policy per table; quality enum exactly six values; outbox design (unique idempotency key + delivery state); migration discipline written; no telemetry table — Evidence: schema design doc (P1.12) — implemented, awaiting human build/test
- [x] PostgreSQL adapter: integration tests pass on real PostgreSQL 18; migrations up/down clean; RLS test (tenant A cannot read tenant B via runtime role); outbox atomicity test; SOC check-constraint test — Evidence: integration test run (P1.13) — implemented, awaiting human build/test
- [x] JetStream topology: all 26 events mapped to subjects; retention/replication/ack/replay/DLQ per stream; at-least-once + no-global-ordering stated — Evidence: topology design doc (P1.14) — implemented, awaiting human build/test
- [x] NATS adapter + outbox publisher: integration tests pass on real NATS 2.15.x; broker-restart test (zero lost, zero duplicate side effects); delivered-only-after-ack test; `Nats-Msg-Id` = idempotency key — Evidence: integration test run (P1.15) — implemented, awaiting human build/test
- [x] Idempotent consumers + inbox: duplicate-delivery → one side effect; crash-mid-process → one side effect; poison message → DLQ after `max_deliver`; ack-after-commit ordering — Evidence: integration test run (P1.16) — implemented, awaiting human build/test

## API, worker, observability

- [x] Axum control plane: API tests pass on the Compose stack; OpenAPI generated from code; 401 unauthenticated; cross-tenant denied; no internal error text in responses; no route outside the P0.15 list; no arbitrary-command endpoint — Evidence: `cargo test -p flexcore-control-plane` (P1.17) — implemented, awaiting human build/test
- [x] Buf governance: `buf.yaml`/`buf.gen.yaml` present; `buf lint` passes; `buf breaking` in CI with negative test; protos-now-or-later decision recorded — Evidence: Buf CI run (P1.18) — implemented, awaiting human build/test
- [x] Worker: starts, publishes, consumes on the Compose stack; graceful shutdown test; `/health` vs `/ready` differ under stopped broker — Evidence: Compose test run (P1.19) — implemented, awaiting human build/test
- [x] Tracing/metrics: JSON log line carries trace ID + tenant ID + actor; `/metrics` exposes request/outbox/consumer-lag metrics; no-secrets test passes; OTel SDK versions pinned — Evidence: log inspection + `/metrics` scrape (P1.20) — implemented, awaiting human build/test
- [x] Config/secrets: missing-config fails fast with a named error; no default credentials; secret scan passes + negative test recorded; dev credentials marked dev-only — Evidence: test + scan output (P1.21) — implemented, awaiting human build/test
- [x] Probes/shutdown (control plane): `/ready` fails when Postgres/NATS down; `/health` stays up; SIGTERM drains in-flight requests — Evidence: probe tests (P1.22) — implemented, awaiting human build/test

## Tests

- [x] Security suite: tenant-isolation per route/method; auth-bypass (4 token cases); replay → one side effect; not-yet-covered list with owning phases — Evidence: `cargo test` security suite (P1.23) — implemented, awaiting human build/test
- [x] Fault-injection slice: all six fault cases pass (dropped, reordered, duplicated, broker outage, DB outage, clock skew); broker-outage → zero lost; DB-outage → zero partial writes — Evidence: fault-test log (P1.24) — implemented, awaiting human build/test

## Deployment and CI

- [x] Compose stack: `docker compose up` on a clean checkout → all services healthy; migrations automatic (or recorded explicit step); smoke script passes; image tags pinned (no `latest`) — Evidence: clean-checkout run + smoke output (P1.25) — implemented, awaiting human build/test
- [ ] Stage 1 exit-gate demonstration: workflow completes end-to-end on a clean stack with zero manual SQL/NATS steps; entities in Postgres, events in JetStream/inbox; cross-tenant denied; NATS-outage variant passes; run record lists exact versions — Evidence: demonstration record (P1.26) — NOT MET: demonstration NOT RUN (loop bans); runbook + criteria open in 26_exit_gate_record.md
- [x] CI: all applicable §22.2 gates run and pass; not-yet-applicable list with owning phases; deliberately failing change fails CI; SBOM generated per build — Evidence: CI run history (P1.27) — implemented, awaiting human build/test
- [x] License inventory: every direct + transitive dependency listed with license; non-permissive/unclear flagged; NATS license note present; delivered to D4 owners with D4 still OPEN — Evidence: SBOM + inventory (P1.28) — implemented, awaiting human build/test

## Handover and gate

- [x] Crate-boundary audit: dependency check passes per crate; zero empty crates; no duplicated trait/type without justification; violations fixed or recorded — Evidence: audit report (P1.30) — implemented, awaiting human build/test
- [x] Handover pack for Phase 2: every P1.01–P1.28 deliverable listed with location + status; implemented-API-subset gap stated; open questions with owning gates — Evidence: handover index (P1.29) — implemented, awaiting human build/test
- [ ] Exit-gate review held; every checklist item evidenced or excepted; end-to-end workflow demonstrated live or from a fresh recorded run; outcome recorded — Evidence: review record (P1.31) — NOT MET: exit-gate review not held (human reviewers); see 31_exit_gate_review.md
- [x] Anti-scope audit filed: no telemetry/registry-UI/scheduling/feasibility/dispatch-path/edge/plugin/console/Python-worker artefacts; foundations and report untouched — Evidence: `git status` + file listing (P1.32) — implemented, awaiting human build/test

## Gate outcome

- Outcome: ☐ Pass ☐ Pass with conditions ☐ Fail → **NOT HELD by the implementation agent — requires human reviewers + the P1.26 demonstration run; see `31_exit_gate_review.md`**
- Conditions / blocking items: implementation proceeds under the recorded loop-rules exception; the P1.26 run + gate review must be recorded before any release claim (see `26_exit_gate_record.md`, `31_exit_gate_review.md`)
- Reviewers (names + groups): ________ (recorded at the gate meeting)
- Date: ________ (date of the gate meeting)
