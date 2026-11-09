# P2.27 — CI gate inventory update (Phase 2 slice)

| Field | Value |
|---|---|
| Subtask | P2.27 — Extend CI for Phase 2 (migration tests, scenario regression, telemetry contract tests) |
| Report trace | §22.2 (migration tests, scenario regression suite); P1.27; §19.2 test layer 3 (P2.11 contract suite) |
| Inputs | P2.06 (telemetry migrations), P2.13 (determinism scope/versioning), P2.24 (replay entrypoint, exit codes, report verdict) |
| Status | CI configuration + gate artefacts WRITTEN, NOT RUN (loop bans). The workflow has never been executed by this subtask; the first human run records the OBSERVED outcomes in CI run history (the ledger's verification method) |
| Scope note | Simulation-only 0.x. The gates test the design baseline; nothing here is a certification, safety, or field-control claim |

## 1. What changed in the workflow

`.github/workflows/ci.yml` keeps EVERY Phase 1 job and step unchanged
(static-checks: fmt / clippy `-D warnings` / dependency-rule / buf lint;
unit-tests; integration-tests with the postgres:18.6 + nats:2.15.2 service
containers; security-and-supply-chain: gitleaks, dev-auth release gate,
cargo-deny licenses, CycloneDX SBOM; buf-breaking on pull requests). Three
jobs were added:

| New job | Gate | Services |
|---|---|---|
| `migration-tests` | Up/down of ALL migrations on real PostgreSQL 18 (`cargo test -p flexcore-postgres-adapter`) | postgres:18.6 (same pin and health check as the Phase 1 `integration-tests` job; NATS not needed here) |
| `scenario-regression` | Exit-gate scenario through sim-runner: `run` produces the reference, `replay` compares, and a dedicated step asserts the report's `"verdict": "pass"` — not merely exit code 0 (P2.24 exit-code contract: 0/1/2). On a mismatch the job prints the report and the `.mismatches` list, so a generation change fails with the diff visible | none (pure; no database, no broker) |
| `telemetry-contract` | The reusable `TelemetrySource` provider contract suite (`cargo test -p flexcore-ports --test telemetry_source_contract`) | none (pure) |

Job-level honesty: every job that cannot pass without services HAS those
services in the workflow; nothing is `continue-on-error` except the
pre-existing Phase 1 cargo-deny step (whose recorded first-run tightening
note is unchanged). No new allow-fail was introduced.

## 2. The migration-test enumeration (later migrations need no CI edits)

The CI job names no migration numbers. The enumeration lives in the test
file it runs:

- **Up path:** `sqlx::migrate!("migrations/")` in
  `crates/adapters/postgres/src/store.rs` embeds the whole migrations
  directory at compile time — a glob, so migrations 0012+ are picked up
  with no CI or job change.
- **Down path:** the recorded down-file table in
  `crates/adapters/postgres/tests/integration.rs` (`DOWN_FILES`), replayed
  in reverse on a dedicated temporary database, then re-up. Adding a
  migration extends THAT table (owned by the Postgres adapter owner); CI
  does not change.

## 3. DECISION — the contract suite vs the port-only rule (recorded once)

The suite (`flexcore_ports::telemetry_source_contract`) is generic over the
`TelemetrySource` port. The ports crate may depend only on
flexcore-contracts and flexcore-domain (checked by
`tools/check_dependency_rule.py`), and flexcore-simulator already depends on
flexcore-ports — a ports → simulator edge (even dev-only) would be a cycle
and a rule violation. The recorded arrangement:

1. **Now:** CI's `telemetry-contract` job runs the suite through
   `crates/flexcore-ports/tests/telemetry_source_contract.rs`, an in-memory
   deterministic stand-in provider that mirrors the simulator's DECLARED
   contract (P2.14/P2.15): `sim://`-namespaced source_id, the declared
   quality mapping with the original code as `source_quality_code`,
   `ClockQuality::Synchronized`, scenario-cadence event times on the
   provided clock, per-asset streams from the recorded seed rule
   (P2.13 D-SC4: `master_seed XOR splitmix64(FNV-1a-64(asset_key))`),
   `ingest_time` stamped at the release tick, one frame per (asset, tick)
   decoded through the port's `decode`. The suite checks: provider
   declaration, canonical record schema, six-state quality closure
   (compile-enforced, no wildcard arm), per-asset sequence monotonicity,
   event-time-on-clock, ingest-time-at-emission, and deterministic
   same-seed reproduction on the frozen P2.24 comparison tuple (never
   `ingest_time`).
2. **Follow-up (wiring, owned by the sim-runner/simulator owners):** run
   the SAME suite against `flexcore-simulator`'s
   `SimulatorTelemetrySource` from a crate that legally depends on both —
   the natural homes are a `flexcore-sim-runner contract` subcommand or an
   integration test in `apps/sim-runner/tests/` (sim-runner already depends
   on simulator + ports). The suite itself needs no change; the P2.27
   ledger acceptance line "telemetry contract suite runs in CI against the
   simulator" is then satisfied by the real provider with the same job
   surface. Recorded here because this subtask's file ownership excludes
   `apps/**` and the simulator crate.
3. **Future providers** (MQTT, Modbus, OpenEMS — later phases) plug into
   the same suite: instantiate `ContractSpec` (source factory, query,
   deterministic frames, provided clock) and call `run_contract_suite`.

## 4. Gate inventory

### 4.1 Running in CI

| Gate | Job | Added by |
|---|---|---|
| `cargo fmt --all -- --check` | static-checks | P1.27 |
| `cargo clippy --workspace --all-targets -- -D warnings` | static-checks | P1.27 |
| Dependency-rule check (`tools/check_dependency_rule.py`) | static-checks | P1.27 |
| Buf lint (`proto`) | static-checks | P1.27 |
| Buf breaking change check (pull requests) | buf-breaking | P1.27 |
| Workspace unit tests | unit-tests | P1.27 |
| Workspace integration tests (Postgres 18.6 + NATS 2.15.2 services) | integration-tests | P1.27 |
| Migration up/down tests (PostgreSQL 18.6 service; all migrations incl. P2.06 partitions) | migration-tests | **P2.27** |
| Scenario regression with replay-verdict assertion (report `"pass"` + mismatch diff on failure) | scenario-regression | **P2.27** |
| TelemetrySource contract suite (generic; stand-in harness instantiation) | telemetry-contract | **P2.27** |
| Secret scan (gitleaks) | security-and-supply-chain | P1.27 |
| dev-auth release-profile gate | security-and-supply-chain | P1.17/P1.27 |
| License scan (cargo-deny; first-run tightening note per P1.03) | security-and-supply-chain | P1.27 |
| SBOM generation + artifact (CycloneDX) | security-and-supply-chain | P1.27 |

### 4.2 Recorded as not yet applicable (later phases; unchanged from P1.27)

| Gate | Why not applicable | Owning phase |
|---|---|---|
| Python Ruff/pytest | No Python numerical worker exists yet | Phase 7 |
| Frontend lint / type-check / test / build | No operator console exists yet | Phase 5 |
| Plugin manifest / WASM-WIT validation | No plugin SDK or host exists yet | Phase 7 |
| Load / performance / soak tests | §19.2 places them in a later test layer once the compose stack and worker mature | Later phases per §19.2 |
| Contract suite against FUTURE real adapters | Only the simulator exists as a Phase 2 provider (P2.11 step 5) | Per new provider, from Phase 4 on |

### 4.3 Gates that need services

| Gate | Services |
|---|---|
| Workspace integration tests (P1.27) | postgres:18.6, nats:2.15.2 |
| Migration up/down tests (P2.27) | postgres:18.6 |

The scenario-regression and telemetry-contract jobs are deliberately pure
(the recorded P2.24 dependency surface: no database, no broker) so they run
everywhere without infrastructure. Every service-gated job carries its own
service containers in the workflow — no "allowed to fail" substitute.

## 5. VERIFY-CI items and recorded fixes

- **VERIFY-CI (jq):** the verdict-assertion step uses `jq`, which is
  preinstalled on GitHub-hosted ubuntu-latest runners; the step carries a
  comment to replace it with a `python3 -c` JSON parse if a future runner
  image drops it. Nothing else in the workflow uses jq.
- **VERIFY-CI (run/fresh cache):** the scenario-regression job compiles
  flexcore-sim-runner with `cargo run` (no cache-action existed in the
  Phase 1 file, and none was invented here); the first run pays the crate
  build from the network as the Phase 1 note already records.
- **Recorded fix (found while wiring):** the vendor-freedom `SOURCES` table
  in `crates/flexcore-ports/src/lib.rs` declared length 11 while listing
  twelve files — a compile-blocking mismatch in the written-not-run Phase 1
  code. Fixed to the true length (13) while adding
  `telemetry_source_contract.rs`; the vendor-free checks
  (in-crate + the independent mirror in
  `crates/flexcore-ports/tests/security_vendor_free.rs`) are otherwise
  untouched. The mirror's module-list guard only asserts presence of the
  frozen module list, so the new module does not escape the vendor-free
  gate: the new source file is now covered by the in-crate SOURCES check.
- **Not run:** nothing in this change was executed (loop bans). The run
  commands: `cargo test -p flexcore-ports` (suite self-tests + contract
  integration tests, pure), `cargo test -p flexcore-postgres-adapter`
  (Postgres service required), `cargo run -p flexcore-sim-runner -- run/replay`
  (pure), and the workflow itself on first push.

## 6. Scope and control status

The CI gates verify the simulation-only design baseline. FlexCore is a
supervisory orchestration layer; nothing in these gates touches a live
grid, and nothing here constitutes a certification claim, a safety case,
or permission for field control. No performance targets are asserted
anywhere (§19.4).
