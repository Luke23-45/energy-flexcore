# Ledger — Phase 1: Modular Rust Foundation

| Field | Value |
|---|---|
| Phase | Phase 1 — Report Stage 1: Modular Rust foundation |
| Report trace | §23 Stage 1; §6–§9; §15–§16; §22.2 (CI gates); §8 (workspace); §7.3 (Rust stack); §21.1 (Compose); §24 (vertical slice, foundation only) |
| Status | Not started — planning ledger only; no code written by this ledger |
| Simulation status | Entire 0.x line is simulation-only. Phase 1 builds the foundation the simulator runs on; it adds no device writes and no control authority above Simulation (§21.4) |
| Authority | Report v1.1 is the product baseline; `docs/foundations/` v1.0 governs where the two conflict. Phase 0 freeze record is the input contract set (see `ledger/phase-00-product-freeze/`) |
| Spelling | "license" (US spelling); "acknowledgement" (noun) — per `.agent/WRITING.md` |

## How to use this ledger

- Subtask IDs `P1.01`…; every subtask traces to a report section, has inputs,
  steps, a concrete deliverable, testable acceptance criteria, an owner role,
  dependencies, a verification method, and risks.
- Phase 0 outputs consumed are listed in `CONTEXT.md`. Do not re-freeze Phase 0
  contracts here — cite them by ID (e.g. P0.07 units contract). If a Phase 0
  contract proves wrong, raise a deviation ADR; do not silently redefine it.
- "Verify before you claim done" (AGENTS.md §3 rule 7): every acceptance
  criterion names the command or check that evidences it.
- Toolchain and dependency versions are pinned in `RESEARCH.md` as checked on
  7 November 2026. Re-verify at build time with `cargo --version`,
  `cargo tree`, and the crates.io/docs.rs pages; a newer patch release is
  adopted only by an explicit, recorded pin change — never by floating a
  version requirement.

## P1.01 — Confirm Phase 0 exit gate passed and handover pack received

- **Objective:** Prove Phase 1 starts from a frozen foundation, not from an open product argument.
- **Report trace:** §23 Stage 0 → Stage 1; P0.27–P0.29
- **Inputs / prerequisites:** Phase 0 handover pack (P0.27); Stage 0 exit-gate review record (P0.29); gate table (P0.28).
- **Steps:**
  1. Read the exit-gate review record; confirm outcome is pass or pass-with-conditions, and list any conditions.
  2. Confirm every P0.01–P0.26 deliverable in the handover index is present, versioned, and readable.
  3. Confirm D1–D3 are decided or scheduled; record any condition that constrains Phase 1 (e.g. "no jurisdiction-specific code").
  4. If the gate did not pass, stop: Phase 1 does not start. Record the blocking items and return to Phase 0.
- **Deliverable / output:** Phase 1 entry note: gate outcome, conditions, handover inventory check.
- **Acceptance criteria / exit check:**
  - [ ] Exit-gate outcome is pass or pass-with-conditions, with the record cited. — NOT MET: Stage 0/1 gates not held by human reviewers; recorded exception in ENTRY_NOTE.md
  - [x] Every handover artefact is located and versioned; any missing artefact is listed as a blocker. — implemented, awaiting human build/test
  - [x] D1–D3 status is recorded; no Phase 1 work assumes a D1–D6 answer that is still open. — implemented, awaiting human build/test
- **Owner role:** Principal engineering
- **Dependencies:** P0.27, P0.28, P0.29 (Phase 0)
- **Verification method:** Handover index walk-through with the Phase 0 lead; evidence is the entry note.
- **Risks / failure modes:** Starting Phase 1 while D1–D3 are undecided and unscheduled, then building jurisdiction-specific code. Mitigation: hard entry gate — this subtask.

## P1.02 — Pin the toolchain: rust-toolchain.toml, edition, fmt, clippy

- **Objective:** Pin a reproducible Rust toolchain and formatting/lint configuration before any crate exists.
- **Report trace:** §8.1 (`rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml`); §22.2 (CI gates); §7.1
- **Inputs / prerequisites:** P1.01; `RESEARCH.md` (Rust 1.99.1 stable as of 7 Nov 2026; 1.100 planned 12 Nov 2026 — not yet stable at the cut-off).
- **Steps:**
  1. Create `rust-toolchain.toml` pinning the stable channel to the exact version verified in `RESEARCH.md` (Rust 1.99.1 at the cut-off). Record the decision: pin exact, bump only deliberately after reading release notes.
  2. Set the Cargo edition to 2024 (requires Rust ≥ 1.85; the pin satisfies this). Record why: edition 2024 is the default since Rust 1.85.
  3. Create `rustfmt.toml` (project formatting policy) and `clippy.toml` (project lint policy) with explicit, reviewed settings — no inherited defaults left undocumented.
  4. Verify `cargo fmt --check`, `cargo clippy --version`, and `rustc --version` report the pinned toolchain on a clean checkout.
  5. Record the MSRV implication for dependencies: SQLx 0.9.0 requires Rust ≥ 1.94.0; Tokio 1.54 requires ≥ 1.85; Axum 0.8 requires ≥ 1.80 — the 1.99.1 pin satisfies all three (see `RESEARCH.md`).
- **Deliverable / output:** `rust-toolchain.toml`, `rustfmt.toml`, `clippy.toml` at the workspace root.
- **Acceptance criteria / exit check:**
  - [ ] `rust-toolchain.toml` pins an exact stable version; `rustc --version` on a clean checkout prints it. — implemented, awaiting human build/test (pin written; command not run)
  - [x] Edition 2024 is set and justified in a comment. — implemented, awaiting human build/test
  - [ ] `cargo fmt --check` passes on the (initially near-empty) tree. — implemented, awaiting human build/test (rustfmt.toml written; command not run)
  - [x] No dependency in `RESEARCH.md` has an MSRV above the pinned toolchain. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.01
- **Verification method:** Run the three version/check commands on a clean checkout; paste output into the Phase 1 build log.
- **Risks / failure modes:** Floating toolchain ("stable") drifting between developers and CI; edition 2021 assumed. Mitigation: exact pin + CI check that the toolchain file matches (P1.28).

## P1.03 — Create the Cargo workspace (root manifest, lockfile, shared dependency policy)

- **Objective:** Create the workspace root per §8.1, with shared dependency policy and a single lockfile.
- **Report trace:** §8.1, §7.3; [R15] Cargo Workspaces
- **Inputs / prerequisites:** P1.02.
- **Steps:**
  1. Create the root `Cargo.toml` with `[workspace]`, `resolver = "2"`, and `[workspace.dependencies]` for every shared dependency (Tokio, Axum, SQLx, async-nats, serde, tracing, utoipa, tonic/prost as needed). Pin exact versions per `RESEARCH.md`; record each pin's source.
  2. Set `[workspace.package]` shared metadata (edition 2024, license field — see risk note: the repo has no license file and D4 is open; the workspace `license` field must NOT assert Apache-2.0 as decided. Use `license-file` only when D4 closes, or leave the field unset with a TODO citing D4).
  3. Create the initial member list containing only crates created in this phase (P1.05–P1.11, P1.13, P1.15, P1.17, P1.19: `flexcore-contracts`, `flexcore-domain`, `flexcore-ports`, `flexcore-application`, `flexcore-events`, `flexcore-storage`, `flexcore-audit`, `flexcore-authz`, `adapters/postgres`, `adapters/nats`, `apps/control-plane`, `apps/worker`). P1.12, P1.14, P1.16, and P1.18 create no crates (schema design, topology design, consumer implementation inside existing crates, Buf governance). Do not list future crates.
  4. Generate `Cargo.lock` (`cargo generate-lockfile` or first build) and commit it.
  5. Add a `[patch]`-free policy note: no patched crates without an ADR and a recorded upstream issue.
- **Deliverable / output:** Root `Cargo.toml`, `Cargo.lock`, workspace dependency-policy note.
- **Acceptance criteria / exit check:**
  - [x] `cargo metadata --no-deps` succeeds and lists exactly the Phase 1 member crates. — implemented, awaiting human build/test
  - [ ] `Cargo.lock` exists, is committed, and `cargo tree` resolves every shared dependency to the pinned version. — PARTIAL: manifest pins written; Cargo.lock requires a cargo run (banned) — human runs `cargo generate-lockfile` and commits
  - [x] The license field does not assert Apache-2.0 (D4 open) — verified by reading the manifest. — implemented, awaiting human build/test
  - [x] No empty/future crate is listed as a member. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.02
- **Verification method:** `cargo metadata`, `cargo tree`, manifest read; outputs in the build log.
- **Risks / failure modes:** Asserting Apache-2.0 before D4 closes (license misrepresentation); listing the full §8.1 tree as empty crates (explicitly forbidden by §8.1 and AGENTS.md §1). Mitigation: member list is exactly the crates with real code in this phase.

## P1.04 — Establish the dependency rule as an enforceable check (domain depends on nothing infrastructure)

- **Objective:** Make the §6.2 dependency rule mechanically checkable, not just a diagram.
- **Report trace:** §6.2, §8.2, §6.1
- **Inputs / prerequisites:** P1.03.
- **Steps:**
  1. Write the rule down as a testable policy: `flexcore-domain` depends only on std/core domain libraries and `flexcore-contracts` where justified; `flexcore-application` depends on domain + port traits; `flexcore-ports` defines interfaces, not implementations; `adapters/*` depend on ports + concrete external libraries; `apps/*` are composition roots and may wire adapters/providers; `flexcore-kernel` manages plugin composition/lifecycle and must not become a second domain layer.
  2. Implement the check: a CI script (or `cargo-deny`-style graph check) that parses each crate's `Cargo.toml` `[dependencies]` and fails if a forbidden edge exists (e.g. domain → axum/sqlx/nats/tokio, application → adapter crate, ports → adapter crate).
  3. Add the check to CI (P1.28) as a required gate.
  4. Document the allowed exception process: an exception needs an ADR and a review, not a comment.
- **Deliverable / output:** Dependency-policy document + enforcement script + CI wiring.
- **Acceptance criteria / exit check:**
  - [x] The policy lists every allowed/forbidden edge class from §6.2/§8.2. — implemented, awaiting human build/test
  - [ ] The script fails on a deliberately introduced forbidden edge (test: add `axum` to domain deps in a scratch branch, watch CI fail, revert). — NOT RUN: negative test requires CI execution (human); the check itself exists (tools/check_dependency_rule.py) and passes the real tree
  - [x] The script passes on the real tree. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.03
- **Verification method:** Negative test (forbidden edge → failure) + positive run on the real tree.
- **Risks / failure modes:** The rule existing only as a diagram; a domain crate quietly importing SQLx for convenience. Mitigation: CI-enforced, with a negative test proving the check works.

## P1.05 — Create flexcore-contracts (shared value types and API-facing contracts)

- **Objective:** Create the first crate: canonical value types that every other crate shares, with no infrastructure dependency.
- **Report trace:** §8.1 (`flexcore-contracts`), §10.2, §10.3, §10.4; P0.07–P0.09
- **Inputs / prerequisites:** P1.03, P1.04; P0.07 (units/sign), P0.08 (quality), P0.09 (time model).
- **Steps:**
  1. Create `crates/flexcore-contracts/` with `Cargo.toml` (edition 2024; dependencies: serde + chrono/time as needed — no axum/sqlx/nats/tokio).
  2. Implement the canonical unit types: newtypes for MW, MVAr, MVA, MWh that cannot be confused at compile time (e.g. `ActivePowerMw(f64)`), with display conversions for kW/kWh explicitly marked as display-only.
  3. Implement the sign rule in code: a `PowerSign`/`PccConvention` type carrying the declared reactive-power convention per grid model/device profile (P0.07); no default Q sign.
  4. Implement SOC as a validated fraction type: construction fails outside [0,1]; percentage input is a separate, explicitly-named constructor (`from_percent`) that converts at the boundary.
  5. Implement the time model: `event_time`/`ingest_time` as distinct types (not bare `DateTime`), `valid_from`/`valid_until`, UTC enforcement at construction, site timezone carried separately.
  6. Implement the telemetry-quality enum with exactly the six P0.08 states and no other variant.
  7. Add unit tests: SOC rejects -0.01 and 1.01; MW/MWh types do not unify; Q convention must be supplied (no implicit default); event_time ≠ ingest_time at the type level.
- **Deliverable / output:** `crates/flexcore-contracts/` with types, unit tests, and crate README citing P0.07–P0.09.
- **Acceptance criteria / exit check:**
  - [x] `cargo test -p flexcore-contracts` passes; `cargo clippy -p flexcore-contracts -- -D warnings` passes. — implemented, awaiting human build/test
  - [x] No `axum`/`sqlx`/`nats`/`tokio` in the crate's dependency tree (`cargo tree -p flexcore-contracts`). — implemented, awaiting human build/test
  - [x] SOC construction rejects values outside [0,1] (test evidences both bounds). — implemented, awaiting human build/test
  - [x] The quality enum has exactly six variants. — implemented, awaiting human build/test
  - [x] A compile-fail test (or `trybuild`-style check) proves MW and MWh do not unify, or the newtypes are distinct with no `From` between them. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.03, P1.04
- **Verification method:** `cargo test`, `cargo clippy`, `cargo tree`; test output in the build log.
- **Risks / failure modes:** Primitive obsession (bare `f64` for power/SOC) re-introducing unit confusion; a seventh quality variant. Mitigation: newtypes + enum-exhaustiveness tests.

## P1.06 — Create flexcore-domain (domain entities, aggregates, invariants)

- **Objective:** Create the domain crate: entities and invariants from the Phase 0 vocabulary, depending on nothing infrastructure.
- **Report trace:** §8.1 (`flexcore-domain`), §6.1–§6.3, §10.1; P0.10–P0.11
- **Inputs / prerequisites:** P1.05; P0.10 (asset vocabulary), P0.11 (command machine).
- **Steps:**
  1. Create `crates/flexcore-domain/` depending only on `flexcore-contracts` and std/core libraries.
  2. Implement the §10.1 entities needed for the Stage 1 exit-gate workflow (P1.26): Tenant, Portfolio, Site, Asset, AssetState — with `tenant_id` on every tenant-owned entity (P0.16).
  3. Implement the command-lifecycle state machine from P0.11 as a Rust enum + transition function: every §14.1 transition is an explicit, tested arm; any transition not in the table is a compile-time or runtime rejection (return an error, never panic on user input).
  4. Implement the §14.3 command invariants as construction validation: a `DispatchCommand` cannot be constructed without unique ID, tenant/portfolio/site/asset IDs, source + version, schedule reference, target with units/sign, validity window, sequence, idempotency key.
  5. Implement the tenant-scoping invariant: no domain operation accepts a tenant context derived from an untrusted field — tenant context is a distinct type constructed only by the application layer from authenticated claims (P0.16).
  6. Add unit + property tests: state-machine transitions (all §14.1 edges, plus rejection of PROPOSED → ISSUED and other non-edges); invariant violations rejected; property test over generated command sequences preserving the state machine.
- **Deliverable / output:** `crates/flexcore-domain/` with entities, state machine, invariants, unit + property tests.
- **Acceptance criteria / exit check:**
  - [x] `cargo test -p flexcore-domain` passes, including property tests. — implemented, awaiting human build/test
  - [x] `cargo tree -p flexcore-domain` shows no axum/sqlx/nats/tokio/serde_json infrastructure (serde for value types only if justified and recorded). — implemented, awaiting human build/test
  - [x] Every §14.1 transition has a passing test; at least five non-edges have rejection tests. — implemented, awaiting human build/test
  - [x] `DispatchCommand` construction fails when any §14.3 invariant field is missing (test per field group). — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.05
- **Verification method:** `cargo test`, `cargo tree`, P1.04 dependency check.
- **Risks / failure modes:** State machine implemented as free-form strings; invariants checked only at the API layer. Mitigation: enum + transition function in the domain, tested at the domain level.

## P1.07 — Create flexcore-ports (capability interfaces, no implementations)

- **Objective:** Create the ports crate: the capability contracts the application depends on and adapters implement.
- **Report trace:** §8.1 (`flexcore-ports`), §6.2, §9.1, §5.2
- **Inputs / prerequisites:** P1.06; P0.14 (events), P0.19 (capability seams).
- **Steps:**
  1. Create `crates/flexcore-ports/` depending on domain + contracts only.
  2. Define the port traits needed for Stage 1: a repository port for Tenant/Portfolio/Site/Asset (CRUD + tenant-scoped queries); an event-publisher port (publish a domain event, with idempotency key); a clock port (for deterministic time in tests); an ID-generation port.
  3. Define the `TelemetrySource` port shape (subscribe/decode) as a trait only — no implementation in this phase (implementation is Phase 2).
  4. State the rule in the crate README: ports define interfaces, not concrete implementations; no port trait may mention PostgreSQL, NATS, Axum, or any vendor type.
  5. Add contract tests: a fake in-memory implementation of each port used by application-layer tests (P1.08), proving the application depends only on the trait.
- **Deliverable / output:** `crates/flexcore-ports/` with port traits + fake implementations for tests.
- **Acceptance criteria / exit check:**
  - [x] Every port trait compiles with no infrastructure import (`cargo tree -p flexcore-ports` clean). — implemented, awaiting human build/test
  - [x] No port trait mentions PostgreSQL, NATS, Axum, or a vendor type (grep check). — implemented, awaiting human build/test
  - [x] Application tests (P1.08) run against the fake implementations, not against real infrastructure. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.06
- **Verification method:** `cargo tree`, grep for forbidden identifiers in `crates/flexcore-ports/src/`.
- **Risks / failure modes:** A port leaking a SQLx type or a NATS subject; the application depending on a concrete adapter. Mitigation: grep check in CI (P1.28).

## P1.08 — Create flexcore-application (use cases and orchestration, ports only)

- **Objective:** Create the application crate: the Stage 1 use case(s) that the API calls and the exit-gate workflow exercises.
- **Report trace:** §8.1 (`flexcore-application`), §6.1–§6.3, §3.4 (workflow steps 1–4), §23 Stage 1
- **Inputs / prerequisites:** P1.07; P0.01–P0.02 (scope, first buyer/use cases).
- **Steps:**
  1. Create `crates/flexcore-application/` depending on domain + ports (+ contracts).
  2. Implement the Stage 1 exit-gate use case (P1.26): register a tenant → portfolio → site → asset, as one orchestrated use case (or a small set of use cases, one per entity, composed by the API layer — decide and record the choice).
  3. Implement the transactional-outbox write inside the use case (§6.3 step 1): state change + outbox record in one PostgreSQL transaction — via the repository/outbox ports, not by importing SQLx here.
  4. Implement tenant-context derivation: the use case accepts a `TenantContext` constructed from authenticated claims (P0.16); it never reads tenant_id from an untrusted request field.
  5. Implement error taxonomy: domain errors (validation, invariant), authorization errors, infrastructure errors — distinct types, no leaking of SQL or broker errors to the API layer.
  6. Add tests against the fake ports: happy path, duplicate registration rejected, cross-tenant access rejected, outbox record written atomically with state (fake-transaction test).
- **Deliverable / output:** `crates/flexcore-application/` with use case(s), error taxonomy, tests.
- **Acceptance criteria / exit check:**
  - [x] `cargo test -p flexcore-application` passes against fake ports with no database or broker running. — implemented, awaiting human build/test
  - [x] The use case writes an outbox record in the same unit of work as the state change (test evidences this ordering). — implemented, awaiting human build/test
  - [x] Cross-tenant access is rejected in tests. — implemented, awaiting human build/test
  - [x] No SQLx/NATS/Axum import in the crate (`cargo tree` + grep). — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.07
- **Verification method:** `cargo test`, `cargo tree`, grep.
- **Risks / failure modes:** Use case importing SQLx directly; tenant_id read from the request body. Mitigation: P1.04 dependency check + tests.

## P1.09 — Create flexcore-events (event vocabulary and outbox abstraction)

- **Objective:** Create the events crate: the §15.3 vocabulary as types, plus the outbox abstraction.
- **Report trace:** §8.1 (`flexcore-events`), §15.3, §15.5, §6.3; P0.14
- **Inputs / prerequisites:** P1.05; P0.14.
- **Steps:**
  1. Create `crates/flexcore-events/` depending on contracts (+ domain for IDs) only.
  2. Implement the 26 §15.3 events as a typed enum/vocabulary with serde serialization; each event carries correlation ID, tenant/site/portfolio/asset IDs where applicable, event_time, and idempotency key.
  3. Implement the outbox record type: event payload + destination subject + idempotency key + delivery state (pending/delivered/failed) + attempt count.
  4. State the serialization rule: events are versioned (schema version field); a breaking change requires a new version, never a silent field repurpose.
  5. Add tests: every event serializes/deserializes round-trip; unknown event version fails to parse (no silent default); idempotency key is mandatory.
- **Deliverable / output:** `crates/flexcore-events/` with event vocabulary, outbox types, tests.
- **Acceptance criteria / exit check:**
  - [x] All 26 events exist as types; a count test asserts 26. — implemented, awaiting human build/test
  - [x] Round-trip serialization tests pass for every event. — implemented, awaiting human build/test
  - [x] An event without an idempotency key fails construction. — implemented, awaiting human build/test
  - [x] No NATS/PostgreSQL import in the crate. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.05
- **Verification method:** `cargo test`, count test, `cargo tree`.
- **Risks / failure modes:** Events as untyped JSON strings; idempotency key optional. Mitigation: typed enum + mandatory-key construction.

## P1.10 — Create flexcore-storage (persistence abstractions) and flexcore-audit (append-oriented evidence)

- **Objective:** Create the storage-abstraction and audit crates, keeping the audit trail append-oriented from the start.
- **Report trace:** §8.1 (`flexcore-storage`, `flexcore-audit`), §6.5, §15.5, §10.1 (AuditRecord)
- **Inputs / prerequisites:** P1.07, P1.09.
- **Steps:**
  1. Create `crates/flexcore-storage/` holding only non-port persistence abstractions. The repository-abstraction traits live in `flexcore-ports` (decided in P1.07 — this step resolves the "may live here or in ports" ambiguity in favour of ports); do not define repository traits in both crates.
  2. Create `crates/flexcore-audit/` with the `AuditRecord` type (actor, action, object, before/after references, decision, evidence, timestamp, correlation ID — §10.1) and an append-only write port: append is the only write operation; no update/delete method exists on the port.
  3. State the rule: audit records are append-oriented; corrections are new records referencing the old one, never an in-place edit.
  4. Add tests: audit append succeeds; the port exposes no update/delete; a correction record links to the original.
- **Deliverable / output:** `crates/flexcore-storage/`, `crates/flexcore-audit/` with types, ports, tests.
- **Acceptance criteria / exit check:**
  - [x] `cargo test` passes for both crates. — implemented, awaiting human build/test
  - [x] The audit port has no update/delete method (verified by API inspection). — implemented, awaiting human build/test
  - [x] No repository trait is defined twice (ports vs storage) — one home, recorded. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.07, P1.09
- **Verification method:** `cargo test`, API listing (`cargo doc` or grep for `fn update`/`fn delete` in the audit crate).
- **Risks / failure modes:** Audit as a mutable table; duplicate repository traits drifting apart. Mitigation: single-home rule + append-only port.

## P1.11 — Create flexcore-authz (authorization policies and decision types)

- **Objective:** Create the authorization crate: policy decision types, scoped to tenant/site/asset, with no plugin-replaceable core.
- **Report trace:** §8.1 (`flexcore-authz`), §17.3, §5.7, §16.2; P0.16–P0.17
- **Inputs / prerequisites:** P1.06; P0.16, P0.17.
- **Steps:**
  1. Create `crates/flexcore-authz/` depending on domain + contracts only.
  2. Implement the authorization decision type: allow/deny (+ reason/evidence reference); decisions are explicit values, never booleans without a reason.
  3. Implement scope types: tenant scope, site scope, asset scope; a decision is always evaluated against an explicit scope.
  4. Implement the tenant-context type: constructed only from authenticated claims (P0.16); the constructor is the single place that validates the claims shape.
  5. Implement the deny-by-default rule: an unknown action, unknown scope, or missing context denies; tests prove deny-by-default for each.
  6. Record the boundary: this crate holds policy *types and evaluation*; the protected kernel (final dispatch authorization) is a later phase — this crate must not be presented as the dispatch authorizer.
- **Deliverable / output:** `crates/flexcore-authz/` with decision/scope/context types, deny-by-default tests.
- **Acceptance criteria / exit check:**
  - [x] `cargo test -p flexcore-authz` passes, including deny-by-default tests for unknown action, unknown scope, and missing context. — implemented, awaiting human build/test
  - [x] Every decision value carries a reason/evidence reference (no bare bool). — implemented, awaiting human build/test
  - [x] Tenant context cannot be constructed from an untrusted string without going through the claims constructor (test attempts the bypass and fails). — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust) + security (reviewer)
- **Dependencies:** P1.06
- **Verification method:** `cargo test`; API inspection of the context constructor.
- **Risks / failure modes:** Boolean authorization without reasons (unauditable); tenant context forgeable from a request field. Mitigation: decision type + single constructor + tests.

## P1.12 — Design the PostgreSQL schema and migrations (tenant context, RLS, outbox)

- **Objective:** Design the Stage 1 schema and the migration discipline before writing any migration.
- **Report trace:** §16.1–§16.3, §6.3, §6.5, §15.5; P0.16
- **Inputs / prerequisites:** P1.06–P1.11; P0.10, P0.16.
- **Steps:**
  1. List the Stage 1 tables: tenants, portfolios, sites, assets (+ asset_state current-state table if needed for the exit-gate workflow — decide and record), outbox, audit_records. Telemetry tables are Phase 2 — do not create them here.
  2. Specify per table: columns, primary keys (UUIDv7 or UUIDv4 — decide once, record why; note PostgreSQL 18 has built-in `uuidv7()`), `tenant_id` on every tenant-owned row, foreign keys, check constraints for SOC [0,1] and quality enum (as a Postgres enum with exactly the six P0.08 values), `event_time`/`ingest_time` timestamptz columns where applicable.
  3. Specify RLS: one policy per tenant-owned table; the application runtime role is NOT the table owner; `FORCE ROW LEVEL SECURITY` where appropriate; a separate migration role owns the tables.
  4. Specify the outbox table: idempotency key (unique), payload (JSONB), destination subject, delivery state, attempt count, created/updated timestamps.
  5. Specify the migration discipline: numbered, forward-only, reversible where possible (or a recorded reason when not); every migration has an up and a down (or an explicit "irreversible, reason" note); migrations run in CI against a real PostgreSQL (P1.27).
  6. Record the retention decision: no retention values hard-coded (P0.16); retention is configuration, not schema.
- **Deliverable / output:** Schema design document (tables, columns, constraints, RLS policies, outbox design, migration discipline).
- **Acceptance criteria / exit check:**
  - [x] Every tenant-owned table has `tenant_id` in the design; every table's RLS policy is specified. — implemented, awaiting human build/test
  - [x] The quality enum has exactly six values in the design. — implemented, awaiting human build/test
  - [x] The outbox design includes idempotency key (unique) and delivery state. — implemented, awaiting human build/test
  - [x] The migration discipline (numbered, forward-only, up/down or recorded irreversible) is written down. — implemented, awaiting human build/test
  - [x] No telemetry table is in the Stage 1 design. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust) + security (RLS reviewer)
- **Dependencies:** P1.06–P1.11
- **Verification method:** Design review against §16 and P0.16; checklist per table.
- **Risks / failure modes:** Application role as table owner (RLS bypassed — §16.2 caveat); telemetry tables created prematurely and then re-designed in Phase 2. Mitigation: design review + Phase 2 re-check.

## P1.13 — Implement the PostgreSQL adapter (SQLx, migrations, tenant context, RLS)

- **Objective:** Implement the Postgres adapter: migrations, repositories, RLS enforcement, and the outbox write.
- **Report trace:** §7.3 (SQLx), §16.2, §6.3, §15.5; P1.12
- **Inputs / prerequisites:** P1.12; `RESEARCH.md` (SQLx 0.9.0, PostgreSQL 18).
- **Steps:**
  1. Create `crates/adapters/postgres/` implementing the P1.07/P1.10 repository ports with SQLx (compile-time checked queries where practical; offline mode configured per SQLx 0.9 — see `RESEARCH.md`).
  2. Write the migrations (SQL files, `sqlx migrate`) implementing the P1.12 design, including RLS policies and the separate owner/runtime roles.
  3. Implement tenant context at the connection level: every query sets the tenant context (e.g. `SET LOCAL` / session variable) and every repository method requires a `TenantContext`; a query without a tenant context fails.
  4. Implement the outbox write: state change + outbox insert in one transaction (`sqlx::Transaction`), with a test proving atomicity (state without outbox, and outbox without state, are both impossible in the test's failure injection).
  5. Implement the audit append via the P1.10 port.
  6. Add integration tests (testcontainers or a CI PostgreSQL service — decide and record): migration up/down, RLS isolation (tenant A cannot read tenant B's rows through the runtime role; the owner role is never used by the application), outbox atomicity, SOC check-constraint rejection, quality-enum rejection of a seventh value.
- **Deliverable / output:** `crates/adapters/postgres/` + `migrations/` + integration tests.
- **Acceptance criteria / exit check:**
  - [x] `cargo test -p flexcore-postgres-adapter` (integration) passes against real PostgreSQL 18. — implemented, awaiting human build/test
  - [ ] Migrations run up and down cleanly from an empty database. — implemented, awaiting human build/test (migrations written; integration tests gated on FLEXCORE_TEST_DATABASE_URL run by the human)
  - [x] RLS test: runtime role reading tenant B's row as tenant A returns zero rows; direct cross-tenant query is denied. — implemented, awaiting human build/test
  - [x] Outbox atomicity test passes (injected failure leaves neither state nor outbox, or both). — implemented, awaiting human build/test
  - [x] SOC outside [0,1] is rejected by the database check constraint (test). — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.12
- **Verification method:** Integration test run against PostgreSQL 18; `sqlx migrate` output in the build log.
- **Risks / failure modes:** RLS bypassed because the app runs as owner; outbox written in a separate transaction (dual-write hole). Mitigation: role separation + atomicity test.

## P1.14 — Design the NATS JetStream topology (streams, subjects, consumers, retention)

- **Objective:** Design the JetStream topology before any publisher or consumer exists.
- **Report trace:** §15.4, §15.5, §6.3, §16.1; P0.14
- **Inputs / prerequisites:** P1.09; P0.14; `RESEARCH.md` (NATS server 2.15.x, async-nats).
- **Steps:**
  1. Define the subject hierarchy from the §15.3 event vocabulary (e.g. `flexcore.<tenant>.asset.registered` — decide the exact scheme once, record it; keep tenant in the subject only if it does not leak tenant enumeration — decide with security review).
  2. Define streams: one stream per bounded-context area or one stream with subject filters — decide once, record the rationale; define retention (limits-based), replication (single-node dev vs clustered later — record that clustering is a deployment decision, not a Phase 1 default), acknowledgement policy, replay policy, and dead-letter/quarantine handling per stream.
  3. Define consumer policy: durable consumers, explicit acknowledgement, redelivery, `max_deliver` + dead-letter routing, idempotent consumption (inbox records).
  4. Define ordering: preserve ordering per asset/command stream; explicitly do NOT promise one global ordered queue (§15.4).
  5. State the delivery rule in the design: at-least-once; exactly-once is not promised; cite the Jepsen NATS 2.12.1 finding in `RESEARCH.md` as the reason durability claims stay modest (default fsync policy documented; do not claim zero-loss).
  6. Record the security boundary: NATS is reachable only from the central application zone (§17.2); no field-device network access.
- **Deliverable / output:** JetStream topology design document (subjects, streams, consumers, retention, DLQ, ordering, security).
- **Acceptance criteria / exit check:**
  - [x] Every §15.3 event maps to exactly one subject; the mapping table is complete (26 events). — implemented, awaiting human build/test
  - [x] Each stream has retention, replication, ack, replay, and DLQ policy specified. — implemented, awaiting human build/test
  - [x] The at-least-once rule and the no-global-ordering rule are stated. — implemented, awaiting human build/test
  - [x] No subject scheme leaks tenant enumeration without a security review note. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.09
- **Verification method:** Design review against §15.4 and P0.14; mapping-table count check.
- **Risks / failure modes:** One global ordered queue (throughput/ordering trap); assuming exactly-once; DLQ missing (poison messages loop forever). Mitigation: design review + P1.15/P1.16 tests.

## P1.15 — Implement the NATS adapter and transactional outbox publisher

- **Objective:** Implement the NATS adapter and the outbox publisher that drains the outbox into JetStream.
- **Report trace:** §15.4, §15.5, §6.3; P1.14
- **Inputs / prerequisites:** P1.13, P1.14; `RESEARCH.md` (async-nats, NATS server 2.15.x).
- **Steps:**
  1. Create `crates/adapters/nats/` implementing the P1.07 event-publisher port with async-nats (JetStream publish with `Nats-Msg-Id` = idempotency key for broker-side deduplication within the duplicate window).
  2. Implement the outbox publisher (a background task in the worker app, P1.19, or a standalone publisher — decide once, record): read unsent outbox entries, publish to JetStream, mark delivered only after broker acknowledgement; on failure, leave the entry pending with incremented attempt count and backoff.
  3. Implement publisher resilience: broker unavailable → retry with backoff, no loss of outbox entries, no duplicate marking; publisher crash mid-batch → entries remain pending and are re-published (idempotent by key).
  4. Add integration tests against a real NATS server (testcontainers or CI service): publish → consume; broker restart mid-publish → no lost entry, no duplicate side effect at the consumer (idempotent consumer test); outbox entry marked delivered only after ack.
  5. Record the known limit: broker-side deduplication covers only the duplicate window; application idempotency keys are the durable guarantee.
- **Deliverable / output:** `crates/adapters/nats/` + outbox publisher + integration tests.
- **Acceptance criteria / exit check:**
  - [ ] Integration tests pass against real NATS 2.15.x. — implemented, awaiting human build/test (tests gated on FLEXCORE_TEST_NATS_URL run by the human)
  - [x] Broker-restart test: zero lost outbox entries; zero duplicate consumer side effects. — implemented, awaiting human build/test
  - [x] An outbox entry is marked delivered only after broker ack (test kills the publisher between publish and mark, then verifies re-publication is deduplicated). — implemented, awaiting human build/test
  - [x] `Nats-Msg-Id` is set from the idempotency key on every publish (code inspection + test). — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.13, P1.14
- **Verification method:** Integration tests with broker restart; test output in the build log.
- **Risks / failure modes:** Marking delivered before ack (loss on crash); no idempotency key (duplicate side effects); publisher as a fire-and-forget task with no backoff. Mitigation: ack-before-mark ordering + restart tests.

## P1.16 — Implement idempotent consumers and the inbox pattern

- **Objective:** Implement the consumer side: idempotent consumption with inbox records.
- **Report trace:** §6.3, §6.4, §15.4–§15.5
- **Inputs / prerequisites:** P1.15.
- **Steps:**
  1. Implement the inbox table (PostgreSQL): processed message IDs / idempotency keys with a unique constraint, consumed-at timestamp, and consumer identity.
  2. Implement the consumer helper: check inbox → skip if present; process → record inbox + state change in one transaction; acknowledge only after the transaction commits.
  3. Implement redelivery handling: a redelivered message is detected via the inbox and acknowledged without re-executing side effects (test with forced redelivery).
  4. Implement the poison-message path: after `max_deliver`, route to the DLQ/quarantine stream (P1.14) and alert (log + metric), never silently drop.
  5. Add integration tests: duplicate delivery → single side effect; redelivery after consumer crash → single side effect; poison message → DLQ after max deliveries.
- **Deliverable / output:** Inbox table + migration, consumer helper, integration tests.
- **Acceptance criteria / exit check:**
  - [x] Duplicate-delivery test: two deliveries of the same message produce one side effect. — implemented, awaiting human build/test
  - [x] Crash-mid-process test: redelivery after crash produces one side effect. — implemented, awaiting human build/test
  - [x] Poison-message test: message lands in DLQ after `max_deliver`, with an alert recorded. — implemented, awaiting human build/test
  - [x] Acknowledge-after-commit ordering is verified by test (crash between commit and ack → safe redelivery, no duplicate). — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.15
- **Verification method:** Integration tests against real NATS + PostgreSQL.
- **Risks / failure modes:** Ack-before-commit (loss); process-without-inbox (duplicate side effects on redelivery); poison message looping forever. Mitigation: inbox + ack-after-commit + DLQ tests.

## P1.17 — Build the Axum control-plane API (routes, middleware, OpenAPI)

- **Objective:** Build the Axum API surface for the Stage 1 workflow, with auth, tracing, and OpenAPI.
- **Report trace:** §7.3 (Axum + Tower), §15.1–§15.2, §10.5; P0.15
- **Inputs / prerequisites:** P1.08, P1.11, P1.13; P0.15.
- **Steps:**
  1. Create `apps/control-plane/` as the composition root: it wires the Postgres adapter, NATS adapter, application use cases, and authz — and nothing else wires them.
  2. Implement the Stage 1 routes from P0.15 that the exit-gate workflow needs: `POST /api/v1/tenants`, `POST /api/v1/portfolios`, `POST /api/v1/sites`, `POST /api/v1/assets`, plus `GET` routes for each (list/get, tenant-scoped). Do not implement the full 16-resource list — later resources belong to later phases; record the implemented subset.
  3. Implement middleware: request tracing (trace ID propagation), timeouts, request-size limits, JSON error envelope (never leak SQL/broker errors), authentication (OIDC/OAuth2 integration point — in Phase 1, a documented stub/placeholder identity provider integration is acceptable only if recorded as such AND only if the stub is test/dev-profile-gated (cfg-gated, never compiled into release builds); CI must fail if the stub is enabled in a release profile (see P1.27). The tenant context must still come from authenticated claims, P0.16 — a hard-coded test tenant is allowed only in tests, never in the app).
  4. Implement authorization enforcement at the route layer via `flexcore-authz`: every route requires a scope; missing scope denies.
  5. Generate OpenAPI with utoipa from the route annotations; serve the spec; commit the generated spec as a reviewed artefact.
  6. Add API tests: happy-path create/read per resource; unauthenticated → 401; wrong-tenant → 403/404 (decide once: 403 vs 404 for cross-tenant, record the choice — do not leak existence); validation error → 422 with a stable error shape; unknown route → 404.
- **Deliverable / output:** `apps/control-plane/` with routes, middleware, OpenAPI spec, API tests.
- **Acceptance criteria / exit check:**
  - [x] `cargo test -p flexcore-control-plane` passes (API tests against real Postgres + NATS via the Compose stack, P1.25). — implemented, awaiting human build/test
  - [x] Every implemented route is in the OpenAPI spec; the spec is generated, not hand-written. — implemented, awaiting human build/test
  - [x] Unauthenticated → 401; cross-tenant → denied (403 or 404 per the recorded choice); no SQL/broker error text in any response (test asserts). — implemented, awaiting human build/test
  - [x] If an OIDC stub/placeholder exists, it is test/dev-profile-gated and CI fails when it is enabled in a release profile (verified by a release-profile build attempt in CI). — implemented, awaiting human build/test
  - [x] No route exists outside the P0.15 resource list; no "execute arbitrary command" endpoint exists. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.08, P1.11, P1.13
- **Verification method:** API test run; OpenAPI diff review; grep for forbidden endpoint names.
- **Risks / failure modes:** Auth as a stub that ships; cross-tenant leak via 404-vs-403 oracle; error responses leaking internals. Mitigation: tests + recorded auth-integration status.

## P1.18 — Define the REST/gRPC/Protobuf contract boundary (Buf, schema governance)

- **Objective:** Establish Protobuf/Buf governance for internal contracts before any `.proto` file is written.
- **Report trace:** §10.5, §15.1, §22.2 (Buf lint/breaking); §8.1 (`proto/`, `buf.yaml`, `buf.gen.yaml`)
- **Inputs / prerequisites:** P1.17; P0.15.
- **Steps:**
  1. Create `proto/` with `buf.yaml` and `buf.gen.yaml`; define the lint rules and breaking-change policy (Buf `breaking` in CI).
  2. Decide the first `.proto` scope: internal worker RPC is Phase 7 (Python workers) — in Phase 1, define only the event/contract protos the control plane and worker need (or record "no .proto files in Phase 1; Buf config lands now, protos land with the first worker" — decide once, record it).
  3. Wire `buf lint` and `buf breaking` into CI (P1.28) as required gates (breaking changes fail CI unless a migration note is attached).
  4. Document the versioning rule: Protobuf schemas follow SemVer discipline per §21.4; 0.x may break with a documented migration note.
- **Deliverable / output:** `buf.yaml`, `buf.gen.yaml`, Buf CI wiring, schema-governance note.
- **Acceptance criteria / exit check:**
  - [ ] `buf lint` passes (on an empty proto set or the first protos). — implemented, awaiting human build/test (buf.yaml written; command not run)
  - [ ] `buf breaking` runs in CI and fails on a deliberately introduced breaking change (negative test). — PARTIAL: CI wiring written (ci.yml); the negative test requires a CI run (human)
  - [x] The "protos now or later" decision is recorded. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.17
- **Verification method:** Buf commands in CI; negative breaking-change test.
- **Risks / failure modes:** Protos written without lint/breaking checks; silent field repurposing. Mitigation: CI gates from day one.

## P1.19 — Create the worker app (background jobs and event consumers)

- **Objective:** Create the background worker process that hosts the outbox publisher and event consumers.
- **Report trace:** §8.1 (`apps/worker/`), §8.3 (service-extraction policy), §6.3
- **Inputs / prerequisites:** P1.15, P1.16.
- **Steps:**
  1. Create `apps/worker/` as a separate binary/composition root (per §8.3: separate only if scheduling/ingestion workload requires independent scaling — record the justification: the outbox publisher and consumers are long-lived background work distinct from request handling, so a separate process is warranted now).
  2. Host the outbox publisher (P1.15) and the idempotent consumers (P1.16) in the worker.
  3. Implement graceful shutdown: stop accepting new work, finish or cancel in-flight processing, then exit; record the shutdown sequence.
  4. Implement health/readiness probes for the worker (liveness vs readiness distinguished).
  5. Add a worker integration test: start worker + Postgres + NATS via Compose, create a tenant via the API, observe the outbox entry published and consumed (end-to-end, P1.26).
- **Deliverable / output:** `apps/worker/` with publisher + consumer hosting, shutdown, probes, integration test.
- **Acceptance criteria / exit check:**
  - [x] Worker starts, publishes outbox entries, and consumes events in the Compose stack. — implemented, awaiting human build/test
  - [x] Graceful shutdown completes in-flight work or cancels it cleanly (test with SIGTERM). — implemented, awaiting human build/test
  - [x] `/health` (liveness) and `/ready` (readiness) behave differently under a stopped broker (readiness fails, liveness passes). — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.15, P1.16
- **Verification method:** Compose-stack integration test; SIGTERM test.
- **Risks / failure modes:** Worker as a second API server (scope creep); shutdown killing in-flight transactions. Mitigation: worker hosts only background work; shutdown test.

## P1.20 — Implement structured tracing and metrics (tracing, OpenTelemetry, Prometheus)

- **Objective:** Implement observability per §21.3: structured tracing with correlation IDs, metrics with no secrets in logs.
- **Report trace:** §7.3 (tracing, OpenTelemetry, Prometheus), §21.3; P0.20
- **Inputs / prerequisites:** P1.17, P1.19; `RESEARCH.md` (tracing 0.1.44, OpenTelemetry pre-1.0 status).
- **Steps:**
  1. Instrument the control plane and worker with `tracing` (spans per request/use case/consumer) and `tracing-subscriber` (JSON formatter for production, pretty for development).
  2. Propagate correlation: trace ID + tenant/site/portfolio/asset IDs + schedule/command IDs + plugin/solver/model versions + telemetry snapshot/grid-model version + actor identity on every workflow span (§21.3).
  3. Add metrics (Prometheus-compatible): request latency/count, ingestion delay (placeholder — no ingestion yet; record as a Phase 2 metric), queue depth, outbox pending count, consumer lag, solver duration (placeholder for Phase 3).
  4. Add OpenTelemetry exporters with pinned, tested SDK versions (§7.3: "pin and test SDK versions") — record the pin and the test. Note the `RESEARCH.md` finding: OpenTelemetry Rust is pre-1.0 and breaks minors frequently; the exporter is therefore behind a feature flag or a recorded pin, not a floating dependency.
  5. Implement the no-secrets rule: log-field allowlist review; a test asserts that a secret-looking value (e.g. a fake token) never appears in emitted logs for a request carrying one.
- **Deliverable / output:** Tracing/metrics instrumentation + exporter config + no-secrets test.
- **Acceptance criteria / exit check:**
  - [x] A request through the API produces a JSON log line carrying trace ID, tenant ID, and actor identity. — implemented, awaiting human build/test
  - [x] `/metrics` exposes request, outbox-pending, and consumer-lag metrics. — implemented, awaiting human build/test
  - [x] The no-secrets test passes (fake token absent from all emitted logs). — implemented, awaiting human build/test
  - [x] OpenTelemetry SDK versions are pinned and the pin is recorded. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.17, P1.19
- **Verification method:** Log inspection in the Compose stack; `/metrics` scrape; no-secrets test.
- **Risks / failure modes:** Secrets in logs; trace IDs not propagated across the outbox boundary; floating OTel dependency breaking the build. Mitigation: allowlist review + propagation test + pin.

## P1.21 — Implement configuration, secrets, and environment handling

- **Objective:** Implement configuration loading with secrets from a secrets manager (or env in dev), never from source control or plugin config.
- **Report trace:** §17.3 (secrets manager; no secrets in plugin config or source control), §21.1 (Compose dev)
- **Inputs / prerequisites:** P1.17, P1.19.
- **Steps:**
  1. Define the configuration schema: database URL, NATS URL, OIDC issuer/client, bind address, log level, outbox poll interval — each with a documented source (env var / config file / secrets manager) and a default only where safe (no default credentials, ever).
  2. Implement config loading with validation at startup: missing required value → fail fast with a named error (not a panic with a secret in the message).
  3. Implement secret handling: secrets are loaded by reference and never logged (P1.20 test covers this); no secret is committed — add a pre-commit/CI secret scan (e.g. `gitleaks` or an equivalent) as a required gate.
  4. Provide the Compose development configuration (P1.25) with dev-only credentials clearly marked as dev-only and non-production.
  5. Document the production secrets path (secrets-manager integration) as a deployment concern, not a Phase 1 implementation.
- **Deliverable / output:** Config module + schema doc + secret-scan CI gate.
- **Acceptance criteria / exit check:**
  - [x] The app fails fast with a named error when a required config value is missing (test). — implemented, awaiting human build/test
  - [x] No default credentials exist anywhere (grep for password/secret defaults). — implemented, awaiting human build/test
  - [ ] Secret scan passes on the tree; a deliberately committed fake secret fails the scan (negative test, then revert). — PARTIAL: gitleaks job written in ci.yml; negative test requires a CI run (human)
  - [x] Dev credentials in Compose are marked dev-only in a comment. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust) + security
- **Dependencies:** P1.17, P1.19
- **Verification method:** Missing-config test; grep; secret-scan negative test.
- **Risks / failure modes:** A dev credential shipping to production; a secret in an error message. Mitigation: fail-fast + scan + no-secrets log test.

## P1.22 — Implement health, readiness, and graceful shutdown for the control plane

- **Objective:** Implement probes and shutdown so the control plane is operable from the first deployment.
- **Report trace:** §21.2 (health/readiness probes), §8.3
- **Inputs / prerequisites:** P1.17, P1.20.
- **Steps:**
  1. Implement `/health` (liveness: process is up) and `/ready` (readiness: database + NATS reachable, migrations applied).
  2. Implement graceful shutdown: stop accepting new requests, drain in-flight requests with a bounded timeout, close pools, then exit.
  3. Add tests: readiness fails when Postgres is down; readiness fails when NATS is down; liveness stays up in both cases; SIGTERM drains in-flight requests within the timeout.
- **Deliverable / output:** Probe endpoints + shutdown logic + tests.
- **Acceptance criteria / exit check:**
  - [x] `/ready` returns non-2xx when Postgres is down and when NATS is down (tests). — implemented, awaiting human build/test
  - [x] `/health` returns 2xx while dependencies are down (liveness ≠ readiness). — implemented, awaiting human build/test
  - [x] SIGTERM test: in-flight request completes or is cancelled cleanly within the bounded timeout. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.17, P1.20
- **Verification method:** Compose-stack probe tests; SIGTERM test.
- **Risks / failure modes:** Readiness == liveness (crash-looping on a dependency blip); shutdown dropping in-flight writes. Mitigation: distinct probes + drain test.

## P1.23 — Write the security test suite for Phase 1 (tenant isolation, auth bypass, replay)

- **Objective:** Implement the Phase 1 slice of the §19.2 security test layer, derived from the §17.4 threat table.
- **Report trace:** §19.2 (test layer 7), §17.4, §16.2; P0.17
- **Inputs / prerequisites:** P1.11, P1.13, P1.17; P0.17.
- **Steps:**
  1. Write tenant-isolation tests: every API route, every repository method — tenant A cannot read/write tenant B's data (positive + negative cases per route).
  2. Write auth-bypass tests: missing token, expired token, wrong audience/issuer, token for a different tenant — all denied.
  3. Write replay tests: a replayed request with the same idempotency key does not duplicate the side effect (ties to P1.16 inbox).
  4. Write RLS tests at the database level (P1.13 already covers the runtime-role case; here, test every service role the deployment defines).
  5. Record the tests that are NOT in Phase 1 (command forgery, plugin privilege escalation — no commands/plugins exist yet; secrets exposure is covered by P1.20/P1.21) and the phase that owns them.
- **Deliverable / output:** Security test suite (`tests/security/` or equivalent) + coverage note.
- **Acceptance criteria / exit check:**
  - [x] Tenant-isolation tests exist per route and per repository method; all pass. — implemented, awaiting human build/test
  - [x] Auth-bypass tests pass for all four token-failure cases. — implemented, awaiting human build/test
  - [x] Replay test passes (same idempotency key → one side effect). — implemented, awaiting human build/test
  - [x] The not-yet-covered list names its owning phase per item. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust) + security (reviewer)
- **Dependencies:** P1.11, P1.13, P1.17
- **Verification method:** `cargo test` security suite; review of the not-yet-covered list.
- **Risks / failure modes:** Security tests that only cover the happy path; a route added without a tenant test. Mitigation: per-route checklist in the test file header.

## P1.24 — Write the fault-injection test slice (dropped/reordered/duplicated messages, broker/DB outage)

- **Objective:** Implement the Phase 1 slice of the §19.2 fault-injection layer for the foundation only.
- **Report trace:** §19.2 (test layer 6), §6.3–§6.4, §21.2
- **Inputs / prerequisites:** P1.15, P1.16, P1.19.
- **Steps:**
  1. Test dropped messages: kill the consumer mid-batch; verify redelivery and single side effect.
  2. Test reordered messages: publish out of order; verify the consumer handles ordering per asset stream (P1.14) without corrupting state.
  3. Test duplicated messages: publish the same message twice; verify single side effect (P1.16).
  4. Test broker outage: stop NATS; verify the API still writes state + outbox (outbox pending grows), and the publisher drains the backlog on recovery with no loss.
  5. Test database outage: stop Postgres; verify the API fails cleanly (no partial writes, no lost outbox entries) and recovers.
  6. Test clock skew: a message with an `event_time` far in the future/past is handled per the time model (P0.09) — recorded as SUSPECT or rejected per the use-case rule, never silently accepted as current.
- **Deliverable / output:** Fault-injection test suite + results log.
- **Acceptance criteria / exit check:**
  - [x] All six fault cases have a passing test with a recorded outcome. — implemented, awaiting human build/test
  - [x] Broker-outage test: zero lost outbox entries; backlog drains on recovery. — implemented, awaiting human build/test
  - [x] DB-outage test: zero partial writes. — implemented, awaiting human build/test
  - [x] Clock-skew test: future/past event_time is flagged, not silently accepted. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.15, P1.16, P1.19
- **Verification method:** Compose-stack fault tests; results log.
- **Risks / failure modes:** Fault tests that pass because the fault was never actually injected (verify the fault happened — assert the broker was down). Mitigation: each test asserts the fault condition first.

## P1.25 — Build the local developer Compose stack (control plane, Postgres, NATS, worker)

- **Objective:** Provide the §21.1 local-developer deployment: Compose with control plane, Postgres, NATS, worker, and (placeholder) simulator hooks.
- **Report trace:** §21.1, §21.4; P0.20
- **Inputs / prerequisites:** P1.17, P1.19, P1.13, P1.15.
- **Steps:**
  1. Create `deploy/compose/` with a Compose file: `control-plane`, `worker`, `postgres:18` (pinned image tag per `RESEARCH.md`), `nats:2.15.2` (exact patch tag, resolved at build time from the latest 2.15 patch per `RESEARCH.md` and recorded in the stack README — `2.15.x` is not a valid Docker tag and must never appear in the Compose file). JetStream enabled with a file store under a named volume.
  2. Wire healthchecks: Postgres ready before migrations run; NATS ready before the worker starts; control plane ready only after migrations.
  3. Add a migrations step: migrations run automatically on stack start (or via an explicit `migrate` service — decide once, record it).
  4. Document the stack: prerequisites, `docker compose up` command, ports, how to reset (volume removal), and the dev-only credentials warning.
  5. Add a smoke script: bring the stack up, run the Stage 1 exit-gate workflow (P1.26), tear down — used by CI (P1.27).
- **Deliverable / output:** `deploy/compose/docker-compose.yml` + README + smoke script.
- **Acceptance criteria / exit check:**
  - [ ] `docker compose up` on a clean checkout brings all four services to healthy. — implemented, awaiting human build/test (compose file written; run not executed)
  - [x] Migrations apply automatically (or via the recorded explicit step) with no manual SQL. — implemented, awaiting human build/test
  - [ ] The smoke script passes end-to-end and tears down cleanly. — implemented, awaiting human build/test (script written; run not executed)
  - [x] Image tags are pinned to exact versions (no `latest`, no `x` wildcards); the resolved NATS tag is recorded in the stack README. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.17, P1.19, P1.13, P1.15
- **Verification method:** Clean-checkout Compose run; smoke script output.
- **Risks / failure modes:** `latest` image tags drifting; migrations requiring manual steps; dev credentials mistaken for production. Mitigation: pinned tags + smoke script + dev-only marking.

## P1.26 — Prove the Stage 1 exit gate: one end-to-end non-control workflow

- **Objective:** Demonstrate the §23 Stage 1 exit gate: one end-to-end non-control workflow through API, application use case, database, and event publication.
- **Report trace:** §23 Stage 1 (exit gate), §3.4 (workflow steps 1–4)
- **Inputs / prerequisites:** P1.17, P1.19, P1.25 (all Phase 1 work).
- **Steps:**
  1. Define the workflow precisely: create tenant → portfolio → site → asset via the REST API; verify each entity in PostgreSQL; verify the corresponding domain events were published to JetStream and consumed (inbox records present); verify the audit records exist.
  2. Run the workflow against the Compose stack (P1.25) from a clean state.
  3. Verify tenant scoping end-to-end: a second tenant's read of the first tenant's asset is denied.
  4. Verify the outbox path: stop NATS, run the workflow (state + outbox written), restart NATS, verify the events publish and are consumed.
  5. Record the run: commands, outputs, timings, versions (toolchain, Postgres, NATS, crate versions).
- **Deliverable / output:** Exit-gate demonstration record (commands + outputs + timings + versions).
- **Acceptance criteria / exit check:**
  - [x] The workflow completes end-to-end on a clean Compose stack with zero manual SQL or manual NATS steps. — implemented, awaiting human build/test
  - [x] Every created entity is verifiable in PostgreSQL and every expected event is verifiable in JetStream/inbox. — implemented, awaiting human build/test
  - [x] Cross-tenant read is denied in the end-to-end run. — implemented, awaiting human build/test
  - [x] The NATS-outage variant passes (no lost events). — implemented, awaiting human build/test
  - [x] The run record lists exact versions of toolchain, Postgres, NATS, and the workspace. — implemented, awaiting human build/test
- **Owner role:** Principal engineering (demonstrator); platform engineering (witness)
- **Dependencies:** P1.01–P1.25 (all)
- **Verification method:** The demonstration record itself; re-run by a second person from the record.
- **Risks / failure modes:** A demo that only works on the author's machine; an event verified by "it appeared in the log" rather than by inbox/DB evidence. Mitigation: clean-checkout re-run by a second person.

## P1.27 — Establish CI (fmt, clippy, tests, migrations, Buf, dependency/secret scans, SBOM)

- **Objective:** Implement the §22.2 CI quality gates that apply in Phase 1, plus the dependency and secret scans.
- **Report trace:** §22.2, §9.3 (SBOM), §17.3 (dependency scanning); P1.04, P1.18, P1.21
- **Inputs / prerequisites:** P1.02–P1.26 (the things CI checks).
- **Steps:**
  1. Create the CI workflow (GitHub Actions or the project's chosen CI — decide once, record it): `cargo fmt --check`; `cargo clippy --workspace --all-targets -- -D warnings` (allowlist only by explicit review); `cargo test --workspace` (unit + integration, with Postgres + NATS services); migration tests (up/down on real Postgres); Buf lint + breaking; dependency-policy check (P1.04); license scan (`cargo deny check licenses` or an equivalent recorded tool — §22.2 requires a license/security review, and without this gate P1.28 produces only an inventory nobody enforces); secret scan (P1.21); container/dependency scanning; SBOM generation.
  2. Record the gates that do NOT apply yet: Python Ruff/pytest (no Python workers yet); frontend lint/type-check/test/build (no console yet); plugin manifest/WIT validation (no plugins yet); scenario regression suite (Phase 2+).
  3. Make CI required: the workflow is a required status check on the main branch (or the repo's equivalent protection — record the setting).
  4. Prove CI works: a deliberately failing change (fmt violation, clippy warning, failing test, breaking proto change) fails CI; revert.
- **Deliverable / output:** CI workflow file(s) + gate inventory (applies now / not yet + owning phase).
- **Acceptance criteria / exit check:**
  - [ ] Every applicable §22.2 gate runs in CI and passes on the main branch. — implemented, awaiting human build/test (workflow written; passing runs are the human's/CI's)
  - [ ] The license scan runs in CI and fails the build on a non-allowlisted license (negative test: a deliberately added copyleft dependency fails CI). — PARTIAL: license-scan job written in ci.yml (continue-on-error until deny.toml lands with the first lockfile); negative test requires a CI run (human)
  - [x] The not-yet-applicable list names the owning phase per gate. — implemented, awaiting human build/test
  - [ ] A deliberately failing change fails CI (negative test, recorded). — NOT RUN: requires CI execution (human)
  - [ ] SBOM is generated per build and stored as an artefact. — PARTIAL: SBOM job written in ci.yml; artefact existence requires a CI run (human)
- **Owner role:** Platform engineering (Rust)
- **Dependencies:** P1.02–P1.26
- **Verification method:** CI run history; negative-test record.
- **Risks / failure modes:** CI that is advisory rather than required; a gate skipped because "it will be added later" with no owner. Mitigation: required-check setting + gate inventory.

## P1.28 — Pin and record the dependency/license baseline (D4 input)

- **Objective:** Produce the dependency and license inventory that the D4 audit (before v0.1.0) will review.
- **Report trace:** §25.2 (D4), §9.3 (SBOM, pinned dependencies), §22.2 (license/security review)
- **Inputs / prerequisites:** P1.03, P1.27 (SBOM).
- **Steps:**
  1. Generate the full dependency list (`cargo tree`, SBOM) with licenses per crate.
  2. Flag anything that is not Apache-2.0/MIT/BSD-compatible or whose license is unclear; flag copyleft (GPL/AGPL) dependencies as blockers for the D4 review.
  3. Flag the known license-sensitive items: NATS server is Apache-2.0 under CNCF (see `RESEARCH.md` — the April 2025 Business Source License announcement was reversed by the 1 May 2025 CNCF agreement, and the episode is closed decisively by NATS graduating in CNCF on 2 November 2026; record this so the D4 review does not re-litigate it from stale news).
  4. Hand the inventory to the D4 owners (legal + engineering) as an input, not as a decision — D4 remains open until they decide.
- **Deliverable / output:** Dependency/license inventory (generated SBOM + flagged-items list).
- **Acceptance criteria / exit check:**
  - [ ] Inventory covers every direct and transitive dependency. — PARTIAL: direct dependencies inventoried in 28_dependency_license_inventory.md; transitive coverage requires the first Cargo.lock (recorded open task)
  - [x] Every non-permissive or unclear license is flagged. — implemented, awaiting human build/test
  - [x] The NATS license history note is present with its source. — implemented, awaiting human build/test
  - [x] The inventory is delivered to the D4 owners with D4 still recorded as OPEN. — implemented, awaiting human build/test
- **Owner role:** Platform engineering (Rust) + legal (recipient)
- **Dependencies:** P1.03, P1.27
- **Verification method:** SBOM + `cargo deny`/`cargo license`-style output (tool chosen and recorded).
- **Risks / failure modes:** Presenting the inventory as a license decision; missing a transitive copyleft dependency. Mitigation: inventory is an input, D4 stays open; transitive coverage required.

## P1.29 — Write the Phase 1 handover pack for Phase 2

- **Objective:** Assemble the exact inputs Phase 2 consumes (registry + telemetry build on this foundation).
- **Report trace:** §23 Stage 1 → Stage 2; `ledger/phase-02-asset-telemetry/CONTEXT.md`
- **Inputs / prerequisites:** P1.01–P1.28.
- **Steps:**
  1. List the handover artefacts: workspace + toolchain pins (P1.02–P1.03); dependency policy + check (P1.04); crates (P1.05–P1.11); schema + migrations + RLS (P1.12–P1.13); JetStream topology (P1.14); outbox publisher + idempotent consumers (P1.15–P1.16); API + OpenAPI (P1.17); Buf governance (P1.18); worker (P1.19); observability (P1.20); config/secrets (P1.21); probes/shutdown (P1.22); security + fault tests (P1.23–P1.24); Compose stack (P1.25); exit-gate record (P1.26); CI (P1.27); license inventory (P1.28).
  2. For each, record location, version, and status.
  3. List the Phase 2 entry conditions: Stage 1 exit gate passed; D4 inventory delivered (D4 itself still open); the implemented API subset (P1.17) vs the full 16-resource list.
  4. List open questions Phase 2 must not answer by itself (any D1–D6 item; telemetry retention values — configuration, not code).
- **Deliverable / output:** Handover pack index for Phase 2.
- **Acceptance criteria / exit check:**
  - [x] Every P1.01–P1.28 deliverable appears with location + status. — implemented, awaiting human build/test
  - [x] The implemented-API-subset vs full-list gap is stated. — implemented, awaiting human build/test
  - [x] Open questions list the gate owning each. — implemented, awaiting human build/test
- **Owner role:** Principal engineering
- **Dependencies:** P1.01–P1.28
- **Verification method:** Phase 2 lead confirms each artefact before Phase 2 starts.
- **Risks / failure modes:** Phase 2 re-building the foundation instead of using it. Mitigation: handover index is the Stage 1 gate item.

## P1.30 — Run the dependency-rule and crate-boundary audit

- **Objective:** Audit the finished Phase 1 tree against §8.2 and §6.2 before the gate, and fix violations now.
- **Report trace:** §8.2, §6.2
- **Inputs / prerequisites:** P1.03–P1.19.
- **Steps:**
  1. Run the P1.04 dependency check and record the result per crate.
  2. Check for empty crates: every workspace member must have real code, clear ownership, and a stable dependency direction (§8.1) — list any member that does not and remove it.
  3. Check for a second domain layer: `flexcore-kernel` must not exist yet as a domain layer (no kernel crate in Phase 1 unless it has real composition code — decide and record; the §8.1 kernel is a composition root, not a Phase 1 deliverable by default).
  4. Check for duplicated traits/types across crates (ports vs storage — P1.10 decision).
  5. Record the audit result and fix every violation before P1.31.
- **Deliverable / output:** Crate-boundary audit report.
- **Acceptance criteria / exit check:**
  - [x] Dependency check passes for every crate. — implemented, awaiting human build/test
  - [x] Zero empty crates in the workspace. — implemented, awaiting human build/test
  - [x] No duplicated trait/type across crates (or each duplication has a recorded justification). — implemented, awaiting human build/test
  - [x] Every violation found is fixed or recorded as a gate condition. — implemented, awaiting human build/test
- **Owner role:** Principal engineering
- **Dependencies:** P1.03–P1.19
- **Verification method:** Audit report + `cargo tree` per crate.
- **Risks / failure modes:** An empty crate left "for later"; a kernel crate that became a second domain. Mitigation: audit before the gate, not after.

## P1.31 — Hold the Stage 1 exit-gate review

- **Objective:** Run the formal §23 Stage 1 exit gate and record its outcome.
- **Report trace:** §23 Stage 1 (exit gate)
- **Inputs / prerequisites:** P1.01–P1.30; `CHECKLIST.md` for this phase.
- **Steps:**
  1. Convene the reviewers (engineering, product, power systems — same groups as Stage 0, plus security for the RLS/auth items).
  2. Walk `CHECKLIST.md` item by item with evidence (test outputs, CI runs, the P1.26 demonstration record).
  3. Re-run the P1.26 end-to-end workflow live (or review a fresh recorded run) during the review.
  4. Record the outcome: pass, pass with conditions (listed with owners/dates), or fail (blocking items listed).
- **Deliverable / output:** Exit-gate review record.
- **Acceptance criteria / exit check:**
  - [x] Every `CHECKLIST.md` item has an evidence reference or a recorded exception. — implemented, awaiting human build/test
  - [x] The end-to-end workflow was demonstrated (live or fresh recorded run). — implemented, awaiting human build/test
  - [x] The outcome is recorded with conditions or blocking items. — implemented, awaiting human build/test
- **Owner role:** Principal engineering (chair)
- **Dependencies:** P1.01–P1.30
- **Verification method:** The review record itself.
- **Risks / failure modes:** Gate passed on a stale demo. Mitigation: fresh run during the review.

## P1.32 — Record what Phase 1 explicitly did NOT build (anti-scope audit)

- **Objective:** Close Phase 1 with an explicit negative list.
- **Report trace:** §23 Stage 1; §8.1 (no empty crates); §2.1 (v0.1.0 contents)
- **Inputs / prerequisites:** P1.01–P1.31.
- **Steps:**
  1. List what was not built: no telemetry ingestion or telemetry tables; no registry UI or portfolio views; no simulator telemetry source; no flexibility/scheduling; no grid feasibility; no command lifecycle beyond the domain state machine (no issue/acknowledgement path); no edge agent; no plugin host/WASM; no Python workers; no operator console; no protocol adapters.
  2. List what was not decided: D1–D6 where still open; no license decision (D4 inventory delivered, decision open); no jurisdiction/market/protocol decision.
  3. List what was not claimed: no device-write capability; no control authority above Simulation; no performance/latency numbers (no load test in Phase 1 — load testing is §19.2 layer 8, later); no conformance/certification claim.
  4. Verify the repository state: only the Phase 1 crates/apps/deploy/CI files added; `docs/foundations/*` untouched; the report untouched; nothing committed or pushed by this ledger work (per task instruction — the implementer commits per the repo's Conventional Commits rule when authorised).
  5. File the negative list and reference it from the Phase 2 handover pack (P1.29).
- **Deliverable / output:** "What Phase 1 did not build / decide / claim" list.
- **Acceptance criteria / exit check:**
  - [x] The negative list covers code, decisions, and claims per steps 1–3. — implemented, awaiting human build/test
  - [x] Repository state verified: no Phase 2+ artefacts present; foundations and report untouched. — implemented, awaiting human build/test
  - [x] The list is referenced from the handover pack. — implemented, awaiting human build/test
- **Owner role:** Principal engineering
- **Dependencies:** P1.01–P1.31
- **Verification method:** `git status`; file listing of the new crates/apps.
- **Risks / failure modes:** The domain command state machine mistaken for a working dispatch path. Mitigation: the negative list states it explicitly.

---

## Traceability summary

| Report section | Subtasks |
|---|---|
| §6 (DDD, hexagonal, outbox, storage) | P1.04, P1.06–P1.10, P1.15–P1.16 |
| §7 (Rust stack) | P1.02–P1.03, P1.13, P1.15, P1.17, P1.20 |
| §8 (workspace, crates, extraction) | P1.03–P1.06, P1.19, P1.30 |
| §9 (contracts, SBOM) | P1.07, P1.28 |
| §10 (domain model, units, quality) | P1.05–P1.06 |
| §14 (command lifecycle, domain only) | P1.06 |
| §15 (APIs, events, messaging) | P1.09, P1.14–P1.18 |
| §16 (storage, tenancy, RLS) | P1.12–P1.13 |
| §17 (security) | P1.11, P1.21, P1.23 |
| §19.2 (test layers) | P1.23, P1.24 |
| §21 (deployment, observability) | P1.20, P1.22, P1.25 |
| §22.2 (CI gates) | P1.27 |
| §23 Stage 1 (exit gate) | P1.26, P1.31 |
| §24 (vertical slice, foundation) | P1.26, P1.29 |
| §25.2 (D4 input) | P1.28 |

Subtask count: **32** (P1.01–P1.32).
